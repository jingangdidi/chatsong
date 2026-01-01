use std::fs::create_dir_all;
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

/// params for integer create_directory
#[derive(Deserialize)]
struct Params {
    path: String,
}

/// built-in tool
pub struct CreateDirectory;

impl CreateDirectory {
    /// new
    pub fn new() -> Self {
        CreateDirectory
    }
}

impl BuiltIn for CreateDirectory {
    /// get tool name
    fn name(&self) -> String {
        "create_directory".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Create a new directory or ensure a directory exists. Can create multiple nested directories in one operation. If the directory already exists, this operation will succeed silently. Perfect for setting up directory structures for projects or ensuring required paths exist. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path where the directory will be created.",
                },
            },
            "required": ["path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(&params.path), false)?;
        create_dir_all(&valid_path)?;
        Ok(format!("successfully created directory {}", params.path))
    }
}
