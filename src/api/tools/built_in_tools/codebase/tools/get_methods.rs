use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        codebase::project::Project,
    },
};

/// params for search source code or signature
#[derive(Deserialize)]
struct Params {
    path: String,
    query: String,
    #[serde(default)]
    only_signature: bool,
}

/// built-in tool
pub struct GetMethods;

impl GetMethods {
    /// new
    pub fn new() -> Self {
        GetMethods
    }

    /// get all mtehods source code or signature of struct/enum/class
    fn get_methods(&self, path: &str, query: &str, only_signature: bool) -> Result<String, MyError> {
        let project = Project::load_or_create_project(path)?;
        project.get_methods(query, only_signature)
    }
}

impl BuiltIn for GetMethods {
    /// get tool name
    fn name(&self) -> String {
        "get_methods".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Retrieves all methods defined within a specific type (struct, enum, class) at a given project path. Supports signature-only output to reduce token usage.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path to the project directory where the target type is located.",
                },
                "query": {
                    "type": "string",
                    "description": "The name of the type to search for (e.g., struct name, enum name, class name).",
                },
                "only_signature": {
                    "type": "boolean",
                    "description": "If true, returns only method signatures (name, parameters, return type) without full source code. This reduces token usage and is suitable when detailed implementation is not needed."
                },
            },
            "required": ["path", "query"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((self.get_methods(&params.path, &params.query, params.only_signature)?, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
