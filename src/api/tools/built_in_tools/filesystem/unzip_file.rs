use std::fs;
use std::io;
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

/// params for integer unzip_file
#[derive(Deserialize)]
struct Params {
    zip_file:   String,
    target_dir: String,
}

/// built-in tool
pub struct UnzipFile;

impl UnzipFile {
    /// new
    pub fn new() -> Self {
        UnzipFile
    }
}

impl BuiltIn for UnzipFile {
    /// get tool name
    fn name(&self) -> String {
        "unzip_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Extracts the contents of a ZIP archive to a specified target directory. It takes a source ZIP file path and a target extraction directory. The tool decompresses all files and directories stored in the ZIP, recreating their structure in the target location. Both the source ZIP file and the target directory should reside within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "zip_file": {
                    "type": "string",
                    "description": "A filesystem path to an existing ZIP file to be extracted.",
                },
                "target_dir": {
                    "type": "string",
                    "description": "Path to the target directory where the contents of the ZIP file will be extracted.",
                },
            },
            "required": ["zip_file", "target_dir"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;

        let zip_file = validate_path(&PARAS.allowed_path, Path::new(&params.zip_file), true)?;
        let target_dir_path = Path::new(&params.target_dir);
        let target_dir_abs_path = validate_path(&PARAS.allowed_path, target_dir_path, false)?;

        if target_dir_abs_path.exists() {
            return Err(MyError::OtherError{info: format!("'{}' directory already exists!", params.target_dir)})
        }

        let file = fs::File::open(&zip_file)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| MyError::ZipArchiveError{file: params.zip_file.clone(), error: e})?;
        let file_count = archive.len();
        let mut result_files: Vec<String> = Vec::new();
        for i in 0..file_count {
            let mut file = archive.by_index(i).map_err(|e| MyError::ZipArchiveError{file: params.zip_file.clone(), error: e})?;
            let outpath = match file.enclosed_name() {
                Some(path) => if file.is_dir() {
                    path
                } else {
                    /*
                    let components = path.components();
                    if components.clone().count() > 1 {
                        target_dir_path.join(components.skip(1).collect::<PathBuf>())
                    } else {
                        target_dir_path.join(path)
                    }
                    */
                    target_dir_path.join(path)
                },
                None => continue,
            };

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = fs::File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
                result_files.push(format!("{}", outpath.display()));
            }
        }

        Ok(format!("Successfully extracted {} {} into '{}':\n{}", file_count, if file_count == 1 { "file" } else { "files" }, target_dir_path.display(), result_files.join("\n")))
    }
}
