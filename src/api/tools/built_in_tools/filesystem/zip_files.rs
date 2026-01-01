use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
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

/// params for integer zip_files
#[derive(Deserialize)]
struct Params {
    input_files:     Vec<String>,
    target_zip_file: String,
}

/// built-in tool
pub struct ZipFiles;

impl ZipFiles {
    /// new
    pub fn new() -> Self {
        ZipFiles
    }
}

impl BuiltIn for ZipFiles {
    /// get tool name
    fn name(&self) -> String {
        "zip_files".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Creates a ZIP archive by compressing files. It takes a list of files to compress and a target path for the resulting ZIP file. Both the source files and the target ZIP file should reside within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "input_files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "The list of files to include in the ZIP archive.",
                },
                "target_zip_file": {
                    "type": "string",
                    "description": "Path to save the resulting ZIP file, including filename and .zip extension.",
                },
            },
            "required": ["input_files", "target_zip_file"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

        let file_count = params.input_files.len();
        if file_count == 0 {
            return Err(MyError::OtherError{info: "No file(s) to zip. The input files array is empty.".to_string()})
        }

        let target_path = validate_path(&PARAS.allowed_path, Path::new(&params.target_zip_file), false)?;
        if target_path.exists() {
            return Err(MyError::OtherError{info: format!("zip file {} already exist.", params.target_zip_file)})
        }

        let source_paths = params.input_files
            .iter()
            .map(|p| validate_path(&PARAS.allowed_path, Path::new(p), true))
            .collect::<Result<Vec<_>, _>>()?;

        let zip_file = fs::File::create(&target_path)?;
        let mut zip_writer = ZipWriter::new(zip_file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o755);
        for path in source_paths {
            let filename = path.file_name().ok_or(MyError::OtherError{info: format!("invalid path: {}", path.display())})?;
            let filename = filename.to_str().ok_or(MyError::OtherError{info: format!("invalid UTF-8 in file name: {}", path.display())})?;

            let mut input_file = fs::File::open(&path)?;
            let input_file_size = input_file.metadata()?.len() as usize;

            let mut buffer = Vec::with_capacity(input_file_size);
            input_file.read_to_end(&mut buffer)?;
            zip_writer.start_file(filename, options).map_err(|e| MyError::ZipArchiveError{file: filename.to_string(), error: e})?;
            zip_writer.write_all(&buffer)?;
        }
        zip_writer.finish().map_err(|e| MyError::ZipArchiveError{file: params.target_zip_file, error: e})?;
        Ok(format!("Successfully compressed {} {} into '{}'.", file_count, if file_count == 1 { "file" } else { "files" }, target_path.display()))
    }
}
