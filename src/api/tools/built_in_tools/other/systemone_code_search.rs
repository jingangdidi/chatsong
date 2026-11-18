//! Semantic code search tool (Jev-powered), extracted from the `jegrep` project
//! (https://github.com/can1357/jegrep).
//!
//! Given a natural-language description, searches a target project tree and
//! returns the most relevant files with the strongest matching line ranges
//! (`path:start-end`), each with a calibrated 0..1 score. Pipeline:
//!
//!   1. walk the tree (gitignore-chained; lockfiles, binaries and credential
//!      files are never listed or read),
//!   2. content-check every file: line-tagged file head + one `noul`
//!      ("relevant") and one `choice` ("where" heatmap over line ranges),
//!   3. report hits above the threshold, merging adjacent positive spans.
//!
//! The Jev model is reached through YOUR OWN local HTTP endpoint (the
//! `--endpoint local` shape of jegrep). Requests are sent directly: no API
//! key, no model id, no environment configuration. Wire protocol:
//!
//!   POST <JEV_URL>  Content-Type: application/json
//!   { "state": "<plain text: task/search/file/note + line-tagged content>",
//!     "questions": { "q0": { "type": "noul", "instructions": "…",
//!                            "criteria": {"true": "…", "false": "…"} },
//!                    "q1": { "type": "choice", "instructions": "…",
//!                            "criteria": { "R00": "lines 1-40", … } } } }
//!   -> 200
//!   { "answers": { "q0": {"type": "noul", "noul": 0.87},
//!                  "q1": {"type": "choice", "probabilities": {…}} },
//!     "usage": {"input_tokens": 0, "output_tokens": 0} }
//!
//! Endpoint: edit the `JEV_URL` constant below to your local server address.
//!
//! Extra crates required in Cargo.toml (besides serde / serde_json):
//!
//!   reqwest = { version = "0.12", features = ["blocking"] }
//!   ignore  = "0.4"

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    error::MyError,
    tools::{
        parse_tool_args,
        ArgFixSpec,
        built_in_tools::BuiltIn,
    },
};

// ---------------------------------------------------------------------------
// Tunables (defaults mirror jegrep's where applicable)
// ---------------------------------------------------------------------------

/// URL of your local Jev-compatible judge — edit to match your server.
const JEV_URL: &str = "http://127.0.0.1:8091/v1/systemone";
/// Relevance threshold for hits.
const DEFAULT_TAU: f64 = 0.2;
/// Heatmap line ranges per file (the `where` choice options).
const RANGES: usize = 12;
/// Bytes of each file sent for the content check.
const MAX_BYTES: usize = 131072;
/// Content checks in flight at once.
const PARALLEL: usize = 4;
/// Spans with p below this are not localized/merged into the report.
const SPAN_FLOOR: f64 = 0.1;
/// Walk cap: collected files.
const MAX_WALK_FILES: usize = 4000;

// ---------------------------------------------------------------------------
// Tool entry
// ---------------------------------------------------------------------------

/// `code_search` tool params.
#[derive(Deserialize)]
pub struct Params {
    /// Natural-language description of what to find.
    pub query: String,
    /// Root of the target project. Default: ".".
    #[serde(default = "default_path")]
    pub path: String,
    /// Relevance threshold in 0..1 for hits. Default 0.2.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub threshold: Option<f64>,
}

fn default_path() -> String {
    ".".to_string()
}

/// Accept numbers that arrive as JSON strings ("0.3") or booleans.
fn de_opt_f64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    match Option::<Value>::deserialize(d)? {
        None => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_f64()),
        Some(Value::String(s)) => Ok(s.trim().parse::<f64>().ok()),
        Some(Value::Bool(b)) => Ok(Some(if b { 1.0 } else { 0.0 })),
        Some(_) => Ok(None),
    }
}

/// Semantic code search powered by the Jev model.
pub struct CodeSearch;

impl CodeSearch {
    /// new
    pub fn new() -> Self {
        CodeSearch
    }
}

impl BuiltIn for CodeSearch {
    /// get tool name
    fn name(&self) -> String {
        "code_search".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Semantic code search powered by the Jev model. Given a natural-language description, \
         searches the target project tree and returns the most relevant files together with the \
         strongest matching line ranges as `path:start-end` references (scores in 0..1, best \
         first). File names and code excerpts are sent to the local Jev endpoint; \
         credential files (keys, .env, tokens) are never listed or sent."
            .to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural-language description of what to find, e.g. \"where do we verify JWT tokens?\".",
                },
                "path": {
                    "type": "string",
                    "description": "Root of the target project to search (absolute or relative). Default: \".\".",
                },
                "threshold": {
                    "type": "number",
                    "description": "Relevance threshold in 0..1 that a file must clear to be a hit. Default 0.2.",
                },
            },
            "required": ["query"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<(String, Option<String>), MyError> {
        let params: Params = parse_tool_args(
            args,
            ArgFixSpec { array_fields: None, object_fields: None },
        )?;

        let root_in = params.path.trim().replace('\\', "/");
        let root = Path::new(&root_in);
        if !root.exists() {
            return Err(MyError::FileNotExistError { file: root_in.clone() });
        }
        if params.query.trim().is_empty() {
            return Ok(("Error: `query` must not be empty.".to_string(), Some(root_in)));
        }
        if !root.is_dir() {
            return Ok((format!("Error: '{}' is not a directory.", root_in), Some(root_in)));
        }

        let report = run_search(&params, root);
        Ok((report, Some(root_in)))
    }

    /// get approval message
    fn get_approval(
        &self,
        args: &str,
        info: Option<String>,
        is_en: bool,
    ) -> Result<Option<String>, MyError> {
        // Return Ok(None) here to skip the confirmation prompt entirely.
        let params: Params = parse_tool_args(
            args,
            ArgFixSpec { array_fields: None, object_fields: None },
        )?;
        let root = info.unwrap_or_else(|| params.path.clone());
        Ok(Some(if is_en {
            format!(
                "Semantic search \"{}\" under '{}': file names and code excerpts will be sent to \
                 the Jev endpoint ({}) for relevance judging.",
                params.query, root, JEV_URL
            )
        } else {
            format!(
                "在 '{}' 下语义搜索 \"{}\"：文件名与代码片段将发送到 Jev 接口 ({}) 进行相关性判断。",
                root, params.query, JEV_URL
            )
        }))
    }
}

/// One localized line range with its relevance probability.
#[derive(Clone, Debug)]
struct HeatRange {
    start: usize,
    end:   usize,
    p:     f64,
}

/// A content-checked file.
struct Check {
    rel:         String,
    score:       f64,
    heat:        Vec<HeatRange>,
    total_lines: usize,
    truncated:   bool,
}

/// A file to content-check (the walker's output; cloned into worker threads).
#[derive(Clone)]
struct Job {
    path: PathBuf,
    rel:  String,
    size: u64,
}

/// Result of one content check, produced in the worker thread.
struct CheckOutcome {
    rel:    String,
    result: Result<Check, String>,
}

// ---------------------------------------------------------------------------
// The search pipeline
// ---------------------------------------------------------------------------

fn run_search(params: &Params, root: &Path) -> String {
    let query = params.query.trim();
    let tau = params.threshold.unwrap_or(DEFAULT_TAU).clamp(0.0, 1.0);

    let client = JevClient::new();

    // 1. walk -----------------------------------------------------------------
    let real_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let jobs = walk(&real_root, MAX_WALK_FILES);
    if jobs.is_empty() {
        return format!("no searchable files under {}\n", real_root.display());
    }

    // 2. content checks (relevance noul + where-choice heatmap) ---------------
    let mut checks: Vec<Check> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for group in jobs.chunks(PARALLEL) {
        let mut handles = Vec::new();
        for job in group {
            let c = client.clone();
            let j = job.clone();
            let q = query.to_string();
            handles.push(thread::spawn(move || check_file(&c, &j, &q)));
        }
        for h in handles {
            let outcome = match h.join() {
                Ok(o) => o,
                Err(_) => continue,
            };
            let CheckOutcome { rel, result } = outcome;
            match result {
                Ok(check) => checks.push(check),
                Err(_) => failures.push(rel),
            }
        }
    }

    build_report(query, &real_root, tau, &checks, &failures)
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

fn build_report(
    query: &str,
    root: &Path,
    tau: f64,
    checks: &[Check],
    failures: &[String],
) -> String {
    let mut hits: Vec<&Check> = checks.iter().filter(|c| c.score >= tau).collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.rel.cmp(&b.rel)));

    let mut out = String::new();
    if hits.is_empty() {
        let _ = writeln!(out, "Semantic search \"{query}\" → no hits · τ {tau:.2}");
    } else {
        let _ = writeln!(out, "Semantic search \"{query}\" → {} hit(s) · τ {tau:.2}", hits.len());
    }
    let _ = writeln!(out, "search root path: {}", root.display());

    if hits.is_empty() {
        let mut near: Vec<&Check> = checks.iter().filter(|c| c.score > 0.0).collect();
        near.sort_by(|a, b| b.score.total_cmp(&a.score));
        near.truncate(3);
        if !near.is_empty() {
            let _ = writeln!(out, "closest by content:");
            for c in near {
                let _ = writeln!(out, "\t{}\t{:.2}\t{}", c.rel, c.score, span_list(c));
            }
        }
    } else {
        let _ = writeln!(out, "file\tscore\tspan");
        for c in &hits {
            let _ = writeln!(out, "{}\t{:.2}\t{}", c.rel, c.score, span_list(c));
        }
    }

    if !failures.is_empty() {
        let _ = writeln!(out, "! {} file check(s) failed:", failures.len());
        for rel in failures {
            let _ = writeln!(out, "  {rel}");
        }
    }
    out
}

/// Up to `n` strongest spans, best first.
fn top_spans(heat: &[HeatRange], n: usize) -> Vec<&HeatRange> {
    let mut v: Vec<&HeatRange> = heat.iter().filter(|h| h.p > 0.0).collect();
    v.sort_by(|a, b| b.p.total_cmp(&a.p).then(a.start.cmp(&b.start)));
    v.truncate(n);
    v
}

/// Compact `start-end` list for one hit row.
fn span_list(c: &Check) -> String {
    let merged = merge_heat(c.heat.clone(), SPAN_FLOOR);
    let top = top_spans(&merged, 3);
    if top.is_empty() {
        if !c.truncated && c.total_lines > 0 {
            format!("1-{}", c.total_lines)
        } else {
            "?".to_string()
        }
    } else {
        top.iter().map(|h| line_span(h.start, h.end)).collect::<Vec<_>>().join(",")
    }
}

fn line_span(start: usize, end: usize) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

// ---------------------------------------------------------------------------
// Walker: gitignore-chained candidate collection (port of jegrep tree.rs)
// ---------------------------------------------------------------------------

fn walk(root: &Path, cap: usize) -> Vec<Job> {
    let mut out = Vec::new();
    let mut ig: Vec<Arc<Gitignore>> = Vec::new();
    walk_dir(root, "", &mut ig, cap, &mut out);
    out
}

fn walk_dir(
    dir: &Path,
    rel_prefix: &str,
    ig: &mut Vec<Arc<Gitignore>>,
    cap: usize,
    out: &mut Vec<Job>,
) {
    if out.len() >= cap {
        return;
    }
    let own = load_gitignore(dir);
    if let Some(g) = &own {
        ig.push(g.clone());
    }
    // (name, path, is_dir, size) — symlinks and special files are skipped.
    let mut entries: Vec<(String, PathBuf, bool, u64)> = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let md = match e.metadata() {
                // `DirEntry::metadata` does not traverse symlinks.
                Ok(md) => md,
                Err(_) => continue,
            };
            let is_dir = md.is_dir();
            let is_file = md.is_file();
            if !is_dir && !is_file {
                continue;
            }
            if !eligible(&name, is_dir, md.len()) {
                continue;
            }
            if ignored(ig, &path, is_dir) {
                continue;
            }
            entries.push((name, path, is_dir, md.len()));
        }
    }
    // Directories first, then alphabetical.
    entries.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    for (name, path, is_dir, size) in entries {
        if out.len() >= cap {
            break;
        }
        let rel = format!("{rel_prefix}{name}{}", if is_dir { "/" } else { "" });
        if is_dir {
            walk_dir(&path, &rel, ig, cap, out);
        } else {
            out.push(Job { path, rel, size });
        }
    }
    if own.is_some() {
        ig.pop();
    }
}

/// `.gitignore` of `dir`, when present and parseable.
fn load_gitignore(dir: &Path) -> Option<Arc<Gitignore>> {
    let gi_path = dir.join(".gitignore");
    if !gi_path.is_file() {
        return None;
    }
    let mut b = GitignoreBuilder::new(dir);
    if b.add(&gi_path).is_some() {
        return None;
    }
    b.build().ok().map(Arc::new)
}

/// Whether any `.gitignore` on the directory chain ignores `path`.
fn ignored(ig: &[Arc<Gitignore>], path: &Path, is_dir: bool) -> bool {
    ig.iter().any(|g| g.matched_path_or_any_parents(path, is_dir).is_ignore())
}

/// Listing-time eligibility (port of jegrep tree.rs). Dot-files are skipped.
fn eligible(name: &str, is_dir: bool, size: u64) -> bool {
    if name.starts_with('.') {
        return false;
    }
    if is_dir {
        return !DENY_DIRS.contains(&name);
    }
    size > 0
        && !DENY_FILES.contains(&name)
        && !secret(name)
        && !has_ext(&name.to_ascii_lowercase(), BINARY_EXT)
}

/// `lower` ends with `.<ext>` for some `ext` in `exts`.
fn has_ext(lower: &str, exts: &[&str]) -> bool {
    exts.iter().any(|ext| {
        lower.len() > ext.len()
            && lower.ends_with(ext)
            && lower.as_bytes()[lower.len() - ext.len() - 1] == b'.'
    })
}

/// Credential material by name: exact names, `.env.*` variants (except
/// committed templates), and key/vault extensions.
fn secret(name: &str) -> bool {
    const ENV_TEMPLATES: &[&str] = &[".env.example", ".env.sample", ".env.template", ".env.dist"];
    if SECRET_FILES.contains(&name) {
        return true;
    }
    if name.starts_with(".env.") {
        return !ENV_TEMPLATES.contains(&name);
    }
    has_ext(&name.to_ascii_lowercase(), SECRET_EXT)
}

const DENY_DIRS: &[&str] = &[
    ".git", "node_modules", "target", "dist", "build", "out", ".next", ".nuxt", ".turbo",
    ".cache", "__pycache__", ".venv", "venv", ".tox", "coverage", ".idea", ".vscode", ".gradle",
    ".mypy_cache", ".pytest_cache", ".ruff_cache", ".parcel-cache",
];

const DENY_FILES: &[&str] = &[
    "Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "bun.lock", "bun.lockb",
    "poetry.lock", "Pipfile.lock", "composer.lock", "Gemfile.lock", "go.sum", "flake.lock",
    ".DS_Store", "Thumbs.db",
];

/// Credential files by exact name. Never listed or read.
const SECRET_FILES: &[&str] = &[
    ".env", ".envrc", ".netrc", ".npmrc", ".pypirc", ".pgpass", ".boto", ".s3cfg", ".dockercfg",
    ".git-credentials", ".htpasswd", "htpasswd", "credentials", "credentials.json",
    "client_secret.json", "service-account.json", "id_rsa", "id_dsa", "id_ecdsa", "id_ed25519",
];

/// Credential files by extension: keys, certificate stores, encrypted vaults,
/// and infrastructure state that embeds secrets.
const SECRET_EXT: &[&str] = &[
    "pem", "key", "p12", "pfx", "jks", "keystore", "bks", "ppk", "kdbx", "gpg", "pgp", "asc",
    "der", "crt", "cer", "tfvars", "tfvars.json", "tfstate", "tfstate.backup",
];

const BINARY_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "bmp", "tiff", "psd", "svg", "woff",
    "woff2", "ttf", "otf", "eot", "zip", "gz", "tgz", "tar", "bz2", "xz", "zst", "7z", "rar",
    "pdf", "mp3", "mp4", "mov", "avi", "mkv", "wav", "ogg", "flac", "wasm", "so", "dylib", "dll",
    "exe", "o", "a", "class", "jar", "pyc", "pyo", "bin", "dat", "db", "sqlite", "sqlite3",
    "lock", "map", "min.js", "min.css", "snap", "pb", "onnx", "safetensors", "parquet", "arrow",
    "ipynb",
];

// ---------------------------------------------------------------------------
// Question builders (port of jegrep questions.rs)
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
struct Criteria {
    #[serde(rename = "true")]
    yes: String,
    #[serde(rename = "false")]
    no:  String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Question {
    Noul {
        instructions: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<Criteria>,
    },
    Choice {
        instructions: String,
        criteria:     BTreeMap<String, Value>,
    },
}

fn content_criteria() -> Criteria {
    Criteria {
        yes: "The file contains code, text, or data that directly matches, implements, defines, \
              or documents what the search describes."
            .to_string(),
        no:  "The file is unrelated, or only shares surface keywords with the search without \
              containing the thing itself."
            .to_string(),
    }
}

fn range_key(k: usize) -> String {
    format!("R{k:02}")
}

/// One content check: read the file head, tag lines, split into contiguous
/// ranges, ask `relevant` (noul) + `where` (choice over the ranges).
struct Prep {
    state:       Value,
    questions:   BTreeMap<String, Question>,
    ranges:      Vec<(usize, usize)>,
    total_lines: usize,
    truncated:   bool,
}

fn prepare_file(path: &Path, rel: &str, size: u64, query: &str) -> Result<Prep, String> {
    let rt = read_text(path, MAX_BYTES).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = rt.text.lines().collect();
    let tagged = tag_lines(&lines);
    let (ranges, criteria) = line_ranges(&lines, RANGES);
    let note = if rt.truncated {
        format!("only the first {} of {} bytes are shown", rt.bytes, size)
    } else {
        "complete file".to_string()
    };

    let mut questions = BTreeMap::new();
    questions.insert(
        "relevant".to_string(),
        Question::Noul {
            instructions: format!(
                "Does the content of \"{rel}\" (in `content`) actually contain what this search \
                 is looking for: \"{query}\"?"
            ),
            criteria: Some(content_criteria()),
        },
    );
    if ranges.len() >= 2 {
        questions.insert(
            "where".to_string(),
            Question::Choice {
                instructions: format!(
                    "Which range of lines in `content` (every line is prefixed with its number, \
                     e.g. L0042|) best matches this search: \"{query}\"? Prefer the range where \
                     it is implemented or defined over ranges that merely import or reference it."
                ),
                criteria,
            },
        );
    }
    // Plain text state: the judge's `state_text` passes strings through
    // verbatim (an object would be JSON-escaped, collapsing every line).
    let state = Value::String(format!(
        "task: Semantic grep: decide whether this file's content contains what the search \
         describes, and where.\nsearch: {query}\nfile: {rel}\nnote: {note}\ncontent:\n{tagged}"
    ));
    Ok(Prep {
        state,
        questions,
        ranges,
        total_lines: lines.len(),
        truncated: rt.truncated,
    })
}

/// `L0001| …` per line, 1-based.
fn tag_lines(lines: &[&str]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let _ = writeln!(out, "L{:04}| {line}", i + 1);
    }
    out
}

/// Contiguous line chunks; keys `R00`, `R01`, …
fn line_ranges(lines: &[&str], n_ranges: usize) -> (Vec<(usize, usize)>, BTreeMap<String, Value>) {
    let r = n_ranges.clamp(1, 26).min(lines.len().max(1));
    let per = ((lines.len() + r - 1) / r).max(1);
    let mut ranges = Vec::new();
    let mut criteria = BTreeMap::new();
    let mut start = 0usize;
    while start < lines.len() {
        let end = (start + per).min(lines.len());
        criteria.insert(
            range_key(ranges.len()),
            Value::String(format!("lines {}-{}", start + 1, end)),
        );
        ranges.push((start + 1, end));
        start = end;
    }
    (ranges, criteria)
}

/// Decode the `where` choice probabilities back into per-range heat.
fn heat_from(answers: &BTreeMap<String, Value>, ranges: &[(usize, usize)]) -> Vec<HeatRange> {
    let probs = match answer_probs(answers, "where") {
        Some(p) => p,
        None => return Vec::new(),
    };
    ranges
        .iter()
        .enumerate()
        .map(|(k, (start, end))| HeatRange {
            start: *start,
            end:   *end,
            p:     probs.get(&range_key(k)).copied().unwrap_or(0.0),
        })
        .collect()
}

/// Union judged-positive adjacent spans (never bridge an unjudged gap);
/// merged span ranked by max p. Port of jegrep window.rs `merge_heat`.
fn merge_heat(mut heat: Vec<HeatRange>, floor: f64) -> Vec<HeatRange> {
    heat.retain(|h| h.p >= floor && h.p > 0.0 && h.start <= h.end);
    heat.sort_by_key(|h| (h.start, h.end));
    let mut merged: Vec<HeatRange> = Vec::new();
    for h in heat {
        let mut absorbed = false;
        if let Some(last) = merged.last_mut() {
            if h.start <= last.end.saturating_add(1) {
                last.end = last.end.max(h.end);
                if h.p > last.p {
                    last.p = h.p;
                }
                absorbed = true;
            }
        }
        if !absorbed {
            merged.push(h);
        }
    }
    merged.sort_by(|a, b| b.p.total_cmp(&a.p).then(a.start.cmp(&b.start)));
    merged
}

// ---------------------------------------------------------------------------
// Content check (one per survivor file, run on worker threads)
// ---------------------------------------------------------------------------

fn check_file(client: &JevClient, job: &Job, query: &str) -> CheckOutcome {
    let rel = job.rel.clone();
    let prep = match prepare_file(&job.path, &job.rel, job.size, query) {
        Ok(p) => p,
        Err(e) => return CheckOutcome { rel, result: Err(e) },
    };
    match client.ask(&prep.state, &prep.questions) {
        Err(e) => CheckOutcome { rel, result: Err(e) },
        Ok(resp) => {
            let score = answer_noul(&resp.answers, "relevant").unwrap_or(0.0);
            let heat = heat_from(&resp.answers, &prep.ranges);
            let check = Check {
                rel: job.rel.clone(),
                score,
                heat,
                total_lines: prep.total_lines,
                truncated: prep.truncated,
            };
            CheckOutcome { rel, result: Ok(check) }
        },
    }
}

// ---------------------------------------------------------------------------
// Jev HTTP client (local endpoint shape; port of jegrep jev.rs)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Default)]
struct Resp {
    #[serde(default)]
    answers: BTreeMap<String, Value>,
}

#[derive(Serialize)]
struct ReqBody<'a> {
    state:     &'a Value,
    questions: &'a BTreeMap<String, Question>,
}

/// Lenient answer extraction: accepts tagged (`{"type":"noul","noul":…}`) and
/// untagged (`{"noul":…}`) shapes.
fn answer_noul(answers: &BTreeMap<String, Value>, key: &str) -> Option<f64> {
    let v = answers.get(key)?;
    if let Some(n) = v.get("noul").and_then(Value::as_f64) {
        return Some(n);
    }
    if let Some(n) = v.get("score").and_then(Value::as_f64) {
        return Some(n);
    }
    v.as_f64()
}

fn answer_probs(answers: &BTreeMap<String, Value>, key: &str) -> Option<BTreeMap<String, f64>> {
    let obj = answers.get(key)?.get("probabilities")?.as_object()?;
    Some(
        obj.iter()
            .filter_map(|(k, x)| x.as_f64().map(|f| (k.clone(), f)))
            .collect(),
    )
}

#[derive(Clone)]
struct JevClient {
    client:   Client,
    endpoint: String,
}

impl JevClient {
    fn new() -> Self {
        // reqwest treats non-2xx as `Ok(resp)` (like ureq's
        // `http_status_as_error(false)`); the status is checked in `ask`.
        // A failed build falls back to the default client.
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, endpoint: JEV_URL.to_string() }
    }

    /// POST one question batch: status 200 parses the answer, anything else
    /// is returned as an error right away.
    fn ask(&self, state: &Value, questions: &BTreeMap<String, Question>) -> Result<Resp, String> {
        let body = ReqBody { state, questions };
        // Serialize manually so only reqwest's `blocking` feature is needed.
        let payload = serde_json::to_string(&body).map_err(|e| format!("encode: {e}"))?;
        match self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
        {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == 200 {
                    let text = resp.text().map_err(|e| format!("decode: {e}"))?;
                    serde_json::from_str::<Resp>(&text).map_err(|e| format!("decode: {e}"))
                } else {
                    let text = resp.text().unwrap_or_default();
                    Err(format!("HTTP {status}: {}", clip(&text, 200)))
                }
            },
            Err(e) => Err(format!("transport: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// File reading (port of jegrep questions.rs)
// ---------------------------------------------------------------------------

struct ReadText {
    text:      String,
    bytes:     usize,
    truncated: bool,
}

#[derive(Debug)]
enum ReadErr {
    Binary,
    Empty,
    Io(String),
}

impl std::fmt::Display for ReadErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binary => write!(f, "binary"),
            Self::Empty => write!(f, "empty"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

/// Read at most `max_bytes` of whole lines; reject binaries (NUL sniff) and
/// blank files.
fn read_text(path: &Path, max_bytes: usize) -> Result<ReadText, ReadErr> {
    let f = File::open(path).map_err(|e| ReadErr::Io(e.to_string()))?;
    let mut buf = Vec::new();
    f.take(max_bytes as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| ReadErr::Io(e.to_string()))?;
    if buf[..buf.len().min(8192)].contains(&0u8) {
        return Err(ReadErr::Binary);
    }
    let truncated = buf.len() > max_bytes;
    if truncated {
        buf.truncate(max_bytes);
        // Trim back to the last full line so only whole lines are sent.
        if let Some(p) = buf.iter().rposition(|&b| b == b'\n') {
            buf.truncate(p + 1);
        }
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    if text.lines().all(|l| l.trim().is_empty()) {
        return Err(ReadErr::Empty);
    }
    let bytes = buf.len();
    Ok(ReadText { text, bytes, truncated })
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn clip(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}
