use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    error::MyError,
    mcp::{
        InitRequest,
        InitResult,
        CallTool,
    },
};

/*
This request is sent from the client to the server when it first connects, asking it to begin initialization.
*/

/// 2025-06-18 <--diff--> 2025-11-25
/// https://modelcontextprotocol.io/specification/2025-06-18/schema#initializerequest
/// https://modelcontextprotocol.io/specification/2025-11-25/schema#initializerequestparams
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitializeRequestParams {
    pub capabilities: ClientCapabilities,

    #[serde(rename = "clientInfo")]
    pub client_info: Implementation,

    /// The latest version of the Model Context Protocol that the client supports. The client MAY decide to support older versions as well.
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
}

impl InitRequest for InitializeRequestParams {
    fn to_value(name: &str, version: &str) -> Result<Value, MyError> {
        let params = Self {
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::new(name, version),
            protocol_version: "2025-06-18".to_string(),
        };
        serde_json::to_value(&params).map_err(|e| MyError::StructToJsonValueError{error: e})
    }
}

/// Capabilities a client may support. Known capabilities are defined here, in this schema, but this is not a closed set: any client can define its own, additional capabilities.
/// https://modelcontextprotocol.io/specification/2025-06-18/schema#clientcapabilities
/// https://docs.rs/rust-mcp-schema/0.7.5/src/rust_mcp_schema/generated_schema/2025_06_18/mcp_schema.rs.html#575-589
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ClientCapabilities {
    /// Present if the client supports elicitation from the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<Map<String, Value>>,

    /// Experimental, non-standard capabilities that the client supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, Map<String, Value>>>,

    /// Present if the client supports listing roots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<ClientCapabilitiesRoots>,

    /// Present if the client supports sampling from an LLM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<Map<String, Value>>,
}

/// https://docs.rs/rust-mcp-schema/0.7.5/src/rust_mcp_schema/generated_schema/2025_06_18/mcp_schema.rs.html#608-612
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientCapabilitiesRoots {
    /// Whether the client supports notifications for changes to the roots list.
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Describes the name and version of an MCP implementation, with an optional title for UI representation.
/// https://modelcontextprotocol.io/specification/2025-06-18/schema#implementation
/// https://docs.rs/rust-mcp-schema/0.7.5/src/rust_mcp_schema/generated_schema/2025_06_18/mcp_schema.rs.html#2324-2335
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Implementation {
    /// Intended for programmatic or logical use, but used as a display name in past specs or fallback (if title isn’t present).
    pub name: String,

    /// Intended for UI and end-user contexts — optimized to be human-readable and easily understood, even by those unfamiliar with domain-specific terminology.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    pub version: String,
}

impl Implementation {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            title: None,
            version: version.to_string(),
        }
    }
}

/*
After receiving an initialize request from the client, the server sends this response.
*/

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#initializeresult
/// https://docs.rs/rust-mcp-sdk/0.7.4/rust_mcp_sdk/schema/struct.InitializeResult.html
#[derive(Deserialize)]
#[allow(unused)]
pub struct InitializeResult {
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Map<String, Value>>,

    pub capabilities: ServerCapabilities,

    /// Instructions describing how to use the server and its features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// The version of the Model Context Protocol that the server wants to use.
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,

    #[serde(rename = "serverInfo")]
    pub server_info: Implementation,
}

impl InitResult for InitializeResult {
    fn get_server_info(value: Value) -> Result<(String, String), MyError> {
        match serde_json::from_value::<Self>(value) {
            Ok(init_result) => Ok((init_result.protocol_version, init_result.server_info.name)),
            Err(e) => Err(MyError::SerdeJsonToStructError{error: e}),
        }
    }
}

/// Capabilities that a server may support. Known capabilities are defined here, in this schema, but this is not a closed set: any server can define its own, additional capabilities.
/// https://modelcontextprotocol.io/specification/2025-06-18/schema#servercapabilities
/// https://docs.rs/rust-mcp-sdk/0.7.4/rust_mcp_sdk/schema/struct.ServerCapabilities.html
#[derive(Deserialize)]
#[allow(unused)]
pub struct ServerCapabilities {
    /// Present if the server supports argument autocompletion suggestions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completions: Option<Map<String, Value>>,

    /// Experimental, non-standard capabilities that the server supports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, Map<String, Value>>>,

    /// Present if the server supports sending log messages to the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Map<String, Value>>,

    /// Present if the server offers any prompt templates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<ServerCapabilitiesPrompts>,

    /// Present if the server offers any resources to read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ServerCapabilitiesResources>,

    /// Present if the server offers any tools to call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ServerCapabilitiesTools>,
}

/// https://docs.rs/rust-mcp-sdk/0.7.4/rust_mcp_sdk/schema/struct.ServerCapabilitiesPrompts.html
#[derive(Deserialize)]
#[allow(unused)]
pub struct ServerCapabilitiesPrompts {
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// https://docs.rs/rust-mcp-sdk/0.7.4/rust_mcp_sdk/schema/struct.ServerCapabilitiesResources.html
#[derive(Deserialize)]
#[allow(unused)]
pub struct ServerCapabilitiesResources {
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
}

/// https://docs.rs/rust-mcp-sdk/0.7.4/rust_mcp_sdk/schema/struct.ServerCapabilitiesTools.html
#[derive(Deserialize)]
#[allow(unused)]
pub struct ServerCapabilitiesTools {
    #[serde(rename = "listChanged", default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/*
The server’s response to a tool call.
*/

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#calltoolresult
#[derive(Deserialize)]
#[allow(unused)]
pub struct CallToolResult {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// A list of content objects that represent the unstructured result of the tool call.
    pub content: Vec<ContentBlock>,

    /// Whether the tool call ended in an error.
    #[serde(rename = "isError", default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,

    /// An optional JSON object that represents the structured result of the tool call.
    #[serde(rename = "structuredContent", default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Map<String, Value>>,
}

impl CallTool for CallToolResult {
    fn get_call_tool_result(value: Value, tool_name: &str, server_name: &str) -> Result<String, MyError> {
        // Parse the MCP tool result
        let tool_result: Self = serde_json::from_value(value).map_err(|e| MyError::SerdeJsonToStructError{error: e})?;
        // get result content
        let result_text = tool_result.content.into_iter().map(|c| {
            // currently only support TextContent
            if let ContentBlock::TextContent(t) = c {
                t.text
            } else {
                "".to_string()
            }
        }).collect::<Vec<String>>().join("\n");
        // Check if the result indicates an error
        if tool_result.is_error.unwrap_or(false) {
            return Err(MyError::McpError{info: format!("mcp stdio tool {} ({}) execution failed: {:?}", server_name, tool_name, result_text)});
        }
        Ok(result_text)
    }
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#contentblock
#[derive(Deserialize)]
#[serde(untagged)]
#[allow(unused)]
pub enum ContentBlock {
    TextContent(TextContent),
    ImageContent(ImageContent),
    AudioContent(AudioContent),
    ResourceLink(ResourceLink),
    EmbeddedResource(EmbeddedResource),
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#role
#[derive(Deserialize)]
pub enum Role {
    #[serde(rename = "assistant")]
    Assistant,

    #[serde(rename = "user")]
    User,
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#annotations
#[derive(Deserialize)]
#[allow(unused)]
pub struct Annotations {
    //// Describes who the intended audience of this object or data is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audience: Vec<Role>,

    /// The moment the resource was last modified, as an ISO 8601 formatted string.
    #[serde(rename = "lastModified", default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,

    /// Describes how important this data is for operating the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#textcontent
#[derive(Deserialize)]
#[allow(unused)]
pub struct TextContent {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// Optional annotations for the client. The client can use annotations to inform how objects are used or displayed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,

    /// The text content of the message.
    pub text: String,

    /// type: text
    #[serde(rename = "type")]
    pub type_: String,
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#imagecontent
#[derive(Deserialize)]
#[allow(unused)]
pub struct ImageContent {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// Optional annotations for the client. The client can use annotations to inform how objects are used or displayed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,

    /// The base64-encoded image data.
    pub data: String,

    /// The MIME type of the image. Different providers may support different image types.
    #[serde(rename = "mimeType")]
    pub mime_type: String,

    /// type: image
    #[serde(rename = "type")]
    pub type_: String,
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#audiocontent
#[derive(Deserialize)]
#[allow(unused)]
pub struct AudioContent {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// Optional annotations for the client. The client can use annotations to inform how objects are used or displayed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,

    /// The base64-encoded audio data.
    pub data: String,

    /// The MIME type of the image. Different providers may support different image types.
    #[serde(rename = "mimeType")]
    pub mime_type: String,

    /// type: audio
    #[serde(rename = "type")]
    pub type_: String,
}

/// 2025-06-18 <--diff--> 2025-11-25
/// https://modelcontextprotocol.io/specification/2025-06-18/schema#resourcelink
/// https://modelcontextprotocol.io/specification/2025-11-25/schema#resourcelink
#[derive(Deserialize)]
#[allow(unused)]
pub struct ResourceLink {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// Optional annotations for the client. The client can use annotations to inform how objects are used or displayed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,

    /// A description of what this resource represents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The MIME type of the image. Different providers may support different image types.
    #[serde(rename = "mimeType")]
    pub mime_type: String,

    /// Intended for programmatic or logical use, but used as a display name in past specs or fallback (if title isn’t present).
    pub name: String,

    /// The size of the raw resource content, in bytes (i.e., before base64 encoding or any tokenization), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,

    /// Intended for UI and end-user contexts — optimized to be human-readable and easily understood, even by those unfamiliar with domain-specific terminology.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// type: resource_link
    #[serde(rename = "type")]
    pub type_: String,

    /// The URI of this resource.
    pub uri: String,
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#textresourcecontents
#[derive(Deserialize)]
#[allow(unused)]
pub struct TextResourceContents {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// The MIME type of the image. Different providers may support different image types.
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// The text of the item. This must only be set if the item can actually be represented as text (not binary data).
    pub text: String,

    /// The URI of this resource.
    pub uri: String,
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#blobresourcecontents
#[derive(Deserialize)]
#[allow(unused)]
pub struct BlobResourceContents {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// The MIME type of the image. Different providers may support different image types.
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// A base64-encoded string representing the binary data of the item.
    pub blob: String,

    /// The URI of this resource.
    pub uri: String,
}

#[derive(Deserialize)]
#[allow(unused)]
pub enum EmbeddedResourceResource {
    TextResourceContents(TextResourceContents),
    BlobResourceContents(BlobResourceContents),
}

/// https://modelcontextprotocol.io/specification/2025-06-18/schema#embeddedresource
#[derive(Deserialize)]
#[allow(unused)]
pub struct EmbeddedResource {
    /// allow clients and servers to attach additional metadata to their interactions.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, Value>>,

    /// Optional annotations for the client. The client can use annotations to inform how objects are used or displayed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,

    pub resource: EmbeddedResourceResource,

    /// type: resource
    #[serde(rename = "type")]
    pub type_: String,
}
