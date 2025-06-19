use tracing::{event, Level};

/// Handler for `/嵌套的前缀/items/:id` GET
/// 使用`axum::extract::Path`从路由路径中提取变量
pub async fn get_items_id(axum::extract::Path(id): axum::extract::Path<String>, uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "GET {}, Get items with path id: {}", uri.path(), id); // 2024-10-10T01:07:46.894105Z  INFO server_for_api::api::handlers::get_items_id: GET /v1/items/123, Get items with path id: 123
    format!("Get items with path id: {:?}", id) // Get items with path id: "123"
}
