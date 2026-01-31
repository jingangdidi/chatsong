use std::fs;
use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use walkdir::WalkDir;

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::validate_path,
    },
};

/// params for integer directory_tree
#[derive(Deserialize)]
struct Params {
    root_path: String,
    max_depth: Option<usize>,
}

/// built-in tool
pub struct DirectoryTree;

impl DirectoryTree {
    /// new
    pub fn new() -> Self {
        DirectoryTree
    }

    /// Generates a JSON representation of a directory tree starting at the given path.
    ///
    /// This function recursively builds a JSON array object representing the directory structure,
    /// where each entry includes a `name` (file or directory name), `type` ("file" or "directory"),
    /// and for directories, a `children` array containing their contents. Files do not have a
    /// `children` field.
    ///
    /// The function supports optional constraints to limit the tree size:
    /// - `max_depth`: Limits the depth of directory traversal.
    /// - `max_files`: Limits the total number of entries (files and directories).
    ///
    /// # IMPORTANT NOTE
    ///
    /// use max_depth or max_files could lead to partial or skewed representations of actual directory tree
    fn directory_tree_helper(
        &self,
        root_path: &Path,
        max_depth: Option<usize>,
        max_files: Option<usize>,
        current_count: &mut usize,
    ) -> Result<(Value, bool), MyError> {
        let valid_path = validate_path(&PARAS.allowed_path, root_path, true)?;

        let metadata = fs::metadata(&valid_path)?;
        if !metadata.is_dir() {
            return Err(MyError::OtherError{info: format!("root path must be a directory: {}", valid_path.display())})
        }

        let mut children = Vec::new();
        let mut reached_max_depth = false;

        if max_depth != Some(0) {
            for entry in WalkDir::new(valid_path)
                .min_depth(1)
                .max_depth(1)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let child_path = entry.path();
                let metadata = fs::metadata(child_path)?;

                let entry_name = child_path
                    .file_name()
                    .ok_or(MyError::OtherError{info: format!("invalid path: {}", child_path.display())})?
                    .to_string_lossy()
                    .into_owned();

                // Increment the count for this entry
                *current_count += 1;

                // Check if we've exceeded max_files (if set)
                if let Some(max) = max_files {
                    if *current_count > max {
                        continue; // Skip this entry but continue processing others
                    }
                }

                let mut json_entry = json!({
                    "name": entry_name,
                    "type": if metadata.is_dir() { "directory" } else { "file" }
                });

                if metadata.is_dir() {
                    let next_depth = max_depth.map(|d| d - 1);
                    let (child_children, child_reached_max_depth) = self.directory_tree_helper(child_path, next_depth, max_files, current_count)?;
                    json_entry
                        .as_object_mut()
                        .unwrap()
                        .insert("children".to_string(), child_children);
                    reached_max_depth |= child_reached_max_depth;
                }
                children.push(json_entry);
            }
        } else {
            // If max_depth is 0, we skip processing this directory's children
            reached_max_depth = true;
        }
        Ok((Value::Array(children), reached_max_depth))
    }
}

impl BuiltIn for DirectoryTree {
    /// get tool name
    fn name(&self) -> String {
        "directory_tree".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Get a recursive tree view of files and directories as a JSON structure. Each entry includes 'name', 'type' (file/directory), and 'children' for directories. Files have no children array, while directories always have a children array (which may be empty). If the 'max_depth' parameter is provided, the traversal will be limited to the specified depth. As a result, the returned directory structure may be incomplete or provide a skewed representation of the full directory tree, since deeper-level files and subdirectories beyond the specified depth will be excluded. The output is formatted with 2-space indentation for readability. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "root_path": {
                    "type": "string",
                    "description": "The root path of the directory tree to generate.",
                },
                "max_depth": {
                    "type": ["integer", "null"],
                    "description": "Limits the depth of directory traversal.",
                },
            },
            "required": ["root_path", "max_depth"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let mut current_count = 0;
        let (entries, _reached_max_depth) = self.directory_tree_helper(Path::new(&params.root_path), params.max_depth, None, &mut current_count)?;
        if current_count == 0 {
            return Err(MyError::OtherError{info: format!("Could not find any entries: {}", params.root_path)})
        }
        Ok(format!("successfully get directory \"{}\" tree: {:?}", &params.root_path, entries))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
