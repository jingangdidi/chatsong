use chrono::Local;
use serde_json::{json, Value};

use crate::{
    error::MyError,
    tools::built_in_tools::BuiltIn,
};

/// built-in tool
pub struct Timestamp;

impl Timestamp {
    /// new
    pub fn new() -> Self {
        Timestamp
    }
}

impl BuiltIn for Timestamp {
    /// get tool name
    fn name(&self) -> String {
        "timestamp".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Returns the current local timestamp with timezone, e.g. `2025-01-19 15:21:56 +08:00`.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {},
            "required": [],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, _args: &str) -> Result<(String, Option<String>), MyError> {
        // 带时区、年月日时分秒，如：2025-01-19 15:21:56 +08:00
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S %:z").to_string();
        Ok((timestamp, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
