use std::fs::rename;
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

/// params for integer move_file
#[derive(Deserialize)]
struct Params {
    src_path:  String,
    dest_path: String,
}

/// built-in tool
pub struct MoveFile;

impl MoveFile {
    /// new
    pub fn new() -> Self {
        MoveFile
    }
}

impl BuiltIn for MoveFile {
    /// get tool name
    fn name(&self) -> String {
        "move_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Move or rename files and directories. Can move files between directories and rename them in a single operation. If the destination exists, the operation will fail. Works across different directories and can be used for simple renaming within the same directory. Both source and destination must be within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "src_path": {
                    "type": "string",
                    "description": "The source path of the file to move.",
                },
                "dest_path": {
                    "type": "string",
                    "description": "The destination path to move the file to.",
                },
            },
            "required": ["src_path", "dest_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let valid_src_path = validate_path(&PARAS.allowed_path, Path::new(&params.src_path), true)?;
        let valid_dest_path = validate_path(&PARAS.allowed_path, Path::new(&params.dest_path), false)?;
        rename(valid_src_path, valid_dest_path)?;
        Ok(format!("Successfully move {} to {}", &params.src_path, &params.dest_path))
    }
}
