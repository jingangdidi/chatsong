use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

use axum::{
    extract::{
        Query,
        OriginalUri,
        ConnectInfo,
    },
    http::StatusCode,
};
use axum_extra::extract::cookie::CookieJar;
use openai_dive::v1::{
    api::Client,
    resources::embedding::{
        EmbeddingEncodingFormat,
        EmbeddingInput,
        EmbeddingParametersBuilder,
        EmbeddingOutput,
    },
};
use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    error::MyError,
    info::{
        get_messages, // 获取指定uuid最近的指定数量个message
        update_qa_msg_num, // 客户端下拉选项`上下文消息数`改变时更新限制的问答对数量、限制的消息数量、提问是否包含prompt
        label_remembered,
        update_token,
    },
    api::handlers::chat::{
        get_qa_msg_p,
        is_local_request,
    },
    memory::{
        MEMORY,
        SimpleMemory,
    },
    tools::built_in_tools::hacker_news::run_single_llm,
    openai::for_chat::get_print_token,
};

/// Handler for `/嵌套的前缀/memory` GET
pub async fn memory(Query(params): Query<HashMap<String, String>>, ConnectInfo(addr): ConnectInfo<SocketAddr>, uri: OriginalUri, jar: CookieJar) -> Result<StatusCode, MyError> {
    // 获取uuid
    if let Some(c) = jar.get("srx-tzn") { // 获取cookie
        let uuid = c.value().to_string();
        // 提取并添加记忆
        if let Some(m) = params.get("memory") {
            // 检查是否服务端所在电脑发起的请求
            let ip = addr.ip();
            let is_local = is_local_request(&ip);
            // 获取要记住的内容
            let (for_memory, from_context) = if m.trim().is_empty() {
                event!(Level::INFO, "{} remember the current conversation", uuid);
                match get_qa_msg_p(params.get("num"), false) {
                    Ok(qa_msg_p) => {
                        update_qa_msg_num(&uuid, qa_msg_p);
                        let messages = get_messages(&uuid);
                        (messages.into_iter().map(|msg| format!("{:?}", msg)).collect::<Vec<String>>().join("\n"), true)
                    },
                    Err(e) => {
                        event!(Level::ERROR, "{} get_qa_msg_p error: {}", uuid, e);
                        return Ok(StatusCode::OK)
                    },
                }
            } else {
                event!(Level::INFO, "{} Remember the following content: {}", uuid, m.trim());
                (m.trim().to_string(), false)
            };
            // 获取用于提取记忆的模型，返回 (api_key, endpoint, model, reasoning)
            let model_for_memory = match params.get("model") {
                Some(model) => match PARAS.api.get_model_by_str(&model) {
                    Ok(model_for_memory) => model_for_memory,
                    Err(e) => {
                        event!(Level::ERROR, "{} get model for memory error: {}", uuid, e);
                        return Ok(StatusCode::OK)
                    },
                },
                None => match PARAS.api.get_default_model() {
                    Ok(model_for_memory) => model_for_memory,
                    Err(e) => {
                        event!(Level::ERROR, "{} get default model for memory error: {}", uuid, e);
                        return Ok(StatusCode::OK)
                    },
                },
            };
            // 提取记忆
            let memory_summary = extract_memory(&for_memory, model_for_memory).await?;
            // 计算 embedding
            let embedding = get_embedding(&uuid, memory_summary.clone()).await?;
            // 如果是服务端所在电脑发起的请求，key使用`local`存储到输出路径根路径下的`memory.json`，否则使用各自uuid并存储到各自uuid路径`uuid_memory.json`
            let key = if is_local {
                "local".to_string()
            } else {
                uuid.clone()
            };
            let mut data = MEMORY.lock().unwrap();
            let old = match data.get_mut(&key) {
                Some(memory) => memory.remember(for_memory, memory_summary, embedding, is_local),
                None => {
                    let memory_file = if is_local {
                        format!("{}/memory.json", PARAS.memory_dir)
                    } else {
                        format!("{}/{}/{}_memory.json", PARAS.outpath, key, key)
                    };
                    let memory_path = Path::new(&memory_file);
                    if memory_path.exists() && memory_path.is_file() {
                        match SimpleMemory::load_from_file(&memory_file, is_local) {
                            Ok(mut memory) => {
                                let old = memory.remember(for_memory, memory_summary, embedding, is_local);
                                data.insert(key, memory);
                                old
                            },
                            Err(e) => {
                                event!(Level::ERROR, "load memory file ({}) error: {}", memory_file, e);
                                None
                            },
                        }
                    } else {
                        let mut memory = SimpleMemory::new(100, memory_file); // 设置最多100条记忆
                        let old = memory.remember(for_memory, memory_summary, embedding, is_local);
                        data.insert(key, memory);
                        old
                    }
                },
            };
            // 如果是本地记忆，且移除了旧记忆，则将旧记忆加到 memory_old.json 中
            if is_local {
                if let Some(old_notes) = old {
                    match data.get_mut("old") {
                        Some(memory) => memory.append_memory(old_notes),
                        None => {
                            let memory_file = format!("{}/memory_old.json", PARAS.memory_dir);
                            let memory_path = Path::new(&memory_file);
                            if memory_path.exists() && memory_path.is_file() {
                                match SimpleMemory::load_from_file(&memory_file, is_local) {
                                    Ok(mut memory) => {
                                        memory.append_memory(old_notes);
                                        data.insert("old".to_string(), memory);
                                    },
                                    Err(e) => event!(Level::ERROR, "load old memory file ({}) error: {}", memory_file, e),
                                }
                            } else {
                                let mut memory = SimpleMemory::new(usize::MAX, memory_file);
                                memory.append_memory(old_notes);
                                data.insert("old".to_string(), memory);
                            }
                        },
                    }
                }
            }
            // 最后将已提取记忆的对话标注为 remembered
            if from_context {
                label_remembered(&uuid);
            }
        }
    } else {
        event!(Level::INFO, "GET {}, set memory failed, no cookie", uri.path()); // 注意：`axum::http::Uri`只能捕获到`/hello`，不包含嵌套的`/嵌套的前缀`前缀，使用`OriginalUri`可以
    }
    Ok(StatusCode::OK)
}

/// 提取记忆的prompt
const MEMORY_PROMPT: &str = r###"You are a memory extraction assistant.

Your task is to extract one concise long-term memory from the conversation.

The host application has already decided that this conversation is allowed to be considered for memory. You do not decide whether learning is allowed. You only decide whether there is any useful memory content to extract.

Extract a memory only if it is:
- Stable beyond the current turn
- Useful for future tasks or future conversations
- Specific enough to act on
- Supported by the user's messages or by verified work done in the conversation

Prefer extracting:
- User preferences, working style, communication style, constraints, or recurring instructions
- Project-specific facts, architecture decisions, conventions, commands, paths, or workflows
- Decisions that should be remembered later
- Corrections from the user about how the assistant should behave
- Reusable lessons learned from completed or verified work

Do not extract:
- Temporary task progress
- One-off requests that are already completed
- Generic conversation summaries
- Unverified external information
- Tool output dumps
- Speculation, guesses, or uncertain claims
- Secrets, credentials, API keys, tokens, private personal data, or sensitive information
- Anything the user explicitly said not to remember

Write the memory as a short, standalone statement. It must make sense without reading the original conversation.

If multiple memories are possible, choose only the most useful one.

If nothing should be remembered, output an empty string.

Output only the final memory string. Do not include JSON, Markdown, headings, explanations, labels, quotes, or bullet points.

Conversation:
"###;

/// 调用 LLM 从指定内容中抽取记忆
/// text: 要提取记忆的原始内容
/// model_for_memory: (api_key, endpoint, 模型名称, 是否支持深度思考)
async fn extract_memory(text: &str, model_for_memory: (String, String, String, bool)) -> Result<String, MyError> {
    let content = format!("{}{}", MEMORY_PROMPT, text);
    run_single_llm("memory", content, model_for_memory.0, model_for_memory.1, model_for_memory.2).await
}

/// 调用 embedding 模型，计算指定字符串的 embedding 向量
pub async fn get_embedding(uuid: &str, text: String) -> Result<Option<Vec<f64>>, MyError> {
    // 获取 embedding 模型
    if let Some((api_key, endpoint, model)) = PARAS.api.get_embedding_modle(None) {
        // 使用api key初始化
        let mut client = Client::new(api_key);
        client.set_base_url(&endpoint); // 从0.7.0开始舍弃了new_with_base
        // https://docs.rs/openai_dive/1.4.3/openai_dive/v1/resources/embedding/struct.EmbeddingParametersBuilder.html
        let mut para_builder = EmbeddingParametersBuilder::default();
        para_builder.model(&model);
        para_builder.input(EmbeddingInput::String(text));
        para_builder.encoding_format(EmbeddingEncodingFormat::Float);
        para_builder.dimensions(1024_u32);
        let parameters = para_builder.build().map_err(|e| MyError::EmbeddingError{error: e})?;
        let result = client.embeddings().create(parameters).await.map_err(|e| MyError::ApiError{uuid: "embedding".to_string(), error: e})?;
        // +----------------------------+                     +---------------------------------+     +----------------------+
        // | struct EmbeddingResponse { |                     | struct Embedding {              |     | EmbeddingOutput {    |
        // |     object: String,        | always 'embedding'  |     index: u32,                 |     |     Float(Vec<f64>), |
        // |     data: Vec<Embedding>,  | ------------------> |     embedding: EmbeddingOutput, | --> |     Base64(String),  |
        // |     model: String,         | model name          |     object: String,             |     | }                    |
        // |     usage: Option<Usage>,  | token usage         +---------------------------------+     +----------------------+
        // | }                          |
        // +----------------------------+
        if let Some(usage) = result.usage {
            // 提取 token 用量，打印并更新到指定 uuid
            if let Some(tokens) = get_print_token(usage, uuid) {
                update_token(uuid, tokens);
            }
        }
        match &result.data[0].embedding {
            EmbeddingOutput::Float(f64_vec) => Ok(Some(f64_vec.to_vec())),
            EmbeddingOutput::Base64(b64) => {
                event!(Level::WARN, "{} embedding return base64 result: {}", uuid, b64);
                Ok(None)
            },
        }
    } else {
        Ok(None)
    }
}
