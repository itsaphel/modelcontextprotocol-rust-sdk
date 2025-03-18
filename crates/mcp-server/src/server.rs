use crate::context::Inject;
use crate::{context::Context, router::CapabilitiesBuilder, Router};
use async_trait::async_trait;
use mcp_core::{
    handler::{PromptError, ResourceError},
    prompt::Prompt,
    Content, Tool, ToolError, ToolResult,
};
use serde_json::Value;
use std::rc::Rc;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
};

#[async_trait(?Send)]
pub trait CtxToolHandler: 'static {
    /// The name of the tool
    fn name(&self) -> &'static str;

    /// A description of what the tool does
    fn description(&self) -> &'static str;

    /// JSON schema describing the tool's parameters
    fn schema(&self) -> Value;

    /// Execute the tool with the given parameters
    async fn call(&self, context: &Context, params: Value) -> ToolResult<Value>;
}

type Tools = HashMap<String, Rc<dyn CtxToolHandler>>;

/// A higher-level server that handles MCP requests.
#[derive(Clone)]
pub struct MCPServer {
    name: String,
    description: String,
    tools: Rc<Tools>,
    ctx: Rc<Context>,
}

/// Build an MCPServer. Tools and structs are defined when the MCPServer is built. They cannot be
/// modified after that time.
pub struct MCPServerBuilder {
    name: String,
    description: String,
    tools: HashMap<String, Rc<dyn CtxToolHandler>>,
    ctx: Context,
}

impl MCPServerBuilder {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            tools: HashMap::new(),
            ctx: Context::default(),
        }
    }

    pub fn with_tool(mut self, tool: impl CtxToolHandler) -> Self {
        self.tools.insert(tool.name().to_string(), Rc::new(tool));
        self
    }

    pub fn with_state<T: 'static>(mut self, state: Inject<T>) -> Self {
        self.ctx.insert(state);
        self
    }

    pub fn build(self) -> MCPServer {
        MCPServer {
            name: self.name,
            description: self.description,
            tools: Rc::new(self.tools),
            ctx: Rc::new(self.ctx),
        }
    }
}

impl Router for MCPServer {
    fn list_tools(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|(name, tool)| Tool::new(name.clone(), tool.description(), tool.schema()))
            .collect()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn instructions(&self) -> String {
        self.description.clone()
    }

    fn capabilities(&self) -> mcp_core::protocol::ServerCapabilities {
        CapabilitiesBuilder::new()
            .with_tools(self.tools.len() > 0)
            .with_resources(false, false)
            .with_prompts(false)
            .build()
    }

    fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Content>, ToolError>> + '_>> {
        let tool = self.tools.get(tool_name).unwrap().clone();
        Box::pin(async move {
            let res = tool.call(&self.ctx, arguments).await?;
            let contents = match res {
                serde_json::Value::Number(n) => vec![Content::text(n.to_string())],
                serde_json::Value::String(s) => vec![Content::text(s)],
                serde_json::Value::Bool(b) => vec![Content::text(b.to_string())],
                serde_json::Value::Array(_) => serde_json::from_value(res)
                    .map_err(|e| ToolError::ExecutionError(e.to_string()))?,
                serde_json::Value::Null => vec![],
                serde_json::Value::Object(_) => serde_json::from_value(res)
                    .map_err(|e| ToolError::ExecutionError(e.to_string()))?,
            };

            Ok(contents)
        })
    }

    fn list_resources(&self) -> Vec<mcp_core::resource::Resource> {
        todo!()
    }

    fn read_resource(
        &self,
        _uri: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, ResourceError>> + 'static>> {
        todo!()
    }

    fn list_prompts(&self) -> Vec<Prompt> {
        todo!()
    }

    fn get_prompt(
        &self,
        _prompt_name: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, PromptError>> + 'static>> {
        todo!()
    }
}
