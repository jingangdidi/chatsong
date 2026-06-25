use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

//! 极简版记忆体
//!
//! 1. 每轮任务开始前，根据“当前用户问题”检索相关记忆，注入模型上下文
//! 2. 每轮任务结束后，由用户决定是否提取对话记忆进行存储
//! 3. agent 时，把 `SimpleMemory` 直接序列化成 JSON 保存到本地，下次直接导入

pub static MEMORY: Lazy<Mutex<SimpleMemory>> = Lazy::new(|| Mutex::new(SimpleMemory::new()));

/// 是否需要记忆
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryDecision {
    Remember, // 允许从本次任务中抽取记忆
    Skip, // 跳过本次任务的记忆写入
}

/// 要提取记忆的原始对话内容
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Messages {
    messages: Vec<String>,
}

impl Messages {
    /// 用第一条用户消息创建
    pub fn new_by_user(user_message: &str) -> Self {
        let mut task = Self::default();
        task.user(user_message);
        task
    }

    /// 记录用户消息
    pub fn user(&mut self, text: &str) {
        self.messages.push(format!("User: {}", text));
    }

    /// 记录助手消息
    pub fn assistant(&mut self, text: &str) {
        self.messages.push(format!("Assistant: {}", text));
    }

    /// 记录工具调用和输出
    pub fn tool(&mut self, name: &str, text: &str) {
        self.messages
            .push(format!("Tool({}): {}", name, text));
    }

    /// 获取完整信息，换行间隔，用于提取记忆
    pub fn as_text(&self) -> String {
        self.messages.join("\n")
    }
}

/// 从对话中提取的一条记忆
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryNote {
    pub text: String,
}

/// 一条和当前问题相关的记忆命中结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelevantMemory {
    pub note: MemoryNote, // 被命中的原始记忆
    pub score: usize, // 简单相关性分数。分数越高，越应该优先注入模型
}

/// 从对话中抽取记忆
/// 这里只是示例，将用户最后一条消息作为提取的记忆，实际应该让LLM提取
fn extract_memory(&self, transcript: &str) -> Option<String> {
    transcript
        .lines()
        .rev()
        .find(|line| line.starts_with("User: "))
        .map(|line| line.trim_start_matches("User: ").trim().to_string())
        .filter(|line| !line.is_empty())
}

/// 根据已有 notes 重建短 summary
/// 调用LLM把当前所有记忆一起总结压缩，这里只是示例
fn summarize(&self, notes: &[MemoryNote]) -> String {
    if notes.is_empty() {
        return String::new();
    }

    let mut summary = String::from("可用记忆摘要：\n");
    for note in notes.iter().rev().take(8) {
        summary.push_str("- ");
        summary.push_str(&note.text);
        summary.push('\n');
    }
    summary
}

/// 极简记忆体
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleMemory {
    summary:   String, // 当前所有 notes 总结的记忆内容，下次读取记忆时，直接将这个字符串注入 prompt
    notes:     Vec<MemoryNote>, // 从每个对话提取的记忆正文
    max_notes: usize, // notes 的数量上限
}

impl SimpleMemory {
    /// 创建一个空记忆体
    pub fn new(max_notes: usize) -> Self {
        Self {
            summary: String::new(),
            notes: Vec::new(),
            max_notes: max_notes.max(1),
        }
    }

    /// 把当前记忆体保存为 JSON 文件
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), MyError> {
        let json = serde_json::to_string_pretty(self).map_err(json_error)?;
        fs::write(path, json)
    }

    /// 从 JSON 文件恢复记忆体
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, MyError> {
        let json = fs::read_to_string(path)?;
        let mut memory = serde_json::from_str::<Self>(&json).map_err(json_error)?;

        // 读取旧文件或手写 JSON 时做一次轻量修正，避免 max_notes 为 0
        memory.max_notes = memory.max_notes.max(1);
        memory.trim_old_notes();
        Ok(memory)
    }

    /// 舍弃旧记忆
    fn trim_old_notes(&mut self) {
        if self.notes.len() > self.max_notes {
            let overflow = self.notes.len() - self.max_notes;
            self.notes.drain(0..overflow);
        }
    }

    /// 任务结束时调用：根据明确决策决定是否学习
    pub fn finish_task(&mut self, msg: Messages, decision: MemoryDecision) {
        if decision != MemoryDecision::Skip {
            let transcript = msg.as_text();
            let Some(text) = extract_memory(&transcript) else {
                return;
            };

            self.notes.push(MemoryNote { text });
            self.trim_old_notes();
            self.summary = summarize(&self.notes);
        }
    }

    /// 手动写入一条记忆
    ///
    /// 用户显式说“记住这个”时，可以直接调用这个方法，而不必走 transcript 抽取。
    pub fn remember(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.notes.push(MemoryNote { text });
        self.trim_old_notes();
        self.summary = summarize(&self.notes);
    }

    /// 按子串搜索 notes
    pub fn search(&self, query: &str) -> Vec<&MemoryNote> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        self.notes
            .iter()
            .filter(|note| note.text.to_lowercase().contains(&query))
            .collect()
    }

    /// 查看所有 notes
    pub fn notes(&self) -> &[MemoryNote] {
        &self.notes
    }

    /// 根据当前问题检索最相关的 notes 记忆
    ///
    /// 这个实现没有依赖向量库或分词库，只做一个很小的相关性打分：
    ///
    /// - 完整 query 出现在 note 中：高分
    /// - 英文/数字词重合：加分
    /// - 中文连续片段的 bigram 或单字重合：加分
    /// - 分数相同：越新的 note 越靠前
    ///
    /// 复杂项目里可以把这里替换成 BM25、tantivy、SQLite FTS、embedding 检索或混合检索
    pub fn search_relevant(&self, query: &str, limit: usize) -> Vec<RelevantMemory> {
        if limit == 0 || query.trim().is_empty() {
            return Vec::new();
        }

        let mut scored = self
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let score = score_note(query, &note.text);
                (score > 0).then_some((index, RelevantMemory { note, score }))
            })
            .collect::<Vec<_>>();

        scored.sort_by(|(left_index, left), (right_index, right)| {
            right
                .score
                .cmp(&left.score) // score 大的排前面
                .then_with(|| right_index.cmp(left_index)) // score 相同则最新的记忆排前面
        });

        scored.into_iter().take(limit).map(|(_, hit)| hit).collect()
    }

    /// 对话开始前，获取所有记忆的总结，这里没有根据相关性筛选，是全部记忆的一个总结
    pub fn prompt(&self) -> Option<String> {
        let summary = self.summary.trim();
        if summary.is_empty() {
            return None;
        }

        Some(format!(
            "## 记忆\n\
             以下是过去任务留下的短摘要。仅在相关时使用；如果事实可能过期，请重新验证。\n\n\
             {summary}"
        ))
    }

    /// 根据当前问题生成要注入模型的记忆 prompt
    ///
    /// 这是推荐在 agent loop 开始时调用的方法
    /// 它会先根据 `current_query` 检索相关 notes，然后只注入前 `max_hits` 条记忆
    pub fn prompt_for(&self, current_query: &str, max_hits: usize) -> Option<String> {
        let summary = self.summary.trim();
        let hits = self.search_relevant(current_query, max_hits);

        if summary.is_empty() && hits.is_empty() {
            return None;
        }

        let mut prompt = String::from(
            "## 记忆\n\
             以下内容来自过去任务。只在和当前问题相关时使用；如果和当前问题冲突，以当前用户消息为准。\n",
        );

        if !summary.is_empty() {
            prompt.push_str("\n### 全局状态\n");
            prompt.push_str(summary);
            prompt.push('\n');
        }

        if !hits.is_empty() {
            prompt.push_str("\n### 与当前问题相关的记忆\n");
            for hit in hits {
                prompt.push_str("- ");
                prompt.push_str(&hit.note.text);
                prompt.push('\n');
            }
        }

        Some(prompt)
    }
}

/// 计算问题与记忆的相关性分数
fn score_note(query: &str, note: &str) -> usize {
    let query = query.trim().to_lowercase();
    let note = note.to_lowercase();
    if query.is_empty() || note.is_empty() {
        return 0;
    }

    let mut score = 0;
    if note.contains(&query) { // 问题完全包含在记忆中
        score += 100;
    }

    let note_terms = tokenize(&note).into_iter().collect::<HashSet<_>>();
    let query_terms = tokenize(&query).into_iter().collect::<HashSet<_>>();

    for term in query_terms {
        if note_terms.contains(&term) || note.contains(&term) {
            score += term_weight(&term);
        }
    }

    score
}

/// 单字符1分，多字符4分
fn term_weight(term: &str) -> usize {
    if term.chars().count() >= 2 {
        4
    } else {
        1
    }
}

/// 对字符串进行分词，支持中英文
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut latin = String::new();
    let mut cjk_run = Vec::new();

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            flush_cjk(&mut cjk_run, &mut tokens);
            latin.push(ch.to_ascii_lowercase());
        } else if is_cjk(ch) {
            flush_latin(&mut latin, &mut tokens);
            cjk_run.push(ch);
        } else {
            flush_latin(&mut latin, &mut tokens);
            flush_cjk(&mut cjk_run, &mut tokens);
        }
    }

    flush_latin(&mut latin, &mut tokens);
    flush_cjk(&mut cjk_run, &mut tokens);
    tokens
}

/// 英文字符
fn flush_latin(latin: &mut String, tokens: &mut Vec<String>) {
    if latin.chars().count() >= 2 {
        tokens.push(std::mem::take(latin));
    } else {
        latin.clear();
    }
}

/// 中文字符
fn flush_cjk(cjk_run: &mut Vec<char>, tokens: &mut Vec<String>) {
    match cjk_run.len() {
        0 => {}
        1 => tokens.push(cjk_run[0].to_string()),
        _ => {
            for pair in cjk_run.windows(2) {
                tokens.push(format!("{}{}", pair[0], pair[1]));
            }
            for ch in cjk_run.iter() {
                tokens.push(ch.to_string());
            }
        }
    }

    cjk_run.clear();
}

/// 判断是否中文字符
fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
    )
}
