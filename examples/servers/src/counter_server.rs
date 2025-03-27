use anyhow::Result;
use mcp_core::ToolError;
use mcp_macros::tool;
use mcp_server::{
    ByteTransport, Server, context::Inject, router::RouterService, server::MCPServerBuilder,
};
use serde::Deserialize;
use std::sync::Mutex;
use tokio::io::{stdin, stdout};
use tracing_subscriber::EnvFilter;

#[derive(Default, Deserialize)]
pub struct Counter {
    counter: Mutex<i32>,
}

impl Counter {
    fn increment(&self, quantity: u32) {
        let mut counter = self.counter.lock().unwrap();
        *counter += quantity as i32;
    }

    fn decrement(&self, quantity: u32) {
        let mut counter = self.counter.lock().unwrap();
        *counter -= quantity as i32;
    }

    fn get_value(&self) -> i32 {
        let counter = self.counter.lock().unwrap();
        *counter
    }
}

#[tool(
    description = "Increment the counter by a specified quantity",
    params(quantity = "How much to increment the counter by")
)]
async fn increment(counter: Inject<Counter>, quantity: u32) -> Result<(), ToolError> {
    counter.increment(quantity);
    Ok(())
}

#[tool(
    description = "Decrement the counter by a specified quantity",
    params(quantity = "How much to decrement the counter by")
)]
async fn decrement(counter: Inject<Counter>, quantity: u32) -> Result<(), ToolError> {
    counter.decrement(quantity);
    Ok(())
}

#[tool(description = "Get current value of counter")]
async fn get_value(counter: Inject<Counter>) -> Result<i32, ToolError> {
    Ok(counter.get_value())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Initialize the tracing subscriber with file and stdout logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    let counter = Counter::default();

    let mcp_server = MCPServerBuilder::new(
        "Counter".to_string(),
        "This server provides a counter tool that can increment and decrement a counter. You can also get the current value of the counter.".to_string()
    )
    // TODO: 'magic structs'
    .with_tool(Increment)
    .with_tool(Decrement)
    .with_tool(GetValue)
    .with_state(Inject::new(counter))
    // TODO: Compile-time safety: can we ensure all contexts required by handlers are provided in the server?
    .build();

    let router = RouterService(mcp_server);
    let server = Server::new(router);
    let transport = ByteTransport::new(stdin(), stdout());

    tracing::info!("Server initialized and ready to handle requests");
    Ok(server.run(transport).await?)
}
