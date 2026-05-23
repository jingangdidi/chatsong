// Copyright 2026 Xiaomi Corp. (authors: Han Zhu)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Text processing utilities for TTS inference.
//!
//! Provides:
//! - [`chunk_text_punctuation`]: Splits long text into model-friendly chunks at
//!   sentence boundaries, with abbreviation-aware punctuation splitting.
//! - [`add_punctuation`]: Appends missing end punctuation (Chinese or English).
//! - [`combine_text`]: Merges reference text and target text with cleanup.

use std::collections::HashSet;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Character sets
// ---------------------------------------------------------------------------

/// Punctuation characters that trigger sentence splits.
static SPLIT_PUNCTUATION: LazyLock<HashSet<char>> =
    LazyLock::new(|| ".,;:!?。，；：！？".chars().collect());

/// Closing marks that should be attached to the preceding sentence.
static CLOSING_MARKS: LazyLock<HashSet<char>> = LazyLock::new(|| {
    "\"'\"'\u{201D}\u{2019}\u{FF09}]\u{300B}>\u{300F}\u{3011}"
        .chars()
        .collect()
});

/// Characters considered valid end punctuation (a superset of split punctuation
/// plus brackets, quotes, etc.).
static END_PUNCTUATION: LazyLock<HashSet<char>> = LazyLock::new(|| {
    [
        ';', ':', ',', '.', '!', '?', '\u{2026}', // ...
        ')', ']', '}', '"', '\'', '\u{201C}', '\u{2018}', // " '
        '\u{201D}', '\u{2019}', // " '
        '\u{FF1B}', '\u{FF1A}', '\u{FF0C}', '\u{3002}', '\u{FF01}',
        '\u{FF1F}', // ；：，。！？
        '\u{3001}', // 、
        '\u{FF09}', '\u{3011}', // ）】
    ]
    .into_iter()
    .collect()
});

/// Common English abbreviations that should NOT trigger a sentence split on
/// their trailing period.
static ABBREVIATIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.", "Rev.", "Fr.", "Hon.", "Pres.", "Gov.",
        "Capt.", "Gen.", "Sen.", "Rep.", "Col.", "Maj.", "Lt.", "Cmdr.", "Sgt.", "Cpl.", "Co.",
        "Corp.", "Inc.", "Ltd.", "Est.", "Dept.", "St.", "Ave.", "Blvd.", "Rd.", "Mt.", "Ft.",
        "No.", "Jan.", "Feb.", "Mar.", "Apr.", "Aug.", "Sep.", "Sept.", "Oct.", "Nov.", "Dec.",
        "i.e.", "e.g.", "vs.", "Vs.", "Etc.", "approx.", "fig.", "def.",
    ]
    .into_iter()
    .collect()
});

/// Emotion tags that must not have preceding whitespace (matching Python's
/// `_EMOTION_TAGS` regex group).
const EMOTION_TAGS: &[&str] = &[
    "[sigh]",
    "[confirmation-en]",
    "[question-en]",
    "[question-ah]",
    "[question-oh]",
    "[question-ei]",
    "[question-yi]",
    "[surprise-ah]",
    "[surprise-oh]",
    "[surprise-wa]",
    "[surprise-yo]",
    "[dissatisfaction-hnn]",
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Append a period (`.` or `。`) if the text has no trailing end punctuation.
///
/// Chooses the Chinese ideographic period if any CJK character is present in
/// the text, otherwise an ASCII period.
pub fn add_punctuation(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    if let Some(last) = text.chars().last() {
        if END_PUNCTUATION.contains(&last) {
            return text.to_string();
        }
    }

    let is_chinese = text.chars().any(is_cjk);
    if is_chinese {
        format!("{text}\u{3002}") // 。
    } else {
        format!("{text}.")
    }
}

/// Split text into chunks at punctuation boundaries, respecting abbreviations.
///
/// * `chunk_len` -- target maximum character count per chunk.
/// * `min_chunk_len` -- chunks smaller than this are merged into an adjacent
///   chunk. Pass `None` to disable.
pub fn chunk_text_punctuation(
    text: &str,
    chunk_len: usize,
    min_chunk_len: Option<usize>,
) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();

    // 1. Split into sentences at punctuation boundaries.
    let mut sentences: Vec<Vec<char>> = Vec::new();
    let mut current: Vec<char> = Vec::new();

    for &ch in &chars {
        // If current sentence is empty and previous sentence exists, attach
        // leading punctuation / closing marks to the previous sentence.
        if current.is_empty()
            && !sentences.is_empty()
            && (SPLIT_PUNCTUATION.contains(&ch) || CLOSING_MARKS.contains(&ch))
        {
            sentences.last_mut().unwrap().push(ch);
        } else {
            current.push(ch);

            if SPLIT_PUNCTUATION.contains(&ch) {
                let mut is_abbreviation = false;

                if ch == '.' {
                    let temp: String = current.iter().collect::<String>();
                    let temp = temp.trim();
                    if let Some(last_word) = temp.split_whitespace().last() {
                        if ABBREVIATIONS.contains(last_word) {
                            is_abbreviation = true;
                        }
                    }
                }

                if !is_abbreviation {
                    sentences.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        sentences.push(current);
    }

    // 2. Merge short sentences up to chunk_len.
    let mut merged: Vec<Vec<char>> = Vec::new();
    let mut chunk: Vec<char> = Vec::new();

    for sentence in sentences {
        if chunk.len() + sentence.len() <= chunk_len {
            chunk.extend(sentence);
        } else {
            if !chunk.is_empty() {
                merged.push(std::mem::take(&mut chunk));
            }
            chunk = sentence;
        }
    }
    if !chunk.is_empty() {
        merged.push(chunk);
    }

    // 3. Post-process: merge undersized chunks.
    let final_chunks = if let Some(min_len) = min_chunk_len {
        let first_short = !merged.is_empty() && merged[0].len() < min_len;
        let mut result: Vec<Vec<char>> = Vec::new();

        for (i, chunk) in merged.into_iter().enumerate() {
            if i == 1 && first_short {
                // Merge second chunk into the (already short) first.
                result.last_mut().unwrap().extend(chunk);
            } else if chunk.len() >= min_len || result.is_empty() {
                result.push(chunk);
            } else {
                result.last_mut().unwrap().extend(chunk);
            }
        }
        result
    } else {
        merged
    };

    final_chunks
        .into_iter()
        .map(|c| c.into_iter().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Combine reference text and target text into a single string.
///
/// Performs the same normalisation as the Python `_combine_text`:
/// - Joins `ref_text + " " + text`
/// - Replaces newlines with `.`
/// - Removes spaces adjacent to CJK characters
/// - Strips whitespace before emotion tags
pub fn combine_text(text: &str, ref_text: Option<&str>) -> String {
    let full = match ref_text {
        Some(rt) if !rt.trim().is_empty() => {
            format!("{} {}", rt.trim(), text.trim())
        }
        _ => text.trim().to_string(),
    };

    // Replace newlines (and surrounding whitespace) with a period.
    let full = replace_newlines_with_period(&full);

    // Remove spaces around CJK characters.
    let full = remove_spaces_around_cjk(&full);

    // Remove whitespace before emotion tags (except [laughter]).
    strip_space_before_emotion_tags(&full)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Replace `\n` (with optional surrounding blanks) by `.`.
fn replace_newlines_with_period(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\n' || (chars[i] == '\r' && i + 1 < len && chars[i + 1] == '\n') {
            // Consume preceding spaces/tabs.
            while result.ends_with(' ') || result.ends_with('\t') {
                result.pop();
            }
            // Skip the newline itself.
            if chars[i] == '\r' {
                i += 1; // skip \r
            }
            i += 1; // skip \n
                    // Skip trailing whitespace.
            while i < len
                && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '\n' || chars[i] == '\r')
            {
                i += 1;
            }
            result.push('.');
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Returns true if `c` is a CJK Unified Ideograph (U+4E00..U+9FFF).
pub fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// Remove whitespace immediately before or after a CJK character.
fn remove_spaces_around_cjk(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len());

    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            // Check if adjacent character is CJK.
            let prev_cjk = i > 0 && is_cjk(chars[i - 1]);
            let next_cjk = i + 1 < chars.len() && is_cjk(chars[i + 1]);
            if prev_cjk || next_cjk {
                continue; // drop the space
            }
        }
        result.push(c);
    }

    result
}

/// Strip whitespace immediately before recognised emotion tags.
fn strip_space_before_emotion_tags(s: &str) -> String {
    let mut result = s.to_string();
    for tag in EMOTION_TAGS {
        // Replace " [tag]" with "[tag]" (one or more spaces).
        loop {
            let with_space = format!(" {tag}");
            if result.contains(&with_space) {
                result = result.replace(&with_space, tag);
            } else {
                break;
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_punctuation_english() {
        assert_eq!(add_punctuation("Hello world"), "Hello world.");
        assert_eq!(add_punctuation("Hello world."), "Hello world.");
        assert_eq!(add_punctuation("Hello world!"), "Hello world!");
    }

    #[test]
    fn test_add_punctuation_chinese() {
        assert_eq!(add_punctuation("你好世界"), "你好世界\u{3002}");
        assert_eq!(add_punctuation("你好世界\u{3002}"), "你好世界\u{3002}");
    }

    #[test]
    fn test_add_punctuation_empty() {
        assert_eq!(add_punctuation(""), "");
        assert_eq!(add_punctuation("  "), "");
    }

    #[test]
    fn test_chunk_text_basic() {
        let text = "Hello. World. How are you? Fine.";
        let chunks = chunk_text_punctuation(text, 15, None);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 20); // some slack from merging
        }
    }

    #[test]
    fn test_chunk_text_abbreviation() {
        let text = "Dr. Smith went to St. Paul. Then he left.";
        let chunks = chunk_text_punctuation(text, 100, None);
        // "Dr." and "St." should not cause splits.
        // Splits at "Paul." and "left." => 2 sentences, merged into 1 chunk.
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chunk_text_min_chunk_len() {
        let text = "Hi. This is a much longer sentence that should form its own chunk.";
        let chunks = chunk_text_punctuation(text, 20, Some(10));
        // "Hi." is short (3 chars < 10), should be merged.
        assert!(!chunks.is_empty());
        // The first chunk should contain "Hi." merged with subsequent text.
        assert!(chunks[0].starts_with("Hi."));
    }

    #[test]
    fn test_combine_text_basic() {
        let result = combine_text("target text", Some("ref text"));
        assert_eq!(result, "ref text target text");
    }

    #[test]
    fn test_combine_text_no_ref() {
        let result = combine_text("target text", None);
        assert_eq!(result, "target text");
    }

    #[test]
    fn test_combine_text_newlines() {
        let result = combine_text("line1\nline2", None);
        assert_eq!(result, "line1.line2");
    }

    #[test]
    fn test_combine_text_cjk_spaces() {
        let result = combine_text("你好 world 世界", None);
        assert_eq!(result, "你好world世界");
    }

    #[test]
    fn test_combine_text_emotion_tags() {
        let result = combine_text("hello [sigh] world", None);
        assert_eq!(result, "hello[sigh] world");
    }
}
