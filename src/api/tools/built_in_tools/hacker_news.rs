use std::fs::File;
use std::io::Write;

use scraper::{Html, Selector, ElementRef};
use url::Url;
use html_escape::decode_html_entities;
use tokio::time::{sleep, Duration};
use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use openai_dive::v1::{
    api::Client,
    resources::chat::{
        ChatCompletionParametersBuilder,
        ChatMessage,
        ChatMessageContent,
        ChatCompletionResponseFormat,
    },
};
use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    error::MyError,
    tools::{
        built_in_tools::BuiltIn,
        call_llm,
    },
};

/// params for integer read_file
#[derive(Deserialize)]
struct Params {
    #[serde(default)]
    save_html: bool,
}

/// Hacker News
pub struct HackerNews;

impl HackerNews {
    /// new
    pub fn new() -> Self {
        HackerNews
    }
}

impl BuiltIn for HackerNews {
    /// get tool name
    fn name(&self) -> String {
        "hacker_news".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Retrieves the titles and comment summaries for Hacker News articles. For each article, the tool returns its title, and a condensed summary of the community comments. The results can optionally be saved as a standalone HTML file for offline review.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "save_html": {
                    "type": ["boolean", "null"],
                    "description": "If true, saves the output as hacker_news_summaries.html in the current working directory. Default is false.",
                },
            },
            "required": [],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        Ok((if params.save_html { "true".to_string() } else { "false".to_string() }, None))
    }

    /// get approval message
    fn get_approval(&self, _args: &str, _info: Option<String>, _is_en: bool) -> Result<Option<String>, MyError> {
        Ok(None)
    }
}

const SUMMARY_PROMPT: &str = r###"You are given all the comments from a Hacker News article. Please provide a concise summary of these comments. Your summary should:
- Highlight the main themes or topics discussed
- Capture the overall sentiment (positive, negative, mixed, or neutral)
- Mention any notable or frequently repeated opinions, suggestions, or criticisms
- Point out any disagreements or diverse viewpoints
- Keep the summary brief, objective, and informative (around 100–200 words)

Here are the comments:
"###;

/// 爬取 hacker news 的文章标题、链接、comment总结
pub async fn hacker_news_summaries(uuid: &str, save_html: bool, model: &str) -> Result<String, MyError> {
    let base_url = Url::parse("https://news.ycombinator.com").map_err(|e| MyError::UrlParseError{error: e})?;

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(30))
        .build().map_err(|e| MyError::OtherError{info: format!("reqwest builder error: {:?}", e)})?;

    // 获取首页
    let resp = client.get("https://news.ycombinator.com/news").send().await.map_err(|e| MyError::SendRequestError{url: "https://news.ycombinator.com/news".to_string(), error: e})?;
    let body = resp.text().await.map_err(|e| MyError::GetResponseTextError{url: "https://news.ycombinator.com/news".to_string(), error: e})?;
    let articles: Vec<(String, Url, Option<String>)> = {
        let document = Html::parse_document(&body);

        let tr_selector = Selector::parse("tr").map_err(|e| MyError::SelectorParseError{info: format!("Selector parse error: {:?}", e)})?;
        let trs: Vec<ElementRef> = document.select(&tr_selector).collect();

        let mut articles: Vec<(String, Url, Option<String>)> = Vec::new();
        let mut i = 0;
        while i < trs.len() {
            let tr = &trs[i];
            if let Some(class) = tr.value().attr("class") {
                if class.contains("athing") {
                    let title_selector = Selector::parse("td.title > span.titleline > a").map_err(|e| MyError::SelectorParseError{info: format!("Selector parse error: {:?}", e)})?;
                    let title = tr
                        .select(&title_selector)
                        .next()
                        .map(|a| a.text().collect::<String>())
                        .unwrap_or_default();
                    let href = tr
                        .select(&title_selector)
                        .next()
                        .and_then(|a| a.value().attr("href"))
                        .unwrap_or("");
                    let full_url = base_url.join(href).unwrap_or_else(|_| base_url.clone());

                    let comment_url = if i + 1 < trs.len() {
                        let next_tr = &trs[i + 1];
                        let subtext_sel = Selector::parse("td.subtext").map_err(|e| MyError::SelectorParseError{info: format!("Selector parse error: {:?}", e)})?;
                        if let Some(subtext_td) = next_tr.select(&subtext_sel).next() {
                            let a_sel = Selector::parse("a").map_err(|e| MyError::SelectorParseError{info: format!("Selector parse error: {:?}", e)})?;
                            subtext_td
                                .select(&a_sel)
                                .find(|a| {
                                    let text = a.text().collect::<String>().to_lowercase();
                                    text.contains("comment") || text.contains("discuss")
                                })
                                .and_then(|a| a.value().attr("href"))
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    articles.push((title, full_url, comment_url));
                    i += 2;
                    continue;
                }
            }
            i += 1;
        }

        if articles.is_empty() {
            return Err(MyError::OtherError{info: "get hacker news failed".to_string()})
        }

        event!(Level::INFO, "{} total hacker news articles: {}", uuid, articles.len());
        articles
    };

    let mut summaries: Vec<Option<String>> = Vec::new();
    let delay = Duration::from_secs_f64(2.0);

    for (idx, (_, _, comment_url)) in articles.iter().enumerate() {
        event!(Level::INFO, "{} parse article {} ...", uuid, idx+1);

        let summary = match comment_url {
            Some(rel) => {
                let abs_url = base_url.join(rel).unwrap_or(base_url.clone());
                let mut retries = 0;
                let max_retries = 3;

                loop {
                    // 随机等待 1~2 秒
                    sleep(delay).await;

                    let result = client.get(abs_url.as_str()).send().await;
                    match result {
                        Ok(resp) => {
                            let status = resp.status();
                            if status == 503 || status == 429 {
                                retries += 1;
                                if retries > max_retries {
                                    event!(Level::INFO, "{} {}. skip {}", uuid, idx+1, abs_url);
                                    break None;
                                }
                                let wait = Duration::from_secs(retries as u64 * 2);
                                event!(Level::INFO, "{} {}. HTTP {} for {}, try again ({}/{})", uuid, idx+1, status, abs_url, retries, max_retries);
                                sleep(wait).await;
                                continue;
                            }
                            if !status.is_success() {
                                eprintln!("   {}. HTTP {} for {}，跳过", idx+1, status, abs_url);
                                event!(Level::INFO, "{} {}. HTTP {} for {}, skip", uuid, idx+1, status, abs_url);
                                break None;
                            }
                            let body = match resp.text().await {
                                Ok(b) => b,
                                Err(e) => {
                                    event!(Level::INFO, "{} {}. body failed: {}: {:?}", uuid, idx+1, abs_url, e);
                                    break None;
                                }
                            };
                            if !body.contains("commtext") {
                                event!(Level::INFO, "{} {}. no comment: {}", uuid, idx+1, abs_url);
                                break None;
                            }
                            let doc = Html::parse_document(&body);
                            let selector = Selector::parse("div.commtext").map_err(|e| MyError::SelectorParseError{info: format!("Selector parse error: {:?}", e)})?;
                            let comments: Vec<String> = doc
                                .select(&selector)
                                .map(|div| {
                                    let raw = div.text().collect::<String>();
                                    decode_html_entities(&raw).to_string()
                                })
                                .collect();
                            if comments.is_empty() {
                                event!(Level::INFO, "{} {}. comment is empty: {}", uuid, idx+1, abs_url);
                                break None;
                            } else {
                                let numbered: Vec<String> = comments
                                    .iter()
                                    .enumerate()
                                    .map(|(i, c)| format!("[comment {}] \n{}", i + 1, c))
                                    .collect();
                                break Some(numbered.join("\n\n"));
                            }
                        }
                        Err(e) => {
                            retries += 1;
                            if retries > max_retries {
                                event!(Level::INFO, "{} {}. request failed: {}: {}", uuid, idx+1, abs_url, e);
                                break None;
                            }
                            let wait = Duration::from_secs(retries as u64 * 2);
                            event!(Level::INFO, "{} {}. request failed: {}: {}, try again ({}/{})", uuid, idx+1, abs_url, e, retries, max_retries);
                            sleep(wait).await;
                        }
                    }
                }
            }
            None => None,
        };

        summaries.push(summary);
    }

    // 生成 HTML 文件
    if save_html {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"zh-CN\">\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str("<title>Hacker News – 标题与评论内容</title>\n");
        html.push_str("<style>\n");
        html.push_str("table { border-collapse: collapse; width: 100%; word-break: break-word; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; vertical-align: top; }\n");
        html.push_str("tr:nth-child(even) { background-color: #f2f2f2; }\n");
        html.push_str("th { background-color: #ff6600; color: white; }\n");
        html.push_str("a { text-decoration: none; color: #000; }\n");
        html.push_str("a:hover { text-decoration: underline; }\n");
        html.push_str("</style>\n</head>\n<body>\n");
        html.push_str("<h1>Hacker News 首页文章（含评论内容）</h1>\n");
        html.push_str("<table>\n");
        html.push_str("<tr><th>标题</th><th>评论详情</th></tr>\n");

        for ((title, url, _), summary_opt) in articles.iter().zip(summaries.iter()) {
            let escaped_title = html_escape::encode_text(title);
            let summary = match summary_opt {
                Some(s) => run_single_llm(uuid, &s, model).await?,
                None => "no comment".to_string(),
            };
            let escaped_summary = html_escape::encode_text(&summary);
            html.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td style=\"white-space: pre-wrap;\">{}</td></tr>\n",
                url, escaped_title, escaped_summary
            ));
        }

        html.push_str("</table>\n</body>\n</html>");

        let mut file = File::create("hacker_news_summaries.html")?;
        file.write_all(html.as_bytes())?;

        event!(Level::INFO, "{} save hacker_news_summaries.html successfully ({} articles)", uuid, articles.len());
        Ok("save hacker news to hacker_news_summaries.html successfully".to_string())
    } else {
        // Markdown 表格
        let mut content = "Here is the markdown table summarizing the comments for each article:\n| Title | Summary |\n|-------|---------|\n".to_string();
        for ((title, url, _), summary_opt) in articles.iter().zip(summaries.iter()) {
            let escaped_title = title.replace('|', "\\|");
            let summary = match summary_opt {
                Some(s) => run_single_llm(uuid, &s, model).await?,
                None => "no comment".to_string(),
            };
            let escaped_summary = summary
                .replace('|', "\\|")
                .replace('\n', "<br>");
            content += &format!("| [{}]({}) | {} |\n", escaped_title, url, escaped_summary);
        }
        if PARAS.english {
            content += "Display each article's summary one by one.";
        } else {
            content += "展示每个文章的总结（不需要再次总结），并把标题翻译为中文";
        }

        Ok(content)
        //Ok("get hacker news successfully".to_string())
    }
}

/// 单词调用LLM
async fn run_single_llm(uuid: &str, content: &str, m: &str) -> Result<String, MyError> {
    // 根据模型名称获取(api_key, endpoint, 模型名称, 是否支持深度思考)
    let (api_key, endpoint, model, _) = PARAS.api.get_model_by_name(m)?;
    // 使用api key初始化
    let mut client = Client::new(api_key.clone());
    client.set_base_url(&endpoint); // 从0.7.0开始舍弃了new_with_base
    let mut para_builder = ChatCompletionParametersBuilder::default();
    para_builder.model(&model); // 指定模型，例如：Gpt4Engine::Gpt4O.to_string()
    para_builder.response_format(ChatCompletionResponseFormat::Text);
    //para_builder.stream(stream); // 这里不需要设置，调用`create_stream`时会设置
    let lowercase_model = model.to_lowercase();
    // 关闭思考，不同模型思考的设置不同
    if lowercase_model.starts_with("deepseek") {
        // deepseek: https://api-docs.deepseek.com/
        para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
    } else if lowercase_model.starts_with("qwen") {
        if endpoint.starts_with("http://") { // local model
            // https://modelscope.cn/models/Qwen/Qwen3.5-397B-A17B
            // https://modelscope.cn/models/Qwen/Qwen3.6-35B-A3B#instruct-or-non-thinking-mode
            para_builder.extra_body(json!({"chat_template_kwargs": {"enable_thinking": false}}));
        } else {
            // Qwen: https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions#05cfceb898csa
            para_builder.extra_body(json!({"enable_thinking": false}));
        }
    } else if lowercase_model.starts_with("kimi") {
        // kimi: https://platform.kimi.com/docs/api/models-overview
        para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
    } else if lowercase_model.starts_with("glm") {
        // glm: https://docs.bigmodel.cn/cn/guide/develop/openai/introduction
        para_builder.extra_body(json!({"thinking": {"type": "disabled"}}));
    } else if lowercase_model.starts_with("minimax") {
        // minimax，目前不支持关闭thinking：https://github.com/MiniMax-AI/MiniMax-M2/issues/68
        //para_builder.extra_body(json!({"reasoning_split": false}));
    }
    let summary_prompt = ChatMessage::User{
        content: ChatMessageContent::Text(format!("{}{}{}", SUMMARY_PROMPT, content, if PARAS.english { "" } else { "\n使用中文总结" })),
        name: None,
    };
    call_llm(vec![summary_prompt], uuid.to_string(), client, para_builder, &model).await
}
