use std::collections::HashMap;
use std::sync::Mutex;

use axum::extract::{
    Query,
    OriginalUri,
};
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

use crate::{
    info::{
        get_messages, // 获取指定uuid最近的指定数量个message
        update_qa_msg_num, // 客户端下拉选项`上下文消息数`改变时更新限制的问答对数量、限制的消息数量、提问是否包含prompt
    },
    api::handlers::chat::get_qa_msg_p,
};

pub static MEMORY: Lazy<Mutex<HashMap<String, Option<String>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Handler for `/嵌套的前缀/memory` GET
pub async fn memory(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        if let Some(m) = match params.get("memory") {
            let for_memory = if m.trim().is_empty() {
                event!(Level::INFO, "{} remember the current conversation", uuid);
                update_qa_msg_num(&uuid, get_qa_msg_p(params.get("num"), false));
                let messages = get_messages(&uuid);
                messages.into_iter().map(|msg| format!("{:?}", msg)).collect::<Vec<String>>().join("\n")
            } else {
                event!(Level::INFO, "{} Remember the following content: {}", uuid, m.trim());
                m.trim().to_string()
            };
            let mut data = MEMORY.lock().unwrap();
            match data.get_mut(&uuid) {
                Some(m) => *m = Some(for_memory),
                None => {
                    data.insert(uuid, Some(for_memory));
                },
            }
        }
    } else {
        event!(Level::INFO, "GET {}, set memory failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}
