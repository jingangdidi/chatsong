use std::fs;
use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::{
            validate_path,
            FileInfo,
        },
    },
};

/// params for integer get_file_info
#[derive(Deserialize)]
struct Params {
    path: String,
}

/// built-in tool
pub struct GetFileInfo;

impl GetFileInfo {
    /// new
    pub fn new() -> Self {
        GetFileInfo
    }
}

impl BuiltIn for GetFileInfo {
    /// get tool name
    fn name(&self) -> String {
        "get_file_info".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Retrieve detailed metadata about a file or directory. Returns comprehensive information including size, creation time, last modified time, permissions, and type. This tool is perfect for understanding file characteristics without reading the actual content. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to get information for.",
                },
            },
            "required": ["path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(&params.path), true)?;
        let metadata = fs::metadata(valid_path)?;

        let size = metadata.len();
        let created = metadata.created().ok();
        let modified = metadata.modified().ok();
        let accessed = metadata.accessed().ok();
        let is_directory = metadata.is_dir();
        let is_file = metadata.is_file();

        let file_info = FileInfo {
            size,
            created,
            modified,
            accessed,
            is_directory,
            is_file,
            metadata,
        };

        Ok(format!("successfully get file info: {:?}", file_info))
    }
}
