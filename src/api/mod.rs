use axum::Router;

pub mod parse_paras;
pub mod info;
pub mod error;
pub mod token;
pub mod prompt;
pub mod web;
pub mod openai;
pub mod code;
pub mod pdf;
pub mod html_page;
pub mod graph;
pub mod ctrlc;

mod handlers;
mod v1;
mod v2;

/// 将不同前缀与不同version的Router嵌套
/// 例如下面v1和v2前缀，访问：
/// http:127.0.0.1/v1/hello
/// http:127.0.0.1/v2/hello
/// 在main中直接调用这个函数创建路由
pub fn configure() -> Router {
    Router::new()
        .nest("/v1", v1::configure()) // nest可以将之前前缀和Router嵌套在一起，这样方便把不同version的Router分开定义
        .nest("/v2", v2::configure()) // 例如这里又定义了v2的Router
}
