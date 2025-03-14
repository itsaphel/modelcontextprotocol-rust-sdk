use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use mcp_core::{handler::{PromptError, ResourceError, ToolHandler}, prompt::Prompt, Content, Tool, ToolError};
use crate::{router::CapabilitiesBuilder, Router};

/// A higher-level server that handles MCP requests.
#[derive(Clone)]
pub struct MCPServer {
    pub name: String,
    pub description: String,
    pub tools: HashMap<String, Arc<dyn ToolHandler>>,
}

impl MCPServer {
    pub fn new(name: String, description: String) -> Self {
        Self { name, description, tools: HashMap::new() }
    }

    pub fn register_tool(&mut self, tool: impl ToolHandler) {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }
}

impl Router for MCPServer {
    fn list_tools(&self) -> Vec<Tool> {
        self.tools.iter().map(|(name, tool)| Tool::new(name.clone(), tool.description(), tool.schema())).collect()
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
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Content>, ToolError>> + Send + 'static>> {
        let tool = self.tools.get(tool_name).unwrap().clone();
        Box::pin(async move { 
            let res = tool.call(arguments).await?;
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
    ) -> Pin<Box<dyn Future<Output = Result<String, ResourceError>> + Send + 'static>> {
        todo!()
    }
    
    fn list_prompts(&self) -> Vec<Prompt> {
        todo!()
    }
    
    fn get_prompt(&self, _prompt_name: &str) -> Pin<Box<dyn Future<Output = Result<String, PromptError>> + Send + 'static>> {
        todo!()
    }
}