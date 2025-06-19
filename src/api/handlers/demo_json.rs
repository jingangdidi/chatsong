use serde_json::{json, Value};
use tracing::{event, Level};

/// Handler for `/嵌套的前缀/demo-json` PUT
/// 使用`axum::extract::Json`提取client发送来的json数据
/// 注意：client发送json数据时，`Content-Type`需要设为`application/json`
/// uri要放在前，否则报错，https://github.com/tokio-rs/axum/discussions/1735
pub async fn put_demo_json(uri: axum::extract::OriginalUri, axum::extract::Json(data): axum::extract::Json<serde_json::Value>) -> String {
    event!(Level::INFO, "PUT {}", uri.path());
    format!("Put demo JSON data: {:?}", data)
}

/// Handler for `/嵌套的前缀/demo-json` GET
/// 使用`axum::extract::Json`向client发送json数据
/// 注意：client接收json数据时，`Accept`需要设为`application/json`
pub async fn get_demo_json(uri: axum::extract::OriginalUri) -> axum::extract::Json<Value> {
    event!(Level::INFO, "GET {}", uri.path());
    json!({"a":"b"}).into()
}
