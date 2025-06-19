use axum::response::Html;
use tracing::{event, Level};

/// Handler for `/嵌套的前缀/hello-html` GET
/// 使用`std::include_str`宏获取相对`hello_html.rs`所在路径的指定html文件，在编译时作为`&'static str`
pub async fn hello_html(uri: axum::extract::OriginalUri) -> Html<&'static str> {
    event!(Level::INFO, "GET {}", uri.path());
    include_str!("../../../hello.html").into()
}
