use std::fs::{read, write};

use base64::{engine::general_purpose, Engine as _};
use openai_dive::v1::{
    api::Client,
    helpers::generate_file_name, // 生成由大写英文字母构成的指定长度随机文件名
    //models::DallEModel, // DallE2, DallE3
    resources::{
        image::{
            CreateImageParametersBuilder,
            EditImageParametersBuilder,
            ImageQuality, // Standard, Hd
            ImageSize, // Size256X256, Size512X512, Size1024X1024, Size1792X1024, Size1024X1792，dall-e-2支持256x256、512x512、1024x1024，dall-e-3支持1024x1024、1792x1024、1024x1792
            ImageStyle, // Vivid, Natural
            ImageResponse,
            ResponseFormat, // Url, B64Json
            ImageData,
        },
        shared::FileUpload,
    },
};

/// error: 定义的错误类型，用于错误传递
use crate::{
    error::MyError,
    parse_paras::PARAS,
};

/// 调用ChatGPT的dall-e-2或dall-e-3进行绘图
/// 格式：`quality:xxx size:xxx style:xxx prompt:xxx`
/// 说明：
///     1. quality：指定图片质量，支持1(Standard)、2(Hd)，默认1，仅对dall-e-3有效
///     2. size：指定图片大小，支持1(1024X1024)、2(1792X1024)、3(1024X1792)、4(256x256)、5(512x512)，其中dall-e-2支持1、4、5，dall-e-3支持1、2、3，默认1
///     3. style：指定图片风格，支持1(natural)、2(vivid)，默认1，仅对dall-e-3有效
///     4. prompt：图片描述信息（必须指定，且最后指定），如果只有prompt，可以省略“prompt:”
/// 注意：
///     1. 同时指定多个参数，`prompt:`要放在最后
///     2. 参数间空格间隔，`quality:xxx size:xxx style:xxx`不要有多余空格
///     3. 如果只指定`prompt:xxx`，可以省略`prompt:`前缀
pub async fn create_image(uuid: &str, q: &str, model: String, endpoint: &str, api_key: String) -> Result<(String, String), MyError> {
    // 绘图参数
    let (tmp_para, prompt_str) = parse_image_para(q, vec!["quality:", "size:", "style:"])?;
    /*
    let (tmp_para, prompt_str): (Vec<&str>, String) = if ["quality:", "size:", "style:"].iter().any(|x| q.starts_with(x)) {
        let mut tmp: Vec<&str> = vec![];
        let mut prompt_str = "".to_string();
        let tmp_vec: Vec<&str> = q.split(" ").collect();
        for i in 0..tmp_vec.len() {
            if ["quality:", "size:", "style:"].iter().any(|x| tmp_vec[i].starts_with(x)) {
                tmp.push(tmp_vec[i])
            } else if q.starts_with("prompt:") { // `prompt:`放在最后，因此后面内容都用空格合并作为prompt描述信息
                prompt_str = tmp_vec[i..].join(" ");
                break
            } else {
                return Err(MyError::ParaError{para: format!("no such parameter for {}: {}", model, tmp_vec[i])})
            }
        }
        (tmp, prompt_str)
    } else if q.starts_with("prompt:") { // 去除“prompt:”前缀
        (vec![], q.strip_prefix("prompt:").unwrap().to_string())
    } else {
        (vec![], q.to_string())
    };
    // 没有输入prompt则报错
    if prompt_str.is_empty() {
        return Err(MyError::ParaError{para: "must specify prompt in input".to_string()})
    }
    */
    let mut tmp_quality: ImageQuality = ImageQuality::Standard; // 图片质量
    let mut tmp_size: ImageSize = ImageSize::Size1024X1024; // 图片大小
    let mut tmp_style: ImageStyle = ImageStyle::Natural; // 图片风格
    // 遍历输入的参数
    for para in tmp_para {
        if para.starts_with("quality:") {
            if para == "quality:2" {
                tmp_quality = ImageQuality::Hd;
            }
        } else if para.starts_with("size:") {
            if para == "size:2" {
                tmp_size = ImageSize::Size1792X1024;
            } else if para == "size:3" {
                tmp_size = ImageSize::Size1024X1792;
            }
        } else if para.starts_with("style:") {
            if para == "style:2" {
                tmp_style = ImageStyle::Vivid;
            }
        }
    }
    // 使用api key初始化
    let mut client = Client::new(api_key);
    client.set_base_url(endpoint); // 从0.7.0开始舍弃了new_with_base
    // 参数
    /*
    let parameters = CreateImageParametersBuilder::default()
        .prompt(prompt_str) // 描述图片的文本，dall-e-2最多1000个字符，dall-e-3最多4000个字符，例如：A cute dog in the park
        .model(model) // 选择模型，支持dall-e-2和dall-e-3
        .n(1u32) // 生成图片的数量，dall-e-2可以指定1~10，dall-e-3只能是1
        .quality(tmp_quality) // 仅对dall-e-3有效，支持：Standard、Hd
        .response_format(ResponseFormat::B64Json) // Url或B64Json
        .size(tmp_size) // 图片大小，dall-e-2支持256x256、512x512、1024x1024，dall-e-3支持1024x1024、1792x1024、1024x1792
        .style(tmp_style) // 仅对dall-e-3有效，支持：vivid、natural
        .build().map_err(|e| MyError::CreateImageError{error: e})?;
    */
    let mut para_builder = CreateImageParametersBuilder::default();
    para_builder.prompt(prompt_str); // 描述图片的文本，dall-e-2最多1000个字符，dall-e-3最多4000个字符，例如：A cute dog in the park
    para_builder.model(&model); // 选择模型，支持dall-e-2和dall-e-3
    para_builder.n(1u32); // 生成图片的数量，dall-e-2可以指定1~10，dall-e-3只能是1
    para_builder.response_format(ResponseFormat::B64Json); // Url或B64Json
    para_builder.size(tmp_size); // 图片大小，dall-e-2支持256x256、512x512、1024x1024，dall-e-3支持1024x1024、1792x1024、1024x1792
    if model == "dall-e-3" {
        para_builder.quality(tmp_quality); // 仅对dall-e-3有效，支持：Standard、Hd
        para_builder.style(tmp_style); // 仅对dall-e-3有效，支持：vivid、natural
    }
    let parameters = para_builder.build().map_err(|e| MyError::CreateImageError{error: e})?;
    // 生成图片
    let result = client.images().create(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;

    // 保存至本地指定路径下的uuid文件夹中，路径不存在则不保存，不会报错，需要在Cargo.toml中指定`features=["download"]`
    let paths = result.save(&format!("{}/{}", PARAS.outpath, uuid)).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?; // 返回保存的图片的路径，例如：["./result/58d89dfa-6358-4505-874b-d22c7cde0bd7/WPZPRRVDAVYCSAAA.png"]
    if paths.len() > 0 {
        //println!("save created image to: {}", paths[0]);
        let name = paths[0].replace(&format!("{}/{}/", PARAS.outpath, uuid), "");
        match &result.data[0] { // 因为只绘制一张图，所以选第一个即可
            ImageData::Url{url, ..} => Ok((name, url.clone())),
            ImageData::B64Json{b64_json, ..} => Ok((name, format!("data:image/svg+xml;base64,{b64_json}"))),
        }
    } else {
        Err(MyError::ParaError{para: "save created image failed".to_string()})
    }
}

/// 调用ChatGPT的gpt-image-1进行绘图
/// q表示用户输入的绘图描述，uploaded_image表示用户上传的图片
/// 如果uploaded_image是Some表示用户上传了图片，则根据用户输入内容基于上传的图片生成图片
/// 如果uploaded_image是None表示用户没有上传图片，则根据用户输入内容生成图片
/// 格式：`quality:xxx size:xxx prompt:xxx`
/// 说明：
///     1. quality：指定图片质量，支持1(low)、2(medium)、3(high)，默认2
///     2. size：指定图片大小，支持1(1024X1024)、2(1536x1024)、3(1024x1536)，默认1
///     3. prompt：图片描述信息（必须指定，且最后指定），如果只有prompt，可以省略“prompt:”
/// 注意：
///     1. 同时指定多个参数，`prompt:`要放在最后
///     2. 参数间空格间隔，`quality:xxx size:xxx`不要有多余空格
///     3. 如果只指定`prompt:xxx`，可以省略`prompt:`前缀
pub async fn create_edit_image(uuid: &str, uploaded_image: Option<String>, q: String, endpoint: &str, api_key: String) -> Result<(String, String), MyError> {
    // 绘图参数
    let (tmp_para, prompt_str) = parse_image_para(&q, vec!["quality:", "size:"])?;
    let mut tmp_quality: ImageQuality = ImageQuality::Medium; // 图片质量
    let mut tmp_size: ImageSize = ImageSize::Size1024X1024; // 图片大小
    // 遍历输入的参数
    for para in tmp_para {
        if para.starts_with("quality:") {
            if para == "quality:1" {
                tmp_quality = ImageQuality::Low;
            } else if para == "quality:3" {
                tmp_quality = ImageQuality::High;
            }
        } else if para.starts_with("size:") {
            if para == "size:2" {
                tmp_size = ImageSize::Size1536X1024;
            } else if para == "size:3" {
                tmp_size = ImageSize::Size1024X1536;
            }
        }
    }
    // 使用api key初始化
    let mut client = Client::new(api_key);
    client.set_base_url(endpoint); // 从0.7.0开始舍弃了new_with_base
    let result: ImageResponse;
    // https://platform.openai.com/docs/api-reference/images/createEdit
    match uploaded_image {
        Some(img_name) => { // 上传了图片，调用`images/createEdit`
            let parameters = EditImageParametersBuilder::default()
                .prompt(prompt_str) // 描述图片的文本，gpt-image-1最多32000个字符
                .image(FileUpload::File(format!("{}/{}/{}", PARAS.outpath, uuid, img_name))) // 格式：png、webp、jpg，大小：<25MB，形状：不需要是正方形
                .model("gpt-image-1".to_string()) // 选择模型，这里固定为gpt-image-1
                .n(1u32) // 生成图片的数量，默认1
                .quality(tmp_quality) // gpt-image-1支持：high、medium、low，auto表示自动选择最高质量
                .size(tmp_size) // 图片大小，gpt-image-1支持1024x1024、1536x1024、1024x1536
                .build().map_err(|e| MyError::EditImageError{error: e})?;
            result = client.images().edit(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;
        },
        None => { // 没有上传图片，调用`images/create`
            let parameters = CreateImageParametersBuilder::default()
                .prompt(prompt_str) // 描述图片的文本，gpt-image-1最多32000个字符
                .model("gpt-image-1".to_string()) // 选择模型，这里固定为gpt-image-1
                .n(1u32) // 生成图片的数量，默认1
                .quality(tmp_quality) // gpt-image-1支持：high、medium、low，auto表示自动选择最高质量
                .size(ImageSize::Size1024X1024) // 图片大小，gpt-image-1支持1024x1024、1536x1024、1024x1536
                .build().map_err(|e| MyError::CreateImageError{error: e})?;
            result = client.images().create(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?;
        },
    }

    // 保存至本地指定路径下的uuid文件夹中，路径不存在则不保存，不会报错，需要在Cargo.toml中指定`features=["download"]`
    // 这个save方法内部会解析返回的是url还是b64，都可以保存
    // https://docs.rs/openai_dive/1.0.1/src/openai_dive/v1/resources/image.rs.html
    let paths = result.save(&format!("{}/{}", PARAS.outpath, uuid)).await.map_err(|e| MyError::ApiError{uuid: uuid.to_string(), error: e})?; // 返回保存的图片的路径，例如：["./result/58d89dfa-6358-4505-874b-d22c7cde0bd7/WPZPRRVDAVYCSAAA.png"]
    if paths.len() > 0 {
        //println!("save created image to: {}", paths[0]);
        let name = paths[0].replace(&format!("{}/{}/", PARAS.outpath, uuid), "");
        match &result.data[0] { // 因为只绘制一张图，所以选第一个即可
            ImageData::Url{url, ..} => Ok((name, url.clone())),
            ImageData::B64Json{b64_json, ..} => Ok((name, format!("data:image/svg+xml;base64,{b64_json}"))),
        }
    } else {
        Err(MyError::ParaError{para: "save created image failed".to_string()})
    }
}

/// 图片转base64，返回base64编码的字符串
/// openai_dive 1.0.0使用的是base64 0.22
pub fn image_to_base64(uuid: &str, image_name: &str) -> Result<String, MyError> {
    let image_file = format!("{}/{}/{}", PARAS.outpath, uuid, image_name);
    let data: Vec<u8> = read(image_file)?; // 相当于`File::open`+`read_to_end`，返回`Result<Vec<u8>>`
    Ok(format!("data:image/svg+xml;base64,{}", general_purpose::STANDARD.encode(data)))
}

/// base64转回图片，返回保存的图片路径
/// 由于`gpt-image-1`返回的是png格式的base64字符串，因此这里固定为png格式
pub fn base64_to_png(uuid: &str, b64: &str) -> Result<String, MyError> {
    let uuid_path = format!("{}/{}", PARAS.outpath, uuid);
    let full_path = generate_file_name(&uuid_path, 16, "png"); // 生成由大写英文字母构成的指定长度随机文件名
    let bytes = if b64.starts_with("data:image/svg+xml;base64,") {
        general_purpose::STANDARD.decode(b64.strip_prefix("data:image/svg+xml;base64,").unwrap()).map_err(|e| MyError::Base64DecodeError{file: full_path.clone(), error: e})?
    } else {
        general_purpose::STANDARD.decode(b64).map_err(|e| MyError::Base64DecodeError{file: full_path.clone(), error: e})?
    };
    write(&full_path, bytes).map_err(|e| MyError::WriteFileError{file: full_path.clone(), error: e})?;
    Ok(full_path)
}

/// 解析绘图参数
/// q：用户输入的字符串
/// p：对应哪些参数
fn parse_image_para(q: &str, p: Vec<&str>) -> Result<(Vec<String>, String), MyError> {
    let (tmp_para, prompt_str): (Vec<String>, String) = if p.iter().any(|x| q.starts_with(x)) {
        let mut tmp: Vec<String> = vec![];
        let mut prompt_str = "".to_string();
        let tmp_vec: Vec<&str> = q.split(" ").collect();
        for i in 0..tmp_vec.len() {
            if p.iter().any(|x| tmp_vec[i].starts_with(x)) {
                tmp.push(tmp_vec[i].to_string())
            } else if tmp_vec[i].starts_with("prompt:") { // `prompt:`放在最后，因此后面内容都用空格合并作为prompt描述信息
                prompt_str = tmp_vec[i..].join(" ").to_string();
                break
            } else {
                return Err(MyError::ParaError{para: format!("no such parameter: {}", tmp_vec[i])})
            }
        }
        (tmp, prompt_str)
    } else if q.starts_with("prompt:") { // 去除“prompt:”前缀
        (vec![], q.strip_prefix("prompt:").unwrap().to_string())
    } else {
        (vec![], q.to_string())
    };
    // 没有输入prompt则报错
    if prompt_str.is_empty() {
        return Err(MyError::ParaError{para: "must specify prompt in input".to_string()})
    }
    Ok((tmp_para, prompt_str))
}
