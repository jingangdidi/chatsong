use std::collections::HashMap;
use std::sync::Mutex;

use axum::extract::{
    Query,
    OriginalUri,
};
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

pub static NEW_INSTRUCTION: Lazy<Mutex<HashMap<String, Option<String>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Handler for `/嵌套的前缀/instruction` GET
pub async fn instruction(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        if let Some(msg) = params.get("msg") {
            event!(Level::INFO, "{} user new instruction: {}", uuid, msg);
            let mut data = NEW_INSTRUCTION.lock().unwrap();
            match data.get_mut(&uuid) {
                Some(instruction) => if msg.trim().is_empty() {
                    *instruction = None;
                } else {
                    *instruction = Some(msg.trim().to_string());
                },
                None => {
                    data.insert(uuid, Some(msg.trim().to_string()));
                },
            }
        }
    } else {
        event!(Level::INFO, "GET {}, set new instruction failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}

/// 获取指定 uuid 最新的指令
pub fn get_new_instruction(uuid: &str) -> Option<String> {
    let mut new_instruction = NEW_INSTRUCTION.lock().unwrap();
    if let Some(instruction) = new_instruction.get_mut(uuid) {
        if let Some(msg) = instruction {
            let instruction_msg = msg.clone();
            *instruction = None;
            return Some(instruction_msg)
        }
    }
    None
}
