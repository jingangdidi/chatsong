use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::BuiltIn,
};

/// built-in tool
pub struct ListAllowedDirectories;

impl ListAllowedDirectories {
    /// new
    pub fn new() -> Self {
        ListAllowedDirectories
    }
}

impl BuiltIn for ListAllowedDirectories {
    /// get tool name
    fn name(&self) -> String {
        "list_allowed_directories".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Returns a list of directories that the server has permission to access Subdirectories within these allowed directories are also accessible. Use this to identify which directories and their nested paths are available before attempting to access files.".to_string()
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
    fn run(&self, _args: &str) -> Result<String, MyError> {
        Ok(format!(
            "allowed directories:\n{}",
            &PARAS
                .allowed_path
                .iter()
                .map(|entry| entry.1.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}
