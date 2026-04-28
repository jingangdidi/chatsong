use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        codebase::project::Project,
    },
};

/// params for get codebase file tree
#[derive(Deserialize)]
struct Params {
    path: String,
    #[serde(default)]
    depth: Option<usize>, // 0 or None means unlimit
}

/// built-in tool
pub struct GetCodebaseFileTree;

impl GetCodebaseFileTree {
    /// new
    pub fn new() -> Self {
        GetCodebaseFileTree
    }

    /// get file tree structure, depth 0 = unlimit
    fn get_codebase_file_tree(&self, path: &str, depth: usize) -> Result<String, MyError> {
        let project = Project::load_or_create_project(path)?;
        Ok(project.get_structure(depth))
    }
}

impl BuiltIn for GetCodebaseFileTree {
    /// get tool name
    fn name(&self) -> String {
        "get_codebase_file_tree".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Retrieves the file tree structure of a given directory path, optionally limiting the depth of the traversal. The result is returned as a formatted string showing the hierarchical structure of files and subdirectories. If no depth is specified or depth is set to 0, the full tree is returned. This tool is useful for inspecting project layouts.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path to the directory whose file tree should be retrieved.",
                },
                "depth": {
                    "type": ["integer", "null"],
                    "description": "Limits the depth of directory traversal.",
                },
            },
            "required": ["path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((self.get_codebase_file_tree(&params.path, params.depth.unwrap_or(0))?, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
