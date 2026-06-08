use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use openai_dive::v1::{
    api::Client,
    resources::{
        image::{
            CreateImageParametersBuilder,
            //ImageQuality, // Standard, Hd
            ImageSize, // Size256X256, Size512X512, Size1024X1024, Size1024X1536, Size1536X1024, Size1792X1024, Size1024X1792, Auto
            //ImageStyle, // Vivid, Natural
            //ImageResponse,
            //ResponseFormat, // Url, B64Json
            ImageData,
        },
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
    prompt: String,
}

/// built-in tool
pub struct ImageGeneration;

impl ImageGeneration {
    /// new
    pub fn new() -> Self {
        ImageGeneration
    }
}

impl BuiltIn for ImageGeneration {
    /// get tool name
    fn name(&self) -> String {
        "image_generation".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Generates an image from a descriptive text prompt and saves it as a local image file. Returns the file path of the saved image.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "A detailed description of the image to create.",
                },
            },
            "required": ["prompt"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((params.prompt, None))
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        if is_en {
            Ok(Some(format!("Do you allow calling the image_generation tool to create image?{}\n{}", info.unwrap_or_default(), params.prompt)))
        } else {
            Ok(Some(format!("是否允许调用 image_generation 工具绘图？{}\n{}", info.unwrap_or_default(), params.prompt)))
        }
    }
}

/// 调用模型绘图
/// https://developers.openai.com/api/reference/resources/images/methods/generate
pub async fn image_generation(uuid: &str, prompt: String, m: &str) -> Result<String, MyError> {
    // 根据模型名称获取(api_key, endpoint, 模型名称, 是否支持深度思考)
    let (api_key, endpoint, model, _) = PARAS.api.get_model_by_name(m)?;
    // 使用api key初始化
    let mut client = Client::new(api_key);
    client.set_base_url(&endpoint); // 从0.7.0开始舍弃了new_with_base
    let mut para_builder = CreateImageParametersBuilder::default();
    para_builder.model(&model); // 绘图模型
    para_builder.prompt(prompt); // 描述图片的文本，dall-e-2最多1000个字符，dall-e-3最多4000个字符，例如：A cute dog in the park
    para_builder.n(1u32); // 生成图片的数量，dall-e-2可以指定1~10，dall-e-3只能是1
    //para_builder.quality(ImageQuality::Standard); // Standard, Hd, High, Medium, Low
    //para_builder.style(ImageStyle::Natural); // Vivid, Natural
    para_builder.size(ImageSize::Auto); // 图片大小，Size256X256, Size512X512, Size1024X1024, Size1024X1536, Size1536X1024, Size1792X1024, Size1024X1792, Auto。gpt-image-2 和 gpt-image-2-2026-04-21 支持任意分辨率，但长宽都要能被 16 整除，宽高比要在 1:3 和 3:1 之间，分辨率 > 2560x1440 处于试验阶段，最大 3840x2160
    //para_builder.response_format(ResponseFormat::B64Json); // Url或B64Json，gpt-image-2 模型不支持 response_format 参数
    let parameters = para_builder.build().map_err(|e| MyError::CreateImageError{error: e})?;
    // 生成图片
    let result = client.images().create(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;
    // 保存至本地指定路径下的uuid文件夹中，路径不存在则不保存，不会报错，需要在Cargo.toml中指定`features=["download"]`
    let paths = result.save(&format!("{}/{}", PARAS.outpath, uuid)).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?; // 返回保存的图片的路径，例如：["./result/58d89dfa-6358-4505-874b-d22c7cde0bd7/WPZPRRVDAVYCSAAA.png"]
    if paths.len() > 0 {
        //println!("save created image to: {}", paths[0]);
        match &result.data[0] { // 因为只绘制一张图，所以选第一个即可
            ImageData::Url{url, ..} => Ok(url.to_string()),
            ImageData::B64Json{b64_json: _, ..} => Ok(paths[0].clone()),
        }
    } else {
        Err(MyError::ParaError{para: "create image failed".to_string()})
    }
}
