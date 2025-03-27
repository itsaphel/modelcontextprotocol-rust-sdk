use mcp_core::protocol::JsonRpcResponse;
use mcp_core::transport::SendableMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, Mutex};

use super::{send_message, Error, PendingRequests, Transport, TransportHandle, TransportMessage};

/// A `StdioTransport` uses a child process's stdin/stdout as a communication channel.
///
/// It uses channels for message passing and handles responses asynchronously through a background task.
///
/// StdioActor needs to be given a `mpsc::Receiver<TransportMessage>` which will receive messages
/// to be sent to the MCPServer. `pending_requests` is a store of message IDs for which we're waiting
/// a response, and a corresponding channel to send the response on. There is a channel for errors
/// to be communicated. Finally, there are handles to the child process's stdin, stdout, and stderr.
pub struct StdioActor {
    receiver: mpsc::Receiver<TransportMessage>,
    pending_requests: Arc<PendingRequests>,
    _process: Child, // we store the process to keep it alive
    error_sender: mpsc::Sender<Error>,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
}

impl StdioActor {
    pub async fn run(mut self) {
        use tokio::pin;

        let incoming = Self::handle_incoming_messages(self.stdout, self.pending_requests.clone());
        let outgoing = Self::handle_outgoing_messages(
            self.receiver,
            self.stdin,
            self.pending_requests.clone(),
        );

        // take ownership of futures for tokio::select
        pin!(incoming);
        pin!(outgoing);

        // Keep the process alive (the incoming and outgoing handlers). The select! will return only
        // if one of the futures returns (due to an unrecoverable error) or the process exits.
        tokio::select! {
            result = &mut incoming => {
                tracing::debug!("Stdin handler completed: {:?}", result);
            }
            result = &mut outgoing => {
                tracing::debug!("Stdout handler completed: {:?}", result);
            }
            // capture the status so we don't need to wait for a timeout
            status = self._process.wait() => {
                tracing::debug!("Process exited with status: {:?}", status);
            }
        }

        // Then always try to read stderr before cleaning up
        let mut stderr_buffer = Vec::new();
        if let Ok(bytes) = self.stderr.read_to_end(&mut stderr_buffer).await {
            let err_msg = if bytes > 0 {
                String::from_utf8_lossy(&stderr_buffer).to_string()
            } else {
                "Process ended unexpectedly".to_string()
            };

            tracing::info!("Process stderr: {}", err_msg);
            let _ = self
                .error_sender
                .send(Error::StdioProcessError(err_msg))
                .await;
        }

        // Clean up
        self.pending_requests.clear().await;
    }

    // Receive messages from the MCP server
    async fn handle_incoming_messages(stdout: ChildStdout, pending_requests: Arc<PendingRequests>) {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    tracing::error!("Child process ended (EOF on stdout)");
                    break;
                } // EOF
                Ok(_) => {
                    // TODO: Support notifications
                    // We take a more opinionated approach, only supporting server responding to
                    // requests, and not server-initiated requests (as the protocol technically allows).
                    if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                        tracing::debug!(
                            message = ?response,
                            "Received incoming message"
                        );

                        let id = match &response {
                            JsonRpcResponse::Success { id, .. } => id.clone(),
                            JsonRpcResponse::Error { id, .. } => id.clone(),
                        };
                        pending_requests.respond(&id, Ok(response)).await;
                    } else {
                        // TODO: remove after testing, or move to trace level
                        tracing::error!(message = ?line, "Received invalid message");
                    }
                    line.clear();
                }
                Err(e) => {
                    tracing::error!(error = ?e, "Error reading line");
                    break;
                }
            }
        }
    }

    // Send messages to the MCP server
    async fn handle_outgoing_messages(
        mut receiver: mpsc::Receiver<TransportMessage>,
        mut stdin: ChildStdin,
        pending_requests: Arc<PendingRequests>,
    ) {
        // Receive submitted messages on the channel and transmit them to the MCP server over the
        // child process's stdin.
        while let Some(mut transport_msg) = receiver.recv().await {
            let message_str = match serde_json::to_string(&transport_msg.message) {
                Ok(s) => s,
                Err(e) => {
                    // If we can't serialize the message, send an error response on the response channel.
                    if let Some(tx) = transport_msg.response_tx.take() {
                        let _ = tx.send(Err(Error::Serialization(e)));
                    }
                    continue;
                }
            };

            tracing::debug!(message = ?transport_msg.message, "Sending outgoing message");

            // If the message requires a response, insert it into the pending requests map.
            if let Some(response_tx) = transport_msg.response_tx.take() {
                if let SendableMessage::Request(request) = &transport_msg.message {
                    pending_requests
                        .insert(request.id.clone(), response_tx)
                        .await;
                }
            }

            if let Err(e) = stdin
                .write_all(format!("{}\n", message_str).as_bytes())
                .await
            {
                tracing::error!(error = ?e, "Error writing message to child process");
                break;
            }

            if let Err(e) = stdin.flush().await {
                tracing::error!(error = ?e, "Error flushing message to child process");
                break;
            }
        }
    }
}

#[derive(Clone)]
pub struct StdioTransportHandle {
    sender: mpsc::Sender<TransportMessage>,
    error_receiver: Arc<Mutex<mpsc::Receiver<Error>>>,
}

#[async_trait::async_trait]
impl TransportHandle for StdioTransportHandle {
    async fn send(&self, message: SendableMessage) -> Result<Option<JsonRpcResponse>, Error> {
        let result = send_message(&self.sender, message).await;
        // Check for any pending errors even if send is successful
        self.check_for_errors().await?;
        result
    }
}

impl StdioTransportHandle {
    /// Check if there are any process errors
    pub async fn check_for_errors(&self) -> Result<(), Error> {
        match self.error_receiver.lock().await.try_recv() {
            Ok(error) => {
                tracing::debug!("Found error: {:?}", error);
                Err(error)
            }
            Err(_) => Ok(()),
        }
    }
}

pub struct StdioTransport {
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
}

impl StdioTransport {
    /// Create a new `StdioTransport`. The command and args are passed directly to `Command::new`,
    /// and used to spawn a new process which runs an MCP server.
    pub fn new<S: Into<String>>(
        command: S,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        Self {
            command: command.into(),
            args,
            env,
        }
    }

    /// Spawn the MCP server as a new process. This method returns handles to communciate with the
    /// MCP server. Namely, the child process, stdin, stdout, and stderr. As MCP servers can be
    /// communicated with using stdin/stdout (see [stdio in the spec]), these handles are used for
    /// communication when using stdio as the transport.
    ///
    /// As an end user building a client, you probably want to use the `Transport` trait to
    /// communicate, which abstracts over stdio details.
    ///
    /// [stdio in the spec]: https://spec.modelcontextprotocol.io/specification/2024-11-05/basic/transports/#stdio
    async fn spawn_process(&self) -> Result<(Child, ChildStdin, ChildStdout, ChildStderr), Error> {
        let mut command = Command::new(&self.command);
        command
            .envs(&self.env)
            .args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        // Set process group only on Unix systems
        #[cfg(unix)]
        command.process_group(0); // don't inherit signal handling from parent process

        // Hide console window on Windows
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW flag

        let mut process = command
            .spawn()
            .map_err(|e| Error::StdioProcessError(e.to_string()))?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| Error::StdioProcessError("Failed to get stdin".into()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| Error::StdioProcessError("Failed to get stdout".into()))?;

        let stderr = process
            .stderr
            .take()
            .ok_or_else(|| Error::StdioProcessError("Failed to get stderr".into()))?;

        Ok((process, stdin, stdout, stderr))
    }
}

#[async_trait]
impl Transport for StdioTransport {
    type Handle = StdioTransportHandle;

    /// Spawn the MCP server as a new process. This method returns a handle which can be used to
    /// send messages to the MCP server.
    async fn start(&self) -> Result<Self::Handle, Error> {
        let (process, stdin, stdout, stderr) = self.spawn_process().await?;
        let (message_tx, message_rx) = mpsc::channel(32);
        let (error_tx, error_rx) = mpsc::channel(1);

        let actor = StdioActor {
            receiver: message_rx,
            pending_requests: Arc::new(PendingRequests::new()),
            _process: process,
            error_sender: error_tx,
            stdin,
            stdout,
            stderr,
        };

        tokio::spawn(actor.run());

        let handle = StdioTransportHandle {
            sender: message_tx,
            error_receiver: Arc::new(Mutex::new(error_rx)),
        };
        Ok(handle)
    }

    async fn close(&self) -> Result<(), Error> {
        Ok(())
    }
}
