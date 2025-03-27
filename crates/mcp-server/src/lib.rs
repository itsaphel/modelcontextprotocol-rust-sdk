use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Future, Stream};
use mcp_core::{
    protocol::{JsonRpcRequest, JsonRpcResponse},
    transport::SendableMessage,
};
use pin_project::pin_project;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tower_service::Service;

pub mod context;
mod errors;
pub use errors::{BoxError, RouterError, ServerError, TransportError};
pub mod router;
pub use router::Router;
pub mod server;
pub use server::MCPServer;

// TODO: Rethink the pins
/// A transport layer that handles JSON-RPC messages over byte
#[pin_project]
pub struct ByteTransport<R, W> {
    // Reader is a BufReader on the underlying stream (stdin or similar) buffering
    // the underlying data across poll calls, we clear one line (\n) during each
    // iteration of poll_next from this buffer
    #[pin]
    reader: BufReader<R>,
    #[pin]
    writer: W,
}

impl<R, W> ByteTransport<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            // TODO: Rethink capacity
            // Default BufReader capacity is 8 * 1024, increase this to 2MB to the file size limit
            // allows the buffer to have the capacity to read very large calls
            reader: BufReader::with_capacity(2 * 1024 * 1024, reader),
            writer,
        }
    }
}

// TODO: Assess this code.
impl<R, W> Stream for ByteTransport<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    type Item = Result<SendableMessage, TransportError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let mut buf = Vec::new();

        let mut reader = this.reader.as_mut();
        let mut read_future = Box::pin(reader.read_until(b'\n', &mut buf));
        match read_future.as_mut().poll(cx) {
            Poll::Ready(Ok(0)) => Poll::Ready(None), // EOF
            Poll::Ready(Ok(_)) => {
                // Convert to UTF-8 string
                let line = match String::from_utf8(buf) {
                    Ok(s) => s,
                    Err(e) => return Poll::Ready(Some(Err(TransportError::Utf8(e)))),
                };
                // Parse JSON and validate message format
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(value) => {
                        // Validate basic JSON-RPC structure
                        if !value.is_object() {
                            return Poll::Ready(Some(Err(TransportError::InvalidMessage(
                                "Message must be a JSON object".into(),
                            ))));
                        }
                        let obj = value.as_object().unwrap(); // Safe due to check above

                        // Check jsonrpc version field
                        if !obj.contains_key("jsonrpc") || obj["jsonrpc"] != "2.0" {
                            return Poll::Ready(Some(Err(TransportError::InvalidMessage(
                                "Missing or invalid jsonrpc version".into(),
                            ))));
                        }

                        // Now try to parse as proper message
                        match serde_json::from_value::<SendableMessage>(value) {
                            Ok(msg) => Poll::Ready(Some(Ok(msg))),
                            Err(e) => Poll::Ready(Some(Err(TransportError::Json(e)))),
                        }
                    }
                    Err(e) => Poll::Ready(Some(Err(TransportError::Json(e)))),
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(TransportError::Io(e)))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<R, W> ByteTransport<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub async fn write_message(&mut self, msg: JsonRpcResponse) -> Result<(), std::io::Error> {
        let json = serde_json::to_string(&msg)?;
        Pin::new(&mut self.writer)
            .write_all(json.as_bytes())
            .await?;
        Pin::new(&mut self.writer).write_all(b"\n").await?;
        Pin::new(&mut self.writer).flush().await?;
        Ok(())
    }
}

/// The main server type that processes incoming requests
pub struct Server<S> {
    service: S,
}

fn trace_log_request(request: &JsonRpcRequest) {
    let request_json = serde_json::to_string(&request)
        .unwrap_or_else(|_| "Failed to serialize request".to_string());
    tracing::debug!(
        request_id = ?request.id,
        method = ?request.method,
        json = %request_json,
        "Received request"
    );
}

fn trace_log_response(response: &Option<JsonRpcResponse>) {
    let response_json = serde_json::to_string(&response)
        .unwrap_or_else(|_| "Failed to serialize response".to_string());
    tracing::debug!(
        json = %response_json,
        "Sending response"
    );
}

impl<S> Server<S>
where
    S: Service<SendableMessage, Response = Option<JsonRpcResponse>>,
    S::Error: Into<BoxError>,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }

    // TODO transport trait instead of byte transport if we implement others
    pub async fn run<R, W>(self, mut transport: ByteTransport<R, W>) -> Result<(), ServerError>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        use futures::StreamExt;
        let mut service = self.service;

        tracing::info!("Server started");
        while let Some(msg_result) = transport.next().await {
            // TODO: This tracing is incorrect for async code.
            let _span = tracing::span!(tracing::Level::INFO, "message_processing");
            let _enter = _span.enter();
            match msg_result {
                Ok(SendableMessage::Request(request)) => {
                    let id = request.id.clone();
                    // TODO: Remove after testing
                    trace_log_request(&request);

                    // Process the request using our service. Respond with the response from
                    // the service, or an error response if the call fails.
                    let response = match service.call(SendableMessage::from(request)).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            let error_msg = e.into().to_string();
                            tracing::debug!(error = %error_msg, "Request processing failed");
                            Some(JsonRpcResponse::Error {
                                jsonrpc: "2.0".to_string(),
                                id,
                                error: mcp_core::protocol::ErrorData {
                                    code: mcp_core::protocol::INTERNAL_ERROR,
                                    message: error_msg,
                                    data: None,
                                },
                            })
                        }
                    };

                    // TODO: Remove after testing
                    trace_log_response(&response);

                    // Send the message over the transport
                    // TODO: Swap JsonRpcMessage for a transport-level abstraction
                    if let Some(response) = response {
                        transport
                            .write_message(response)
                            .await
                            .map_err(|e| ServerError::Transport(TransportError::Io(e)))?;
                    }
                }
                Ok(SendableMessage::Notification(_)) => {
                    // Ignore notifications for now
                    continue;
                }
                Err(e) => {
                    // Transport errors are just logged. No response is sent to the client.
                    tracing::error!(error = ?e, "Transport error");
                }
            }
        }

        Ok(())
    }
}
