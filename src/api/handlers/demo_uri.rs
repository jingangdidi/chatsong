use tracing::{event, Level};

/// Handler for `/嵌套的前缀/get-uri` GET
/// 服务端获取用户访问的uri
pub async fn demo_uri(uri: axum::http::Uri, original_uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "GET {}", original_uri.path());
    format!("The URI is: {}\nThe Original URI is: {}\n", uri, original_uri.path())
}
