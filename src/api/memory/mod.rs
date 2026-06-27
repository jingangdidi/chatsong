use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tracing::{event, Level};

use crate::{
    parse_paras::PARAS,
    error::MyError,
};

// 极简版记忆体
// 1. 每轮任务开始前，根据“当前用户问题”检索相关记忆，注入模型上下文
// 2. 每轮任务结束后，由用户决定是否提取对话记忆进行存储
// 3. agent 时，把 `SimpleMemory` 直接序列化成 JSON 保存到本地，下次直接导入

pub static MEMORY: Lazy<Mutex<HashMap<String, SimpleMemory>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 从对话中提取的一条记忆
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryNote {
    pub raw:     String, // 原始内容
    pub summary: String, // 提取的记忆
}

/// 一条和当前问题相关的记忆命中结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelevantMemory {
    pub note:  MemoryNote, // 被命中的原始记忆
    pub score: usize, // 简单相关性分数。分数越高，越应该优先注入模型
}

/// 极简记忆体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleMemory {
    notes:     Vec<MemoryNote>, // 从每个对话提取的记忆正文
    max_notes: usize, // notes 的数量上限
    path:      String, // 该记忆的文件存储路径
    #[serde(skip, default)]
    pub save:  bool, // 本次加载后是否有更新，没有更新则退出时不需要保存，保存和加载json文件时忽略该字段，加载时默认设为false
}

impl SimpleMemory {
    /// 创建一个空记忆体
    pub fn new(max_notes: usize, path: String) -> Self {
        Self {
            notes: Vec::new(),
            max_notes: max_notes.max(10), // 至少10条记忆
            path,
            save: false,
        }
    }

    /// 把当前记忆体保存为 JSON 文件
    pub fn save_to_file(&self) -> Result<(), MyError> {
        let json = serde_json::to_string_pretty(self).map_err(|e| MyError::ToJsonStirngError{uuid: "memory".to_string(), error: e})?;
        fs::write(&self.path, json).map_err(|e| MyError::WriteFileError{file: self.path.clone(), error: e})
    }

    /// 从 JSON 文件恢复记忆体
    pub fn load_from_file(file: &str) -> Result<Self, MyError> {
        let json = fs::read_to_string(file)?;
        let mut memory = serde_json::from_str::<Self>(&json).map_err(|e| MyError::ResponseTextToJsonError{error: e})?;
        memory.max_notes = memory.max_notes.max(10); // 至少10条记忆
        memory.trim_old_notes(false);
        Ok(memory)
    }

    /// 舍弃uuid的旧记忆，如果是 memory.json 中的记忆，则将旧记忆移到 memory_old.json 中
    fn trim_old_notes(&mut self, is_local: bool) -> Option<Vec<MemoryNote>> {
        if self.notes.len() > self.max_notes {
            let overflow = self.notes.len() - self.max_notes;
            let old = self.notes.drain(0..overflow);
            if is_local {
                Some(old.collect())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// 将指定 Vec<MemoryNote> 加入到记忆中
    pub fn append_memory(&mut self, notes: Vec<MemoryNote>) {
        self.notes.extend(notes);
        self.save = true;
    }

    /// 判断一条候选记忆是否已经存在
    /// 这里既检查规范化后的完全相同，也检查轻量 token 相似度
    fn is_duplicate_memory(&self, summary: &str) -> bool {
        self.notes.iter().any(|note| is_duplicate_note(&note.summary, summary))
    }

    /// 调用 LLM 抽提总结指定字符串作为记忆，返回超出容量的旧记忆
    /// text: 要提取记忆的原始内容
    /// model_for_memory: (api_key, endpoint, 模型名称, 是否支持深度思考)
    pub fn remember(&mut self, raw: String, summary: String, is_local: bool) -> Option<Vec<MemoryNote>> {
        if !summary.trim().is_empty() {
            if self.is_duplicate_memory(&summary) {
                // 重复的记忆，不添加
                None
            } else {
                // 添加新记忆
                self.notes.push(MemoryNote { raw, summary });
                self.save = true;
                self.trim_old_notes(is_local)
            }
        } else {
            None
        }
    }

    /// 获取所有记忆
    fn get_all_memory(&self) -> Option<String> {
        if self.notes.is_empty() {
            None
        } else {
            let mut prompt = "## Memory\nThe following content is from previous tasks. Use it only when relevant to the current issue; if there is a conflict with the current user message, the current user message takes precedence.\n".to_string();
            prompt.push_str("\n### all memories:\n");
            event!(Level::INFO, "got all memory:\n{}", self.notes.iter().map(|m| format!("- {}", m.summary)).collect::<Vec<_>>().join("\n"));
            for note in &self.notes {
                prompt.push_str("- ");
                prompt.push_str(&note.summary);
                prompt.push('\n');
            }
            Some(prompt)
        }
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
    fn search_relevant(&self, query: &str, limit: usize) -> Vec<RelevantMemory> {
        if limit == 0 || query.trim().is_empty() {
            return Vec::new();
        }

        let mut scored = self
            .notes
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let score = score_note(query, &note.summary);
                (score > 0).then_some((index, RelevantMemory { note: note.clone(), score }))
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

    /// 根据当前问题生成要注入模型的记忆 prompt
    /// 这是推荐在 agent loop 开始时调用的方法
    /// 它会先根据 `current_query` 检索相关 notes，然后只注入前 `max_hits` 条记忆
    fn relevant_memory_prompt(&self, current_query: &str, max_hits: usize) -> Option<String> {
        let hits = self.search_relevant(current_query, max_hits);
        if hits.is_empty() {
            None
        } else {
            let mut prompt = "## Memory\nThe following content is from previous tasks. Use it only when relevant to the current issue; if there is a conflict with the current user message, the current user message takes precedence.\n".to_string();
            prompt.push_str("\n### Memories related to the current issue:\n");
            event!(Level::INFO, "got memory:\n{}", hits.iter().map(|m| format!("- score: {}, memory: {}", m.score, m.note.summary)).collect::<Vec<_>>().join("\n"));
            for hit in hits {
                prompt.push_str("- ");
                prompt.push_str(&hit.note.summary);
                prompt.push('\n');
            }
            Some(prompt)
        }
    }
}

/// 判断相似度是否>=85%
fn is_duplicate_note(existing: &str, candidate: &str) -> bool {
    let existing = normalize_memory_text(existing);
    let candidate = normalize_memory_text(candidate);

    if existing.is_empty() || candidate.is_empty() {
        return false;
    }

    if existing == candidate {
        return true;
    }

    let existing_terms = tokenize(&existing).into_iter().collect::<HashSet<_>>();
    let candidate_terms = tokenize(&candidate).into_iter().collect::<HashSet<_>>();
    let smaller_len = existing_terms.len().min(candidate_terms.len());

    if smaller_len < 3 {
        return false;
    }

    let overlap = existing_terms.intersection(&candidate_terms).count();
    overlap * 100 >= smaller_len * 85
}

/// 去除标点和连续空格
fn normalize_memory_text(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;

    for ch in text.trim().to_lowercase().chars() {
        if ch.is_whitespace() {
            if !previous_was_space {
                normalized.push(' ');
                previous_was_space = true;
            }
            continue;
        }

        if is_ignored_punctuation(ch) {
            continue;
        }

        normalized.push(ch);
        previous_was_space = false;
    }

    normalized.trim().to_string()
}

/// 判断中英文标点
fn is_ignored_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch,
            '。' | '，'
                | '、'
                | '；'
                | '：'
                | '！'
                | '？'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '（'
                | '）'
                | '《'
                | '》'
                | '【'
                | '】'
        )
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

/// 获取相关记忆
pub fn get_relevant_memory(uuid: &str, query: &str, max_hits: usize, is_local: bool) -> Option<String> {
    let mut data = MEMORY.lock().unwrap();
    let key = if is_local {
        "local"
    } else {
        uuid
    };
    match data.get_mut(key) {
        Some(memory) => memory.relevant_memory_prompt(query, max_hits),
        None => {
            let memory_file = if is_local {
                format!("{}/memory.json", PARAS.memory_dir)
            } else if key == "old" {
                format!("{}/memory_old.json", PARAS.memory_dir)
            } else {
                format!("{}/{}/{}_memory.json", PARAS.outpath, key, key)
            };
            let memory_path = Path::new(&memory_file);
            if memory_path.exists() && memory_path.is_file() {
                match SimpleMemory::load_from_file(&memory_file) {
                    Ok(memory) => {
                        let result = memory.relevant_memory_prompt(query, max_hits);
                        data.insert(key.to_string(), memory);
                        result
                    },
                    Err(e) => {
                        event!(Level::ERROR, "load memory file ({}) error: {}", memory_file, e);
                        None
                    },
                }
            } else {
                event!(Level::WARN, "no memory in {}", key);
                None
            }
        },
    }
}

/// 获取所有记忆
pub fn get_all_memory(key: &str) -> String {
    let data = MEMORY.lock().unwrap();
    if let Some(memory) = data.get(key) {
        if let Some(m) = memory.get_all_memory() {
            return m
        }
    }
    "No memory".to_string()
}
