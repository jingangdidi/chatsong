use serde::Deserialize;
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

/// 更新 goal 的参数
#[derive(Debug, Deserialize)]
struct Params {
    status: String, // complete or blocked
}

pub struct UpdateGoalStatus;

impl UpdateGoalStatus {
    pub fn new() -> Self {
        UpdateGoalStatus
    }
}

impl BuiltIn for UpdateGoalStatus {
    /// get tool name
    fn name(&self) -> String {
        "update_goal_status".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Updates the current goal's status to either completed or blocked. Call this tool only when the conditions described in the goal context are fully met. Do not call it preemptively or for intermediate progress updates.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["complete", "blocked"],
                    "description": "The new status to set. Must be one of 'complete' or 'blocked'"
                },
            },
            "required": ["status"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;
        Ok((params.status, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
