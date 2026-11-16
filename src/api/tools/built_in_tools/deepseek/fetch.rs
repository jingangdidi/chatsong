use serde::{Deserialize, Serialize};
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    }
};

/// params for deepseek fetch
/// https://api-docs.deepseek.com/zh-cn/guides/fetch/
#[derive(Deserialize)]
struct Params {
    url: String, // 要抓取的网页地址，长度不超过 8192，必须为含 host 的 HTTP/HTTPS URL，且不能携带用户名或密码
}

/// built-in tool
pub struct DeepseekFetch;

impl DeepseekFetch {
    /// new
    pub fn new() -> Self {
        DeepseekFetch
    }
}

impl BuiltIn for DeepseekFetch {
    /// get tool name
    fn name(&self) -> String {
        "fetch_url".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Fetch a single web page and return its extracted content as Markdown. Use this to retrieve the full body of a specific URL discovered via Search or otherwise known to you. Returns an object with `title` (page title, empty string if missing) and `content` (extracted Markdown body). Fetch results depend on the target site's availability and access policy; some pages may fail with HTTP 404 (unavailable, blocked, unsupported content type, or no extractable Markdown).".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The web page URL to fetch. Must be an absolute HTTP/HTTPS URL with a host, must not contain a username or password, and must be no longer than 8192 characters.",
                },
            },
            "required": ["url"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        if PARAS.deepseek_search_api_key.is_empty() {
            return Err(MyError::OtherError{info: "no deepseek api key for fetch".to_string()})
        }

        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;

        validate_fetch_url(&params.url)?;

        Ok((params.url, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        if is_en {
            Ok(Some(format!("Do you allow calling the fetch_url tool?{}", info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 fetch_url 工具？{}", info.unwrap_or_default())))
        }
    }
}

/// 按文档约束校验 URL：长度 ≤ 8192、http/https、含 host、无 username/password
fn validate_fetch_url(raw: &str) -> Result<(), MyError> {
    if raw.chars().count() > 8192 {
        return Err(MyError::OtherError{info: "fetch url must be at most 8192 characters".to_string()})
    }
    if !raw.starts_with("http") && !raw.starts_with("https") {
        return Err(MyError::OtherError{info: "fetch url scheme must be http or https".to_string()})
    }
    Ok(())
}

#[derive(Serialize)]
struct FetchBody<'a> {
    url: &'a str,
}

/// 响应2个字段 title: 页面标题，缺失时为空字符串；content: 抓取后的 Markdown 正文。示例:
/// {
///     "title": "Example article",
///     "content": "# Example article\n\nArticle content."
/// }
#[derive(Debug, Serialize, Deserialize)]
struct FetchResult {
    title: String,
    content: String,
}

const FETCH_URL: &str = "https://api.deepseek.com/v1/tools/fetch";

/// 调用 deepseek fetch 将指定 url 提取为 md
/// 请求示例: curl https://api.deepseek.com/v1/tools/fetch -H "Authorization: Bearer <DeepSeek API Key>" -H "Content-Type: application/json" -d '{"url": "https://example.com/article"}'
pub async fn deepseek_fetch(url: &str) -> Result<String, MyError> {
    let body = FetchBody { url };

    let resp = reqwest::Client::new()
        .post(FETCH_URL)
        .bearer_auth(&PARAS.deepseek_search_api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| MyError::SendRequestError{url: FETCH_URL.to_string(), error: e})?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(MyError::OtherError{info: format!("DeepSeek Tools API error {status}: {body}")})
    }

    let parsed: FetchResult = resp
        .json()
        .await
        .map_err(|e| MyError::OtherError{info: format!("failed to decode fetch response: {}", e)})?;

    serde_json::to_string_pretty(&parsed).map_err(|e| MyError::ToJsonStirngError{uuid: "".to_string(), error: e})
}
