use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::search_files_helper,
    },
};

/// params for integer search_files
#[derive(Deserialize)]
struct Params {
    root_path:        String,
    include_pattern:  String,
    exclude_patterns: Option<Vec<String>>,
}

/// built-in tool
pub struct SearchFiles;

impl SearchFiles {
    /// new
    pub fn new() -> Self {
        SearchFiles
    }
}

impl BuiltIn for SearchFiles {
    /// get tool name
    fn name(&self) -> String {
        "search_files".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Recursively search for files and directories matching a pattern. Searches through all subdirectories from the starting path. The search is case-insensitive and matches partial names. Returns full paths to all matching items. Great for finding files when you don't know their exact location. Only searches within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "root_path": {
                    "type": "string",
                    "description": "The directory path to search in.",
                },
                "include_pattern": {
                    "type": "string",
                    "description": "The file glob pattern to match (e.g., \"*.rs\").",
                },
                "exclude_patterns": {
                    "type": ["array", "null"],
                    "items": {"type": "string"},
                    "description": "Optional list of patterns to exclude from the search.",
                },
            },
            "required": ["root_path", "include_pattern", "exclude_patterns"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let list = search_files_helper(&params.root_path, params.include_pattern, params.exclude_patterns)?;
        let result = if !list.is_empty() {
            list.iter()
                .map(|entry| entry.path().display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            "No matches found".to_string()
        };
        Ok(result)
    }
}
