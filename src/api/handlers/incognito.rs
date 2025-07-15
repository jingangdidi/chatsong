use axum::extract::OriginalUri;
use axum_extra::extract::cookie::CookieJar;
use tracing::{event, Level};

/// info: 记录所有用户的信息
use crate::info::set_incognito; // 设置服务端指定uuid的is_incognito，取反

/// Handler for `/嵌套的前缀/incognito` GET
/// 设置is_incognito
pub async fn incognito(uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        match set_incognito(&uuid) {
            Some(i) => if i {
                event!(Level::INFO, "GET {}, set incognito success: false -> true", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
            } else {
                event!(Level::INFO, "GET {}, set incognito success: true -> false", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
            },
            None => event!(Level::INFO, "GET {}, set incognito failed", uri.path()), // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
        }
    } else {
        event!(Level::INFO, "GET {}, set incognito failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}
