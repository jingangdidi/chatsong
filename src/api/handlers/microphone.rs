use tracing::{event, Level};

use crate::openai::for_chat::STOP_NOTIFY;

/// Handler for `/嵌套的前缀/microphone` GET
/// 关闭语音模式
pub async fn microphone() {
    STOP_NOTIFY.notify_one(); // 通知在等待的流，结束语音对话
    event!(Level::INFO, "deactivate audio mode ...");
}
