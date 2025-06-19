use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;

use axum::{
    extract::OriginalUri,
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use once_cell::sync::Lazy;
use serde::Serialize;
use tracing::{event, Level};

/// 全局变量，可以修改
static DATA: Lazy<Mutex<HashMap<u32, User>>> = Lazy::new(|| Mutex::new(
    HashMap::from([
        (1, User { id: 1, name: "Antigone".into(), email: "Sophocles@gmail.com".into()}),
        (2, User { id: 2, name: "Beloved".into(), email: "Morrison@gmail.com".into()}),
        (3, User { id: 3, name: "Candide".into(), email: "Voltaire@gmail.com".into()}),
    ])
));

/// 存储用户信息
/// `/嵌套的前缀/create-user` POST: 创建新用户（目前测试没有传递信息，仅在服务端固定添加一个新User）
/// `/嵌套的前缀/users` GET: 获取当前全部用户信息
#[derive(Serialize, Clone, Debug)]
pub struct User {
    id: u64,
    name: String,
    email: String,
}

/// Handler for `/嵌套的前缀/create-user` POST
/// 创建新用户，保存到全局变量DATA中
/// 测试：curl --request POST "http://127.0.0.1:8080/create-user"
pub async fn create_user(uri: OriginalUri) -> impl IntoResponse {
    event!(Level::INFO, "POST {}", uri.path());
    thread::spawn(move || {
        let mut data = DATA.lock().unwrap();
        let tmp_new_user = User { id: 4, name: "new".into(), email: "new@gmail.com".into()};
        data.insert(4, tmp_new_user.clone());
        (StatusCode::CREATED, format!("User created successfully: {:?}", tmp_new_user))
    }).join().unwrap()
}

/// Handler for `/嵌套的前缀/users` GET
/// 返回json数据
pub async fn list_users(uri: OriginalUri) -> Json<Vec<User>> {
    event!(Level::INFO, "GET {}", uri.path());
    thread::spawn(move || {
        let data = DATA.lock().unwrap();
        let users: Vec<User> = data.clone().into_values().collect();
        Json(users)
    }).join().unwrap()
}
