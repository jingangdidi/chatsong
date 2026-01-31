use std::fmt::Write;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::{
            format_bytes,
            list_directory_helper,
        },
    },
};

/// params for integer list_directory_with_sizes
#[derive(Deserialize)]
struct Params {
    dir_path: String,
}

/// built-in tool
pub struct ListDirectoryWithSizes;

impl ListDirectoryWithSizes {
    /// new
    pub fn new() -> Self {
        ListDirectoryWithSizes
    }
}

impl BuiltIn for ListDirectoryWithSizes {
    /// get tool name
    fn name(&self) -> String {
        "list_directory_with_sizes".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Get a detailed listing of all files and directories in a specified path, including file sizes. Results clearly distinguish between files and directories with [FILE] and [DIR] prefixes. This tool is useful for understanding directory structure and finding specific files within a directory. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "dir_path": {
                    "type": "string",
                    "description": "The path of the directory to list.",
                },
            },
            "required": ["dir_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let mut entries = list_directory_helper(&params.dir_path)?;

        let mut file_count = 0;
        let mut dir_count = 0;
        let mut total_size: u64 = 0;

        // Estimate initial capacity: assume ~50 bytes per entry + summary
        let mut output = String::with_capacity(entries.len() * 50 + 120);

        // Sort entries by file name
        entries.sort_by_key(|a| a.file_name());

        // build the output string
        for entry in &entries {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if entry.path().is_dir() {
                writeln!(output, "[DIR]  {file_name:<30}").unwrap();
                dir_count += 1;
            } else if entry.path().is_file() {
                let metadata = entry.metadata()?;

                let file_size = metadata.len();
                writeln!(
                    output,
                    "[FILE] {:<30} {:>10}",
                    file_name,
                    format_bytes(file_size)
                ).unwrap();
                file_count += 1;
                total_size += file_size;
            }
        }

        // Append summary
        writeln!(output, "\nTotal: {file_count} files, {dir_count} directories").unwrap();
        writeln!(output, "Total file size: {}", format_bytes(total_size)).unwrap();

        Ok(output)
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}
