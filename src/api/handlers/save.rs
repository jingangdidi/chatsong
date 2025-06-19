use std::io::Error;

use axum::{
    body::Body,
    extract::OriginalUri,
    http::{header, StatusCode, HeaderMap},
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;
use tokio::{
    fs::File,
    io::AsyncReadExt,
};
//use tokio_util::io::ReaderStream;
use tracing::{event, Level};

/// info: 记录所有用户的信息
use crate::{
    info::{
        contain_uuid, // 判断指定uuid是否已存在于DATA中
        get_speech_file, // 获取指定uuid对应路径下`speech.mp3`文件路径
        valid_filename, // 获取保存chat记录时的文件名
        get_file_for_download, // 获取指定uuid对话中，指定索引对应message的图片或音频文件名（包含路径），以及是否是音频，提供给用户下载
    },
    html_page::create_download_page, // 生成chat记录页面html字符串
};

/// Handler for `/嵌套的前缀/save-log` GET
/// 先判断是否有cookie，cookie值对应的服务端uuid文件夹中是否含有`时间戳.log`文件
/// 如果有cookie对应的uuid文件夹中的`时间戳.log`文件，则读取并创建html文件响应给客户端
pub async fn save_log(uri: OriginalUri, jar: CookieJar) -> (HeaderMap, axum::body::Body) {
    event!(Level::INFO, "GET {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    // 准备header
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/html; charset=utf-8".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
    // 获取uuid和错误信息
    let (err_str, uuid): (Option<String>, String) = match jar.get("srx-tzn") { // 获取cookie
        Some(c) => { // 有cookie
            let uuid = c.value().to_string();
            headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", valid_filename(&uuid)).parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
            if contain_uuid(&uuid) { // 有cookie且在服务端存在相应文件夹
                (None, uuid)
            } else { // 服务端没有该uuid
                (Some(format!("The server does not contain the specified uuid: {}, you may first pose a question once, then endeavor to retrieve the chat records", uuid)), "".to_string()) // 有cookie但不存在于服务端，此时可能是由于当前页面还未提问，服务端没有去读取相应uuid内容，可先随便提问一次，服务端读取了该uuid的内容后再保存
            }
        },
        None => {
            headers.insert(header::CONTENT_DISPOSITION, "attachment; filename=\"chat_log.html\"".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
            (None, "Unable to retrieve the relevant chat records due to missing cookie.".to_string()) // 无cookie
        },
    };
    let html_str = create_download_page(&uuid, err_str);
    let stream = async_stream::stream! {
        // 将html字符串按照指定长度（1mb）拆分，然后stream传输
        // https://users.rust-lang.org/t/solved-how-to-split-string-into-multiple-sub-strings-with-given-length/10542/12
        let mut html_chunk = html_str.as_bytes().chunks(1024*1024);
        while let Some(s) = html_chunk.next() {
            let tmp: Result<Vec<u8>, Error> = Ok(s.to_vec());
            yield tmp;
        }
        /*
        let tmp: Result<Vec<u8>, Error> = Ok(html_str.as_bytes().to_vec());
        yield tmp;
        */
    };
    // convert the `Stream` into an `axum::body::Body`
    let body = Body::from_stream(stream); // 0.7版本舍弃了`StreamBody::new(stream);`，改用`Body::from_stream(stream)`
    (headers, body)
}

/// Handler for `/嵌套的前缀/save-speech` GET
/// 下载指定uuid的`speech.mp3`，如果没有，则返回`speech.txt`，内容为提示信息
/// 由于读取文件创建和和直接使用字符串创建`async_stream::stream!`返回类型不同，因此只能分别在每个分支创建最终body，不能先得到stream再最终创建body
pub async fn save_speech(uri: OriginalUri, jar: CookieJar) -> impl IntoResponse {
    event!(Level::INFO, "GET {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    // 先判断是否有cookie，cookie值对应的服务端uuid文件夹中是否含有`speech.mp3`文件
    // 如果有cookie对应的uuid文件夹中的`speech.mp3`文件，则读取并响应给客户端
    // 如果没有cookie或uuid路径下没有`speech.mp3`，则传递`speech.txt`
    let (uuid, tmp_speech, no_uuid) = match jar.get("srx-tzn") { // 获取cookie
        Some(c) => {
            let uuid = c.value().to_string();
            let tmp_speech = get_speech_file(&uuid);
            (uuid, tmp_speech, false)
        },
        None => ("".to_string(), "".to_string(), true),
    };
    // 传递数据
    if tmp_speech.is_empty() { // 没有`speech.mp3`文件，传递`speech.txt`，内容提示信息
        // 准备header
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/txt; charset=utf-8".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"speech_{}.txt\"", uuid).parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        let stream = async_stream::stream! {
            let tmp: Result<Vec<u8>, Error> = if no_uuid { // 没有cookie值
                Ok("Unable to retrieve the speech.mp3 due to missing cookie.".as_bytes().to_vec()) // 没有cookie
            } else { // 有cookie值但是没有`speech.mp3`
                Ok(format!("The server lacks speech.mp3 corresponding to the specified uuid: {}", uuid).as_bytes().to_vec()) // 有cookie且存在于服务端，但是没`speech.mp3`文件
            };
            yield tmp;
        };
        // convert the `Stream` into an `axum::body::Body`
        let body = Body::from_stream(stream); // 0.7版本舍弃了`StreamBody::new(stream);`，改用`Body::from_stream(stream)`
        Ok((headers, body))
    } else { // 传递`speech.mp3`
        // 准备header
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "audio/mpeg".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"speech_{}.mp3\"", uuid).parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        // `File` implements `AsyncRead`
        let mut file = match File::open(&tmp_speech).await {
            Ok(file) => file,
            Err(e) => return Err((StatusCode::NOT_FOUND, format!("open {} error: {}", tmp_speech, e))),
        };
        // convert the `AsyncRead` into a `Stream`
        // ReaderStream::new(file) // 由于其他分支返回值都是`ReaderStream<&[u8]>`，这里返回`ReaderStream<File>`会报错
        let stream = async_stream::stream! {
            const CHUNK_SIZE: usize = 1048576; // 默认每块大小1M，1024*1024=1048576
            let mut n: usize;
            loop { // 循环每块
                // CHUNK_SIZE最大好像是1M（即1024*1024），太大这里会报错`thread xxx has overflowed its stack`，需要改用vec
                // 参考：https://users.rust-lang.org/t/what-can-i-do-to-avoid-thread-main-has-overflowed-its-stack-when-working-with-large-arrays/77091/3
                // let mut my_buffer = [0; CHUNK_SIZE];
                let mut my_buffer = vec![0; CHUNK_SIZE];
                // 读取一个chunk
                n = match file.read(&mut my_buffer).await {
                    Ok(n) => {
                        if n == 0 {
                            break
                        }
                        n
                    },
                    Err(_) => break, // 不能返回错误，否则两个分支返回类型不一致，return Err((StatusCode::NOT_FOUND, format!("read chat log file error: {}", e))),
                };
                if n < CHUNK_SIZE {
                    my_buffer.truncate(n); // 截取读取的内容部分，否则多余未用到的部分也会被传输保存
                }
                let tmp: Result<Vec<u8>, Error> = Ok(my_buffer);
                yield tmp;
            }
        };
        // convert the `Stream` into an `axum::body::Body`
        let body = Body::from_stream(stream); // 0.7版本舍弃了`StreamBody::new(stream);`，改用`Body::from_stream(stream)`
        Ok((headers, body))
    }
}

/// Handler for `/嵌套的前缀/save/:id` GET
/// 下载指定uuid的图片（直接在页面右键保存与下载的大小一样）或音频，如果没有，则返回`not_found.txt`，内容为提示信息
/// 由于读取文件创建和和直接使用字符串创建`async_stream::stream!`返回类型不同，因此只能分别在每个分支创建最终body，不能先得到stream再最终创建body
pub async fn save(axum::extract::Path(id): axum::extract::Path<usize>, uri: OriginalUri, jar: CookieJar) -> impl IntoResponse {
    event!(Level::INFO, "GET {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    // 先判断是否有cookie，cookie值对应的服务端uuid文件夹中是否含有指定message索引对应的图片或音频文件
    // 如果有，则读取并响应给客户端
    // 如果没有，则传递`not_found.txt`
    let (uuid, tmp_downlod, is_voice) = match jar.get("srx-tzn") { // 获取cookie
        Some(c) => {
            let uuid = c.value().to_string();
            if let Some((tmp_downlod, is_voice)) = get_file_for_download(&uuid, id) {
                (uuid, tmp_downlod, is_voice)
            } else {
                (uuid, "".to_string(), false)
            }
        },
        None => ("".to_string(), "".to_string(), false),
    };
    // 传递数据
    if tmp_downlod.is_empty() { // 没有要下载的文件，传递`not_found.txt`，内容提示信息
        // 准备header
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "text/txt; charset=utf-8".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"not_found_{}.txt\"", uuid).parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        let stream = async_stream::stream! {
            let tmp: Result<Vec<u8>, Error> = if uuid.is_empty() { // 没有cookie值
                Ok("Unable to retrieve the file due to missing cookie.".as_bytes().to_vec()) // 没有cookie
            } else { // 有cookie值但是没有要下载的文件
                Ok(format!("The server lacks file corresponding to the specified uuid: {}", uuid).as_bytes().to_vec()) // 有cookie且存在于服务端，但是没要下载的文件
            };
            yield tmp;
        };
        // convert the `Stream` into an `axum::body::Body`
        let body = Body::from_stream(stream); // 0.7版本舍弃了`StreamBody::new(stream);`，改用`Body::from_stream(stream)`
        Ok((headers, body))
    } else { // 传递要下载的图片或音频文件
        // 准备header
        let mut headers = HeaderMap::new();
        if is_voice {
            headers.insert(header::CONTENT_TYPE, "audio/mpeg".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
            headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"speech_{}.mp3\"", uuid).parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        } else {
            headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
            headers.insert(header::CONTENT_DISPOSITION, format!("attachment; filename=\"image_{}.png\"", uuid).parse().unwrap()); // value需要是`header::HeaderValue`，创建：`HeaderValue::from_str("hello").unwrap()`
        }
        // `File` implements `AsyncRead`
        let mut file = match File::open(&tmp_downlod).await {
            Ok(file) => file,
            Err(e) => return Err((StatusCode::NOT_FOUND, format!("open {} error: {}", tmp_downlod, e))),
        };
        // convert the `AsyncRead` into a `Stream`
        // ReaderStream::new(file) // 由于其他分支返回值都是`ReaderStream<&[u8]>`，这里返回`ReaderStream<File>`会报错
        let stream = async_stream::stream! {
            const CHUNK_SIZE: usize = 1048576; // 默认每块大小1M，1024*1024=1048576
            let mut n: usize;
            loop { // 循环每块
                // CHUNK_SIZE最大好像是1M（即1024*1024），太大这里会报错`thread xxx has overflowed its stack`，需要改用vec
                // 参考：https://users.rust-lang.org/t/what-can-i-do-to-avoid-thread-main-has-overflowed-its-stack-when-working-with-large-arrays/77091/3
                // let mut my_buffer = [0; CHUNK_SIZE];
                let mut my_buffer = vec![0; CHUNK_SIZE];
                // 读取一个chunk
                n = match file.read(&mut my_buffer).await {
                    Ok(n) => {
                        if n == 0 {
                            break
                        }
                        n
                    },
                    Err(_) => break, // 不能返回错误，否则两个分支返回类型不一致，return Err((StatusCode::NOT_FOUND, format!("read chat log file error: {}", e))),
                };
                if n < CHUNK_SIZE {
                    my_buffer.truncate(n); // 截取读取的内容部分，否则多余未用到的部分也会被传输保存
                }
                let tmp: Result<Vec<u8>, Error> = Ok(my_buffer);
                yield tmp;
            }
        };
        // convert the `Stream` into an `axum::body::Body`
        let body = Body::from_stream(stream); // 0.7版本舍弃了`StreamBody::new(stream);`，改用`Body::from_stream(stream)`
        Ok((headers, body))
    }
}
