use tracing::{event, Level};

/// Handler for `/嵌套的前缀/multi-foo` GET
pub async fn get_foo(uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "GET {}", uri.path());
   "GET foo".to_string()
}

/// Handler for `/嵌套的前缀/multi-foo` PUT
pub async fn put_foo(uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "PUT {}", uri.path());
   "PUT foo".to_string()
}

/// Handler for `/嵌套的前缀/multi-foo` PATCH
pub async fn patch_foo(uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "PATCH {}", uri.path());
   "PATCH foo".to_string()
}

/// Handler for `/嵌套的前缀/multi-foo` POST
pub async fn post_foo(uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "POST {}", uri.path());
   "POST foo".to_string()
}

/// Handler for `/嵌套的前缀/multi-foo` DELETE
pub async fn delete_foo(uri: axum::extract::OriginalUri) -> String {
    event!(Level::INFO, "DELET {}", uri.path());
   "DELETE foo".to_string()
}
