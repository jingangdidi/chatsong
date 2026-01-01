use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
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

/// params for integer tail_file
#[derive(Deserialize)]
struct Params {
    path:  String,
    lines: u64
}

/// built-in tool
pub struct TailFile;

impl TailFile {
    /// new
    pub fn new() -> Self {
        TailFile
    }

    /// Reads the last n lines from a text file, preserving line endings.
    /// Args:
    ///     file_path: Path to the file
    ///     n: Number of lines to read
    /// Returns a String containing the last n lines with original line endings or an error if the path is invalid or file cannot be read.
    fn tail_file(&self, file_path: &str, n: usize) -> Result<String, MyError> {
        // Validate file path against allowed directories
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(file_path), true)?;

        // Open file asynchronously
        let file = fs::File::open(&valid_path)?;
        let file_size = file.metadata()?.len();

        // If file is empty or n is 0, return empty string
        if file_size == 0 || n == 0 {
            return Ok(String::new());
        }

        // Create a BufReader
        let mut reader = BufReader::new(file);
        let mut line_count = 0;
        let mut pos = file_size;
        let chunk_size = 8192; // 8KB chunks
        let mut buffer = vec![0u8; chunk_size];
        let mut newline_positions = Vec::new();

        // Read backwards to collect all newline positions
        while pos > 0 {
            let read_size = chunk_size.min(pos as usize);
            pos -= read_size as u64;
            reader.seek(SeekFrom::Start(pos))?;
            let read_bytes = reader.read(&mut buffer[..read_size])?;

            // Process chunk in reverse to find newlines
            for (i, byte) in buffer[..read_bytes].iter().enumerate().rev() {
                if *byte == b'\n' {
                    newline_positions.push(pos + i as u64);
                    line_count += 1;
                    if line_count > n {
                        pos = 0;
                        break
                    }
                }
            }
        }

        // Check if file ends with a non-newline character (partial last line)
        if file_size > 0 {
            let mut temp_reader = BufReader::new(fs::File::open(&valid_path)?);
            temp_reader.seek(SeekFrom::End(-1))?;
            let mut last_byte = [0u8; 1];
            temp_reader.read_exact(&mut last_byte)?;
            if last_byte[0] != b'\n' {
                line_count += 1; // srx: this partial last line will be ignored
            }
        }

        // Determine start position for reading the last n lines
        let start_pos = if line_count <= n {
            0 // Read from start if fewer than n lines
        } else {
            // https://github.com/rust-mcp-stack/rust-mcp-filesystem/pull/70
            //*newline_positions.get(line_count - n).unwrap_or(&0) + 1
            *newline_positions.get(n).unwrap_or(&0) + 1
        };

        // Read forward from start_pos
        reader.seek(SeekFrom::Start(start_pos))?;
        let mut result = String::with_capacity(n * 100); // Estimate capacity
        let mut line = Vec::new();
        let mut lines_read = 0;

        while lines_read < n {
            line.clear();
            let bytes_read = reader.read_until(b'\n', &mut line)?;
            if bytes_read == 0 {
                // Handle partial last line at EOF
                if !line.is_empty() {
                    result.push_str(&String::from_utf8_lossy(&line));
                }
                break;
            }
            result.push_str(&String::from_utf8_lossy(&line));
            lines_read += 1;
        }

        Ok(result)
    }
}

impl BuiltIn for TailFile {
    /// get tool name
    fn name(&self) -> String {
        "tail_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Reads and returns the last N lines of a text file. This is useful for quickly previewing file contents without loading the entire file into memory. If the file has fewer than N lines, the entire file will be returned. Only works within allowed directories.".to_string()
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
                    "description": "The number of lines to read from the ending of the file.",
                },
            },
            "required": ["path", "lines"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let result = self.tail_file(&params.path, params.lines as usize)?;
        Ok(format!("Successfully get the last {} lines:\n```\n{}\n```", params.lines, if result.contains("```") { result.replace("```", "\\`\\`\\`") } else { result }))
    }
}
