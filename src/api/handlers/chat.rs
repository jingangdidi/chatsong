use std::collections::HashMap;
use std::fs::write;

use axum::{
    body::Body,
    extract::{Query, OriginalUri},
    response::{Response, IntoResponse},
};
use axum_extra::extract::cookie::CookieJar;
use chrono::Local;
use openai_dive::v1::{
    api::Client,
    resources::{
        chat::{
            ChatCompletionParametersBuilder,
            ChatCompletionResponseFormat,
            ChatMessage,
            ChatMessageContent,
            /*
            ChatMessageImageContentPart,
            ImageUrlType,
            */
        },
        shared::ReasoningEffort,
    },
};
use serde::Serialize;
use tokio::sync::mpsc::channel;
use tracing::{event, Level};
use uuid::Uuid;

/// info: 记录所有用户的信息
/// web: 联网搜索、解析html文件
/// error: 定义的错误类型，用于错误传递
use crate::{
    info::{
        insert_message, // 将指定message插入到指定uuid的messages中
        get_messages, // 获取指定uuid最近的指定数量个message
        contain_uuid, // 检查服务端是否有指定uuid的数据
        create_cookie, // 根据指定uuid创建cookie
        update_cookie_max_age, // 更新指定CookieJar的max-age
        get_token, // 获取指定uuid问题和答案的总token数
        get_prompt, // 获取指定prompt序号对应的prompt字符串，如果不存在则返回空字符串
        create_uuid_dir, // uuid文件夹不存在则创建
        get_log_for_display, // 获取指定uuid最新问答记录，提取字符串，用于在chat页面显示
        get_prompt_name, // 获取当前uuid的prompt名称
        get_query_num, // 获取指定uuid客户端提交的问题数量，用于服务端命令行显示
        pop_message_before_end, // 在跳转到其他uuid之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
        DataType, // 存储问答信息的数据
        get_latest_query, // 获取最后一个message，且必须是用户发送的query字符串
        get_latest_image, // 获取最后一个上传的图片
        get_latest_voice, // 获取最后一个上传的音频文件
        VOICE, // 生成音频时传输给用户端的图标base64
        get_messages_num, // 获取指定uuid的messages总数
        get_msg_token, // 获取当前uuid指定位置message的token数
    },
    graph::{
        add_edge, // 将旧uuid与新uuid建立直接或间接关系
        get_all_related_uuid, // 获取与指定uuid相关的所有uuid
    },
    parse_paras::PARAS,
    web::search::get_search_parse_result, // 解析客户端输入的内容，使用网络搜索、解析url、解析html文件，返回界限结果和报错字符串
    openai::{
        for_chat::{
            use_stream, // stream接收答案
            not_use_stream, // 非stream，接收openai的完整答案
        },
        for_image::{
            create_image, // 调用ChatGPT的dall-e-2或dall-e-3进行绘图
            create_edit_image, // 调用ChatGPT的gpt-image-1进行绘图或改图
        },
        for_speech::create_speech, // 调用openai的api生成speech
        for_transcription::create_transcription, // 调用openai的api从音频提取文本
        for_translation::create_translation, // 调用openai的api将音频翻译为指定语言的文本
    },
    error::MyError,
};

/// 页面左侧显示的信息
#[derive(Serialize)]
struct MetaData {
    current_uuid:  String,                // 当前uuid
    related_uuid:  Vec<(String, String)>, // 相关uuid，Vec<(相关的uuid, uuid对应的prompt---对话名称)>，如果创建该对话时没有指定对话名称，则第2项仅为uuid对应的prompt
    prompt:        String,                // 当前prompt
    in_token:      usize,                 // 输入token总数
    out_token:     usize,                 // 输出token总数
    current_token: usize,                 // 当前问题或答案的token数，流式输出时该值>0，传递最终token数，问题或非流式输出的答案这里为0
}

impl MetaData {
    /// 将Metadata转为SSE格式Vec<u8>
    /// current_token为None表示获取最后一个message的token，为Some则直接使用Some内的数值
    fn prepare_sse(uuid: String, current_token: Option<usize>) -> Result<Vec<u8>, MyError> {
        // 获取与当前uuid相关的所有uuid
        let related_uuid_prompt = get_all_related_uuid(&uuid); // Vec<(相关的uuid, uuid对应的prompt---对话名称)>，如果创建该对话时没有指定对话名称，则第2项仅为uuid对应的prompt
        // 获取当前uuid的prompt名称
        let prompt_name = get_prompt_name(&uuid);
        // 获取当前uuid的问题和答案的总token数
        let token = get_token(&uuid);
        // MetaData
        let data = MetaData{
            current_uuid:  uuid.clone(),              // 当前uuid
            related_uuid:  related_uuid_prompt,       // 相关uuid
            prompt:        prompt_name,               // 当前prompt
            in_token:      token[0],                  // 输入token总数
            out_token:     token[1],                  // 输出token总数
            current_token: match current_token { // 当前问题或答案的token数
                Some(t) => t, // 指定了token
                None => get_msg_token(&uuid, -1), // 未指定则获取最后一个message的token数，调用该方法前，当前message已经插入，因此获取最后一个message的token就是当前插入message的token
            },
        };
        format_sse_message(&uuid, "metadata", &data)
    }
}

/// 传输的消息内容
#[derive(Serialize)]
pub struct MainData {
    id:            usize,          // 该消息在当前对话中的索引，第1条消息是0，如果是问题或非流式输出的答案，调用MetaData前已经插入了，获取总message后要减1，如果是流式输出的答案，调用MetaData前还未插入，获取总message后不需要减1
    content:       String,         // 消息内容
    is_left:       bool,           // true是回答，false是问题
    is_img:        bool,           // true是图片base64，false是常规文本内容
    is_voice:      bool,           // true是语音图片base64，false是常规文本内容
    is_history:    bool,           // true是之前的问答记录，页面需要清空后再加载
    time_model:    Option<String>, // 时间（如果是回答还包含调用的模型名称），Some在json中直接是字符串内容，None在json中是null
    current_token: usize,          // 当前问题或答案的token数，如果使用stream则直接设为0，最终的token数通过MetaData传递
}

impl MainData {
    /// 将MainData转为SSE格式Vec<u8>
    /// current_token为None表示获取最后一个message的token，为Some则直接使用Some内的数值
    pub fn prepare_sse(uuid: &str, id: usize, content: String, is_left: bool, is_img: bool, is_voice: bool, is_history: bool, time_model: Option<String>, current_token: Option<usize>) -> Result<Vec<u8>, MyError> {
        let data = MainData{
            id,
            content,
            is_left,
            is_img,
            is_voice,
            is_history,
            time_model,
            current_token: match current_token { // 当前问题或答案的token数
                Some(t) => t, // 指定了token
                None => get_msg_token(uuid, -1), // 未指定则获取最后一个message的token数，调用该方法前，当前message已经插入，因此获取最后一个message的token就是当前插入message的token
            },
        };
        format_sse_message(uuid, "maindata", &data)
    }
}

/// 将Metadata或MainData转为SSE格式Vec<u8>
fn format_sse_message<T: Serialize>(uuid: &str, event_name: &str, data: &T) -> Result<Vec<u8>, MyError> {
    let json_data = serde_json::to_string(data).map_err(|e| MyError::ToJsonStirngError{uuid: uuid.to_string(), error: e})?;
    Ok(format!("event: {}\ndata: {}\n\n", event_name, json_data).into_bytes())
}

/// Handler for `/嵌套的前缀/chat` GET
/// 解析url中的参数，存储到HashMap
/// 例如访问：http://127.0.0.1:8080/chat?cx=912b8adxxxx8e41a9&q=how+to+use+cubecl&num=10&key=AIzaSyAOi2Dxxxxrv0cZKcl0RX8WLs70-vQwiBM
/// 解析得到：{"cx": "912b8adxxxx8e41a9", "q": "how to use cubecl", "num": "10", "key": "AIzaSyAOi2Dxxxxrv0cZKcl0RX8WLs70-vQwiBM"}
/// stream格式：https://www.ruanyifeng.com/blog/2017/05/server-sent_events.html
pub async fn chat(Query(params): Query<HashMap<String, String>>, uri: OriginalUri, jar: CookieJar, body: String) -> Result<(CookieJar, impl IntoResponse), MyError> {
    // 解析传递的prompt，prompt为`-1`表示不开启新会话，0表示开启新会话但无prompt，`>0`表示使用指定prompt开启新会话
    let (prompt, chat_name): (Option<usize>, Option<String>) = match params.get("prompt") {
        Some(p) => match p.as_str() {
            "-1" => (None, None), // 不开启新会话
            pp => (
                Some(pp.parse::<usize>().map_err(|e| MyError::ParseStringError{from: pp.to_string(), to: "usize".to_string(), error: e})?),
                params.get("chatname").cloned(),
            ),
        },
        None => (None, None), // 不开启新会话
    };
    // 解析要调用的模型
    let (api_key, endpoint, model, cof) = match params.get("model") {
        Some(m) => PARAS.api.get_model_by_str(&m)?,
        None => PARAS.api.get_default_model()?,
    };
    // 获取cookie值
    let cookie_uuid = match jar.get("srx-tzn") { // 获取cookie
        Some(c) => c.value().to_string(),
        None => "".to_string(),
    };
    // 提问时最多提交几对问答，或几个消息，以及是否包含prompt
    // 返回(问答对数量, 消息数量, 是否包含prompt)
    let qa_msg_p: Option<(usize, usize, bool)> = match params.get("num") {
        Some(n) => match n.as_str() {
            "unlimit" => Some((usize::MAX, usize::MAX, true)), // Some((无限制, 无限制, 包含prompt))
            p_num_qa_msg => { // 格式：`数量qa`（指定数量个问答对，不包含prompt）、`p数量qa`（指定数量个问答对，包含prompt）、`数量`（指定数量个消息，不包含prompt）、`p数量`（指定数量个消息，包含prompt）
                if p_num_qa_msg.ends_with("qa") { // 问答对
                    let p_num = p_num_qa_msg.strip_suffix("qa").unwrap(); // 这里可以直接unwrap
                    if p_num.starts_with("p") { // `p数量qa`（指定数量个问答对，包含prompt），例如：`p1qa`
                        let num = p_num.strip_prefix("p").unwrap().parse::<usize>().map_err(|e| MyError::ParseStringError{from: p_num.strip_prefix("p").unwrap().to_string(), to: "usize".to_string(), error: e})?;
                        Some((num, 0, true)) // Some((指定问答对数量, 0, 包含prompt))
                    } else { // `数量qa`（指定数量个问答对，不包含prompt），例如：`1qa`
                        let num = p_num.parse::<usize>().map_err(|e| MyError::ParseStringError{from: p_num.to_string(), to: "usize".to_string(), error: e})?;
                        Some((num, 0, false)) // Some((指定问答对数量, 0, 不包含prompt))
                    }
                } else { // 视为指定的消息数
                    if p_num_qa_msg.starts_with("p") { // `p数量`（指定数量个消息，包含prompt），例如：`p1`
                        let num = p_num_qa_msg.strip_prefix("p").unwrap().parse::<usize>().map_err(|e| MyError::ParseStringError{from: p_num_qa_msg.strip_prefix("p").unwrap().to_string(), to: "usize".to_string(), error: e})?;
                        Some((0, num, true)) // Some((0, 指定消息数量, 包含prompt))
                    } else { // `数量`（指定数量个消息，不包含prompt），例如：`1`
                        let num = p_num_qa_msg.parse::<usize>().map_err(|e| MyError::ParseStringError{from: p_num_qa_msg.to_string(), to: "usize".to_string(), error: e})?;
                        Some((0, num, false)) // Some((0, 指定消息数量, 不包含prompt))
                    }
                }
            },
        },
        None => None, // 这里None表示不修改问答对或消息数、以及是否包含prompt
    };
    // 解析传递的uuid，并设置cookie
    let (uuid, cookie_jar, load_uuid/*, clear_page*/) = match prompt {
        Some(p) => {
            let prompt_name_str = get_prompt(p); // 获取内置的prompt名称和内容
            let tmp_uuid = Uuid::new_v4().to_string();
            if !prompt_name_str[1].is_empty() { // 获取到prompt则写入该uuid的message中
                // 保存`prompt.txt`，记录该prompt的序号
                create_uuid_dir(&tmp_uuid)?;
                let prompt_file = format!("{}/{}/prompt.txt", PARAS.outpath, &tmp_uuid);
                write(&prompt_file, p.to_string()).map_err(|e| MyError::WriteFileError{file: prompt_file, error: e})?;
                // prompt插入到messages中
                let message = ChatMessage::User{
                    content: ChatMessageContent::Text(prompt_name_str[1].clone()),
                    name: None,
                };
                insert_message(&tmp_uuid, message, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Normal, qa_msg_p, &model, chat_name.clone());
            }
            // 添加新的直接关系
            add_edge(&cookie_uuid, &tmp_uuid, true);
            // 在跳转到其他uuid之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
            pop_message_before_end(&cookie_uuid);
            (
                tmp_uuid.clone(),
                jar.add(create_cookie(tmp_uuid)),
                true,
                //false,
            )
        },
        None => match params.get("uuid") { // 不开启新会话，则解析uuid参数
            Some(u) => {
                if u.is_empty() {
                    if cookie_uuid.is_empty() { // 没有cookie，则重新生成一个uuid作为cookie
                        let tmp_uuid = Uuid::new_v4().to_string();
                        (
                            tmp_uuid.clone(),
                            jar.add(create_cookie(tmp_uuid)),
                            false,
                            //true,
                        )
                    } else { // 有cookie则cookie作为uuid，不重新生成uuid
                        (
                            cookie_uuid,
                            update_cookie_max_age(jar), // 仅修改内部cookie的max-age
                            false,
                            //false,
                        )
                    }
                } else {
                    if contain_uuid(u) { // 发送的请求中指定了uuid，且存在于服务端，则不重新生成uuid，并用该uuid设置cookie
                        if &cookie_uuid == u { // 如果指定的uuid与cookie值相同，则不需要修改
                            (
                                cookie_uuid,
                                update_cookie_max_age(jar), // 仅修改内部cookie的max-age
                                false,
                                //false,
                            )
                        } else { // 如果指定的uuid与cookie值不相同，则用指定的uuid设置为cookie
                            // 添加新的间接关系
                            add_edge(&cookie_uuid, &u, false);
                            // 在跳转到其他uuid之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
                            pop_message_before_end(&cookie_uuid);
                            (
                                u.to_string(),
                                jar.add(create_cookie(u.clone())), // 已有cookie则更新，不存在则添加
                                true,
                                //false,
                            )
                        }
                    } else { // 发送的请求中指定了uuid，但不存在于服务端，则舍弃指定的uuid，重新生成uuid，并用该uuid设置cookie
                        let tmp_uuid = Uuid::new_v4().to_string();
                        // 添加新的直接关系
                        add_edge(&cookie_uuid, &tmp_uuid, true);
                        // 在跳转到其他uuid之前，先去除当前uuid的messages末尾连续的问题，这些问题没有实际调用OpenAI api
                        pop_message_before_end(&cookie_uuid);
                        (
                            tmp_uuid.clone(),
                            jar.add(create_cookie(tmp_uuid)),
                            false,
                            //true,
                        )
                    }
                }
            },
            None => { // 发送的请求中没有指定uuid
                if cookie_uuid.is_empty() { // 没有cookie，则重新生成一个uuid作为cookie
                    let tmp_uuid = Uuid::new_v4().to_string();
                    (
                        tmp_uuid.clone(),
                        jar.add(create_cookie(tmp_uuid)),
                        false,
                        //true,
                    )
                } else { // 有cookie则cookie作为uuid，不重新生成uuid
                    ( // 有cookie则cookie作为uuid，不重新生成uuid
                        cookie_uuid,
                        update_cookie_max_age(jar), // 仅修改内部cookie的max-age
                        false,
                        //false,
                    )
                }
            },
        },
    };
    // uuid文件夹不存在则创建
    create_uuid_dir(&uuid)?;
    // 获取temperature
    let tmp_temperature = match params.get("temp") {
        Some(t) => match t.parse::<f32>() {
            Ok(n) => {
                if n >= 0.0 && n <= 2.0 {
                    Some(n)
                } else {
                    None // 参数不在[0, 2]范围内则设为None，表示不设置temperature
                }
            },
            Err(_) => None, // None表示不设置temperature
        },
        None => None, // None表示不设置temperature
    };
    // 是否流式输出
    let stream = match params.get("stream") {
        Some(s) => {
            if s == "true" {
                true
            } else {
                false
            }
        },
        None => false,
    };
    // 是否网络搜索
    let web_search = match params.get("web") {
        Some(w) => {
            if w == "true" {
                true
            } else {
                false
            }
        },
        None => false,
    };
    // 选择生成音频的声音
    let tmp_voice = match params.get("voice") {
        Some(v) => match v.parse::<usize>() {
            Ok(n) => n,
            Err(_) => 1, // 参数不对则使用默认Alloy
        },
        None => 1, // 默认Alloy
    };
    // 思维链模型的effort以及是否显示思考过程，仅对思维链模型有效
    let (effort, show_thought) = match params.get("effort") {
        Some(e) => {
            match e.as_str() {
                "1" => (ReasoningEffort::Low, true), // 思考的少
                "2" => (ReasoningEffort::Low, false), // 思考的少
                "3" => (ReasoningEffort::Medium, true), // 思考适中
                "4" => (ReasoningEffort::Medium, false), // 思考适中
                "5" => (ReasoningEffort::High, true), // 思考更多
                "6" => (ReasoningEffort::High, false), // 思考更多
                _ => (ReasoningEffort::Low, true),
            }
        },
        None => (ReasoningEffort::Low, true),
    };
    // 记录提问内容或提交请求
    if let Some(q) = params.get("q") {
        if q == "0" { // 0表示body是空，1表示body是问题，空内容时发送提问，参考：openai-client-0.6.4/examples/chat/create_chat_completion_stream
            event!(Level::INFO, "{} POST {}, query: {}, waiting for anwser ...", uuid, uri.path(), get_query_num(&uuid));
            if ["gpt-image-1", "dall-e-2", "dall-e-3", "whisper-1", "tts-1", "tts-1-hd"].iter().any(|x| x == &model) { // 非常规文本问题
                let (res, data_type, to_client, is_img, is_voice) = match model.as_ref() {
                    "gpt-image-1" => match get_latest_query(&uuid) {
                        Some(query) => {
                            match create_edit_image(&uuid, get_latest_image(&uuid), query, &endpoint, api_key.clone()).await {
                                Ok((image_name, base64)) => (image_name, DataType::Image(base64.clone()), base64, true, false), // 如果绘图成功，则回答的message存储生成的图片名，并记录图片的base64字符串
                                Err(e) => {
                                    event!(Level::ERROR, "{} gpt-image-1 image error: {}", uuid, e);
                                    let tmp = format!("gpt-image-1 image error: {}", e);
                                    (tmp.clone(), DataType::Normal, tmp, false, false)
                                },
                            }
                        },
                        None => { // 最后一项message必须是user，且是提出的绘图要求，如果不是则报错
                            let tmp = "gpt-image-1 need input prompt first".to_string();
                            (tmp.clone(), DataType::Normal, tmp, false, false)
                        },
                    },
                    "dall-e-2" | "dall-e-3" => match get_latest_query(&uuid) {
                        Some(query) => {
                            match create_image(&uuid, &query, model.clone(), &endpoint, api_key.clone()).await {
                                Ok((image_name, base64)) => (image_name, DataType::Image(base64.clone()), base64, true, false), // 如果绘图成功，则回答的message存储生成的图片名，并记录图片的base64字符串
                                Err(e) => {
                                    event!(Level::ERROR, "{} dall-e-2 or dall-e-3 image error: {}", uuid, e);
                                    let tmp = format!("{}<br>usage: quality:xxx size:xxx style:xxx format:xxx prompt:xxx", e);
                                    (tmp.clone(), DataType::Normal, tmp, false, false)
                                },
                            }
                        },
                        None => { // 最后一项message必须是user，且是提出的绘图要求，如果不是则报错
                            let tmp = "dall-e-2 or dall-e-3 need input prompt first".to_string();
                            (tmp.clone(), DataType::Normal, tmp, false, false)
                        },
                    },
                    "whisper-1" => match get_latest_query(&uuid) {
                        Some(query) => {
                            if query == "transc" { // 调用openai的api从音频提取文本
                                match create_transcription(&uuid, get_latest_voice(&uuid), &PARAS.outpath, &endpoint, api_key.clone()).await {
                                    Ok(res) => (res.clone(), DataType::Normal, res, false, false),
                                    Err(e) => {
                                        event!(Level::ERROR, "{} transcription error: {}", uuid, e);
                                        let tmp = format!("transcription error: {}", e);
                                        (tmp.clone(), DataType::Normal, tmp, false, false)
                                    },
                                }
                            } else if query == "transl" { // 调用openai的api将音频翻译为指定语言的文本
                                match create_translation(&uuid, get_latest_voice(&uuid), &PARAS.outpath, &endpoint, api_key.clone()).await {
                                    Ok(res) => (res.clone(), DataType::Normal, res, false, false),
                                    Err(e) => {
                                        event!(Level::ERROR, "{} translation error: {}", uuid, e);
                                        let tmp = format!("translation error: {}", e);
                                        (tmp.clone(), DataType::Normal, tmp, false, false)
                                    },
                                }
                            } else {
                                let tmp = format!("only support transc or transl, not {}", query);
                                (tmp.clone(), DataType::Normal, tmp, false, false)
                            }
                        },
                        None => { // 最后一项message必须是user，且是提出的绘图要求，如果不是则报错
                            let tmp = "whisper-1 need input transc or transl first".to_string();
                            (tmp.clone(), DataType::Normal, tmp, false, false)
                        },
                    },
                    "tts-1" | "tts-1-hd" => match get_latest_query(&uuid) {
                        Some(query) => {
                            match create_speech(&uuid, query, tmp_voice, &PARAS.outpath, &endpoint, api_key.clone()).await {
                                Ok(res) => (res, DataType::Voice, VOICE.to_string(), false, true), // 返回的res是生成的音频文件名，第3项是传输给用户的音频图像的base64
                                Err(e) => {
                                    event!(Level::ERROR, "{} tts-1 and tts-1-hd speech error: {}", uuid, e);
                                    let tmp = format!("tts-1 and tts-1-hd speech error: {}", e);
                                    (tmp.clone(), DataType::Normal, tmp, false, false)
                                },
                            }
                        },
                        None => { // 最后一项message必须是user，且是提出的绘图要求，如果不是则报错
                            let tmp = "tts-1 and tts-1-hd need input prompt first".to_string();
                            (tmp.clone(), DataType::Normal, tmp, false, false)
                        },
                    },
                    _ => unreachable!(),
                };
                let message = ChatMessage::Assistant{
                    content: Some(ChatMessageContent::Text(res)),
                    reasoning_content: None,
                    refusal: None,
                    name: None,
                    audio: None,
                    tool_calls: None,
                };
                insert_message(&uuid, message, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, data_type, None, &model, None);
                // 创建stream对象，接收管道传递的数据
                let tmp_uuid = uuid.clone();
                let tmp_stream = async_stream::stream! {
                    // 传输图片base64字符串、从音频文件提取的文本内容、从音频文件翻译的文本内容。此时消息已经插入了，因此总消息数减1作为id
                    let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, get_messages_num(&tmp_uuid) - 1, to_client, true, is_img, is_voice, false, None, None)?);
                    yield tmp;
                    // 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: showinfo{}srx{}srx{}srx{}srx{}\n\n", tmp_uuid, token[0], token[1], prompt_name, related_uuid_prompt.into_iter().map(|up| up.0+"*"+&up.1).collect::<Vec<_>>().join("#")).as_bytes().to_vec()); // 传递数据以`data: `起始，以`\n\n`终止
                    let tmp: Result<Vec<u8>, MyError> = Ok(MetaData::prepare_sse(tmp_uuid, Some(0))?);
                    yield tmp;
                    let tmp: Result<Vec<u8>, MyError> = Ok(b"event: close\ndata: {\"key\": \"close\"}\n\n".to_vec()); // 最后以`event: close\ndata: {"key": "close"}\n\n`结束stream，data需要是json格式，否则js的`JSON.parse`解析时报错
                    yield tmp;
                };
                // Convert the stream into a response
                match Response::builder()
                    //.header("Content-Type", "text/plain")
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Body::from_stream(tmp_stream))
                {
                    Ok(resp) => Ok((cookie_jar, resp)),
                    Err(e) => Err(MyError::ResponseError{uuid: uuid, error: e}),
                }
            } else {
                // 使用api key初始化
                let mut client = Client::new(api_key.clone());
                client.set_base_url(&endpoint); // 从0.7.0开始舍弃了new_with_base
                // 准备参数
                /*
                let parameters = ChatCompletionParametersBuilder::default()
                    .model(model.clone()) // 指定模型，例如：Gpt4Engine::Gpt4O.to_string()
                    .messages(get_messages(&uuid, true))
                    .reasoning_effort(ReasoningEffort::Low) // 设置使用思维链，Low, Medium, High
                    .response_format(ChatCompletionResponseFormat::Text)
                    //.stream(stream) // 这里不需要设置，调用`create_stream`时会设置
                    .build().map_err(|e| MyError::ChatCompletionError{error: e})?;
                */
                let mut para_builder = ChatCompletionParametersBuilder::default();
                para_builder.model(model.clone()); // 指定模型，例如：Gpt4Engine::Gpt4O.to_string()
                para_builder.messages(get_messages(&uuid, true));
                para_builder.response_format(ChatCompletionResponseFormat::Text);
                //para_builder.stream(stream); // 这里不需要设置，调用`create_stream`时会设置
                if cof { // 对思维链模型设置effort
                    para_builder.reasoning_effort(effort.clone()); // 设置使用思维链，Low（思考的少，简单问答）, Medium（思考适中，多步骤推理）, High（思考更多，复杂逻辑推导）
                }
                if let Some(temp) = tmp_temperature {
                    para_builder.temperature(temp);
                }
                let parameters = para_builder.build().map_err(|e| MyError::ChatCompletionError{error: e})?;
                // 提问
                if stream {
                    let tmp_uuid = uuid.clone();
                    let (sender, mut receiver) = channel(100); // 设置管道缓存大小，管道中缓存满了，则send将会阻塞
                    // 从openai接收stream答案，并返回完整答案字符串
                    tokio::spawn(async move {
                        if let Err(e) = use_stream(tmp_uuid.clone(), sender, client, parameters, &model, show_thought).await {
                            event!(Level::ERROR, "{} receive stream error: {}", tmp_uuid, e);
                        }
                    });
                    // 创建stream对象，接收管道传递的数据
                    let tmp_uuid = uuid.clone();
                    let tmp_stream = async_stream::stream! {
                        // 传输指定uuid的chat记录
                        if load_uuid {
                            // Vec<(是否是提问, 问题或答案字符串, 作为html中tag的id的序号, 时间)>
                            for (i, log) in get_log_for_display(&tmp_uuid, false).2.into_iter().enumerate() {
                                if log.is_query { // 显示在问答页面右侧的用户输入内容
                                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: rightsrx{}timesrx{}\n\n", log.3, log.1).as_bytes().to_vec()); // 这里要声明类型，否则报错
                                    let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, i, log.content, false, log.is_img, log.is_voice, true, Some(log.time), Some(log.token))?);
                                    yield tmp;
                                } else { // 显示在问答页面左侧的回答内容
                                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: leftsrx{}timesrx{}\n\n", log.3, log.1).as_bytes().to_vec()); // 这里要声明类型，否则报错
                                    let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, i, log.content, true, log.is_img, log.is_voice, true, Some(log.time), Some(log.token))?);
                                    yield tmp;
                                }
                            }
                        }
                        // 传输答案
                        while let Some(m) = receiver.recv().await {
                            let tmp: Result<Vec<u8>, MyError> = Ok(m); // 这里要声明类型，否则报错
                            yield tmp;
                        }
                        // 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                        //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: showinfo{}srx{}srx{}srx{}srx{}\n\n", tmp_uuid, token[0], token[1], prompt_name, related_uuid_prompt.into_iter().map(|up| up.0+"*"+&up.1).collect::<Vec<_>>().join("#")).as_bytes().to_vec()); // 传递数据以`data: `起始，以`\n\n`终止
                        let tmp: Result<Vec<u8>, MyError> = Ok(MetaData::prepare_sse(tmp_uuid, None)?);
                        yield tmp;
                        // 结束stream
                        let tmp: Result<Vec<u8>, MyError> = Ok(b"event: close\ndata: {\"key\": \"close\"}\n\n".to_vec()); // 最后以`event: close\ndata: {"key": "close"}\n\n`结束stream，data需要是json格式，否则js的`JSON.parse`解析时报错
                        yield tmp;
                    };
                    // Convert the stream into a response
                    match Response::builder()
                        //.header("Content-Type", "text/plain")
                        .header("Content-Type", "text/event-stream")
                        .header("Cache-Control", "no-cache")
                        .header("Connection", "keep-alive")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Body::from_stream(tmp_stream))
                    {
                        Ok(resp) => Ok((cookie_jar, resp)),
                        Err(e) => Err(MyError::ResponseError{uuid: uuid, error: e}),
                    }
                } else {
                    // 从openai接收完整答案字符串
                    let whole_answer = not_use_stream(uuid.clone(), client, parameters, &model, show_thought).await?;
                    // 创建stream对象，接收管道传递的数据
                    let tmp_uuid = uuid.clone();
                    let tmp_stream = async_stream::stream! {
                        // 传输指定uuid的chat记录
                        if load_uuid {
                            // Vec<(是否是提问, 问题或答案字符串, 作为html中tag的id的序号, 时间)>
                            for (i, log) in get_log_for_display(&tmp_uuid, false).2.into_iter().enumerate() {
                                if log.is_query { // 显示在问答页面右侧的用户输入内容
                                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: rightsrx{}timesrx{}\n\n", log.3, log.1).as_bytes().to_vec()); // 这里要声明类型，否则报错
                                    let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, i, log.content, false, log.is_img, log.is_voice, true, Some(log.time), Some(log.token))?);
                                    yield tmp;
                                } else { // 显示在问答页面左侧的回答内容
                                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: leftsrx{}timesrx{}\n\n", log.3, log.1).as_bytes().to_vec()); // 这里要声明类型，否则报错
                                    let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, i, log.content, true, log.is_img, log.is_voice, true, Some(log.time), Some(log.token))?);
                                    yield tmp;
                                }
                            }
                        }
                        // 传输答案。非流式输出传输答案时，答案已经插入到服务端记录中，因此这里获取总消息数还需要减1
                        //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: {}\n\n", whole_answer.replace("\n", "<br>")).into_bytes()); // 这里要声明类型，否则报错，传递数据以`data: `起始，以`\n\n`终止
                        let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, get_messages_num(&tmp_uuid) - 1, whole_answer.replace("\n", "<br>"), true, false, false, false, None, None)?);
                        yield tmp;
                        // 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                        //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: showinfo{}srx{}srx{}srx{}srx{}\n\n", tmp_uuid, token[0], token[1], prompt_name, related_uuid_prompt.into_iter().map(|up| up.0+"*"+&up.1).collect::<Vec<_>>().join("#")).as_bytes().to_vec()); // 传递数据以`data: `起始，以`\n\n`终止
                        let tmp: Result<Vec<u8>, MyError> = Ok(MetaData::prepare_sse(tmp_uuid, Some(0))?);
                        yield tmp;
                        // 结束stream
                        let tmp: Result<Vec<u8>, MyError> = Ok(b"event: close\ndata: {\"key\": \"close\"}\n\n".to_vec()); // 最后以`event: close\ndata: {"key": "close"}\n\n`结束stream，data需要是json格式，否则js的`JSON.parse`解析时报错
                        yield tmp;
                    };
                    // Convert the stream into a response
                    match Response::builder()
                        //.header("Content-Type", "text/plain")
                        .header("Content-Type", "text/event-stream")
                        .header("Cache-Control", "no-cache")
                        .header("Connection", "keep-alive")
                        .header("Access-Control-Allow-Origin", "*")
                        .body(Body::from_stream(tmp_stream))
                    {
                        Ok(resp) => Ok((cookie_jar, resp)),
                        Err(e) => Err(MyError::ResponseError{uuid: uuid, error: e}),
                    }
                }
            }
        } else {
            let (message, query, err_msg): (Option<ChatMessage>, String, String) = if body.starts_with("img http") {
                (
                    Some(ChatMessage::User{ // 相较0.6.5版本，1.0.0版本将图片和音频从ChatMessage移出去了，因此暂不支持对图片提问
                        content: ChatMessageContent::Text(body.clone()),
                        name: None,
                        /*
                        content: ChatMessageContent::ImageContentPart(vec![ChatMessageImageContentPart{
                            r#type: "image_url".to_string(),
                            image_url: ImageUrlType {
                                url: body.strip_prefix("img ").unwrap().to_string(),
                                detail: None,
                            },
                        }]),
                        name: None,
                        */
                        /*
                        content: ChatMessageContent::ImageUrl(vec![ImageUrl{
                            r#type: "image_url".to_string(),
                            text: None,
                            image_url: ImageUrlType{
                                url: body.strip_prefix("img ").unwrap().to_string(),
                                detail: None,
                            },
                        }]),
                        name: None,
                        */
                    }),
                    body.clone(),
                    "".to_string(),
                )
            } else if web_search { // 使用网络搜索
                match get_search_parse_result(&uuid, body.clone()) {
                    (Some(res), err_str) => (
                        Some(ChatMessage::User{
                            content: ChatMessageContent::Text(res.clone()),
                            name: None,
                        }),
                        res,
                        err_str
                    ),
                    (None, err_str) => (None, body.clone(), err_str),
                }
            } else { // 常规问题
                (
                    Some(ChatMessage::User{
                        content: ChatMessageContent::Text(body.clone()),
                        name: None,
                    }),
                    body.clone(),
                    "".to_string(),
                )
            };
            if let Some(m) = message {
                // 当前问题插入到messages中
                if web_search { // 使用网络搜索，需记录原始问题
                    insert_message(&uuid, m, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), true, DataType::Raw(body.clone()), qa_msg_p, &model, chat_name);
                } else {
                    insert_message(&uuid, m, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Normal, qa_msg_p, &model, chat_name);
                }
            } else {
                // 插入原始问题
                let m = ChatMessage::User{
                    content: ChatMessageContent::Text(body.clone()),
                    name: None,
                };
                if web_search { // 使用网络搜索，需记录原始问题
                    insert_message(&uuid, m, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), true, DataType::Raw(body.clone()), qa_msg_p, &model, chat_name);
                } else {
                    insert_message(&uuid, m, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Normal, qa_msg_p, &model, chat_name);
                }
                // 插入错误提示
                let m = ChatMessage::Assistant{
                    content: Some(ChatMessageContent::Text(err_msg.clone())),
                    reasoning_content: None,
                    refusal: None,
                    name: None,
                    audio: None,
                    tool_calls: None,
                };
                insert_message(&uuid, m, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), false, DataType::Normal, qa_msg_p, &model, None);
            }
            event!(Level::INFO, "{} GET {}, query: {}", uuid, uri.path(), get_query_num(&uuid));
            // 创建stream对象，接收管道传递的数据
            //let tmp_query = body.clone();
            let tmp_uuid = uuid.clone();
            let tmp_stream = async_stream::stream! {
                // 传输指定uuid的chat记录
                if load_uuid {
                    // Vec<(是否是提问, 问题或答案字符串, 作为html中tag的id的序号, 时间)>
                    for (i, log) in get_log_for_display(&tmp_uuid, false).2.into_iter().enumerate() {
                        if log.is_query {
                            //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: rightsrx{}timesrx{}\n\n", log.3, log.1).as_bytes().to_vec()); // 这里要声明类型，否则报错
                            let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, i, log.content, false, log.is_img, log.is_voice, true, Some(log.time), Some(log.token))?);
                            yield tmp;
                        } else {
                            //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: leftsrx{}timesrx{}\n\n", log.3, log.1).as_bytes().to_vec()); // 这里要声明类型，否则报错
                            let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, i, log.content, true, log.is_img, log.is_voice, true, Some(log.time), Some(log.token))?);
                            yield tmp;
                        }
                    }
                    let tmp: Result<Vec<u8>, MyError> = Ok(MetaData::prepare_sse(tmp_uuid, Some(0))?);
                    yield tmp;
                /*} else if clear_page { // 清空页面之前的chat记录，显示当前问题
                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: rightsrx{}timesrx{}\n\n", Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), tmp_query.replace("\n", "srxtzn")).as_bytes().to_vec()); // 传递数据以`data: `起始，以`\n\n`终止
                    let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, get_messages_num(&tmp_uuid) - 1, tmp_query.replace("\n", "srxtzn"), false, false, false, true, Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string()))?);
                    yield tmp;
                }*/
                } else {
                    // 如果网络搜索或解析url和html有报错，则将错误信息恢复给客户端显示。由于前面`insert_message`插入了错误信息，因此这里返回给用户的id要减1
                    if !err_msg.is_empty() {
                        // 传输原始问题
                        let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, get_messages_num(&tmp_uuid) - 2, query.replace("\n", "srxtzn"), false, false, false, false, None, Some(get_msg_token(&tmp_uuid, -2)))?);
                        yield tmp;
                        // 传输错误信息
                        //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: {}\n\n", err_msg.replace("\n", "srxtzn")).as_bytes().to_vec()); // 传递数据以`data: `起始，以`\n\n`终止
                        let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, get_messages_num(&tmp_uuid) - 1, err_msg.replace("\n", "srxtzn"), true, false, false, false, None, Some(get_msg_token(&tmp_uuid, -1)))?);
                        yield tmp;
                    } else {
                        // 传输原始问题
                        let tmp: Result<Vec<u8>, MyError> = Ok(MainData::prepare_sse(&tmp_uuid, get_messages_num(&tmp_uuid) - 1, query.replace("\n", "srxtzn"), false, false, false, false, None, Some(get_msg_token(&tmp_uuid, -1)))?);
                        yield tmp;
                    }
                    // 显示在页面的信息，包括：当前uuid、当前uuid的问题和答案的总token数、当前uuid的prompt名称、与当前uuid相关的所有uuid
                    //let tmp: Result<Vec<u8>, Error> = Ok(format!("data: showinfo{}srx{}srx{}srx{}srx{}\n\n", tmp_uuid, token[0], token[1], prompt_name, related_uuid_prompt.into_iter().map(|up| up.0+"*"+&up.1).collect::<Vec<_>>().join("#")).as_bytes().to_vec()); // 传递数据以`data: `起始，以`\n\n`终止
                    let tmp: Result<Vec<u8>, MyError> = Ok(MetaData::prepare_sse(tmp_uuid, Some(0))?);
                    yield tmp;
                }
                let tmp: Result<Vec<u8>, MyError> = Ok(b"event: close\ndata: {\"key\": \"close\"}\n\n".to_vec()); // 最后以`event: close\ndata: {"key": "close"}\n\n`结束stream，data需要是json格式，否则js的`JSON.parse`解析时报错
                yield tmp;
            };
            // Convert the stream into a response
            match Response::builder()
                //.header("Content-Type", "text/plain")
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from_stream(tmp_stream))
            {
                Ok(resp) => Ok((cookie_jar, resp)),
                Err(e) => Err(MyError::ResponseError{uuid: uuid, error: e}),
            }
        }
    } else {
        event!(Level::INFO, "{} POST {}, no query", uuid, uri.path());
        // 创建stream对象，接收管道传递的数据
        let tmp_stream = async_stream::stream! {
            let tmp: Result<Vec<u8>, MyError> = Ok(b"data: no query\n\n".to_vec()); // 这里要声明类型，否则报错，传递数据以`data: `起始，以`\n\n`终止
            yield tmp;
            let tmp: Result<Vec<u8>, MyError> = Ok(b"event: close\ndata: {\"key\": \"close\"}\n\n".to_vec()); // 最后以`event: close\ndata: {"key": "close"}\n\n`结束stream，data需要是json格式，否则js的`JSON.parse`解析时报错
            yield tmp;
        };
        // Convert the stream into a response
        match Response::builder()
            //.header("Content-Type", "text/plain")
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header("Access-Control-Allow-Origin", "*")
            .body(Body::from_stream(tmp_stream))
        {
            Ok(resp) => Ok((cookie_jar, resp)),
            Err(e) => Err(MyError::ResponseError{uuid: uuid, error: e}),
        }
    }
}
