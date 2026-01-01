use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::validate_path,
    },
};

/// params for integer head_file
#[derive(Deserialize)]
struct Params {
    path:  String,
    lines: u64
}

/// built-in tool
pub struct HeadFile;

impl HeadFile {
    /// new
    pub fn new() -> Self {
        HeadFile
    }

    /// Reads the first n lines from a text file, preserving line endings.
    /// Args:
    ///     file_path: Path to the file
    ///     n: Number of lines to read
    /// Returns a String containing the first n lines with original line endings or an error if the path is invalid or file cannot be read.
    fn head_file(&self, file_path: &str, n: usize) -> Result<String, MyError> {
        // Validate file path against allowed directories
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(file_path), true)?;

        // Open file and create a BufReader
        let file = fs::File::open(&valid_path)?;
        let mut reader = BufReader::new(file);
        let mut result = String::with_capacity(n * 100); // Estimate capacity (avg 100 bytes/line)
        let mut count = 0;

        // Read lines, preserving line endings
        let mut line = Vec::new();
        while count < n {
            line.clear();
            let bytes_read = reader.read_until(b'\n', &mut line)?;
            if bytes_read == 0 {
                break; // Reached EOF
            }
            result.push_str(&String::from_utf8_lossy(&line));
            count += 1;
        }

        Ok(result)
    }
}

impl BuiltIn for HeadFile {
    /// get tool name
    fn name(&self) -> String {
        "head_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Reads and returns the first N lines of a text file. This is useful for quickly previewing file contents without loading the entire file into memory. If the file has fewer than N lines, the entire file will be returned. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to get information for.",
                },
                "lines": {
                    "type": "integer",
                    "description": "The number of lines to read from the beginning of the file.",
                },
            },
            "required": ["path", "lines"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let result = self.head_file(&params.path, params.lines as usize)?;
        Ok(format!("Successfully get the first {} lines:\n```\n{}\n```", params.lines, if result.contains("```") { result.replace("```", "\\`\\`\\`") } else { result }))
    }
}
