use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::list_directory_helper,
    },
};

/// params for integer list_directory
#[derive(Deserialize)]
struct Params {
    dir_path: String,
}

/// built-in tool
pub struct ListDirectory;

impl ListDirectory {
    /// new
    pub fn new() -> Self {
        ListDirectory
    }
}

impl BuiltIn for ListDirectory {
    /// get tool name
    fn name(&self) -> String {
        "list_directory".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Get a detailed listing of all files and directories in a specified path. Results clearly distinguish between files and directories with [FILE] and [DIR] prefixes. This tool is essential for understanding directory structure and finding specific files within a directory. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "dir_path": {
                    "type": "string",
                    "description": "The path of the directory to list.",
                },
            },
            "required": ["dir_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let entries = list_directory_helper(&params.dir_path)?;
        let formatted = entries
            .iter()
            .map(|entry| {
                format!(
                    "{} {}",
                    if entry.path().is_dir() {
                        "[DIR]"
                    } else {
                        "[FILE]"
                    },
                    entry.file_name().to_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(format!("successfully get all files and directories:\n{}", formatted))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
