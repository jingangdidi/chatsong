use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use glob::Pattern;
use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use walkdir::WalkDir;
use zip::{
    write::SimpleFileOptions,
    ZipWriter,
};

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::validate_path,
    },
};

/// params for integer zip_directory
#[derive(Deserialize)]
struct Params {
    input_dir:       String,
    target_zip_file: String,
    pattern:         Option<String>,
}

/// built-in tool
pub struct ZipDirectory;

impl ZipDirectory {
    /// new
    pub fn new() -> Self {
        ZipDirectory
    }
}

impl BuiltIn for ZipDirectory {
    /// get tool name
    fn name(&self) -> String {
        "zip_directory".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Creates a ZIP archive by compressing a directory, including files and subdirectories matching a specified glob pattern. It takes a path to the folder and a glob pattern to identify files to compress and a target path for the resulting ZIP file. Both the source directory and the target ZIP file should reside within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "input_dir": {
                    "type": "string",
                    "description": "Path to the directory to zip.",
                },
                "target_zip_file": {
                    "type": "string",
                    "description": "Path to save the resulting ZIP file, including filename and .zip extension.",
                },
                "pattern": {
                    "type": ["string", "null"],
                    "description": "A optional glob pattern to match files and subdirectories to zip, defaults to **/*",
                },
            },
            "required": ["input_dir", "target_zip_file"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

        let valid_dir_path = validate_path(&PARAS.allowed_path, Path::new(&params.input_dir), true)?;
        let input_dir_str = &valid_dir_path
            .as_os_str()
            .to_str()
            .ok_or(MyError::OtherError{info: format!("Invalid UTF-8 in file name: {}", valid_dir_path.display())})?;

        let target_path = validate_path(&PARAS.allowed_path, Path::new(&params.target_zip_file), false)?;
        if target_path.exists() {
            return Err(MyError::OtherError{info: format!("'{}' already exists!", params.target_zip_file)})
        }

        let updated_pattern = match params.pattern {
            Some(p) => if p.contains('*') {
                p.to_lowercase()
            } else {
                format!("*{}*", &p.to_lowercase())
            },
            None => "**/*".to_string(),
        };

        let glob_pattern = Pattern::new(&updated_pattern).map_err(|e| MyError::CreatePatternError{pattern: updated_pattern, error: e})?;

        let entries: Vec<_> = WalkDir::new(&valid_dir_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let full_path = entry.path();
                validate_path(&PARAS.allowed_path, full_path, true).ok().and_then(|path| {
                    if path != valid_dir_path && glob_pattern.matches(&path.display().to_string()) {
                        Some(path)
                    } else {
                        None
                    }
                })
            })
            .collect();

        let zip_file = fs::File::create(&target_path)?;
        let mut zip_writer = ZipWriter::new(zip_file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o755);
        for entry_path_buf in &entries {
            if entry_path_buf.is_dir() {
                continue
            }
            let entry_path = entry_path_buf.as_path();
            let entry_str = entry_path.as_os_str().to_str().ok_or(MyError::OtherError{info: format!("Invalid UTF-8 in file name: {}", entry_path.display())})?;

            if !entry_str.starts_with(input_dir_str) {
                return Err(MyError::OtherError{info: format!("Entry file path ({}) does not start with base input directory path ({})", entry_str, input_dir_str)})
            }

            let entry_str = &entry_str[input_dir_str.len() + 1..];

            let mut input_file = fs::File::open(entry_path_buf)?;
            let input_file_size = input_file.metadata()?.len() as usize;

            let mut buffer = Vec::with_capacity(input_file_size);
            input_file.read_to_end(&mut buffer)?;
            zip_writer.start_file(entry_str, options).map_err(|e| MyError::ZipArchiveError{file: entry_str.to_string(), error: e})?;
            zip_writer.write_all(&buffer)?;
        }
        zip_writer.finish().map_err(|e| MyError::ZipArchiveError{file: params.target_zip_file, error: e})?;
        Ok(format!("Successfully compressed '{}' directory into '{}'.", params.input_dir, target_path.display()))
    }
}
