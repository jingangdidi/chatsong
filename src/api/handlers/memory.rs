use std::collections::HashMap;

use axum::extract::{
    Query,
    OriginalUri,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{event, Level};

use crate::{
    info::{
        get_messages, // 获取指定uuid最近的指定数量个message
        update_qa_msg_num, // 客户端下拉选项`上下文消息数`改变时更新限制的问答对数量、限制的消息数量、提问是否包含prompt
    },
    api::handlers::chat::get_qa_msg_p,
    memory::{
        MEMORY,
        SimpleMemory,
    },
};

/// Handler for `/嵌套的前缀/memory` GET
pub async fn memory(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        if let Some(m) = params.get("memory") {
            let for_memory = if m.trim().is_empty() {
                event!(Level::INFO, "{} remember the current conversation", uuid);
                match get_qa_msg_p(params.get("num"), false) {
                    Ok(qa_msg_p) => {
                        update_qa_msg_num(&uuid, qa_msg_p);
                        let messages = get_messages(&uuid);
                        messages.into_iter().map(|msg| format!("{:?}", msg)).collect::<Vec<String>>().join("\n")
                    },
                    Err(e) => {
                        event!(Level::ERROR, "{} get_qa_msg_p error: {}", uuid, e);
                        return
                    },
                }
            } else {
                event!(Level::INFO, "{} Remember the following content: {}", uuid, m.trim());
                m.trim().to_string()
            };
            let mut data = MEMORY.lock().unwrap();
            match data.get_mut(&uuid) {
                Some(m) => m.remember(for_memory),
                None => {
                    let mut m = SimpleMemory::new(100); // 最多100条记忆
                    m.remember(for_memory);
                    data.insert(uuid, m);
                },
            }
        }
    } else {
        event!(Level::INFO, "GET {}, set memory failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}
