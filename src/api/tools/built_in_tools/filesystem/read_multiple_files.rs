use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::read_file_helper,
    },
};

/// params for integer read_multiple_files
#[derive(Deserialize)]
struct Params {
    paths: Vec<String>,
}

/// built-in tool
pub struct ReadMultipleFiles;

impl ReadMultipleFiles {
    /// new
    pub fn new() -> Self {
        ReadMultipleFiles
    }
}

impl BuiltIn for ReadMultipleFiles {
    /// get tool name
    fn name(&self) -> String {
        "read_multiple_files".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Read the contents of multiple files simultaneously. This is more efficient than reading files one by one when you need to analyze or compare multiple files. Each file's content is returned with its path as a reference. Failed reads for individual files won't stop the entire operation. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "paths": {
                    "type": "array",
                    "description": "The list of file paths to read.",
                },
            },
            "required": ["paths"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

        let contents: Vec<String> = params
            .paths
            .iter()
            .map(|path| {
                match read_file_helper(&path) {
                    Ok(c) => format!("{path}:\n{c}\n"),
                    Err(e) => format!("{path}: Error - {e}"),
                }
            })
            .collect();
        Ok(format!("Successfully read multiple files:\n---\n{}", contents.join("\n---\n")))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
