use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    //parse_paras::PARAS,
    error::MyError,
};

/*
提问json:
{
    "state": {
        "question": "解析下这个readme.md文件",
        "description": "Calculates the MD5 hash of one or more files. Returns the MD5 value for each file in the format 'MD5  filename'"
    },
    "questions": [
        {
            "id": "relevance",
            "type": "choice",
            "instructions": "description描述的工具功能，是否与question相关？",
            "criteria": {
                "relevant": "description描述的工具功能，与question相关，需要调用该工具",
                "possibly_relevant": "description描述的工具功能，可能会被用到",
                "not_relevant": "description描述的工具功能，与question无关，不会被用到"
            }
        }
    ]
}

回答 json:
{
    "model":"StartLux-Decision-0.8B",
    "answers":{
        "relevance":{
            "type":"choice",
            "choice":"relevant",
            "confidence":0.4834200158227987,
            "probabilities":{
                "relevant":0.4834200158227987,
                "possibly_relevant":0.23309645576782664,
                "not_relevant":0.28348352840937474
            }
        }
    },
    "usage":{
        "input_tokens":180,
        "output_tokens":0
    }
}
*/

/// 一个候选（工具 / 提示词 / skill）
#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl Item {
    pub fn new(id: String, name: String, description: String) -> Self {
        Self { id, name, description }
    }
}

#[derive(Serialize)]
struct QuestionRequest {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    instructions: String,
    criteria: Value,
}

#[derive(Serialize)]
struct DecideRequest {
    state: Value,
    questions: Vec<QuestionRequest>
}

#[derive(Deserialize, Default)]
struct DecideResponse {
    #[serde(default)]
    answers: HashMap<String, Answer>,
}

#[derive(Deserialize, Default)]
struct Answer {
    #[serde(default)]
    choice: String,
    #[serde(default)]
    probabilities: HashMap<String, f64>,
}

/// 题目：问什么 + 选项 + 哪个算命中
#[derive(Debug, Clone)]
pub struct Criteria {
    /// 如何选择，比如: description描述的工具功能，是否与question相关？
    pub instructions: String,
    /// (选项名, 选项说明)，例如: ("relevant", "description描述的工具功能，与question相关，需要调用该工具")
    pub options: Vec<(String, String)>,
    /// 视为"命中"的选项名，可多个，例如选择relevant和possibly_relevant
    pub accept: Vec<String>,
}

impl Criteria {
    pub fn new(instructions: String, options: &[(&str, &str)], accept: &[&str]) -> Self {
        Self {
            instructions,
            options: options.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
            accept: accept.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn options_json(&self) -> Value {
        let mut map = serde_json::Map::with_capacity(self.options.len());
        for (k, v) in &self.options {
            map.insert(k.clone(), Value::String(v.clone()));
        }
        Value::Object(map)
    }

    fn matches(&self, choice: &str) -> bool {
        self.accept.iter().any(|a| a == choice)
    }
}

/// 用 `question` 逐个判断 `items`，一次批量请求，返回顺序与 `items` 一致。
async fn evaluate(url: &str, question: &str, criteria: &Criteria, items: &[Item]) -> Result<Vec<(String, String, bool, f64)>, MyError> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|e| MyError::SendRequestError { url: url.to_owned(), error: e })?;

    let criteria_json = criteria.options_json();
    let reqs: Vec<DecideRequest> = items
        .iter()
        .map(|item| DecideRequest {
            state: json!({ "question": question, "description": item.description }),
            questions: vec![QuestionRequest {
                id: "relevance".into(),
                kind: "choice".into(),
                instructions: criteria.instructions.clone(),
                criteria: criteria_json.clone(),
            }],
        })
        .collect();

    let resp = client.post(url).json(&reqs).send().await.map_err(|e| MyError::SendRequestError { url: url.to_owned(), error: e })?;
    let status = resp.status();
    let body = resp.bytes().await.map_err(|e| MyError::OtherError { info: format!("read body failed: {e}") })?;
    if !status.is_success() {
        return Err(MyError::OtherError { info: format!("server returned HTTP {}", status.as_u16()) });
    }
    let resps: Vec<DecideResponse> = serde_json::from_slice(&body).map_err(|e| MyError::OtherError { info: format!("invalid decision response: {e}") })?;
    if resps.len() != items.len() {
        return Err(MyError::OtherError { info: format!("got {} responses for {} items", resps.len(), items.len()) });
    }

    Ok(items
        .iter()
        .zip(resps)
        .map(|(item, resp)| {
            let ans = resp.answers.get("relevance");
            let matched = ans.is_some_and(|a| criteria.matches(&a.choice));
            let score = ans.and_then(|a| a.probabilities.get(&a.choice).copied()).unwrap_or(0.0);
            (item.id.clone(), item.name.clone(), matched, score)
        })
        .collect())
}

/// 只要命中的，按分数降序
pub async fn evaluate_matched(url: &str, question: &str, criteria: &Criteria, items: &[Item]) -> Result<Vec<(String, String, f64)>, MyError> {
    let mut picked: Vec<_> = evaluate(url, question, criteria, items).await?
        .into_iter().filter(|p| p.2).map(|t| (t.0, t.1, t.3)).collect();
    picked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    Ok(picked)
}
