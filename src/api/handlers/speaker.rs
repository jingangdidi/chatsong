use std::net::SocketAddr;
use std::sync::Mutex;

use axum::extract::{OriginalUri, ConnectInfo};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

use crate::api::handlers::chat::is_local_request;

pub static SPEAKER: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

/// Handler for `/嵌套的前缀/speaker` GET
/// 开启朗读模式
pub async fn speaker(uri: OriginalUri, ConnectInfo(addr): ConnectInfo<SocketAddr>, jar: CookieJar)-> Json<bool> {
    // 检查是否服务端所在电脑发起的请求
    let ip = addr.ip();
    let is_local = is_local_request(&ip);
    if is_local {
        // 获取uuid
        if let Some(c) = jar.get("srx-tzn") { // 获取cookie
            let uuid = c.value().to_string();
            if cfg!(any(feature = "tts", feature = "tts-cuda", feature = "tts-metal")) {
                let mut data = SPEAKER.lock().unwrap();
                *data = !*data;
                event!(Level::INFO, "GET {}, {} {} speaker mode", uri.path(), uuid, if *data { "start" } else { "close" }); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
            } else {
                event!(Level::WARN, "GET {}, {} you need to specify '--features' during compilation", uri.path(), uuid); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
                return Json(false)
            }
        } else {
            event!(Level::INFO, "GET {}, set speaker mode failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
            return Json(false)
        }
    } else {
        event!(Level::INFO, "GET {}, set speaker mode failed, not local", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
    Json(is_local)
}

/// 判断是否开启的朗读模式
pub fn use_speaker() -> bool {
    let data = SPEAKER.lock().unwrap();
    *data
}
