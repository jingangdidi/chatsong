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
    #[serde(default)]
    limit: Option<usize>, // None means unlimit
}

/// built-in tool
pub struct SearchCodeSignature;

impl SearchCodeSignature {
    /// new
    pub fn new() -> Self {
        SearchCodeSignature
    }

    /// search source code or signature
    fn search_code_or_signature(&self, path: &str, query: &str, limit: usize, only_signature: bool) -> Result<String, MyError> {
        let project = Project::load_or_create_project(path)?;
        project.search_symbols(query, limit, only_signature)
    }
}

impl BuiltIn for SearchCodeSignature {
    /// get tool name
    fn name(&self) -> String {
        "search_code_or_signature".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Searches for source code or function signatures within files under a specified directory path. Matches the provided query string against file contents. When 'only_signature' is true, only function/method signatures are returned, reducing token usage when full source code is unnecessary. The 'limit' parameter restricts the number of results returned (unlimited if not specified). Useful for quickly retrieving relevant code snippets or declarations.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path where the search should be performed.",
                },
                "query": {
                    "type": "string",
                    "description": "The text to search for in the files (e.g., function name, struct name).",
                },
                "limit": {
                    "type": ["integer", "null"],
                    "description": "Maximum number of results to return. If omitted or null, no limit is applied.",
                },
                "only_signature": {
                    "type": "boolean",
                    "description": "If true, only returns function/method signatures (e.g., function declarations), avoiding full source code. This helps reduce token usage when only structural information is needed.",
                },
            },
            "required": ["path", "query"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((self.search_code_or_signature(&params.path, &params.query, params.limit.unwrap_or(usize::MAX), params.only_signature)?, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
