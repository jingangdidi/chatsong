use std::fs::read;

use axum::{
    extract::{Multipart, OriginalUri},
    //response::Redirect,
};
use axum_extra::extract::cookie::CookieJar;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
};
use chrono::Local;
use openai_dive::v1::resources::chat::{
    ChatMessage,
    ChatMessageContent,
};
//use tokio_util::io::ReaderStream;
use tracing::{event, Level};
use uuid::Uuid;

/// info: 记录所有用户的信息
/// error: 定义的错误类型，用于错误传递
use crate::{
    code::project::merge_code, // 从上传的代码压缩包中获取所有脚本的代码，合并到一起，作为提问的问题
    error::MyError,
    info::{
        create_cookie, // 根据指定uuid创建cookie
        update_cookie_max_age, // 更新指定CookieJar的max-age
        create_uuid_dir, // uuid文件夹不存在则创建
        insert_message, // 将指定message插入到指定uuid的messages中
        DataType, // 存储问答信息的数据
        try_read_file, // 判断指定字符串是否是指定uuid中的文件，如果是则读取内容
    },
    openai::for_image::image_to_base64, // 图片转base64，返回base64编码的字符串
    parse_paras::PARAS,
    pdf::extract_pdf_content, // 读取pdf文件，提取文本内容
    web::parse_html::parse_single_html_str, // 从html文件提取内容
};

/// Handler for `/嵌套的前缀/upload` POST
/// 将客户的上传的文件保存至服务端指定路径的uuid文件夹中
/// html页面实现了upload后保持当前页面不变，因此这里不需要最后再重定向到cht页面，因此返回值不需要是Redirect
//pub async fn upload(uri: OriginalUri, jar: CookieJar, mut multipart: Multipart) -> Result<(CookieJar, Redirect), MyError> {
pub async fn upload(uri: OriginalUri, jar: CookieJar, mut multipart: Multipart) -> Result<CookieJar, MyError> {
    event!(Level::INFO, "POST {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    // 先判断是否有cookie，cookie值作为服务端uuid文件夹，不存在则生成uuid作为cookie
    let (uuid, cookie_jar) = match jar.get("srx-tzn") {
        Some(c) => (c.value().to_string(), update_cookie_max_age(jar)), // 仅修改内部cookie的max-age
        None => {
            let tmp_uuid = Uuid::new_v4().to_string();
            (tmp_uuid.clone(), jar.add(create_cookie(tmp_uuid)))
        },
    };
    // uuid文件夹不存在则创建
    create_uuid_dir(&uuid)?;
    // 获取上传的每个文件，保存到服务端
    while let Some(mut field) = multipart.next_field().await.map_err(|e| MyError::ParseMultipartError{error: e})? {
        // 获取文件名
        let name = match field.file_name() {
            Some(n) => n.to_string(),
            None => return Err(MyError::ParaError{para: "Multipart no file name".to_string()}),
        };

        // 获取上传文件的完整大小
        //let data = field.bytes().await.map_err(|e| MyError::ParseMultipartError{error: e})?;

        // 在服务端创建文件
        let upload_file = format!("{}/{}/{}", PARAS.outpath, uuid, name);
        let file = File::create(&upload_file).await.map_err(|e| MyError::CreateFileError{file: upload_file.clone(), error: e})?;
        let mut file_writer = BufWriter::new(file);

        // stream获取
        while let Some(chunk) = field.chunk().await.map_err(|e| MyError::ParseMultipartError{error: e})? {
            //println!("{}", chunk.len());
            // Write a byte to the buffer.
            file_writer.write(&chunk).await.map_err(|e| MyError::ParaError{para: format!("{}", e)})?;
        }
        // Flush the buffer before it goes out of scope.
        file_writer.flush().await.map_err(|e| MyError::ParaError{para: format!("{}", e)})?;
        event!(Level::INFO, "upload {} done", upload_file);

        // 插入message
        let lowercase_name = name.to_lowercase();
        if lowercase_name.ends_with(".png") || lowercase_name.ends_with(".jpg") {
            let message = ChatMessage::User{
                content: ChatMessageContent::Text(name.clone()),
                name: None,
            };
            insert_message(&uuid, message, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Image(image_to_base64(&uuid, &name)?), None, "", None); // 以图片名称作为用户提问内容，并记录图片的base64字符串
        } else if [".flac", ".mp3", ".mp4", ".mpeg", ".mpga", ".m4a", ".ogg", ".wav", ".webm"].iter().any(|x| lowercase_name.ends_with(x)) {
            let message = ChatMessage::User{
                content: ChatMessageContent::Text(name),
                name: None,
            };
            insert_message(&uuid, message, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Voice, None, "", None); // 以音频文件名称作为用户提问内容
        } else {
            let content = if lowercase_name.ends_with(".pdf") {
                match extract_pdf_content(&uuid, &PARAS.outpath, &name) {
                    Ok(res) => res,
                    Err(e) => {
                        event!(Level::ERROR, "{} pdf error: {}", uuid, e);
                        format!("extract content from {} error: {}", name, e)
                    },
                }
            } else if lowercase_name.ends_with(".zip") {
                let command = format!("code {name}");
                match merge_code(&uuid, &command, &PARAS.outpath) {
                    Ok(res) => res,
                    Err(e) => {
                        event!(Level::ERROR, "{} code error: {}", uuid, e);
                        format!("extract content from {} error: {}", name, e)
                    },
                }
            } else if lowercase_name.ends_with(".html") {
                let html_content = String::from_utf8(read(&upload_file)?).map_err(|e| MyError::FileContentToUtf8Error{file: upload_file, error: e})?; // 读取单个html文件，例如使用SingleFile保存的单个html文件
                match parse_single_html_str(&html_content, false) {
                    Ok(res) => res,
                    Err(e) => {
                        event!(Level::ERROR, "{} extract content from {} error: {}", uuid, name, e);
                        format!("extract content from {} error: {}", name, e)
                    },
                }
            } else { // 其他格式视为文本文件
                let file_content = try_read_file(&uuid, &name);
                if file_content.is_empty() {
                    event!(Level::INFO, "{} no such file in server: {}", uuid, name);
                    format!("no such file in server: {}", name)
                } else {
                    file_content
                }
            };
            let message = ChatMessage::User{
                content: ChatMessageContent::Text(content),
                name: None,
            };
            insert_message(&uuid, message, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Raw(name), None, "", None); // 以文件名称作为用户提问内容，并记录提取的内容字符串
        }
    }
    //Ok((cookie_jar, Redirect::to(uri.path().to_string().strip_suffix("/upload").unwrap()))) // 这里上传完成后重定向到chat页面，`/嵌套的前缀/upload` --> `/嵌套的前缀`
    Ok(cookie_jar)
}
