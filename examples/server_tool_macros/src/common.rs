use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use mcp_core::{handler::{PromptError, ResourceError, ToolHandler}, prompt::Prompt, Content, Tool, ToolError};
use mcp_server::{router::CapabilitiesBuilder, Router};

#[derive(Clone, Default)]
pub struct MCPServer {
    pub tools: HashMap<String, Arc<dyn ToolHandler>>,
}

impl Router for MCPServer {
    fn list_tools(&self) -> Vec<Tool> {
        self.tools.iter().map(|(name, tool)| Tool::new(name.clone(), tool.description(), tool.schema())).collect()
    }
    
    fn name(&self) -> String {
        "Stateless server".to_string()
    }
    
    fn instructions(&self) -> String {
        "This server provides a calculator tool that can perform basic arithmetic operations. Use the 'calculator' tool to perform calculations.".to_string()
    }
    
    fn capabilities(&self) -> mcp_core::protocol::ServerCapabilities {
        CapabilitiesBuilder::new()
            .with_tools(true)
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
                _ => vec![Content::text(format!("{:?}", res))],
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