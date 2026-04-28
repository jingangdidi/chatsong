use reqwest::blocking::Client;
use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::BuiltIn,
    web::parse_html::parse_single_html_str,
};

/// params for get web page content
#[derive(Deserialize)]
struct Params {
    url: String,
}

/// built-in tool
pub struct GetWebPageContent;

impl GetWebPageContent {
    /// new
    pub fn new() -> Self {
        GetWebPageContent
    }
}

impl BuiltIn for GetWebPageContent {
    /// get tool name
    fn name(&self) -> String {
        "get_web_page_content".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "This tool allows the LLM to retrieve the content of a specified web page by providing a valid URL. The tool fetches the HTML content of the requested URL and returns it as a plain text response, enabling the LLM to analyze or summarize the page's content.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The full URL of the web page to be accessed. Must be a valid HTTP or HTTPS address (e.g., https://www.example.com).",
                },
            },
            "required": ["url"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let client = Client::new();
        let response = client.get(&params.url).send().map_err(|e| MyError::SendRequestError{url: params.url.clone(), error: e})?;
        let content = response.text().map_err(|e| MyError::GetResponseTextError{url: params.url, error: e})?;
        let clean_content = parse_single_html_str(&content, false)?;
        Ok((format!("successfully get web page content:\n{}", clean_content), None))
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        if is_en {
            Ok(Some(format!("Do you allow calling the get_web_page_content tool to visit {} ?{}", params.url, info.unwrap_or_default())))
        } else {
            Ok(Some(format!("是否允许调用 get_web_page_content 工具访问 {} ？{}", params.url, info.unwrap_or_default())))
        }
    }
}
