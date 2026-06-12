use std::collections::HashMap;

use axum::extract::{
    Query,
    OriginalUri,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{event, Level};

/// info: 记录所有用户的信息
use crate::info::update_approval;

/// Handler for `/嵌套的前缀/approval` GET
pub async fn approval(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        let approval = match params.get("approval") {
            Some(p) => {
                event!(Level::INFO, "{} user approval: {}", uuid, p);
                p.clone()
            },
            None => "false".to_string(), // 不允许
        };
        update_approval(&uuid, Some(approval));
    } else {
        event!(Level::INFO, "GET {}, set approval failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}
