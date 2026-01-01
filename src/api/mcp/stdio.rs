use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{
    Arc,
    atomic::{
        AtomicU64,
        Ordering::SeqCst,
    },
};

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{
    io::{
        BufReader,
        AsyncBufReadExt,
        AsyncWriteExt,
    },
    process::{
        Child,
        ChildStdin,
        ChildStdout,
        Command,
    },
    sync::Mutex,
};
use tracing::{event, Level};
use uuid::Uuid;

use crate::{
    error::MyError,
    mcp::{
        MyMcp,
        ProtocolVersion,
        ToolInfo,
    },
};

/// StdIo-based MCP transport using stdin/stdout communication
pub struct StdIoTransport {
    child:         Arc<Mutex<Child>>,
    request_id:    Arc<AtomicU64>,
    stdin:         Arc<Mutex<ChildStdin>>,
    stdout_reader: Arc<Mutex<BufReader<ChildStdout>>>,
}

impl StdIoTransport {
    async fn new(command: &str, args: &Vec<String>, env: &Option<HashMap<String, String>>) -> Result<Self, MyError> {
        // run command
        let mut cmd = Command::new(command);
        if !args.is_empty() {
            cmd.args(args);
        }
        if let Some(envs) = env {
            cmd.envs(envs);
        }
        cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        // stdin
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| MyError::McpError{info: "Failed to get stdin handle".to_string()})?;
        // stdout
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| MyError::McpError{info: "Failed to get stdout handle".to_string()})?;
        let stdout_reader = BufReader::new(stdout);
        // return
        Ok(Self {
            child:         Arc::new(Mutex::new(child)),
            request_id:    Arc::new(AtomicU64::new(1)),
            stdin:         Arc::new(Mutex::new(stdin)),
            stdout_reader: Arc::new(Mutex::new(stdout_reader)),
        })
    }

    /// Sends an MCP request to the child process and returns the response
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, MyError> {
        // Ensure params is an object, not null
        let params = if params.is_null() {
            json!({})
        } else {
            params
        };
        // Generate unique request ID
        let id = self.request_id.fetch_add(1, SeqCst);
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        // Send request via stdin
        let request_line = serde_json::to_string(&request_body).map_err(|e| MyError::JsonToStringError{error: e.into()})? + "\n";
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(request_line.as_bytes()).await?;
        stdin.flush().await?;
        drop(stdin);
        // Read response from stdout
        let mut stdout_reader = self.stdout_reader.lock().await;
        let mut response_line = String::new();
        stdout_reader.read_line(&mut response_line).await?;
        drop(stdout_reader);
        // Check for JSON-RPC errors
        let response_body: Value = serde_json::from_str(response_line.trim()).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        if let Some(error) = response_body.get("error") {
            return Err(MyError::McpError{info: format!("stdio MCP server error: {}", error)});
        }
        // return
        response_body
            .get("result")
            .cloned()
            .ok_or_else(|| MyError::McpError{info: "No result in stdio MCP response".to_string()})
    }

    /// The initialization phase MUST be the first interaction between client and server.
    /// During this phase, the client and server:
    /// - Establish protocol version compatibility
    /// - Exchange and negotiate capabilities
    /// - Share implementation details
    /// https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
    async fn initialize(&self, command: &str) -> Result<(String, String), MyError> {
        //let protocol_versions = ["2025-06-18", "2025-11-25"];
        let protocol_versions = ProtocolVersion::get_all_supported_version();
        for pv in &protocol_versions {
            let current_version = ProtocolVersion::new(pv)?;
            let params = current_version.get_initialize_request_param_value("chatsong", "0.4.0")?;
            match self.send_request("initialize", params).await {
                Ok(result) => {
                    // Parse the MCP initialize result
                    match current_version.get_server_info(result) {
                        Ok((version, name)) => {
                            if &version == pv {
                                return Ok((version, name))
                            } else {
                                event!(Level::WARN, "MCP server {} protocol version not match: {}(server) <--> {}(client)", name, version, pv);
                                continue
                            }
                        }
                        Err(e) => {
                            event!(Level::WARN, "parse MCP {} initialization result: {}", command, e);
                            continue
                        },
                    }
                },
                Err(e) => {
                    event!(Level::WARN, "initialize MCP {}: {}", command, e);
                    continue
                },
            }
        }
        Err(MyError::McpError{info: format!("mcp not support protocol version: {:?}", protocol_versions)})
    }

    /// Terminates the child process and cleans up resources
    /// This method forcefully terminates the MCP server process and closes
    /// all associated pipes. Any pending requests will fail after this call.
    /// The transport cannot be used after closing.
    /// This method sends SIGKILL to the process, which may not allow for
    /// graceful cleanup. Consider implementing graceful shutdown through
    /// MCP protocol methods before calling this method.
    async fn close(&self) -> Result<(), MyError> {
        let mut child = self.child.lock().await;
        child.kill().await?;
        Ok(())
    }

    /// list all tools in this MCP server
    async fn list_tools(&self, command: &str) -> Result<Vec<(String, Option<String>, Value)>, MyError> {
        // send request
        let result = self.send_request("tools/list", Value::Null).await?;
        // Parse the MCP tool result
        let tools = result
            .get("tools")
            .and_then(|t| t.as_array())
            .ok_or_else(|| MyError::McpError{info: format!("Invalid mcp stdio ({}) tools response format", command)})?;
        // get tools info
        let mut tool_infos = Vec::new();
        for tool in tools {
            let name = tool
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or_else(|| MyError::McpError{info: "Tool missing name".to_string()})?
                .to_string();

            let description = tool
                .get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());

            let input_schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));

            tool_infos.push((name, description, input_schema));
        }
        // sort tool by name
        tool_infos.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(tool_infos)
    }

    /// call tool
    async fn call_tool(&self, tool_name: &str, arguments: Value, server_name: &str, protocol_version: &str) -> Result<String, MyError> {
        // send request
        let params = json!({
            "name": tool_name,
            "arguments": arguments
        });
        let result = self.send_request("tools/call", params).await?;
        let current_version = ProtocolVersion::new(protocol_version)?;
        current_version.get_call_tool_result(result, tool_name, server_name)
    }
}

/// Stdio-based MCP server
#[derive(Clone, Deserialize)]
pub struct StdIoServer {
    pub command: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    #[serde(default, skip_serializing_if = "None::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// name from server initialization
    #[serde(skip)]
    pub name: String,

    /// protocol version from server initialization
    #[serde(skip)]
    pub protocol_version: String,
}

/// all mcp stdio servers
pub struct StdIoServers {
    pub id_map: HashMap<String, (StdIoTransport, StdIoServer)>, // key: tool id, value: (StdIoServer, StdIoTransport)
    pub tools:  Vec<ToolInfo>, // all tools
}

impl StdIoServers {
    pub async fn new(mut servers: Vec<StdIoServer>) -> Result<Self, MyError> {
        let mut id_map: HashMap<String, (StdIoTransport, StdIoServer)> = HashMap::new();
        let mut tools: Vec<ToolInfo> = Vec::new();
        for server in servers.iter_mut() {
            let transport = StdIoTransport::new(&server.command, &server.args, &server.env).await?;
            let (server_version, server_name) = transport.initialize(&server.command).await?;
            server.name = server_name.clone(); // name from server initialization
            server.protocol_version = server_version; // protocol version from server initialization
            let id = Uuid::new_v4().to_string();
            tools.extend(transport.list_tools(&server.command).await?.into_iter().map(|t| ToolInfo {
                name_id:     format!("{}__{}", t.0, id), // name__id
                name:        t.0, // tool name
                id:          id.clone(), // server id
                description: t.1,
                schema:      t.2,
                server_name: server_name.clone(),
            }).collect::<Vec<ToolInfo>>());
            id_map.insert(id, (transport, server.clone()));
        }
        // sort server by name
        tools.sort_by(|a, b| a.server_name.cmp(&b.server_name));
        Ok(Self {id_map, tools})
    }

    // close all mcp servers
    pub async fn close_all(&self) {
        for (_, (transport, server)) in &self.id_map {
            if let Err(e) = transport.close().await {
                event!(Level::WARN, "close MCP server {} error: {}", server.name, e);
            }
        }
    }
}

impl MyMcp for StdIoServers {
    /// run command
    async fn run(&self, id: &str, tool_name: &str, args: &str) -> Result<String, MyError> {
        match self.id_map.get(id) {
            Some(server) => {
                let json_args: Value = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
                server.0.call_tool(tool_name, json_args, &server.1.name, &server.1.protocol_version).await
            },
            None => Err(MyError::ToolNotExistError{id: id.to_string(), info: "StdIoServers::run()".to_string()}),
        }
    }

    /// get all selected tools name (name format: `name__id`, max name length is 26), description and schema
    fn get_desc_and_schema(&self, selected_tools: Vec<String>) -> Vec<ToolInfo> {
        self.tools.iter().filter(|t| selected_tools.contains(&t.name_id)).map(|t| t.clone()).collect()
    }

    /// select all tools, return name__id vector
    fn select_all_tools(&self) -> Vec<String> {
        let mut selected_tools: Vec<String> = Vec::new();
        for tool in self.tools.iter() {
            selected_tools.push(tool.name_id.clone());
        }
        selected_tools
    }

    /// select tools by server id, return selected name__id vector
    fn select_tools_by_server_id(&self, id: &str) -> Vec<String> {
        let mut selected_tools: Vec<String> = Vec::new();
        for tool in self.tools.iter() {
            if tool.id == id {
                selected_tools.push(tool.name_id.clone());
            }
        }
        selected_tools
    }

    /// select tool by tool name and server id, return selected name__id vector
    fn select_tool_by_name_and_id(&self, name_id: &str) -> Vec<String> {
        let mut selected_tools: Vec<String> = Vec::new();
        for tool in self.tools.iter() {
            if tool.name_id == name_id {
                selected_tools.push(tool.name_id.clone());
                break
            }
        }
        selected_tools
    }
}
