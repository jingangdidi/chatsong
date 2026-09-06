use std::io::{BufRead, BufReader};
use std::fs::File;
use std::path::Path;

use md5::{Md5, Digest};
use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

/// params for md5sum
#[derive(Deserialize)]
pub struct Params {
    pub files: Vec<String>,
}

/// built-in tool
pub struct Md5Sum;

impl Md5Sum {
    /// new
    pub fn new() -> Self {
        Md5Sum
    }
}

impl BuiltIn for Md5Sum {
    /// get tool name
    fn name(&self) -> String {
        "md5sum".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Calculates the MD5 hash of one or more files. Returns the MD5 value for each file in the format 'MD5  filename'".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "files": {
                    "type": "array",
                    "description": "The list of file paths to calculate the MD5 hash.",
                },
            },
            "required": ["files"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: Some(vec!["files".to_string()]), object_fields: None })?;

        let (contents, has_error) = params.files.iter().fold(
            (Vec::new(), false),
            |(mut contents, mut has_error), path| {
                match calc_md5(&path.replace("\\", "/")) {
                    Ok(m) => contents.push(format!("{m}  {path}")),
                    Err(e) => {
                        contents.push(format!("{path}: Error - {e}"));
                        has_error = true;
                    }
                }
                (contents, has_error)
            },
        );
        if has_error {
            Ok((format!("calculate MD5 hash failed:\n---\n{}", contents.join("\n")), Some(params.files[0].clone())))
        } else {
            Ok((format!("Successfully calculate MD5 hash:\n---\n{}", contents.join("\n")), Some(params.files[0].clone())))
        }
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}

/// 计算文件 md5
fn calc_md5(filepath: &str) -> Result<String, MyError> {
    let file_path = Path::new(filepath);
    if !(file_path.exists() && file_path.is_file()) {
        return Err(MyError::FileNotExistError{file: filepath.to_string()})
    }
    let file = File::open(filepath).unwrap();
    //let reader = BufReader::new(file); // 默认buffer大小为8kb
    let mut reader = BufReader::with_capacity(10240000, file); // 指定buffer大小，单位byte
    let mut hasher = Md5::new();
    loop {
        let length = {
            let buffer = match reader.fill_buf() {
                Ok(buffer) => buffer,
                Err(_) => panic!("Error reading Data"),
            };
            hasher.update(buffer);
            buffer.len()
        };
        if length == 0 {
            break;
        }
        reader.consume(length)
    }
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
