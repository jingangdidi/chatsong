use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::{
            search_files_helper,
            format_bytes,
        },
    },
};

#[derive(Deserialize)]
enum FileSizeOutputFormat {
    #[serde(rename = "human-readable")]
    HumanReadable,
    #[serde(rename = "bytes")]
    Bytes,
}

/// params for integer calculate_directory_size
#[derive(Deserialize)]
struct Params {
    root_path: String,
    #[serde(default)]
    output_format: Option<FileSizeOutputFormat>,
}

/// built-in tool
pub struct CalculateDirectorySize;

impl CalculateDirectorySize {
    /// new
    pub fn new() -> Self {
        CalculateDirectorySize
    }

    /// Calculates the total size (in bytes) of all files within a directory tree.
    ///
    /// This function recursively searches the specified `root_path` for files,
    /// filters out directories and non-file entries, and sums the sizes of all found files.
    ///
    /// # Arguments
    /// * `root_path` - The root directory path to start the size calculation.
    ///
    /// # Returns
    /// Returns a `Result<u64>` containing the total size in bytes of all files under the `root_path`.
    ///
    /// # Notes
    /// - Only files are included in the size calculation; directories and other non-file entries are ignored.
    /// - The search pattern is `"**/*"` (all files) and no exclusions are applied.
    fn calculate_directory_size(&self, root_path: &str) -> Result<u64, MyError> {
        let entries = search_files_helper(root_path, "**/*".to_string(), None)?
            .into_iter()
            .filter(|e| e.file_type().is_file()); // Only process files

        // Use rayon to parallelize size summation
        let total_size: u64 = entries
            .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
            .sum();

        Ok(total_size)
    }
}

impl BuiltIn for CalculateDirectorySize {
    /// get tool name
    fn name(&self) -> String {
        "calculate_directory_size".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Calculates the total size of a directory specified by `root_path`. It recursively searches for files and sums their sizes. The result can be returned in either a `human-readable` format or as `bytes`, depending on the specified `output_format` argument. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "root_path": {
                    "type": "string",
                    "description": "The root directory path to start the size calculation.",
                },
                "output_format": {
                    "oneOf": [
                        {
                            "enum": ["human-readable", "bytes"]
                        },
                        {
                            "type": "null"
                        }
                    ],
                    "description": "Defines the output format, which can be either `human-readable` or `bytes`.",
                },
            },
            "required": ["root_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let total_bytes = self.calculate_directory_size(&params.root_path)?;
        let total_size = match params.output_format.unwrap_or(FileSizeOutputFormat::HumanReadable) {
            FileSizeOutputFormat::HumanReadable => format_bytes(total_bytes),
            FileSizeOutputFormat::Bytes => format!("{total_bytes} bytes"),
        };
        Ok(format!("successfully calculate directory \"{}\" size: {}", &params.root_path, total_size))
    }
}
