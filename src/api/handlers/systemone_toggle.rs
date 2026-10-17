use std::collections::HashMap;
use std::sync::Mutex;

use axum::extract::{Query, OriginalUri};
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

static SYSTEMONE: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Handler for `/嵌套的前缀/systemone` GET
pub async fn systemone_toggle(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        let set_systemone = params.get("systemone").unwrap() == "true";
        let mut data = SYSTEMONE.lock().unwrap();
        *data.entry(uuid).or_insert(set_systemone) = set_systemone;
        event!(Level::INFO, "GET {}, set systemone mode to {}", uri.path(), set_systemone); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    } else {
        event!(Level::INFO, "GET {}, set systemone failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}

/// 判断是否自动选择工具
pub fn is_systemone(uuid: &str) -> bool {
    let data = SYSTEMONE.lock().unwrap();
    data.get(uuid).copied().unwrap_or(false)
}
