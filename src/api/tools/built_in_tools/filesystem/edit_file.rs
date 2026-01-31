use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value}; // https://docs.rs/serde_json/latest/serde_json/enum.Value.html
use similar::TextDiff;

use crate::{
    error::MyError,
    parse_paras::PARAS,
    tools::built_in_tools::{
        BuiltIn,
        filesystem::utils::{
            validate_path,
            normalize_line_endings,
        },
    },
};

/// Represents a text replacement operation.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct EditOperation {
    /// Text to search for - must match exactly.
    #[serde(rename = "oldText")]
    pub old_text: String,

    #[serde(rename = "newText")]
    /// Text to replace the matched text with.
    pub new_text: String,
}

/// params for integer edit_file
#[derive(Deserialize, Serialize)]
pub struct Params {
    pub file_path: String,
    pub edits:     Vec<EditOperation>,
    pub dry_run:   Option<bool>,
    //save_to:   Option<String>,
}

/// built-in tool
pub struct EditFile;

impl EditFile {
    /// new
    pub fn new() -> Self {
        EditFile
    }

    fn create_unified_diff(&self, original_content: &str, new_content: &str, file_path: Option<String>) -> Result<String, MyError> {
        // Ensure consistent line endings for diff
        let normalized_original = normalize_line_endings(original_content);
        let normalized_new = normalize_line_endings(new_content);

        // Generate the diff using TextDiff
        let diff = TextDiff::from_lines(&normalized_original, &normalized_new);

        let file_name = file_path.unwrap_or("file".to_string());
        // Format the diff as a unified diff
        let patch = diff
            .unified_diff()
            .header(
                format!("{file_name}").as_str(),
                format!("{file_name}").as_str(),
            )
            .context_radius(4)
            .to_string();

        //Ok(format!("Index: {}\n{}\n{}", file_name, "=".repeat(68), patch))
        Ok(patch)
    }

    fn detect_line_ending(&self, text: &str) -> &str {
        if text.contains("\r\n") {
            "\r\n"
        } else if text.contains('\r') {
            "\r"
        } else {
            "\n"
        }
    }
}

impl BuiltIn for EditFile {
    /// get tool name
    fn name(&self) -> String {
        "edit_file".to_string()
    }

    /// get tool description
    fn description(&self) -> String {
        "Make line-based edits to a text file. Each edit replaces exact line sequences with new content. Returns a git-style diff showing the changes made. Only works within allowed directories.".to_string()
    }

    /// get tool schema
    fn schema(&self) -> Value {
        json!({
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path of the file to edit.",
                },
                "edits": {
                    "type": "array",
                    "description": "The list of edit operations to apply.",
                    "items": {
                        "$ref": "#/definitions/EditOperation"
                    },
                },
                "dry_run": {
                    "type": ["boolean", "null"],
                    "description": "Preview changes using git-style diff format without applying them.",
                },
            },
            "required": ["file_path", "edits"],
            "type": "object",
            "definitions": {
                "EditOperation": {
                    "type": "object",
                    "properties": {
                        "oldText": {
                            "type": "string",
                            "description": "Text to search for - must match exactly.",
                        },
                        "newText": {
                            "type": "string",
                            "description": "Text to replace the matched text with.",
                        }
                    },
                    "required": ["oldText", "newText"],
                    "description": "Represents a text replacement operation.",
                }
            }
        })
    }

    /// run tool
    fn run(&self, args: &str) -> Result<String, MyError> {
        println!("\n{}\n", args);
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        let valid_path = validate_path(&PARAS.allowed_path, &Path::new(&params.file_path), false)?;

        // Read file content and normalize line endings
        let content_str = fs::read_to_string(&valid_path)?;
        let original_line_ending = self.detect_line_ending(&content_str);
        let content_str = normalize_line_endings(&content_str);

        // Apply edits sequentially
        let mut modified_content = content_str.clone();

        for edit in params.edits {
            let normalized_old = normalize_line_endings(&edit.old_text);
            let normalized_new = normalize_line_endings(&edit.new_text);
            // If exact match exists, use it
            if modified_content.contains(&normalized_old) {
                modified_content = modified_content.replacen(&normalized_old, &normalized_new, 1);
                continue;
            }

            // Otherwise, try line-by-line matching with flexibility for whitespace
            let old_lines: Vec<String> = normalized_old
                .trim_end()
                .split('\n')
                .map(|s| s.to_string())
                .collect();

            let content_lines: Vec<String> = modified_content
                .trim_end()
                .split('\n')
                .map(|s| s.to_string())
                .collect();

            let mut match_found = false;

            // skip when the match is impossible:
            if old_lines.len() > content_lines.len() {
                return Err(MyError::OtherError{info: format!(
                    "Cannot apply edit: the original text spans more lines ({}) than the file content ({}).",
                    old_lines.len(),
                    content_lines.len()
                )})
            }

            let max_start = content_lines.len().saturating_sub(old_lines.len());
            for i in 0..=max_start {
                let potential_match = &content_lines[i..i + old_lines.len()];

                // Compare lines with normalized whitespace
                let is_match = old_lines.iter().enumerate().all(|(j, old_line)| {
                    let content_line = &potential_match[j];
                    old_line.trim() == content_line.trim()
                });

                if is_match {
                    // Preserve original indentation of first line
                    let original_indent = content_lines[i]
                        .chars()
                        .take_while(|&c| c.is_whitespace())
                        .collect::<String>();

                    let new_lines: Vec<String> = normalized_new
                        .split('\n')
                        .enumerate()
                        .map(|(j, line)| {
                            // Keep indentation of the first line
                            if j == 0 {
                                return format!("{}{}", original_indent, line.trim_start());
                            }

                            // For subsequent lines, preserve relative indentation and original whitespace type
                            let old_indent = old_lines
                                .get(j)
                                .map(|line| {
                                    line.chars()
                                        .take_while(|&c| c.is_whitespace())
                                        .collect::<String>()
                                })
                                .unwrap_or_default();

                            let new_indent = line
                                .chars()
                                .take_while(|&c| c.is_whitespace())
                                .collect::<String>();

                            // Use the same whitespace character as original_indent (tabs or spaces)
                            let indent_char = if original_indent.contains('\t') {
                                "\t"
                            } else {
                                " "
                            };
                            let relative_indent = if new_indent.len() >= old_indent.len() {
                                new_indent.len() - old_indent.len()
                            } else {
                                0 // Don't reduce indentation below original
                            };
                            format!(
                                "{}{}{}",
                                &original_indent,
                                &indent_char.repeat(relative_indent),
                                line.trim_start()
                            )
                        })
                        .collect();

                    let mut content_lines = content_lines.clone();
                    content_lines.splice(i..i + old_lines.len(), new_lines);
                    modified_content = content_lines.join("\n");
                    match_found = true;
                    break;
                }
            }
            if !match_found {
                return Err(MyError::OtherError{info: format!("Could not find exact match for edit:\n{}", edit.old_text)})
            }
        }

        let mut relative_path = valid_path.clone();
        for p in &PARAS.allowed_path {
            if valid_path.starts_with(&p.0) {
                relative_path = valid_path.strip_prefix(&p.0).unwrap().to_path_buf();
                break
            } else if valid_path.starts_with(&p.1) {
                relative_path = valid_path.strip_prefix(&p.0).unwrap().to_path_buf();
                break
            }
        }
        let diff = self.create_unified_diff(
            &content_str,
            &modified_content,
            Some(relative_path.display().to_string()),
        )?;

        let is_dry_run = params.dry_run.unwrap_or(false);

        if !is_dry_run {
            //let target = params.save_to.unwrap_or(valid_path.display().to_string());
            let target = valid_path.display().to_string();
            let modified_content = modified_content.replace("\n", original_line_ending);
            fs::write(target, modified_content)?;
        }

        //Ok(format!("successfully edit file:\n{:?}", formatted_diff))
        Ok(diff)
    }

    /// get approval message
    fn get_approval(&self, args: &str, info: Option<String>, is_en: bool) -> Result<Option<String>, MyError> {
        let params: Params = serde_json::from_str(args).map_err(|e| MyError::SerdeJsonFromStrError{error: e})?;
        if is_en {
            Ok(Some(format!("Do you allow calling the edit_file tool to edit a text file {} ?{}\n{:?}", params.file_path, info.unwrap_or_default(), params.edits)))
        } else {
            Ok(Some(format!("是否允许调用 edit_file 工具编辑 {}？{}\n{:?}", params.file_path, info.unwrap_or_default(), params.edits)))
        }
    }
}
