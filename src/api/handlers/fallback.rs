use axum::http::StatusCode;

/// Handler for any request that fails to match the router routes
/// 没有匹配到任何路由时，执行这里，返回404，页面不显示返回的字符串
pub async fn fallback(uri: axum::http::Uri) -> impl axum::response::IntoResponse {
    //event!(Level::INFO, "fails to match any route");
    (StatusCode::NOT_FOUND, format!("No route {}", uri))
}
