use axum::http::StatusCode;
use tracing::{event, Level};

/// Handler for `/嵌套的前缀/demo-status` GET
/// 返回HTTP状态码和字符串，例如OK (200)
pub async fn demo_status(uri: axum::extract::OriginalUri) -> (StatusCode, String) {
    event!(Level::INFO, "GET {}", uri.path());
    (StatusCode::OK, "Everything is OK".to_string())
}
