use axum::{
    extract::OriginalUri,
    http::StatusCode,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{event, Level};

/// info: 记录所有用户的信息
use crate::info::delete_msg_by_id; // 从服务端指定uuid中删除指定id的信息，这里id格式是“d”+序号索引，比如“d0”表示第一条信息，成功则返回true，失败返回false

/// Handler for `/嵌套的前缀/delmsg/:id` GET
/// url指定要删除的信息id，从服务端删除
pub async fn del_msg(axum::extract::Path(id): axum::extract::Path<String>, uri: OriginalUri, jar: CookieJar) -> StatusCode {
    // 返回的状态码
    let mut status_code = StatusCode::OK;
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        // 从服务端该uuid中删除指定id的信息，这里id格式是“dm”+序号索引，比如“dm0”表示第一条信息，成功则返回true，失败返回false
        match delete_msg_by_id(&uuid, &id) {
            (true, None) => event!(Level::INFO, "{} GET {}, delete message {} success", uuid, uri.path(), id), // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
            (false, Some(e)) => {
                event!(Level::INFO, "{} GET {}, delete message {} failed: {}", uuid, uri.path(), id, e); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
                status_code = StatusCode::BAD_REQUEST;
            },
            _ => unreachable!(),
        }
    } else {
        event!(Level::INFO, "GET {}, delete message failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
    // 返回状态码
    status_code
}
