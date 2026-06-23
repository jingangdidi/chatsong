use std::collections::HashMap;
use std::sync::Mutex;

use axum::extract::OriginalUri;
use axum_extra::extract::cookie::CookieJar;
use once_cell::sync::Lazy;
use tracing::{event, Level};

pub static GOAL: Lazy<Mutex<HashMap<String, Option<String>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Handler for `/嵌套的前缀/goal` GET
/// 开启 goal 模式
pub async fn goal(uri: OriginalUri, jar: CookieJar) {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        let mut data = GOAL.lock().unwrap();
        match data.get_mut(&uuid) {
            Some(g) => if let Some(goal_content) = g {
                if goal_content.is_empty() {
                    event!(Level::INFO, "{} close goal mode", uuid);
                    *g = None; // 开启 goal 模式后，没有输入内容，又关闭了 goal 则重置为 None
                } else {
                    event!(Level::INFO, "{} can not close active goal", uuid);
                }
            } else {
                event!(Level::INFO, "{} start goal mode", uuid);
                *g = Some("".to_string()); // 还没有 goal，先设为空字符串，然后用问题更新实际 goal 内容
            }
            None => { // 还没有 goal，先设为空字符串，然后用问题更新实际 goal 内容
                event!(Level::INFO, "{} start goal mode", uuid);
                data.insert(uuid, Some("".to_string()));
            },
        }
    } else {
        event!(Level::INFO, "GET {}, set goal failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
}

/// 获取指定 uuid 的 goal
pub fn get_goal(uuid: &str) -> Option<String> {
    let data = GOAL.lock().unwrap();
    if let Some(goal) = data.get(uuid) {
        if let Some(g) = &goal {
            if g.is_empty() {
                return None
            } else {
                return Some(g.clone())
            }
        }
    }
    None
}

/// 判断指定 uuid 是否是 goal 模式
pub fn running_goal(uuid: &str) -> bool {
    let data = GOAL.lock().unwrap();
    if let Some(goal) = data.get(uuid) {
        goal.is_some()
    } else {
        false
    }
}

/// 追加更新 goal，返回 true 则表示不是 goal 模式
pub fn append_goal(uuid: &str, content: &str) -> bool {
    let mut not_goal_mode = true;
    let mut data = GOAL.lock().unwrap();
    if let Some(goal) = data.get_mut(uuid) {
        if let Some(g) = goal {
            *g += "\n\n";
            *g += content;
            not_goal_mode = false;
        }
    }
    not_goal_mode
}

/// 将指定 uuid 的 goal 设为 None
pub fn reset_goal(uuid: &str) {
    let mut data = GOAL.lock().unwrap();
    if let Some(goal) = data.get_mut(uuid) {
        if goal.is_some() {
            *goal = None;
        }
    }
}
