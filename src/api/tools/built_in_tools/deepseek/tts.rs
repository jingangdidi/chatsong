use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

const TTS_URL: &str = "https://api.deepseek.com/v1/tools/tts";
const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Deserialize, Serialize)]
struct Params {
    text: String,
    voice_id: Option<String>,
    format: Option<String>,
}

pub struct DeepseekTts;

impl DeepseekTts {
    pub fn new() -> Self {
        Self
    }
}

impl BuiltIn for DeepseekTts {
    fn name(&self) -> String {
        "tts".to_string()
    }

    fn description(&self) -> String {
        "Convert complete text to speech using DeepSeek TTS and save the audio to a local file. Supports stella (default) and tide voices, WAV (default) and raw PCM output. Language is detected automatically. Text is limited to 10,000 billing characters (each Han character counts as two); the JSON request body is limited to 64 KiB. Markdown formatting is always removed. The output location is supplied by the host. Existing output files are never overwritten. Returns output_path, format, and audio_bytes.".to_string()
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Complete text to read aloud. At most 10,000 billing characters; each Han character counts as two."
                },
                "voice_id": {
                    "type": "string",
                    "enum": ["stella", "tide"],
                    "default": "stella",
                    "description": "stella: female voice; tide: male voice."
                },
                "format": {
                    "type": "string",
                    "enum": ["wav", "pcm"],
                    "default": "wav",
                    "description": "Audio format. Raw PCM is 24 kHz, 16-bit, mono."
                }
            },
            "required": ["text"],
            "additionalProperties": false
        })
    }

    // 返回完整参数 JSON，供现有异步调度器传给 deepseek_tts
    fn run(&self, _args: &str) -> Result<(String, Option<String>), MyError> {
        Ok((String::new(), None))
    }

    fn get_approval(&self, _args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let info = info.unwrap_or_default();
        let message = if is_en {
            format!("Do you allow sending this text to DeepSeek for speech synthesis?{}", info)
        } else {
            format!("是否允许将文本发送至 DeepSeek 进行语音合成？{}", info)
        };
        Ok(Some(message))
    }
}

#[derive(Serialize)]
struct TtsBody<'a> {
    text: &'a str,
    voice_id: &'a str,
    format: &'a str,
    filter_markdown: bool,
}

// 校验参数并序列化实际发送的请求体。
fn build_tts_body(params: &Params) -> Result<Vec<u8>, MyError> {
    if params.text.trim().is_empty() {
        return Err(MyError::OtherError {
            info: "TTS text must not be empty".to_string(),
        });
    }
    // 这里只检查字符数量下界；汉字双倍计费的精确上限由服务端校验。
    if params.text.chars().count() > 10_000 {
        return Err(MyError::OtherError {
            info: "TTS text exceeds 10,000 billing characters".to_string(),
        });
    }
    let voice_id = params.voice_id.as_deref().unwrap_or("stella");
    if !matches!(voice_id, "stella" | "tide") {
        return Err(MyError::OtherError {
            info: "TTS voice_id must be stella or tide".to_string(),
        });
    }
    let format = params.format.as_deref().unwrap_or("wav");
    if !matches!(format, "wav" | "pcm") {
        return Err(MyError::OtherError {
            info: "TTS format must be wav or pcm".to_string(),
        });
    }
    let body = TtsBody {
        text: &params.text,
        voice_id,
        format,
        filter_markdown: true,
    };
    let bytes = serde_json::to_vec(&body).map_err(|error| MyError::ToJsonStirngError {
        uuid: String::new(),
        error,
    })?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(MyError::OtherError {
            info: "TTS JSON request body must be at most 64 KiB".to_string(),
        });
    }
    Ok(bytes)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TtsResult {
    pub output_path: String,
    pub format: String,
    pub audio_bytes: u64,
}

/// 整段提交文本，完整接收二进制音频后保存到本地，返回文件信息 JSON。
/// args 是 run() 返回的参数 JSON；output_path 由调用方指定，不属于工具参数。
pub async fn deepseek_tts(args: &str, output_path: &str) -> Result<String, MyError> {
    if PARAS.deepseek_search_api_key.trim().is_empty() {
        return Err(MyError::OtherError {info: "no deepseek api key for TTS".to_string()});
    }
    let params: Params = parse_tool_args(args, ArgFixSpec {array_fields: None, object_fields: None})?;
    let body = build_tts_body(&params)?;

    let resp = reqwest::Client::new()
        .post(TTS_URL)
        .bearer_auth(&PARAS.deepseek_search_api_key)
        // 使用 serde_json 序列化 + body，无需 reqwest 的 json/multipart feature
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .timeout(Duration::from_secs(2_100))
        .send()
        .await
        .map_err(|error| MyError::SendRequestError {url: TTS_URL.to_string(), error})?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|e| format!("failed to read error response: {e}"));
        return Err(MyError::OtherError {info: format!("DeepSeek TTS API error {status}: {body}")});
    }

    // bytes() 完整消费响应；HTTP 层检测到传输不完整时会报错，不保存音频片段
    // 整段音频会暂存在内存中；不自动重试合成请求
    let audio = resp.bytes().await.map_err(|e| MyError::OtherError {info: format!("failed to receive complete TTS audio: {e}")})?;
    if audio.is_empty() {
        return Err(MyError::OtherError {info: "DeepSeek TTS returned empty audio".to_string()});
    }

    let format = params.format.as_deref().unwrap_or("wav");
    let output_path = format!("{output_path}.{format}");

    // create_new 保证即使检查后其他进程创建同名文件，也不会覆盖它
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .await
        .map_err(|e| MyError::OtherError {info: format!("cannot create TTS output file: {e}")})?;
    let saved = async {
        file.write_all(&audio).await?;
        file.flush().await
    }
    .await;
    drop(file);
    if let Err(e) = saved {
        let cleanup = tokio::fs::remove_file(&output_path).await;
        let detail = cleanup
            .err()
            .map(|e| format!("; failed to remove partial output: {e}"))
            .unwrap_or_default();
        return Err(MyError::OtherError {info: format!("cannot write TTS audio: {e}{detail}")});
    }

    let result = TtsResult {
        output_path: output_path,
        format: format.to_string(),
        audio_bytes: audio.len() as u64,
    };
    serde_json::to_string_pretty(&result).map_err(|error| MyError::ToJsonStirngError {
        uuid: String::new(),
        error,
    })
}
