use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

/// params for sub-agent
#[derive(Deserialize)]
struct Params {
    prompt: String,
    #[serde(default)]
    tools: Option<Vec<String>>,
}

/// sub agent
pub struct SubAgent;

impl SubAgent {
    /// new
    pub fn new() -> Self {
        SubAgent
    }
}

impl BuiltIn for SubAgent {
    /// get tool name
    fn name(&self) -> String {
        "sub_agent".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        //"Spawn a dedicated sub-agent to handle a complex, multi-step task autonomously. The sub-agent runs its own agentic loop with access to the specified tools and returns the final result when done. Use this to delegate focused subtasks (e.g., research, multi-step reasoning) without polluting the main agent's conversation history. Do not ask the user questions, and finish with a concise result.".to_string()
        "Spawn a dedicated sub-agent to handle a focused, context-heavy, or multi-step subtask autonomously.

Use this when a task is broad, exploratory, decomposable, requires many tool calls, needs independent verification, or would pollute the main agent's context.

The main agent should use sub_agent to investigate, research, inspect files, compare options, verify assumptions, review code, analyze documents, or handle isolated implementation subtasks.

The sub-agent receives:
- prompt: a clear bounded task
- tools: the allowed tools for that task

The sub-agent must not ask the user questions. It should complete the task with the provided tools and return a concise result including key findings, evidence, uncertainty, and next steps.".to_string()
    }

    /// get tool schema
    /// https://github.com/pacifio/cersei/blob/main/crates/cersei-agent/src/agent_tool.rs#L59
    /// https://github.com/zeroclaw-labs/zeroclaw/blob/master/crates/zeroclaw-runtime/src/tools/spawn_subagent.rs#L79
    /// https://github.com/moltis-org/moltis/blob/main/crates/tools/src/spawn_agent.rs#L366
    /// https://github.com/microclaw/microclaw/blob/main/src/tools/subagents.rs#L730C41-L731C1
    /// https://github.com/ultraworkers/claw-code/blob/main/rust/crates/tools/src/lib.rs#L4244C10-L4245C8
    /// https://github.com/eikarna/hermes-rs/blob/main/crates/hermes-core/src/tools/sub_agent_tool.rs#L20
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task or question for the SubAgent. Be specific and self-contained — the SubAgent does not see this conversation's history.",
                },
                "tools": {
                    "type": ["array", "null"],
                    "items": {"type": "string"},
                    "description": "Optional list of tool names for the sub-agent. If omitted, the sub-agent operates without additional tools.",
                },
            },
            "required": ["prompt"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        let mut result = params.prompt;
        if let Some(tools) = params.tools {
            result += "---srx---";
            result += &tools.join("---srx---");
        }
        Ok((result, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        if is_en {
            Ok(Some(format!("Do you allow calling the sub-agent tool ?{}", info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 sub-agent 工具？{}", info.unwrap_or_default())))
        }
    }
}
