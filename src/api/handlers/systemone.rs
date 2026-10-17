use axum::extract::OriginalUri;
use axum_extra::extract::cookie::CookieJar;
use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    system_one::select_tools::get_relevant_tools,
    api::handlers::systemone_toggle::is_systemone,
};

/// Handler for `/嵌套的前缀/systemone` GET
pub async fn systemone(uri: OriginalUri, jar: CookieJar, body: String) -> String {
    let mut result = String::new();
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        event!(Level::INFO, "{} POST {}", uuid, uri.path());
        if is_systemone(&uuid) {
            if let Some(url) = PARAS.systemone.as_ref() {
                match get_relevant_tools(url, &body).await {
                    Ok(tools) => {
                        event!(Level::INFO, "{} relevant tools: {}", uuid, tools.iter().map(|t| format!("{}({})", t.1, t.2)).collect::<Vec<_>>().join(", "));
                        result = tools.into_iter().map(|t| t.0).collect::<Vec<_>>().join(",");
                    },
                    Err(e) => event!(Level::ERROR, "{} system one decision error: {}", uuid, e),
                }
            }
        }
    } else {
        event!(Level::INFO, "POST {}, system one failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
    result
}
