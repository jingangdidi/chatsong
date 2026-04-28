use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        codebase::project::Project,
    },
};

/// params for grep source code or signature by pattern
#[derive(Deserialize)]
struct Params {
    path: String,
    pattern: String,
    #[serde(default)]
    only_signature: bool,
}

/// built-in tool
pub struct GrepCodeSignature;

impl GrepCodeSignature {
    /// new
    pub fn new() -> Self {
        GrepCodeSignature
    }

    /// grep source code or signature by pattern
    fn grep_code_or_signature(&self, path: &str, pattern: &str, only_signature: bool) -> Result<String, MyError> {
        let project = Project::load_or_create_project(path)?;
        project.grep_code(pattern, only_signature)
    }
}

impl BuiltIn for GrepCodeSignature {
    /// get tool name
    fn name(&self) -> String {
        "grep_code_or_signature".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Searches for code or function signatures matching a given pattern within a specified directory path. If 'only_signature' is true, only function/method signatures are returned, reducing token usage. Otherwise, full source code lines are returned.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path to search within.",
                },
                "pattern": {
                    "type": "string",
                    "description": "The search pattern (e.g., function name, class name, or regex-like pattern) to match in files.",
                },
                "only_signature": {
                    "type": "boolean",
                    "description": "If true, returns only function/method signatures (e.g., method declarations). If false, returns full source code lines matching the pattern. Setting this to true can reduce token usage when source code is not needed.",
                },
            },
            "required": ["path", "pattern"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((self.grep_code_or_signature(&params.path, &params.pattern, params.only_signature)?, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
