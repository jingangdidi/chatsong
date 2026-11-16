use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

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
    }
};

/// params for deepseek ocr
/// https://api-docs.deepseek.com/zh-cn/guides/ocr/
#[derive(Deserialize)]
struct Params {
    /// 本地文件路径（建议绝对路径），支持 PDF、Word(DOC/DOCX)、PowerPoint(PPT/PPTX) 及 PNG/JPEG/WebP/GIF/BMP/TIFF
    file: String,
    /// 识别用途，缺省为 text_recognition
    #[serde(default)]
    purpose: Purpose,
}

/// 请求中的 purpose 字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// 不含图片的 Word、PowerPoint 直接提取文字，其他文件逐页识别
    TextRecognition,
    /// 所有文件都逐页识别（保留 <｜ref｜><｜box｜> 等版式标记）
    RichLayoutRecognition,
}

impl Default for Purpose {
    fn default() -> Self {
        Purpose::TextRecognition
    }
}

impl Purpose {
    /// 序列化成 API 需要的字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Purpose::TextRecognition => "text_recognition",
            Purpose::RichLayoutRecognition => "rich_layout_recognition",
        }
    }
}

/// built-in tool
pub struct DeepseekOcr;

impl DeepseekOcr {
    /// new
    pub fn new() -> Self {
        DeepseekOcr
    }
}

impl BuiltIn for DeepseekOcr {
    /// get tool name
    fn name(&self) -> String {
        "ocr".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Run OCR on a local file and return the recognized text page by page. Supports PDF, Word (DOC/DOCX), PowerPoint (PPT/PPTX) and images (PNG/JPEG/WebP/GIF/BMP/TIFF); the real format is detected from the file content, so a correct file name helps but is not authoritative. Set `purpose` to `text_recognition` (default) to extract text directly from Word/PowerPoint files that contain no images and to recognize other files page by page, or to `rich_layout_recognition` to recognize every file page by page and keep layout markers such as <|ref|> and <|box|>[[x1, y1, x2, y2]] (coordinates are normalized to 0-1000). Returns `id`, `purpose`, `page_count`, `pages[]` (each with `index`, `text`, `finish_reason`; `finish_reason` is `length` when that page was truncated) and `usage`. Limits: files up to 100 MiB, documents up to 1000 pages, 180 seconds total; recognized pages are billed at 1.5 CNY per 1000 pages. The file is not stored in the Files API and is not sent to any chat model.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "file": {
                    "type": "string",
                    "description": "Path of the local file to recognize. Must be an existing regular file whose size is at most 100 MiB. Supported: PDF, Word (DOC, DOCX), PowerPoint (PPT, PPTX), PNG, JPEG, WebP, GIF, BMP, TIFF.",
                },
                "purpose": {
                    "type": "string",
                    "enum": ["text_recognition", "rich_layout_recognition"],
                    "default": "text_recognition",
                    "description": "text_recognition: extract text directly for Word/PowerPoint files without images, recognize other files page by page. rich_layout_recognition: recognize every file page by page and keep layout markers and coordinates.",
                },
            },
            "required": ["file"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        if PARAS.deepseek_search_api_key.is_empty() {
            return Err(MyError::OtherError{info: "no deepseek api key for ocr".to_string()})
        }

        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;

        let valid_path = validate_path(&PARAS.allowed_path, &Path::new(&params.file.replace("\\", "/")), false)?;
        let file_str = valid_path.to_str().unwrap_or_default();
        validate_ocr_file(&file_str)?;

        Ok((format!("{}---srx---{}", file_str, params.purpose.as_str()), None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        if is_en {
            Ok(Some(format!("Do you allow calling the ocr tool?{}", info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 ocr 工具？{}", info.unwrap_or_default())))
        }
    }
}

/// 按文档约束校验本地文件：存在、是普通文件、≤ 100 MiB，返回文件大小
fn validate_ocr_file(file: &str) -> Result<u64, MyError> {
    let path = Path::new(file);
    let meta = std::fs::metadata(path)
        .map_err(|e| MyError::OtherError{info: format!("cannot stat file {file}: {e}")})?;
    if !meta.is_file() {
        return Err(MyError::OtherError{info: format!("not a regular file: {file}")})
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(MyError::OtherError{info: format!("file is too large: {} bytes, max 100 MiB", meta.len())})
    }
    if meta.len() == 0 {
        return Err(MyError::OtherError{info: format!("file is empty: {file}")})
    }
    Ok(meta.len())
}

/// 按扩展名猜 MIME；真实格式由服务端按内容判断，猜不到就用 octet-stream
fn mime_by_ext(ext: Option<&str>) -> Option<&'static str> {
    let mime = match ext.map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("pdf") => "application/pdf",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("tif") | Some("tiff") => "image/tiff",
        _ => "application/octet-stream",
    };
    Some(mime)
}

/// 单页识别结果
#[derive(Debug, Serialize, Deserialize)]
pub struct OcrPage {
    /// 页码，从 0 开始
    pub index: u32,
    /// 该页的模型原始输出，不做任何清理
    pub text: String,
    /// stop；输出达到上限时为 length，该页文本不完整
    #[serde(default)]
    pub finish_reason: String,
}

/// token 用量，仅供参考，不用于计费
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OcrUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
}

/// 响应示例:
/// {
///   "id": "ocr-9b2f4c1e-...",
///   "object": "ocr.result",
///   "created": 1762790000,
///   "purpose": "text_recognition",
///   "page_count": 2,
///   "pages": [
///     {"index": 0, "text": "...", "finish_reason": "stop"},
///     {"index": 1, "text": "...", "finish_reason": "length"}
///   ],
///   "usage": {"input_tokens": 1536, "output_tokens": 412, "total_tokens": 1948}
/// }
#[derive(Debug, Serialize, Deserialize)]
pub struct OcrResult {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub page_count: u32,
    #[serde(default)]
    pub pages: Vec<OcrPage>,
    #[serde(default)]
    pub usage: Option<OcrUsage>,
}

const OCR_URL: &str = "https://api.deepseek.com/v1/tools/ocr";

/// 100 MiB
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

/// 文档总时长 180 秒，客户端多留一点余量
const OCR_TIMEOUT_SECS: u64 = 200;

/// 调用 deepseek ocr 识别本地文件，返回格式化后的 JSON 结果
/// 请求示例: curl https://api.deepseek.com/v1/tools/ocr -H "Authorization: Bearer <DeepSeek API Key>" -F purpose="text_recognition" -F file="@document.pdf"
pub async fn deepseek_ocr(file: &str, purpose: &str) -> Result<String, MyError> {
    let path = Path::new(file);

    // file 字段必须带文件名
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| MyError::OtherError{info: format!("invalid file path: {file}")})?
        .to_string();

    // 最大 100 MiB，直接读进内存；若担心内存占用可改用
    // tokio_util::io::ReaderStream + Part::stream_with_length 流式上传
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| MyError::OtherError{info: format!("failed to read file {file}: {e}")})?;

    let mut part = reqwest::multipart::Part::bytes(bytes).file_name(file_name);
    if let Some(mime) = mime_by_ext(path.extension().and_then(|e| e.to_str())) {
        part = part
            .mime_str(mime)
            .map_err(|e| MyError::OtherError{info: format!("invalid mime type {mime}: {e}")})?;
    }

    // 只有 file 与 purpose 两个字段，多传会 400
    let form = reqwest::multipart::Form::new()
        .text("purpose", purpose.to_string())
        .part("file", part);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(OCR_TIMEOUT_SECS))
        .build()
        .map_err(|e| MyError::OtherError{info: format!("failed to build http client: {e}")})?;

    let resp = client
        .post(OCR_URL)
        .bearer_auth(&PARAS.deepseek_search_api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| MyError::SendRequestError{url: OCR_URL.to_string(), error: e})?;

    let status = resp.status();
    // 识别过程中服务端持续发送换行符保活，换行属于 JSON 的前导空白，
    // 这里先取文本再 trim，既能正常解析，也方便把错误体带进报错信息
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(MyError::OtherError{info: describe_http_error(status.as_u16(), &body)})
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(MyError::OtherError{info: "empty ocr response body".to_string()})
    }

    let value: Value = serde_json::from_str(trimmed)
        .map_err(|e| MyError::OtherError{
            info: format!("failed to decode ocr response: {e}; raw: {}", truncate(trimmed, 500)),
        })?;

    // 识别开始后状态码已经是 200，失败/超时会在响应体里返回 error 对象
    if let Some(err) = value.get("error") {
        let message = err.get("message").and_then(Value::as_str).unwrap_or("unknown error");
        let code = err.get("code").and_then(Value::as_str).unwrap_or("");
        return Err(MyError::OtherError{
            info: format!("ocr failed: {message} (code: {code})"),
        })
    }

    let result: OcrResult = serde_json::from_value(value).map_err(|e| MyError::OtherError{
        info: format!("failed to parse ocr result: {e}"),
    })?;

    // 有页被截断时提示一下（pages[].finish_reason == "length"）
    let truncated: Vec<u32> = result
        .pages
        .iter()
        .filter(|p| p.finish_reason != "stop")
        .map(|p| p.index)
        .collect();
    if !truncated.is_empty() {
        eprintln!(
            "[ocr] warning: page(s) {:?} reached the output limit and are incomplete",
            truncated
        );
    }

    serde_json::to_string_pretty(&result).map_err(|e| MyError::ToJsonStirngError{uuid: "".to_string(), error: e})
}

/// 上传阶段错误以 HTTP 状态码返回，翻译成可读原因
fn describe_http_error(status: u16, body: &str) -> String {
    let hint = match status {
        400 => "invalid form or purpose, or the file cannot be processed".to_string(),
        402 => "insufficient balance".to_string(),
        408 => "upload too slow: must finish within 120s and must not pause longer than 30s".to_string(),
        413 => "file is too large: max 100 MiB".to_string(),
        429 => "too many concurrent recognitions (max 15 per account), retry later".to_string(),
        500 => "file upload or recognition submission failed".to_string(),
        503 => "the ocr service is temporarily unavailable, retry later".to_string(),
        504 => "upload and submission exceeded the 180s limit".to_string(),
        _ => "unexpected error".to_string(),
    };
    let detail = body.trim();
    if detail.is_empty() {
        format!("DeepSeek OCR API error {status}: {hint}")
    } else {
        format!("DeepSeek OCR API error {status}: {hint}; body: {}", truncate(detail, 500))
    }
}

/// 截断超长字符串，避免把整个文件内容塞进错误信息
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}…")
    }
}

