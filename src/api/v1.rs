use axum::routing::{get, post};
use axum::Router;
use axum::extract::DefaultBodyLimit;
/*
use tower_http::services::{
    ServeDir,
    //ServeFile,
};
*/

/// handlers: 每个路由的函数
use crate::api::handlers::{
    hello::hello,
    demo_status::demo_status,
    user::{create_user, list_users},
    demo_uri::demo_uri,
    multi_foo::{get_foo, put_foo, patch_foo, post_foo, delete_foo},
    get_items_id::get_items_id,
    get_items::get_items,
    demo_json::{get_demo_json, put_demo_json},
    demo_csv::get_demo_csv,
    index::index,
    chat::chat,
    save::{save_log, save_speech, save},
    upload::upload,
    usage::usage,
    fallback::fallback,
};

/// 创建version1的路由
/// https://github.com/tokio-rs/axum/blob/main/examples/templates/src/main.rs
/// https://dev.to/shuttle_dev/building-a-simple-web-server-in-rust-5c57
/// https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs
/// https://matze.github.io/axum-notes/notes/templating/with_askama/index.html
pub fn configure() -> Router {
    Router::new()
        //.nest_service("/templates", ServeDir::new("templates")) // 可访问该路径内的所有文件，例如：`/v1/templates/css/style.css`，在html文件中调用css文件时，需指定包括`/v1/templates/`的路径前缀，例如：`/v1/templates/css/style.css`
        //.route_service("/style.css", ServeFile::new("templates/style.css")) // 可访问指定路由路径的单个文件
        .route("/hello", get(hello)) // GET /v1/hello，返回状态码或字符串
        .route("/demo-status", get(demo_status)) // GET /demo-status，返回状态码和字符串
        .route("/create-user", post(create_user)) // POST /create-user，创建信息，返回状态码和字符串
        .route("/users", get(list_users)) // GET /users，获取信息，返回json数据
        .route("/get-uri", get(demo_uri)) // GET /get-uri，服务端获取访问的uri
        .route("/multi-foo", get(get_foo).put(put_foo).patch(patch_foo).post(post_foo).delete(delete_foo)) // GET,PUT,PATCH,POST,DELET /v1/multi-foo，同一个路由支持GET、PUT、PATCH、POST、DELETE
        .route("/items/:id",get(get_items_id)) // GET /v1/items/:id，获取路由变量
        .route("/items", get(get_items)) // GET /v1/items，解析url中的参数，存储到HashMap
        .route("/demo-json", get(get_demo_json).put(put_demo_json)) // `PUT /v1/demo-json`和`GET /v1/demo-json`，发送及返回json数据
        .route("/demo-csv", get(get_demo_csv)) // GET /v1/demo-csv，返回csv数据
        .route("/", get(index)) // GET /v1，返回chat页面
        .route("/chat", post(chat)) // POST /v1/chat，返回答案，需要把问题放到body中，因此这里使用post
        .route("/save-log", get(save_log)) // GET /v1/save-log，下载问答记录
        .route("/save-speech", get(save_speech)) // GET /v1/save-speech，下载生成的音频文件
        .route("/save/:id", get(save)) // GET /v1/save/:id，下载生成图片或音频文件
        .route("/upload", post(upload)) // POST /v1/upload，上传文件
        .route("/usage", get(usage)) // GET /v1/usage，查看使用说明
        .layer(DefaultBodyLimit::max(1024*1024*100)) // 设置上传文件大小限制为1024*1024*100=104857600=100M
        .fallback(fallback) // 没有匹配到任何路由，执行fallback
}
