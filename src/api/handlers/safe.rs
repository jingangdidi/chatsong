use std::collections::HashMap;
use std::sync::Mutex;

use axum::extract::{Query, OriginalUri};
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

static SAFE: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Handler for `/嵌套的前缀/safe` GET
pub async fn safe(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        let set_safe = params.get("safe").unwrap() == "true";
        let mut data = SAFE.lock().unwrap();
        *data.entry(uuid).or_insert(set_safe) = set_safe;
        event!(Level::INFO, "GET {}, set safe mode to {}", uri.path(), set_safe); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    } else {
        event!(Level::INFO, "GET {}, set safe failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}

/// 判断是否安全模式
pub fn is_safe(uuid: &str) -> bool {
    let data = SAFE.lock().unwrap();
    data.get(uuid).copied().unwrap_or(true)
}
