use axum::{http::StatusCode, extract::OriginalUri};
use tracing::{event, Level};

/// Handler for `/嵌套的前缀/hello` GET
pub async fn hello(uri: OriginalUri) -> Result<String, StatusCode> {
    event!(Level::INFO, "GET {}", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    Ok("Hello world!".to_string())
}
