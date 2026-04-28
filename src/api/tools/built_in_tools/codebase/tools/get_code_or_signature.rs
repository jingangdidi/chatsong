use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        codebase::{
            project::Project,
            symbol_table::SymbolKind,
        },
    },
};

/// params for get source code or signature
#[derive(Deserialize)]
struct Params {
    path: String,
    #[serde(default)]
    kind_filter: Option<SymbolKind>,
    #[serde(default)]
    file_filter: Option<String>,
    #[serde(default)]
    only_signature: bool,
}

/// built-in tool
pub struct GetCodeSignature;

impl GetCodeSignature {
    /// new
    pub fn new() -> Self {
        GetCodeSignature
    }

    /// get source code or signature
    fn get_code_or_signature(&self, path: &str, kind_filter: Option<SymbolKind>, file_filter: Option<String>, only_signature: bool) -> Result<String, MyError> {
        let project = Project::load_or_create_project(path)?;
        project.get_code_or_signature(kind_filter, file_filter, only_signature)
    }
}

impl BuiltIn for GetCodeSignature {
    /// get tool name
    fn name(&self) -> String {
        "get_code_or_signature".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Extracts symbols (functions, classes, structs, enums, variables, etc.) from a project path with optional filtering by kind, file, and signature-only mode. Returns only signatures when requested to reduce token usage.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The root directory path of the project to scan for symbols. Must be a valid local filesystem path.",
                },
                "kind_filter": {
                    "oneOf": [
                        {
                            "enum": ["function", "method", "class", "struct", "enum", "trait", "interface", "constant", "variable", "type", "module", "import", "other"]
                        },
                        {
                            "type": "null"
                        }
                    ],
                    "description": "Optional filter to restrict results to symbols of a specific kind. If omitted, all symbol kinds are included.",
                },
                "file_filter": {
                    "type": ["string", "null"],
                    "description": "Optional path to a specific file within the project. If provided, only symbols defined in this file will be returned."
                },
                "only_signature": {
                    "type": "boolean",
                    "description": "If true, only returns the symbol's signature (declaration) without the full source code. This reduces token usage and is useful when full context is not required.",
                },
            },
            "required": ["path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((self.get_code_or_signature(&params.path, params.kind_filter, params.file_filter, params.only_signature)?, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
