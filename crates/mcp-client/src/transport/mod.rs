use async_trait::async_trait;
use mcp_core::{protocol::JsonRpcResponse, transport::SendableMessage};
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, RwLock};

pub type BoxError = Box<dyn std::error::Error + Sync + Send>;

/// A generic error type for transport operations.
// TODO: Rename to TransportError, to make it clear from logical errors.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Transport was not connected or is already closed")]
    NotConnected,

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Unsupported message type. JsonRpcMessage can only be Request or Notification.")]
    UnsupportedMessage,

    #[error("Stdio process error: {0}")]
    StdioProcessError(String),

    #[error("SSE connection error: {0}")]
    SseConnection(String),

    #[error("HTTP error: {status} - {message}")]
    HttpError { status: u16, message: String },
}

/// A message that can be sent through the transport
#[derive(Debug)]
pub struct TransportMessage {
    /// The JSON-RPC message to send
    pub message: SendableMessage,
    /// Channel to receive the response on (None for notifications)
    pub response_tx: Option<oneshot::Sender<Result<JsonRpcResponse, Error>>>,
}

/// A generic asynchronous transport trait, used to abstract over the underlying transport mechanism.
///
/// The transport can be started and closed. Starting the transport returns a handle, which can be
/// used to send messages over the transport.
#[async_trait]
pub trait Transport {
    type Handle: TransportHandle;

    /// Start the transport and establish the underlying connection.
    /// Returns the transport handle for sending messages.
    async fn start(&self) -> Result<Self::Handle, Error>;

    /// Close the transport and free any resources.
    async fn close(&self) -> Result<(), Error>;
}

#[async_trait]
pub trait TransportHandle: Send + Sync + Clone + 'static {
    /// Send a message over the transport.
    ///
    /// The SendableMessage may be either a JSON-RPC request or a notification.
    /// For requests, a `JsonRpcResponse` (or error) is returned. For notifications, there is no
    /// response if the request is successful.
    async fn send(&self, message: SendableMessage) -> Result<Option<JsonRpcResponse>, Error>;
}

// Helper function that contains the common send implementation
pub async fn send_message(
    sender: &mpsc::Sender<TransportMessage>,
    message: SendableMessage,
) -> Result<Option<JsonRpcResponse>, Error> {
    match message {
        SendableMessage::Request(_) => {
            let (respond_to, response) = oneshot::channel();
            let msg = TransportMessage {
                message,
                response_tx: Some(respond_to),
            };
            sender.send(msg).await.map_err(|_| Error::ChannelClosed)?;
            Ok(Some(response.await.map_err(|_| Error::ChannelClosed)??))
        }
        SendableMessage::Notification(_) => {
            let msg = TransportMessage {
                message,
                response_tx: None,
            };
            sender.send(msg).await.map_err(|_| Error::ChannelClosed)?;
            Ok(None)
        }
    }
}

// A data structure to store pending requests and their response channels
pub struct PendingRequests {
    requests: RwLock<HashMap<String, oneshot::Sender<Result<JsonRpcResponse, Error>>>>,
}

impl Default for PendingRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingRequests {
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
        }
    }

    pub async fn insert(
        &self,
        id: String,
        sender: oneshot::Sender<Result<JsonRpcResponse, Error>>,
    ) {
        self.requests.write().await.insert(id, sender);
    }

    pub async fn respond(&self, id: &str, response: Result<JsonRpcResponse, Error>) {
        if let Some(tx) = self.requests.write().await.remove(id) {
            let _ = tx.send(response);
        }
    }

    pub async fn clear(&self) {
        self.requests.write().await.clear();
    }
}

pub mod stdio;
pub use stdio::StdioTransport;

pub mod sse;
pub use sse::SseTransport;
