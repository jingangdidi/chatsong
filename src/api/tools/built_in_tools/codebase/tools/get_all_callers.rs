use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        codebase::project::Project,
    },
};

/// params for get all callers source code or signature
#[derive(Deserialize)]
struct Params {
    path: String,
    query: String,
    file: String,
    #[serde(default)]
    only_signature: bool,
}

/// built-in tool
pub struct GetAllCallers;

impl GetAllCallers {
    /// new
    pub fn new() -> Self {
        GetAllCallers
    }

    /// get all callers source code or signature
    fn get_all_callers(&self, path: &str, query: &str, file: &str, only_signature: bool) -> Result<String, MyError> {
        let project = Project::load_or_create_project(path)?;
        let all_callers = project.get_all_callers(query, file)?;
        Ok(all_callers.get_all_callers_src_code_or_signature(only_signature))
    }
}

impl BuiltIn for GetAllCallers {
    /// get tool name
    fn name(&self) -> String {
        "get_all_callers".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Searches for a specific function or method in a given file within a project path, then retrieves the complete call chain — all functions and methods involved in calling it — either with full source code or only their signatures.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path of the project where the search should be performed.",
                },
                "query": {
                    "type": "string",
                    "description": "The name of the function or method to search for.",
                },
                "file": {
                    "type": "string",
                    "description": "The relative file path (e.g., 'tool/service.py') in which the function/method is expected to be defined.",
                },
                "only_signature": {
                    "type": "boolean",
                    "description": "If true, returns only function/method signatures (e.g., 'def my_func(a: int) -> str'). If false, returns full source code lines of each function/method in the call chain. Using true reduces token usage when detailed source isn't required.",
                },
            },
            "required": ["path", "query", "file"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((self.get_all_callers(&params.path, &params.query, &params.file, params.only_signature)?, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
