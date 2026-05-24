use tracing::{event, Level};

use crate::openai::for_chat::START_AUDIO;

/// Handler for `/嵌套的前缀/microphone/:id` GET
/// 开启或关闭语音模式
pub async fn microphone(axum::extract::Path(id): axum::extract::Path<String>) {
    if id == "true" {
        // 开启语音模式
        if *START_AUDIO.lock().await {
            event!(Level::INFO, "audio mode has already been activated");
        } else {
            event!(Level::INFO, "audio mode successfully activated");
            *START_AUDIO.lock().await = true;
        }
    } else {
        // 关闭语音模式
        if *START_AUDIO.lock().await {
            event!(Level::INFO, "audio mode successfully deactivated");
            *START_AUDIO.lock().await = false;
        } else {
            event!(Level::INFO, "audio mode has already been deactivated");
        }
    }
}
