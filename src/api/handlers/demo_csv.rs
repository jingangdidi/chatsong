use tracing::{event, Level};

/// Handler for `/嵌套的前缀/demo-csv` GET
/// 设置自定义的HTTP header为`text/csv`，然后发送csv数据，浏览器可能直接下载存储为`demo-csv.csv`
pub async fn get_demo_csv(uri: axum::extract::OriginalUri) -> impl axum::response::IntoResponse {
    event!(Level::INFO, "GET {}", uri.path());
    let mut headers = axum::http::HeaderMap::new();
    headers.insert( // 插入自定义的类型`text/csv`
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static(&"text/csv")
    );
    (
        headers,
        concat!(
            "alpha,bravo,charlie\n",
            "delta,echo,foxtrot\n",
        )
    )
}
