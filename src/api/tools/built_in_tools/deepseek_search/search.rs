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

/// params for deepseek search
/// https://api-docs.deepseek.com/zh-cn/guides/search/
/// +---------+
/// | site:   | 将结果限制在指定站点，指定多个时用空格分隔并通过 OR 连接，例如 `DeepSeek site:github.com OR site:huggingface.co`
/// | -site:  | 排除指定站点，指定多个时用空格分隔，例如 `DeepSeek -site:example.com`
/// | -       | 排除包含指定关键词的网页，指定多个时用空格分隔，例如 `DeepSeek -广告`
/// | -"..."  | 排除包含指定短语的网页，短语内可以包含空格，指定多个时用空格分隔，例如 `DeepSeek -"release notes"`
/// | "..."   | 精确匹配指定短语，指定多个时用空格分隔，例如 `DeepSeek "精确短语"`
/// | lang:   | 将结果限制在指定语言，多种语言用逗号分隔，例如 `DeepSeek lang:zh,en`
/// | after:  | 将结果限制在指定日期之后发布的网页，例如 `DeepSeek after:2025` 或 `DeepSeek after:2025-01-01`
/// | before: | 将结果限制在指定日期之前发布的网页，例如 `DeepSeek before:2026` 或 `DeepSeek before:2026-06-01`
/// +---------+
#[derive(Deserialize)]
struct Params {
    query: String, // 搜索词，1～128 个 Unicode 字符，可以在一个 query 中组合多个操作符
    #[serde(default)]
    count: Option<u32>, // 返回条数，1～50，默认 10
    //ori_question: String, // 触发搜索的用户原始问题，1～128 个字符，有助于更好地理解搜索意图，传 null 视为未提供
    //localization: String, // 用于优先返回本地化搜索结果的 IP 地址
    //need_content: bool,   // 是否在 content 中返回网页正文，为 false 时 content 恒为空字符串，默认 false
}

/// built-in tool
pub struct DeepseekSearch;

impl DeepseekSearch {
    /// new
    pub fn new() -> Self {
        DeepseekSearch
    }
}

impl BuiltIn for DeepseekSearch {
    /// get tool name
    fn name(&self) -> String {
        "web_search".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Search the web by query and return structured search results powered by DeepSeek's own search engine. Each result includes a title, URL, snippet, long summary, site name/icon, publish date, and authority score. Use this to find up-to-date web information for Agent or RAG workflows.

The query supports search operators:
- `site:example.com` restrict to site (combine multiple with OR: `site:a.com OR site:b.com`)
- `-site:example.com` exclude a site
- `-keyword` exclude pages containing the keyword
- `-\"phrase\"` exclude pages containing the exact phrase
- `\"phrase\"` require exact phrase match
- `lang:zh,en` restrict result language(s), comma-separated
- `after:2025` or `after:2025-01-01` only pages published after the date
- `before:2026` or `before:2026-06-01` only pages published before the date

Returns a JSON object with a `results` array (empty if no matches). Each item contains: `rerank_score` (0–3), `title`, `url`, `snippet`, `summary`, `content`, `site_name`, `site_icon`, `date` (RFC 3339 UTC or null), and `authority` (0–4). Results with `rerank_score >= 1.5` and `authority >= 3` are high-quality/authoritative.
".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query (1-128 Unicode characters, cannot be all whitespace). Supports search operators such as site:, -site:, -, -\"...\", \"...\", lang:, after:, before:.",
                },
                "count": {
                    "type": ["integer", "null"],
                    "description": "Number of results to return, between 1 and 50. Defaults to 10.",
                },
            },
            "required": ["query"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        if PARAS.deepseek_search_api_key.is_empty() {
            return Err(MyError::OtherError{info: "no deepseek api key for search".to_string()})
        }

        //let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let params: Params = parse_tool_args(args, ArgFixSpec{ array_fields: None, object_fields: None })?;

        let query = params.query.trim();
        if query.is_empty() {
            return Err(MyError::OtherError{info: "search query must not be blank".to_string()})
        }

        if query.chars().count() > 128 {
            return Err(MyError::OtherError{info: "search query must be at most 128 characters".to_string()})
        }

        let count = match params.count {
            Some(c) => c.clamp(1, 50),
            None => 10,
        };

        Ok((format!("{}---srx---{}", query, count), None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        if is_en {
            Ok(Some(format!("Do you allow calling the web_search tool?{}", info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 web_search 工具？{}", info.unwrap_or_default())))
        }
    }
}

#[derive(Serialize)]
struct SearchBody<'a> {
    query: &'a str,
    count: u8,
}

/// 请求成功时返回 HTTP 200 与 results 数组。搜索正常结束但没有匹配结果时，返回空数组。响应示例:
/// {
///     "results": [
///         {
///             "rerank_score": 2.4,
///             "title": "DeepSeek V4 模型发布",
///             "url": "https://example.com/deepseek-v4",
///             "snippet": "DeepSeek V4 已上线，支持工具调用与长上下文。",
///             "summary": "DeepSeek 发布了 V4 模型。此次发布改进了工具调用与长上下文能力，适合 Agent 工作流。",
///             "content": "DeepSeek V4 现已正式上线……",
///             "site_name": "Example News",
///             "site_icon": "https://example.com/favicon.ico",
///             "date": "2026-06-01T00:00:00+00:00",
///             "authority": 3
///         }
///     ]
/// }
#[derive(Debug, Serialize, Deserialize)]
struct SearchResult {
    rerank_score: f64,
    title: String,
    url: String,
    snippet: String,
    summary: String,
    content: String,
    site_name: Option<String>,
    site_icon: Option<String>,
    date: Option<String>,
    authority: i32,
}

#[derive(Debug, Deserialize)]
struct SearchApiResponse {
    results: Vec<SearchResult>,
}

const SEARCH_URL: &str = "https://api.deepseek.com/v1/tools/search";

/// 调用 deepseek search 进行网页搜索
/// 请求示例: curl https://api.deepseek.com/v1/tools/search -H "Authorization: Bearer <DeepSeek API Key>" -H "Content-Type: application/json" -d '{"query": "DeepSeek V4 模型 发布", "count": 5, "need_content": true}'
pub async fn deepseek_search(query: &str, count: u8) -> Result<String, MyError> {
    let body = SearchBody { query, count };

    let resp = reqwest::Client::new()
        .post(SEARCH_URL)
        .bearer_auth(&PARAS.deepseek_search_api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| MyError::SendRequestError{url: SEARCH_URL.to_string(), error: e})?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(MyError::OtherError{info: format!("DeepSeek Tools API error {status}: {body}")})
    }

    let parsed: SearchApiResponse = resp
        .json()
        .await
        .map_err(|e| MyError::OtherError{info: format!("failed to decode search response: {}", e)})?;

    serde_json::to_string_pretty(&parsed.results).map_err(|e| MyError::ToJsonStirngError{uuid: "".to_string(), error: e})
}
