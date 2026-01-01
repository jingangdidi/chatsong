use std::fs::write;
use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::validate_path,
    },
};

/// params for integer write_file
#[derive(Deserialize)]
struct Params {
    file_path: String,
    content:   String,
}

/// built-in tool
pub struct WriteFile;

impl WriteFile {
    /// new
    pub fn new() -> Self {
        WriteFile
    }
}

impl BuiltIn for WriteFile {
    /// get tool name
    fn name(&self) -> String {
        "write_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Create a new file or completely overwrite an existing file with new content. Use with caution as it will overwrite existing files without warning. Handles text content with proper encoding. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path of the file to write to.",
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file.",
                },
            },
            "required": ["file_path", "content"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(&params.file_path), false)?;
        write(valid_path, params.content)?;
        Ok(format!("Successfully wrote to {}", params.file_path))
    }
}
