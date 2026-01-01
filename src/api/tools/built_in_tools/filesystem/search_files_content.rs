use std::fmt::Write;
use std::path::{Path, PathBuf};

use grep::{
    matcher::{
        Match,
        Matcher,
    },
    regex::RegexMatcherBuilder,
    searcher::{
        sinks::UTF8,
        BinaryDetection,
        Searcher,
    },
};
use serde::Deserialize; // Serialize
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html

use crate::{
    error::MyError,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::search_files_helper,
    },
};

const SNIPPET_MAX_LENGTH: usize = 200;
const SNIPPET_BACKWARD_CHARS: usize = 30;

/// params for integer search_files_content
#[derive(Deserialize)]
struct Params {
    root_path:        String,
    pattern:          String,
    query:            String,
    is_regex:         bool,
    exclude_patterns: Option<Vec<String>>,
}

/// Represents a single match found in a file's content.
#[derive(Debug, Clone)]
struct ContentMatchResult {
    /// The line number where the match occurred (1-based).
    line_number: u64,

    start_pos: usize,

    /// The line of text containing the match.
    /// If the line exceeds 255 characters (excluding the search term), only a truncated portion will be shown.
    line_text: String,
}

/// Represents all matches found in a specific file.
#[derive(Debug, Clone)]
struct FileSearchResult {
    /// The path to the file where matches were found.
    file_path: PathBuf,

    /// All individual match results within the file.
    matches: Vec<ContentMatchResult>,
}

/// built-in tool
pub struct SearchFilesContent;

impl SearchFilesContent {
    /// new
    pub fn new() -> Self {
        SearchFilesContent
    }

    fn escape_regex(&self, text: &str) -> String {
        // Covers special characters in regex engines (RE2, PCRE, JS, Python)
        const SPECIAL_CHARS: &[char] = &[
            '.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '\\', '|', '/',
        ];

        let mut escaped = String::with_capacity(text.len());

        for ch in text.chars() {
            if SPECIAL_CHARS.contains(&ch) {
                escaped.push('\\');
            }
            escaped.push(ch);
        }

        escaped
    }

    /// Extracts a snippet from a given line of text around a match.
    ///
    /// It extracts a substring starting a fixed number of characters (`SNIPPET_BACKWARD_CHARS`)
    /// before the start position of the `match`, and extends up to `max_length` characters
    /// If the snippet does not include the beginning or end of the original line, ellipses (`"..."`) are added
    /// to indicate the truncation.
    fn extract_snippet(
        &self,
        line: &str,
        match_result: Match,
        max_length: Option<usize>,
        backward_chars: Option<usize>,
    ) -> String {
        let max_length = max_length.unwrap_or(SNIPPET_MAX_LENGTH);
        let backward_chars = backward_chars.unwrap_or(SNIPPET_BACKWARD_CHARS);

        // Calculate the number of leading whitespace bytes to adjust for trimmed input
        let start_pos = line.len() - line.trim_start().len();
        // Trim leading and trailing whitespace from the input line
        let line = line.trim();

        // Calculate the desired start byte index by adjusting match start for trimming and backward chars
        // match_result.start() is the byte index in the original string
        // Subtract start_pos to account for trimmed whitespace and backward_chars to include context before the match
        let desired_start = (match_result.start() - start_pos).saturating_sub(backward_chars);

        // Find the nearest valid UTF-8 character boundary at or after desired_start
        // Prevents "byte index is not a char boundary" panic by ensuring the slice starts at a valid character (issue #37)
        let snippet_start = line
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= desired_start)
            .unwrap_or(desired_start.min(line.len()));
        // Initialize a counter for tracking characters to respect max_length
        let mut char_count = 0;

        // Calculate the desired end byte index by counting max_length characters from snippet_start
        // Take max_length + 1 to find the boundary after the last desired character
        let desired_end = line[snippet_start..]
            .char_indices()
            .take(max_length + 1)
            .find(|&(_, _)| {
                char_count += 1;
                char_count > max_length
            })
            .map(|(i, _)| snippet_start + i)
            .unwrap_or(line.len());

        // Ensure snippet_end is a valid UTF-8 character boundary at or after desired_end
        // This prevents slicing issues with multi-byte characters
        let snippet_end = line
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= desired_end)
            .unwrap_or(line.len());

        // Cap snippet_end to avoid exceeding the string length
        let snippet_end = snippet_end.min(line.len());

        // Extract the snippet from the trimmed line using the calculated byte indices
        let snippet = &line[snippet_start..snippet_end];

        let mut result = String::new();
        // Add leading ellipsis if the snippet doesn't start at the beginning of the trimmed line
        if snippet_start > 0 {
            result.push_str("...");
        }

        result.push_str(snippet);

        // Add trailing ellipsis if the snippet doesn't reach the end of the trimmed line
        if snippet_end < line.len() {
            result.push_str("...");
        }
        result
    }

    /// Searches the content of a file for occurrences of the given query string.
    ///
    /// This method searches the file specified by `file_path` for lines matching the `query`.
    /// The search can be performed as a regular expression or as a literal string, depending on the `is_regex` flag.
    /// If matched line is larger than 255 characters, a snippet will be extracted around the matched text.
    fn content_search(&self, query: &str, file_path: &Path, is_regex: Option<bool>) -> Result<Option<FileSearchResult>, MyError> {
        let query = if is_regex.unwrap_or_default() {
            query.to_string()
        } else {
            self.escape_regex(query)
        };

        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build(query.as_str()).map_err(|e| MyError::GrepError{error: e})?;

        let mut searcher = Searcher::new();
        let mut result = FileSearchResult {
            file_path: file_path.to_path_buf(),
            matches: vec![],
        };

        searcher.set_binary_detection(BinaryDetection::quit(b'\x00'));

        searcher.search_path(
            &matcher,
            file_path,
            UTF8(|line_number, line| {
                let actual_match = matcher.find(line.as_bytes())?.unwrap();
                result.matches.push(ContentMatchResult {
                    line_number,
                    start_pos: actual_match.start(),
                    line_text: self.extract_snippet(line, actual_match, None, None),
                });
                Ok(true)
            }),
        )?;

        if result.matches.is_empty() {
            return Ok(None);
        }

        Ok(Some(result))
    }

    fn format_result(&self, results: Vec<FileSearchResult>) -> String {
        // TODO: improve capacity estimation
        let estimated_capacity = 2048;

        let mut output = String::with_capacity(estimated_capacity);

        for file_result in results {
            // Push file path
            let _ = writeln!(output, "{}", file_result.file_path.display());

            // Push each match line
            for m in &file_result.matches {
                // Format: "  line:col: text snippet"
                let _ = writeln!(
                    output,
                    "  {}:{}: {}",
                    m.line_number, m.start_pos, m.line_text
                );
            }

            // double spacing
            output.push('\n');
        }

        output
    }
}

impl BuiltIn for SearchFilesContent {
    /// get tool name
    fn name(&self) -> String {
        "search_files_content".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Searches for text or regex patterns in the content of files matching a GLOB pattern. Returns detailed matches with file path, line number, column number and a preview of matched text. By default, it performs a literal text search; if the 'is_regex' parameter is set to true, it performs a regular expression (regex) search instead. Ideal for finding specific code, comments, or text when you don’t know their exact location.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "root_path": {
                    "type": "string",
                    "description": "The file or directory path to search in.",
                },
                "pattern": {
                    "type": "string",
                    "description": "The file glob pattern to match (e.g., \"*.rs\").",
                },
                "query": {
                    "type": "string",
                    "description": "Text or regex pattern to find in file contents (e.g., 'TODO' or '^function\\s+').",
                },
                "is_regex": {
                    "type": ["boolean", "null"],
                    "description": "Whether the query is a regular expression. If false, the query as plain text. (Default : false)",
                },
                "exclude_patterns": {
                    "type": ["array", "null"],
                    "items": {"type": "string"},
                    "description": "Optional list of patterns to exclude from the search.",
                },
            },
            "required": ["root_path", "pattern", "query"],
            "type": "object",
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let files_iter = search_files_helper(&params.root_path, params.pattern.clone(), params.exclude_patterns.clone())?;

        let results: Vec<FileSearchResult> = files_iter
            .iter()
            .filter_map(|entry| {
                self.content_search(&params.query, entry.path(), Some(params.is_regex))
                    .ok()
                    .and_then(|v| v)
            })
            .collect();

        if results.is_empty() {
            Ok(format!("No matches found in the files content. path: {}, pattern: {}, exclude_patterns: {:?}", params.root_path, params.pattern, params.exclude_patterns))
        } else {
            let formated_result = self.format_result(results);
            Ok(format!("Successfully found matches:\n{}", formated_result))
        }
    }
}
