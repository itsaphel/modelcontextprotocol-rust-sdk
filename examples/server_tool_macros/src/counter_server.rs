use std::sync::Arc;

use anyhow::Result;
use mcp_core::ToolError;
use mcp_macros::tool;
use mcp_server::router::RouterService;
use mcp_server::{ByteTransport, Server, MCPServer};
use tokio::io::{stdin, stdout};
use tokio::sync::Mutex;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{self, EnvFilter};

#[derive(Clone, Default)]
pub struct Counter {
    counter: Arc<Mutex<i32>>,
}

impl Counter {
    async fn increment(&self) -> Result<i32, ToolError> {
        let mut counter = self.counter.lock().await;
        *counter += 1;
        Ok(*counter)
    }

    async fn decrement(&self) -> Result<i32, ToolError> {
        let mut counter = self.counter.lock().await;
        *counter -= 1;
        Ok(*counter)
    }

    async fn get_value(&self) -> Result<i32, ToolError> {
        let counter = self.counter.lock().await;
        Ok(*counter)
    }
}

#[tool(
    name = "increment",
    description = "Increment the counter by 1",
)]
async fn increment() -> Result<i32, ToolError> {
    // TODO: get a global counter from some context
    //counter.increment().await
    todo!()
}

#[tool(
    name = "decrement",
    description = "Decrement the counter by 1",
)]
async fn decrement() -> Result<i32, ToolError> {
    // TODO: get a global counter from some context
    todo!()
}

#[tool(
    name = "get_value",
    description = "Get the current value of the counter",
)]
async fn get_value() -> Result<i32, ToolError> {
    // TODO: get a global counter from some context
    todo!()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up file appender for logging
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "mcp-server.log");

    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(file_appender)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    tracing::info!("Starting MCP server");

    // Initialise a persistent counter
    let counter = Counter::default();

    // Create the server and add our tools
    let mut mcp_server = MCPServer::new(
        "Counter".to_string(),
        "This server provides a counter tool that can increment and decrement a counter. You can also get the current value of the counter.".to_string()
    );
    mcp_server.register_tool(Increment);
    mcp_server.register_tool(Decrement);
    mcp_server.register_tool(GetValue);

    // Create and run the server
    let router = RouterService(mcp_server);
    let server = Server::new(router);
    let transport = ByteTransport::new(stdin(), stdout());

    tracing::info!("Server initialized and ready to handle requests");
    Ok(server.run(transport).await?)
}
