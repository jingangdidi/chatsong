use axum::routing::{get, post};
use axum::Router;

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
    fallback::fallback,
};

/// 创建version1的路由
pub fn configure() -> Router {
    Router::new()
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
        .fallback(fallback) // 没有匹配到任何路由，执行fallback
}
