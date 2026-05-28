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
    openai::for_image::image_to_base64_helper, // 图片转base64，返回base64编码的字符串
};

/// params for load_image
#[derive(Deserialize)]
struct Params {
    image_path: String,
}

/// built-in tool
pub struct LoadImage;

impl LoadImage {
    /// new
    pub fn new() -> Self {
        LoadImage
    }
}

impl BuiltIn for LoadImage {
    /// get tool name
    fn name(&self) -> String {
        "load_image".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Load a PNG or JPG image from the local file system. The tool expects a valid file path to an existing .png, .jpg, or .jpeg file. It will return an error if the file does not exist or is of an unsupported format. This tool can only be invoked by models that support image input (multi-modal models). Text-only models are not allowed to call this tool.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "image_path": {
                    "type": "string",
                    "description": "The path of the png or jpg image to load.",
                },
            },
            "required": ["image_path"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let valid_path = validate_path(&PARAS.allowed_path, Path::new(&params.image_path.replace("\\", "/")), true)?;
        if is_image(&valid_path) {
            let base64 = image_to_base64_helper(&valid_path)?;
            Ok((base64, None))
        } else {
            Err(MyError::OtherError{info: format!("load_image only support png, jpg, jpeg, not {}", params.image_path)})
        }
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}

/// 检查给定路径是否以图片格式后缀（png、jpg、jpeg）结尾，不区分大小写
fn is_image(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        if let Some(ext_str) = ext.to_str() {
            let ext_lower = ext_str.to_ascii_lowercase();
            matches!(ext_lower.as_str(), "png" | "jpg" | "jpeg")
        } else {
            false
        }
    } else {
        false
    }
}
