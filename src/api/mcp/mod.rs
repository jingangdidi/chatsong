use openai_dive::v1::resources::chat::{
    ChatCompletionFunction,
    ChatCompletionTool,
    ChatCompletionToolType,
};
use serde_json::Value;

use crate::{
    error::MyError,
    tools::SelectedTools,
};

mod protocol_version_2025_06_18;
mod protocol_version_2025_11_25;
pub mod stdio;

use stdio::StdIoServers;

/// initialize request
pub trait InitRequest {
    fn to_value(name: &str, version: &str) -> Result<Value, MyError>;
}

/// initialize result
pub trait InitResult {
    fn get_server_info(value: Value) -> Result<(String, String), MyError>;
}

/// call tool result
pub trait CallTool {
    fn get_call_tool_result(value: Value, tool_name: &str, server_name: &str) -> Result<String, MyError>;
}

/// protocol version
pub enum ProtocolVersion {
    Version20250618,
    Version20251125,
}

impl ProtocolVersion {
    /// from str
    pub fn new(version: &str) -> Result<Self, MyError> {
        match version {
            "2025-06-18" => Ok(Self::Version20250618),
            "2025-11-25" => Ok(Self::Version20251125),
            v            => Err(MyError::McpError{info: format!("mcp protocol version only support 2025-06-18 and 2025-11-25, not {}", v)}),
        }
    }

    /// all supported version
    pub fn get_all_supported_version() -> Vec<String> {
        vec!["2025-06-18".to_string(), "2025-11-25".to_string()]
    }

    /// create initialize request param
    pub fn get_initialize_request_param_value(&self, name: &str, version: &str) -> Result<Value, MyError> {
        match self {
            Self::Version20250618 => protocol_version_2025_06_18::InitializeRequestParams::to_value(name, version),
            Self::Version20251125 => protocol_version_2025_11_25::InitializeRequestParams::to_value(name, version),
        }
    }

    /// parse initialize result
    pub fn get_server_info(&self, value: Value) -> Result<(String, String), MyError> {
        match self {
            Self::Version20250618 => protocol_version_2025_06_18::InitializeResult::get_server_info(value),
            Self::Version20251125 => protocol_version_2025_11_25::InitializeResult::get_server_info(value),
        }
    }

    /// parse call tool result
    pub fn get_call_tool_result(&self, value: Value, tool_name: &str, server_name: &str) -> Result<String, MyError> {
        match self {
            Self::Version20250618 => protocol_version_2025_06_18::CallToolResult::get_call_tool_result(value, tool_name, server_name),
            Self::Version20251125 => protocol_version_2025_11_25::CallToolResult::get_call_tool_result(value, tool_name, server_name),
        }
    }
}

/// tool name, description, schema
#[derive(Clone)]
pub struct ToolInfo {
    pub name_id:     String, // name__id
    pub name:        String, // tool name
    pub id:          String, // server id
    pub description: Option<String>,
    pub schema:      Value,
    pub server_name: String,
}

/// https://docs.rs/rust-mcp-sdk/0.7.4/rust_mcp_sdk/struct.StdioTransport.html
/// https://docs.rs/rust-mcp-transport/0.6.3/src/rust_mcp_transport/stdio.rs.html#90-95
/// https://github.com/rust-mcp-stack/rust-mcp-sdk/blob/main/examples/simple-mcp-client-stdio/src/main.rs
/// https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
/// 

/// trait for MCP stdio servers & http servers
pub trait MyMcp {
    /// run tool
    fn run(&self, id: &str, tool_name: &str, args: &str) -> impl std::future::Future<Output = Result<String, MyError>> + Send;

    /// get all selected tools name (name format: `name__id`, max name length is 26), description and schema
    fn get_desc_and_schema(&self, selected_tools: Vec<String>) -> Vec<ToolInfo>;

    /// select all tools, return name__id vector
    fn select_all_tools(&self) -> Vec<String>;

    /// select tools by server id, return selected name__id vector
    fn select_tools_by_server_id(&self, id: &str) -> Vec<String>;

    /// select tool by tool name and server id, return selected name__id vector
    fn select_tool_by_name_and_id(&self, name: &str) -> Vec<String>;
}

/// all servers: stdio tools & http tools
pub struct McpServers {
    stdio: StdIoServers,
    //http:  ExternalTools,
    pub html: String, // html pulldown options
}

impl McpServers {
    /// create new Tools based on config file
    pub fn new(stdio: StdIoServers, english: bool) -> Self {
        let mut options: Vec<String> = Vec::with_capacity(2 + stdio.id_map.len() * 2 + stdio.tools.len());
        // stdio pulldown options
        if !stdio.tools.is_empty() {
            if english {
                options.push("<optgroup label='MCP tools'>".to_string());
                options.push("                    <option value='select_all_mcp'>🟡 select all MCP tools</option>".to_string());
            } else {
                options.push("<optgroup label='MCP工具'>".to_string());
                options.push("                    <option value='select_all_mcp'>🟡 选择所有MCP工具</option>".to_string());
            }
            let mut server_name = &stdio.tools[0].server_name;
            options.push(format!("                    <option disabled>--{}--</option>", server_name));
            if english {
                options.push(format!("                    <option value='mcp_server_{}'>🟡 select all {}</option>", &stdio.tools[0].id, server_name));
            } else {
                options.push(format!("                    <option value='mcp_server_{}'>🟡 选择所有{}</option>", &stdio.tools[0].id, server_name));
            }
            for tool in &stdio.tools {
                if server_name != &tool.server_name {
                    server_name = &tool.server_name;
                    options.push(format!("                    <option disabled>--{}--</option>", server_name));
                    if english {
                        options.push(format!("                    <option value='mcp_server_{}'>🟡 select all {}</option>", tool.id, server_name));
                    } else {
                        options.push(format!("                    <option value='mcp_server_{}'>🟡 选择所有{}</option>", tool.id, server_name));
                    }
                }
                if let Some(desc) = &tool.description {
                    options.push(format!("                    <option value='{}' title=\"{}\">{}</option>", tool.name_id, desc.replace("\"", "&quot;"), tool.name));
                } else {
                    options.push(format!("                    <option value='{}'>{}</option>", tool.name_id, tool.name));
                }
            }
            options.push("                </optgroup>".to_string());
        }
        // return
        Self {stdio, html: options.join("\n")}
    }

    /// run tool
    pub async fn run(&self, name_id: &[&str], args: &str) -> Result<String, MyError> {
        if self.stdio.id_map.contains_key(name_id[1]) {
            self.stdio.run(name_id[1], name_id[0], args).await
        } else {
            Err(MyError::ToolNotExistError{id: name_id[1].to_string(), info: "Tools::run()".to_string()})
        }
    }

    /// get all selected tools for LLM api function calling
    /// tool name format: `name__id`, max name length is 26
    pub async fn get_desc_and_schema(&self, selected_tools: &Option<SelectedTools>) -> Result<Vec<ChatCompletionTool>, MyError> {
        // get selected tools
        let selected_tools_vec = match selected_tools {
            Some(s) => match s {
                SelectedTools::All | SelectedTools::AllMcp => { // all tools
                    self.stdio.select_all_tools()
                    //self.http.select_all_tools()
                },
                SelectedTools::AllBuiltIn => Vec::new(), // all built-in tools
                SelectedTools::AllExternal => Vec::new(), // all external tools
                SelectedTools::Group(_) => Vec::new(), // built-in group, select all tools of one group
                SelectedTools::Single(_) => Vec::new(), // single built-in or external tool id
                SelectedTools::McpServer(server_id) => self.stdio.select_tools_by_server_id(server_id), // single mcp server id, start with `mcp_server_`, select all tools of one server
                SelectedTools::McpTool(name_id) => self.stdio.select_tool_by_name_and_id(name_id), // single mcp tool `name__id`, select by tool name and server id
            },
            None => Vec::new(), // not select any tool
        };
        // get name, description and schema
        let mut tools: Vec<ChatCompletionTool> = Vec::new();
        let desc_and_schema = self.stdio.get_desc_and_schema(selected_tools_vec);
        //desc_and_schema.extend(self.http.get_desc_and_schema());
        for tool in desc_and_schema {
            tools.push(
                ChatCompletionTool {
                    r#type: ChatCompletionToolType::Function,
                    function: ChatCompletionFunction {
                        name: tool.name_id,
                        description: tool.description,
                        parameters: tool.schema,
                    },
                }
            );
        }
        Ok(tools)
    }

    /// check contain server id
    pub fn contain_server_id(&self, id: &str) -> bool {
        if self.stdio.id_map.contains_key(id) {
            true
        } else {
            false
        }
    }

    // close all mcp servers
    pub async fn close_all(&self) {
        self.stdio.close_all().await;
    }
}
