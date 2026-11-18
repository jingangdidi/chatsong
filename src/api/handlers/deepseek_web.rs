use std::collections::HashMap;
use std::sync::Mutex;

use axum::extract::{Query, OriginalUri};
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

static DEEPSEEK_WEB: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Handler for `/嵌套的前缀/deepseek-web` GET
pub async fn deepseek_web(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        let set_ds_web = params.get("dsweb").unwrap() == "true";
        let mut data = DEEPSEEK_WEB.lock().unwrap();
        *data.entry(uuid).or_insert(set_ds_web) = set_ds_web;
        event!(Level::INFO, "GET {}, set deepseek web mode to {}", uri.path(), set_ds_web); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    } else {
        event!(Level::INFO, "GET {}, set deepseek web failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}

/// 判断是否启用 DeepSeek 后端的联网能力
pub fn allow_deepseek_web(uuid: &str) -> bool {
    let data = DEEPSEEK_WEB.lock().unwrap();
    data.get(uuid).copied().unwrap_or(false)
}
