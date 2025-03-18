use anyhow::Result;
use mcp_core::ToolError;
use mcp_macros::tool;
use mcp_server::context::Inject;
use mcp_server::{
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

    // TODO: [1]
    let mcp_server = MCPServerBuilder::new(
        "Counter".to_string(),
        "This server provides a counter tool that can increment and decrement a counter. You can also get the current value of the counter.".to_string()
    )
    // TODO: [3]
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

/*
 * Remaining questions:
 * 1: Is compile-time safety possible? i.e. check that all injections within registered tools
        are present in the server's context.
   2: What to call "Inject" - sounds Java-y. Maybe State, Context, Ctx, etc.
   3: tools are "magic structs" (`Increment` not actually in source code)
   4: At what level should tool handlers be defined? (currently they must be pure functions, but
        could alternatively be methods on a struct)
   5: things are currently !Send & !Sync. maybe doesn't play well with Tokio's executor. Do we care?
   6: Naming: what to call the structs that are injected. Can't reuse words like 'tools';
      & not necessarily 'state'
 */