use anyhow::Result;
use mcp_core::ToolError;
use mcp_macros::tool;
use mcp_server::{
    data::Inject,
    router::RouterService,
    server::MCPServerBuilder,
    ByteTransport, Server,
};
use serde::Deserialize;
use std::sync::Mutex;
use tokio::io::{stdin, stdout};

#[derive(Default, Deserialize)]
pub struct Counter {
    counter: Mutex<i32>,
}

impl Counter {
    fn increment(&self, quantity: u32) -> i32 {
        let mut counter = self.counter.lock().unwrap();
        *counter += quantity as i32;
        *counter
    }
    
    fn decrement(&self, quantity: u32) -> i32 {
        let mut counter = self.counter.lock().unwrap();
        *counter -= quantity as i32;
        *counter
    }

    fn get_value(&self) -> i32 {
        let counter = self.counter.lock().unwrap();
        *counter
    }
}

#[tool(
    description = "Increment the counter by a specified quantity",
    params(
        quantity = "How much to increment the counter by"
    )
)]
async fn increment(counter: Inject<Counter>, quantity: u32) -> Result<(), ToolError> {
    counter.increment(quantity);
    Ok(())
}

#[tool(
    description = "Decrement the counter by a specified quantity",
    params(
        quantity = "How much to decrement the counter by"
    )
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
    let counter = Counter::default();
    let mcp_server = MCPServerBuilder::new(
        "Counter".to_string(),
        "This server provides a counter tool that can increment and decrement a counter. You can also get the current value of the counter.".to_string()
    )
    .with_tool(Increment)
    .with_tool(Decrement)
    .with_tool(GetValue)
    .with_state(Inject::new(counter))
    .build();

    let router = RouterService(mcp_server);
    let server = Server::new(router);
    let transport = ByteTransport::new(stdin(), stdout());

    tracing::info!("Server initialized and ready to handle requests");
    Ok(server.run(transport).await?)
}
