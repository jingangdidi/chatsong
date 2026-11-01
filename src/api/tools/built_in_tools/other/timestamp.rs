use chrono::Local;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

/// 默认格式：可作为文件名，无空格/冒号等非法字符
const DEFAULT_FORMAT: &str = "%Y-%m-%d_%H-%M-%S";

/// params for timestamp
#[derive(Deserialize)]
pub struct Params {
    /// 可选的 strftime 格式串，缺省为 DEFAULT_FORMAT
    #[serde(default)]
    pub format: Option<String>,
}

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
        format!("Returns the current local timestamp. Optionally accepts a strftime `format` string; defaults to `{DEFAULT_FORMAT}` which yields strings such as `2025-01-19_15-21-56` (safe to use as a filename).")
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "format": {
                    "type": "string",
                    "description": "Optional strftime / chrono format string. Defaults to '%Y-%m-%d_%H-%M-%S'.",
                },
            },
            "required": [],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = parse_tool_args(args, ArgFixSpec {array_fields: None, object_fields: None})?;

        let format = params.format.as_deref().unwrap_or(DEFAULT_FORMAT);
        // 例：2025-01-19_15-21-56
        let timestamp = Local::now().format(format).to_string();

        // 如果需要反向校验/解析（比如走默认格式），可以这样做：
        // let dt = chrono::NaiveDateTime::parse_from_str(&timestamp, format)
        //     .map_err(|e| MyError::OtherError { error: format!("invalid format: {e}") })?;
        // let _: chrono::NaiveDateTime = dt;

        Ok((timestamp, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
