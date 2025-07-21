use chrono::Local;
use futures::StreamExt;
use openai_dive::v1::{
    api::Client,
    endpoints::chat::RoleTrackingStream,
    resources::chat::{
        ChatCompletionParameters,
        ChatMessage,
        ChatMessageContent,
        DeltaChatMessage,
    },
};
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};
use tracing::{event, Level};

/// info: 记录所有用户的信息
/// error: 定义的错误类型，用于错误传递
use crate::{
    info::{
        insert_message, // 将指定message插入到指定uuid的messages中
        //save_log, // 保存chat记录至指定路径下的uuid文件夹，以本次对话开始的时间戳为文件名，格式为json
        //update_token, // 计算指定message的token数，并更新指定uuid的问题或答案的总token数
        DataType, // 存储问答信息的数据
        get_messages_num, // 获取指定uuid的messages总数
    },
    api::handlers::chat::MainData,
    error::MyError,
};

/// stream接收答案，通过管道将接受的答案传输出去，并将完整答案记录到该uuid中
/// uuid: 当前对话的uuid
/// sender: 管道发送stream答案
/// client: 创建的openai客户端
/// parameters: 提问的参数
/// model: 模型名称
pub async fn use_stream(
    uuid: String,
    sender: Sender<Vec<u8>>,
    client: Client,
    parameters: ChatCompletionParameters,
    model: &str,
    show_thought: bool,
) -> Result<(), MyError> {
    let mut whole_answer = "".to_string(); // 存储完整答案
    let mut role: u8 = 0; // 1表示User，2表示System，3表示Assistant，4表示Developer
    let mut think = true; // 是否属于think思维链部分
    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
    let messages_num = get_messages_num(&uuid); // 流式输出传输答案时，答案还未插入到服务端记录中，因此这里获取总消息数不需要减1
    // 创建stream
    let stream = client.chat().create_stream(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.clone(), error: e})?;
    let mut tracked_stream = RoleTrackingStream::new(stream);
    // 遍历接受stream信息
    'outer: while let Some(response) = tracked_stream.next().await {
        let chat_response = response.map_err(|e| MyError::ApiError{uuid: uuid.clone(), error: e})?;
        for choice in chat_response.choices { // 这里要用`for`，不能用`for_each()`，`choice`不是async报错
            match &choice.delta {
                DeltaChatMessage::User{content: ChatMessageContent::Text(c), ..} => {
                    whole_answer += &c;
                    role = 1;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                },
                DeltaChatMessage::System{content: ChatMessageContent::Text(c), ..} => {
                    whole_answer += &c;
                    role = 2;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                },
                /*
                DeltaChatMessage::Assistant{content: Some(ChatMessageContent::Text(c)), ..} => {
                    whole_answer += &c;
                    role = 3;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                },
                */
                DeltaChatMessage::Assistant{content: tmp_content, reasoning_content: tmp_reasoning_content, ..} => {
                    let c = match (tmp_content, tmp_reasoning_content) {
                        (Some(ChatMessageContent::Text(c)), None) => c, // 答案
                        (None, Some(c)) => c, // 思考过程
                        _ => continue,
                    };
                    whole_answer += &c;
                    if !show_thought { // 不显示思考过程则不传递，仅记录在whole_answer中
                        continue
                    }
                    role = 3;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                },
                DeltaChatMessage::Developer{content: ChatMessageContent::Text(c), ..} => {
                    whole_answer += &c;
                    role = 4;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                },
                /*
                DeltaChatMessage::Untagged{content: Some(ChatMessageContent::Text(c)), ..} => { // llama.cpp的llama-server的api传输的答案会在这里
                    whole_answer += &c;
                    role = 3;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    //sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                    if think {
                        if whole_answer == "<think>" {
                            whole_answer += "thinking ...<br>"; // 开始think时在最开始显示“thinking ...”
                            //if let Err(e) = sender.send("data: thinking ...<br>\n\n".as_bytes().to_vec()).await { // 传递数据以`data: `起始，以`\n\n`终止
                            if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, "thinking ...<br>".to_string(), true, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                                event!(Level::ERROR, "channel send error: {:?}", e);
                                break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                            }
                        } else if c == "</think>" {
                            whole_answer += "<br>"; // think结束后换行
                            //if let Err(e) = sender.send("data: <br>\n\n".as_bytes().to_vec()).await { // 传递数据以`data: `起始，以`\n\n`终止
                            if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, "<br>".to_string(), true, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                                event!(Level::ERROR, "channel send error: {:?}", e);
                                break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                            }
                            think = false;
                        }
                    }
                },
                */
                DeltaChatMessage::Untagged{content: tmp_content, reasoning_content: tmp_reasoning_content, ..} => { // llama.cpp的llama-server的api传输的答案会在这里
                    let c = match (tmp_content, tmp_reasoning_content) {
                        (Some(ChatMessageContent::Text(c)), None) => c, // 答案
                        (None, Some(c)) => c, // 思考过程
                        _ => continue,
                    };
                    whole_answer += &c;
                    if !show_thought { // 不显示思考过程则不传递，仅记录在whole_answer中
                        continue
                    }
                    role = 3;
                    //print!("{}", c);
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    //if let Err(e) = sender.send(format!("data: {}\n\n", c.replace("\n", "srxtzn")).into_bytes()).await { // 传递数据以`data: `起始，以`\n\n`终止
                    if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, c.replace("\n", "srxtzn"), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                        //println!("channel send error: {:?}", e);
                        event!(Level::ERROR, "channel send error: {:?}", e);
                        break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                    }
                    //sleep(Duration::from_millis(10)).await; // 这里设置间隔10ms，否则输出太快，客户端一大段一大段的输出，不流畅。不是获取stream的问题，确实是每个字符流式输出，只是太快了
                    if think {
                        if whole_answer == "<think>" {
                            whole_answer += "thinking ...<br>"; // 开始think时在最开始显示“thinking ...”
                            //if let Err(e) = sender.send("data: thinking ...<br>\n\n".as_bytes().to_vec()).await { // 传递数据以`data: `起始，以`\n\n`终止
                            if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, "thinking ...<br>".to_string(), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                                event!(Level::ERROR, "channel send error: {:?}", e);
                                break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                            }
                        } else if c == "</think>" {
                            whole_answer += "<br>"; // think结束后换行
                            //if let Err(e) = sender.send("data: <br>\n\n".as_bytes().to_vec()).await { // 传递数据以`data: `起始，以`\n\n`终止
                            if let Err(e) = sender.send(MainData::prepare_sse(&uuid, messages_num, "<br>".to_string(), true, false, false, false, false, None, Some(0))?).await { // 传递数据以`data: `起始，以`\n\n`终止
                                event!(Level::ERROR, "channel send error: {:?}", e);
                                break 'outer; // 可能客户端停止接收答案，这里也要停止，否则服务端依然接收答案，计费没停止
                            }
                            think = false;
                        }
                    }
                },
                _ => (),
            }
        }
    }
    // 记录答案
    let message = match role {
        1 => ChatMessage::User{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
        2 => ChatMessage::System{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
        3 => ChatMessage::Assistant{
            content: Some(ChatMessageContent::Text(whole_answer.clone())),
            reasoning_content: None,
            refusal: None,
            name: None,
            audio: None,
            tool_calls: None,
        },
        4 => ChatMessage::Developer{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
        _ => ChatMessage::User{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
    };
    // 将回答加到问答记录中
    insert_message(&uuid, message, tmp_time, false, DataType::Normal, None, model, None);
    // 计算当前回答的token数，并更新当前uuid的回答的总token数
    //update_token(&uuid, &whole_answer, false);
    // 保存chat记录，每次回答完成后都保存一次
    //save_log(&uuid);
    Ok(())
}

/// 非stream，接收openai的完整答案，并将完整答案记录到该uuid中，最后返回完整答案
/// uuid: 当前对话的uuid
/// client: 创建的openai客户端
/// parameters: 提问的参数
/// model: 模型名称
pub async fn not_use_stream(
    uuid: String,
    client: Client,
    parameters: ChatCompletionParameters,
    model: &str,
    show_thought: bool,
) -> Result<String, MyError> {
    let result = client.chat().create(parameters).await.map_err(|e| MyError::ApiError{uuid: uuid.clone(), error: e})?;
    let mut whole_answer = "".to_string(); // 存储完整答案
    let mut role: u8 = 0; // 1表示User，2表示System，3表示Assistant，4表示Developer
    let tmp_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string(); // 回答的当前时间，例如：2024-10-21 16:35:47
    // 获取答案
    match &result.choices[0].message {
        ChatMessage::System{content, ..} => {
            role = 2;
            match content {
                ChatMessageContent::Text(res) => {
                    whole_answer = res.to_string();
                    //println!("{}", res);
                },
                ChatMessageContent::ContentPart(res_vec) => event!(Level::INFO, "System ChatMessageContent::ContentPart {:?}", res_vec),
                ChatMessageContent::None => event!(Level::INFO, "System ChatMessageContent::None"),
            }
        },
        ChatMessage::User{content, ..} => {
            role = 1;
            match content {
                ChatMessageContent::Text(res) => {
                    whole_answer = res.to_string();
                    //println!("{}", res);
                },
                ChatMessageContent::ContentPart(res_vec) => event!(Level::INFO, "System ChatMessageContent::ContentPart {:?}", res_vec),
                ChatMessageContent::None => event!(Level::INFO, "System ChatMessageContent::None"),
            }
        },
        /*
        ChatMessage::Assistant{content, ..} => {
            role = 3;
            match content {
                Some(c) => {
                    match c {
                        ChatMessageContent::Text(res) => {
                            whole_answer = res.to_string();
                            //println!("{}", res);
                        },
                        ChatMessageContent::ContentPart(res_vec) => event!(Level::INFO, "System ChatMessageContent::ContentPart {:?}", res_vec),
                        ChatMessageContent::None => event!(Level::INFO, "System ChatMessageContent::None"),
                    }
                },
                None => event!(Level::INFO, "Assistant: None"), // println!("Assistant: None"),
            }
        },
        */
        ChatMessage::Assistant{content: tmp_content, reasoning_content: tmp_reasoning_content, ..} => {
            role = 3;
            if show_thought {
                if let Some(c) = tmp_reasoning_content {
                    whole_answer = c.to_string();
                }
            }
            match tmp_content {
                Some(c) => {
                    match c {
                        ChatMessageContent::Text(res) => {
                            whole_answer += res;
                            //println!("{}", res);
                        },
                        ChatMessageContent::ContentPart(res_vec) => event!(Level::INFO, "System ChatMessageContent::ContentPart {:?}", res_vec),
                        ChatMessageContent::None => event!(Level::INFO, "System ChatMessageContent::None"),
                    }
                },
                None => event!(Level::INFO, "Assistant: None"), // println!("Assistant: None"),
            }
        },
        ChatMessage::Developer{content, ..} => {
            role = 4;
            match content {
                ChatMessageContent::Text(res) => {
                    whole_answer = res.to_string();
                    //println!("{}", res);
                },
                ChatMessageContent::ContentPart(res_vec) => event!(Level::INFO, "System ChatMessageContent::ContentPart {:?}", res_vec),
                ChatMessageContent::None => event!(Level::INFO, "System ChatMessageContent::None"),
            }
        },
        ChatMessage::Tool{content, ..} => event!(Level::INFO, "{}", content), // println!("{}", content),
    }
    // 记录答案
    let message = match role {
        1 => ChatMessage::User{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
        2 => ChatMessage::System{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
        3 => ChatMessage::Assistant{
            content: Some(ChatMessageContent::Text(whole_answer.clone())),
            reasoning_content: None,
            refusal: None,
            name: None,
            audio: None,
            tool_calls: None,
        },
        4 => ChatMessage::Developer{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
        _ => ChatMessage::User{
            content: ChatMessageContent::Text(whole_answer.clone()),
            name: None,
        },
    };
    // 将回答加到问答记录中
    insert_message(&uuid, message, tmp_time, false, DataType::Normal, None, model, None);
    // 计算当前回答的token数，并更新当前uuid的回答的总token数
    //update_token(&uuid, &whole_answer, false);
    // 保存chat记录，每次回答完成后都保存一次
    //save_log(&uuid);
    Ok(whole_answer)
}
