use std::path::Path;

use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use openai_dive::v1::{
    api::Client,
    resources::{
        image::{
            EditImageParametersBuilder,
            //ImageQuality, // Standard, Hd
            ImageSize, // Size256X256, Size512X512, Size1024X1024, Size1792X1024, Size1024X1792，dall-e-2支持256x256、512x512、1024x1024，dall-e-3支持1024x1024、1792x1024、1024x1792
            //ImageStyle, // Vivid, Natural
            //ImageResponse,
            //ResponseFormat, // Url, B64Json
            ImageData,
        },
        shared::FileUpload,
    },
};

use crate::{
    error::MyError,
    tools::built_in_tools::BuiltIn,
    parse_paras::PARAS,
};

/// params for image generation
#[derive(Deserialize)]
struct Params {
    image_path: String,
    prompt: String,
}

/// built-in tool
pub struct EditImage;

impl EditImage {
    /// new
    pub fn new() -> Self {
        EditImage
    }
}

impl BuiltIn for EditImage {
    /// get tool name
    fn name(&self) -> String {
        "edit_image".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Edits an image according to a descriptive text prompt. It takes an original image and a user‑supplied description of the desired changes, then applies AI‑powered editing to produce a transformed version. The resulting image is automatically saved to a local file.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "image_path": {
                    "type": "string",
                    "description": "A string that describes the intended modification to the image.",
                },
                "prompt": {
                    "type": "string",
                    "description": "The file path (string) to the source image that will be edited.",
                },
            },
            "required": ["image_path", "prompt"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let tmp_path = Path::new(&params.image_path);
        if tmp_path.exists() && tmp_path.is_file() {
            Ok((format!("{}---srx---{}", params.image_path, params.prompt), None))
        } else {
            Err(MyError::FileNotExistError{file: params.image_path})
        }
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        if is_en {
            Ok(Some(format!("Do you allow calling the edit_image tool to edit image?{}\n{}", info.unwrap_or_default(), params.prompt)))
        } else {
            Ok(Some(format!("是否允许调用 edit_image 工具改图？{}\n{}", info.unwrap_or_default(), params.prompt)))
        }
    }
}

/// 调用模型绘图
/// https://developers.openai.com/api/reference/resources/images/methods/generate
pub async fn edit_image(uuid: &str, image_path: &str, prompt: &str, m: &str) -> Result<String, MyError> {
    // 根据模型名称获取(api_key, endpoint, 模型名称, 是否支持深度思考)
    let (api_key, endpoint, model, _) = PARAS.api.get_model_by_name(m)?;
    // 使用api key初始化
    let mut client = Client::new(api_key);
    client.set_base_url(&endpoint); // 从0.7.0开始舍弃了new_with_base

    let parameters = EditImageParametersBuilder::default()
        .prompt(prompt.to_string()) // 描述图片的文本，gpt-image-1最多32000个字符
        .image(FileUpload::File(image_path.to_string())) // 格式：png、webp、jpg，大小：<25MB，形状：不需要是正方形
        .model(&model) // 选择模型，这里固定为gpt-image-1
        .n(1u32) // 生成图片的数量，默认1
        //.quality(tmp_quality) // gpt-image-1支持：high、medium、low，auto表示自动选择最高质量
        .size(ImageSize::Size1024X1792) // 图片大小，gpt-image-1支持1024x1024、1536x1024、1024x1536
        .build().map_err(|e| MyError::EditImageError{error: e})?;
    let result = client.images().edit(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;

    // 保存至本地指定路径下的uuid文件夹中，路径不存在则不保存，不会报错，需要在Cargo.toml中指定`features=["download"]`
    // 这个save方法内部会解析返回的是url还是b64，都可以保存
    // https://docs.rs/openai_dive/1.0.1/src/openai_dive/v1/resources/image.rs.html
    let paths = result.save(&format!("{}/{}", PARAS.outpath, uuid)).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?; // 返回保存的图片的路径，例如：["./result/58d89dfa-6358-4505-874b-d22c7cde0bd7/WPZPRRVDAVYCSAAA.png"]
    if paths.len() > 0 {
        //println!("save created image to: {}", paths[0]);
        match &result.data[0] { // 因为只绘制一张图，所以选第一个即可
            ImageData::Url{url, ..} => Ok(url.to_string()),
            ImageData::B64Json{b64_json: _, ..} => Ok(paths[0].clone()),
        }
    } else {
        Err(MyError::ParaError{para: "edit image failed".to_string()})
    }
}
