use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::read_file_helper,
    },
};

/// params for integer read_file
#[derive(Deserialize)]
struct Params {
    file_path: String,
}

/// built-in tool
pub struct ReadFile;

impl ReadFile {
    /// new
    pub fn new() -> Self {
        ReadFile
    }
}

impl BuiltIn for ReadFile {
    /// get tool name
    fn name(&self) -> String {
        "read_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Read the complete contents of a file from the file system. Handles various text encodings and provides detailed error messages if the file cannot be read. Use this tool when you need to examine the contents of a single file. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path of the file to read.",
                },
            },
            "required": ["file_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let content = read_file_helper(&params.file_path)?;
        //Ok(format!("Successfully read file:\n{}", content))
        Ok(content)
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
