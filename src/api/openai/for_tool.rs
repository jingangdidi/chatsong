use openai_dive::v1::{
    api::Client,
    resources::chat::{
        ChatCompletionParameters,
        ChatMessage,
        ChatMessageContent,
    },
};
use tracing::{event, Level};

use crate::{
    error::MyError,
    info::update_token,
};

/// function calling result
#[derive(Clone)]
pub enum CallToolResult {
    CallTool((ChatMessage, Vec<(String, String, String, Option<String>)>)), // (ChatMessage, Vec<(tool name, tool args, call tool id, content)>)
    Text(String), // normal text result, not call tool
}

/// 非stream，接收openai的完整答案，并将完整答案记录到该uuid中，最后返回完整答案
/// uuid: 当前对话的uuid
/// client: 创建的openai客户端
/// parameters: 提问的参数
/// model: 模型名称
pub async fn call_tool_not_use_stream(
    uuid: &str,
    client: Client,
    parameters: ChatCompletionParameters,
    //show_thought: bool,
) -> Result<CallToolResult, MyError> {
    //let result = client.chat().create(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.clone(), error: e})?;
    match client.chat().create(parameters).await { // 这里遇到错误不能直接返回，否则服务端与前端id差一个，后面代码insert_message没有执行，下个问题会显示在这个未回答完的答案末尾
        Ok(result) => {
            if let Some(usage) = result.usage {
                update_token(&uuid, (usage.prompt_tokens.unwrap_or(0), usage.completion_tokens.unwrap_or(0), usage.total_tokens));
            }
            if let ChatMessage::Assistant{content: tmp_content, reasoning_content: _tmp_reasoning_content, tool_calls: tmp_tool_calls, ..} = &result.choices[0].message {
                if let Some(tool_calls_vec) = tmp_tool_calls {
                    // https://docs.rs/openai_dive/1.1.0/openai_dive/v1/resources/chat/struct.ToolCall.html
                    // +-----------------------------+     https://docs.rs/openai_dive/1.1.0/openai_dive/v1/resources/chat/struct.Function.html
                    // | pub struct ToolCall {       |     +----------------------------+
                    // |     pub id:       String,   |     | pub struct Function {      |
                    // |     pub type:     String,   |     |     pub name: String,      |
                    // |     pub function: Function, | --> |     pub arguments: String, |
                    // | }                           |     | }                          |
                    // +-----------------------------+     +----------------------------+
                    let mut tool_call_result: Vec<(String, String, String, Option<String>)> = Vec::new();
                    for tool_call in tool_calls_vec {
                        tool_call_result.push((
                            tool_call.function.name.clone(),
                            tool_call.function.arguments.clone(),
                            tool_call.id.clone(),
                            get_assistant_content(tmp_content),
                        ));
                    }
                    Ok(CallToolResult::CallTool((result.choices[0].message.clone(), tool_call_result)))
                } else {
                    Ok(CallToolResult::Text(get_assistant_content(tmp_content).unwrap_or("".to_string())))
                }
            } else {
                unreachable!();
            }
        },
        Err(e) => {
            event!(Level::ERROR, "{} call tool error: {:?}", uuid, e);
            Err(MyError::OtherError{info: format!("call tool error: {}", e)})
        },
    }
}

/// get assistant content
fn get_assistant_content(content: &Option<ChatMessageContent>) -> Option<String> {
    match content {
        Some(c) => {
            match c {
                ChatMessageContent::Text(res) => Some(res.to_string()),
                ChatMessageContent::ContentPart(res_vec) => {
                    event!(Level::INFO, "System ChatMessageContent::ContentPart {:?}", res_vec);
                    None
                },
                ChatMessageContent::None => {
                    event!(Level::INFO, "System ChatMessageContent::None");
                    None
                },
            }
        },
        None => {
            //event!(Level::INFO, "Assistant: None"); // println!("Assistant: None");
            None
        },
    }
}
