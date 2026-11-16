use std::{fs::Metadata, path::Path, time::Duration};

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::{
            BuiltIn,
            filesystem::utils::validate_path,
        },
    },
};

const ASR_URL: &str = "https://api.deepseek.com/v1/tools/asr";
const MAX_AUDIO_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Params {
    file_path: String,
}

pub struct DeepseekAsr;

impl DeepseekAsr {
    pub fn new() -> Self {
        Self
    }
}

impl BuiltIn for DeepseekAsr {
    fn name(&self) -> String {
        "asr".to_string()
    }

    fn description(&self) -> String {
        "Transcribe a local audio file using DeepSeek ASR. Accepts 16-bit PCM WAV or Ogg Opus, up to 8 MiB and 120 seconds. Language is detected automatically. For stereo audio, only the left channel is recognized. Returns an object with request_id, text, and duration_ms.".to_string()
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Path to a local audio file accessible to this process. Prefer an absolute path."
                }
            },
            "required": ["file_path"],
            "additionalProperties": false
        })
    }

    // 只校验并返回参数，由现有调度器调用异步函数
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        if PARAS.deepseek_search_api_key.trim().is_empty() {
            return Err(MyError::OtherError {info: "no deepseek api key for ASR".to_string()});
        }
        let params: Params = parse_tool_args(args, ArgFixSpec {array_fields: None, object_fields: None})?;

        let valid_path = validate_path(&PARAS.allowed_path, &Path::new(&params.file_path.replace("\\", "/")), false)?;
        let metadata = valid_path.metadata().map_err(|e| MyError::OtherError {info: format!("cannot access ASR file: {e}")})?;
        validate_audio_metadata(&metadata)?;

        Ok((valid_path.to_str().unwrap_or_default().to_string(), None))
    }

    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = parse_tool_args(args, ArgFixSpec {array_fields: None, object_fields: None})?;
        let info = info.unwrap_or_default();
        let message = if is_en {
            format!("Do you allow uploading audio file {:?} to DeepSeek for transcription?{}", params.file_path, info)
        } else {
            format!("是否允许将音频文件 {:?} 上传至 DeepSeek 进行语音识别？{}", params.file_path, info)
        };
        Ok(Some(message))
    }
}

fn validate_audio_metadata(metadata: &Metadata) -> Result<(), MyError> {
    if !metadata.is_file() {
        return Err(MyError::OtherError {info: "ASR input must be a regular file".to_string()});
    }
    if metadata.len() == 0 {
        return Err(MyError::OtherError {info: "ASR audio file must not be empty".to_string()});
    }
    if metadata.len() > MAX_AUDIO_BYTES {
        return Err(MyError::OtherError {info: "ASR audio file must be at most 8 MiB".to_string()});
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AsrResult {
    pub request_id: String,
    pub text: String,
    pub duration_ms: u64,
}

/// 上传本地音频并返回 JSON 字符串。
/// 编码、采样率、声道数和时长由服务端根据文件内容校验，不依赖扩展名。
pub async fn deepseek_asr(file_path: &str) -> Result<String, MyError> {
    if PARAS.deepseek_search_api_key.trim().is_empty() {
        return Err(MyError::OtherError {info: "no deepseek api key for ASR".to_string()});
    }

    let file = tokio::fs::File::open(file_path).await.map_err(|e| MyError::OtherError {info: format!("cannot open ASR file: {e}")})?;

    // 限制实际读取量，防止文件在 metadata 检查后增长导致无限制分配。
    let mut audio = Vec::new();
    file.take(MAX_AUDIO_BYTES + 1).read_to_end(&mut audio).await.map_err(|e| MyError::OtherError {info: format!("cannot read ASR file: {e}")})?;
    if audio.is_empty() || audio.len() as u64 > MAX_AUDIO_BYTES {
        return Err(MyError::OtherError {info: "ASR audio size must be between 1 byte and 8 MiB".to_string()});
    }

    let filename = Path::new(file_path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".to_string());
    let form = Form::new().part("file", Part::bytes(audio).file_name(filename));

    let resp = reqwest::Client::new()
        .post(ASR_URL)
        .bearer_auth(&PARAS.deepseek_search_api_key)
        // 由 reqwest 自动设置 multipart Content-Type 和 boundary。
        .multipart(form)
        // 整个请求的客户端超时；服务端另有 10 分钟上传限制。
        .timeout(Duration::from_secs(660))
        .send()
        .await
        .map_err(|error| MyError::SendRequestError {url: ASR_URL.to_string(), error})?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|e| format!("failed to read error response: {e}"));
        return Err(MyError::OtherError {info: format!("DeepSeek ASR API error {status}: {body}")});
    }

    let parsed: AsrResult = resp.json().await.map_err(|e| MyError::OtherError {info: format!("failed to decode ASR response: {e}")})?;

    serde_json::to_string_pretty(&parsed).map_err(|error| MyError::ToJsonStirngError {uuid: String::new(), error})
}
