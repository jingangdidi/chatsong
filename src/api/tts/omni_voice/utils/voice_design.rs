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

//! Voice-design instruct validation and normalisation.
//!
//! Defines speaker attribute tags (gender, age, pitch, accent, dialect) and
//! translation / validation utilities between English and Chinese. Used by
//! `OmniVoice::generate()` for voice design mode.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Category tables
// ---------------------------------------------------------------------------

/// A bidirectional English-Chinese mapping for a single attribute category.
struct BiCategory {
    en_to_zh: HashMap<&'static str, &'static str>,
    zh_to_en: HashMap<&'static str, &'static str>,
    /// All items in this category (both EN and ZH) for mutual-exclusivity check.
    all_items: HashSet<&'static str>,
}

impl BiCategory {
    fn from_pairs(pairs: &[(&'static str, &'static str)]) -> Self {
        let mut en_to_zh = HashMap::new();
        let mut zh_to_en = HashMap::new();
        let mut all_items = HashSet::new();
        for &(en, zh) in pairs {
            en_to_zh.insert(en, zh);
            zh_to_en.insert(zh, en);
            all_items.insert(en);
            all_items.insert(zh);
        }
        Self {
            en_to_zh,
            zh_to_en,
            all_items,
        }
    }

    fn from_flat(items: &[&'static str]) -> Self {
        let all_items: HashSet<&'static str> = items.iter().copied().collect();
        Self {
            en_to_zh: HashMap::new(),
            zh_to_en: HashMap::new(),
            all_items,
        }
    }
}

/// Gender category.
static CAT_GENDER: LazyLock<BiCategory> = LazyLock::new(|| {
    BiCategory::from_pairs(&[
        ("male", "\u{7537}"),   // 男
        ("female", "\u{5973}"), // 女
    ])
});

/// Age category.
static CAT_AGE: LazyLock<BiCategory> = LazyLock::new(|| {
    BiCategory::from_pairs(&[
        ("child", "\u{513F}\u{7AE5}"),       // 儿童
        ("teenager", "\u{5C11}\u{5E74}"),    // 少年
        ("young adult", "\u{9752}\u{5E74}"), // 青年
        ("middle-aged", "\u{4E2D}\u{5E74}"), // 中年
        ("elderly", "\u{8001}\u{5E74}"),     // 老年
    ])
});

/// Pitch category.
static CAT_PITCH: LazyLock<BiCategory> = LazyLock::new(|| {
    BiCategory::from_pairs(&[
        ("very low pitch", "\u{6781}\u{4F4E}\u{97F3}\u{8C03}"), // 极低音调
        ("low pitch", "\u{4F4E}\u{97F3}\u{8C03}"),              // 低音调
        ("moderate pitch", "\u{4E2D}\u{97F3}\u{8C03}"),         // 中音调
        ("high pitch", "\u{9AD8}\u{97F3}\u{8C03}"),             // 高音调
        ("very high pitch", "\u{6781}\u{9AD8}\u{97F3}\u{8C03}"), // 极高音调
    ])
});

/// Style category (whisper).
static CAT_STYLE: LazyLock<BiCategory> = LazyLock::new(|| {
    BiCategory::from_pairs(&[
        ("whisper", "\u{8033}\u{8BED}"), // 耳语
    ])
});

/// English-only accent category (no Chinese counterpart).
static CAT_ACCENT: LazyLock<BiCategory> = LazyLock::new(|| {
    BiCategory::from_flat(&[
        "american accent",
        "british accent",
        "australian accent",
        "chinese accent",
        "canadian accent",
        "indian accent",
        "korean accent",
        "portuguese accent",
        "russian accent",
        "japanese accent",
    ])
});

/// Chinese-only dialect category (no English counterpart).
static CAT_DIALECT: LazyLock<BiCategory> = LazyLock::new(|| {
    BiCategory::from_flat(&[
        "\u{6CB3}\u{5357}\u{8BDD}",         // 河南话
        "\u{9655}\u{897F}\u{8BDD}",         // 陕西话
        "\u{56DB}\u{5DDD}\u{8BDD}",         // 四川话
        "\u{8D35}\u{5DDE}\u{8BDD}",         // 贵州话
        "\u{4E91}\u{5357}\u{8BDD}",         // 云南话
        "\u{6842}\u{6797}\u{8BDD}",         // 桂林话
        "\u{6D4E}\u{5357}\u{8BDD}",         // 济南话
        "\u{77F3}\u{5BB6}\u{5E84}\u{8BDD}", // 石家庄话
        "\u{7518}\u{8083}\u{8BDD}",         // 甘肃话
        "\u{5B81}\u{590F}\u{8BDD}",         // 宁夏话
        "\u{9752}\u{5C9B}\u{8BDD}",         // 青岛话
        "\u{4E1C}\u{5317}\u{8BDD}",         // 东北话
    ])
});

/// All categories in order (for conflict checking).
static ALL_CATEGORIES: LazyLock<Vec<&'static BiCategory>> = LazyLock::new(|| {
    vec![
        &*CAT_GENDER,
        &*CAT_AGE,
        &*CAT_PITCH,
        &*CAT_STYLE,
        &*CAT_ACCENT,
        &*CAT_DIALECT,
    ]
});

/// Flat set of all valid items (EN + ZH).
static ALL_VALID: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ALL_CATEGORIES
        .iter()
        .flat_map(|cat| cat.all_items.iter().copied())
        .collect()
});

/// Global EN-to-ZH mapping (union of all bidirectional categories).
static EN_TO_ZH: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for cat in [&*CAT_GENDER, &*CAT_AGE, &*CAT_PITCH, &*CAT_STYLE] {
        m.extend(cat.en_to_zh.iter());
    }
    m
});

/// Global ZH-to-EN mapping.
static ZH_TO_EN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for cat in [&*CAT_GENDER, &*CAT_AGE, &*CAT_PITCH, &*CAT_STYLE] {
        m.extend(cat.zh_to_en.iter());
    }
    m
});

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate and normalise a voice-design instruct string.
///
/// Splits on commas (both `,` and `\u{FF0C}`), validates each item against the
/// known sets, translates between EN and ZH as needed, checks
/// mutual-exclusivity within each category, and returns the normalised string.
///
/// Returns `Ok(None)` if the input is `None` or empty.
///
/// # Errors
///
/// Returns an error if:
/// - Any item is not recognised (with a suggestion for close matches).
/// - Both a Chinese dialect and an English accent are specified.
/// - Multiple items from the same category are present (e.g. both "male" and
///   "female").
pub fn resolve_instruct(instruct: Option<&str>, use_zh: bool) -> anyhow::Result<Option<String>> {
    let instruct = match instruct {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return Ok(None),
    };

    // Split on both half-width and full-width commas.
    let raw_items: Vec<&str> = instruct
        .split([',', '\u{FF0C}'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // Validate each item.
    let mut normalised: Vec<String> = Vec::new();
    let mut unknown: Vec<(String, Option<String>)> = Vec::new();

    for raw in &raw_items {
        let n = raw.to_lowercase();
        if ALL_VALID.contains(n.as_str()) {
            normalised.push(n);
        } else {
            let suggestion = find_closest(&n, &ALL_VALID);
            unknown.push((raw.to_string(), suggestion));
        }
    }

    if !unknown.is_empty() {
        let mut lines = Vec::new();
        for (raw, sug) in &unknown {
            if let Some(s) = sug {
                lines.push(format!("  '{raw}' (unsupported; did you mean '{s}'?)"));
            } else {
                lines.push(format!("  '{raw}' (unsupported)"));
            }
        }
        anyhow::bail!(
            "Unsupported instruct items:\n{}\n\nValid English items: {}\nValid Chinese items: {}",
            lines.join("\n"),
            sorted_en_items().join(", "),
            sorted_zh_items().join("\u{FF0C}"),
        );
    }

    // Dialect forces Chinese, accent forces English.
    let has_dialect = normalised.iter().any(|n| n.ends_with('\u{8BDD}')); // 话
    let has_accent = normalised.iter().any(|n| n.contains(" accent"));

    if has_dialect && has_accent {
        anyhow::bail!(
            "Cannot mix Chinese dialect and English accent in a single instruct. \
             Dialects are for Chinese speech, accents for English speech."
        );
    }

    let use_zh = if has_dialect {
        true
    } else if has_accent {
        false
    } else {
        use_zh
    };

    // Unify to a single language.
    let normalised: Vec<String> = if use_zh {
        normalised
            .into_iter()
            .map(|n| EN_TO_ZH.get(n.as_str()).map(|s| s.to_string()).unwrap_or(n))
            .collect()
    } else {
        normalised
            .into_iter()
            .map(|n| ZH_TO_EN.get(n.as_str()).map(|s| s.to_string()).unwrap_or(n))
            .collect()
    };

    // Category conflict check.
    let mut conflicts: Vec<Vec<String>> = Vec::new();
    for cat in ALL_CATEGORIES.iter() {
        let hits: Vec<String> = normalised
            .iter()
            .filter(|n| cat.all_items.contains(n.as_str()))
            .cloned()
            .collect();
        if hits.len() > 1 {
            conflicts.push(hits);
        }
    }
    if !conflicts.is_empty() {
        let parts: Vec<String> = conflicts
            .iter()
            .map(|group| {
                group
                    .iter()
                    .map(|x| format!("'{x}'"))
                    .collect::<Vec<_>>()
                    .join(" vs ")
            })
            .collect();
        anyhow::bail!(
            "Conflicting instruct items within the same category: {}. \
             Each category (gender, age, pitch, style, accent, dialect) allows at most one item.",
            parts.join("; ")
        );
    }

    // Determine separator based on language of the final items.
    let has_zh = normalised
        .iter()
        .any(|n| n.chars().any(crate::tts::omni_voice::utils::text::is_cjk));
    let separator = if has_zh { "\u{FF0C}" } else { ", " };

    Ok(Some(normalised.join(separator)))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Simple close-match finder using Levenshtein edit distance.
fn find_closest(query: &str, valid: &HashSet<&str>) -> Option<String> {
    let threshold = (query.len() as f64 * 0.4).ceil() as usize;
    let threshold = threshold.max(2);

    let mut best: Option<(&str, usize)> = None;

    for &candidate in valid {
        let dist = levenshtein(query, candidate);
        if dist <= threshold && (best.is_none() || dist < best.unwrap().1) {
            best = Some((candidate, dist));
        }
    }

    best.map(|(s, _)| s.to_string())
}

/// Compute the Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

fn sorted_en_items() -> Vec<&'static str> {
    let mut items: Vec<&str> = ALL_VALID
        .iter()
        .copied()
        .filter(|s| !s.chars().any(crate::tts::omni_voice::utils::text::is_cjk))
        .collect();
    items.sort();
    items
}

fn sorted_zh_items() -> Vec<&'static str> {
    let mut items: Vec<&str> = ALL_VALID
        .iter()
        .copied()
        .filter(|s| s.chars().any(crate::tts::omni_voice::utils::text::is_cjk))
        .collect();
    items.sort();
    items
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_none() {
        assert!(resolve_instruct(None, false).unwrap().is_none());
        assert!(resolve_instruct(Some(""), false).unwrap().is_none());
        assert!(resolve_instruct(Some("  "), false).unwrap().is_none());
    }

    #[test]
    fn test_resolve_single_en() {
        let result = resolve_instruct(Some("male"), false).unwrap().unwrap();
        assert_eq!(result, "male");
    }

    #[test]
    fn test_resolve_single_zh() {
        let result = resolve_instruct(Some("male"), true).unwrap().unwrap();
        assert_eq!(result, "\u{7537}"); // 男
    }

    #[test]
    fn test_resolve_multiple_en() {
        let result = resolve_instruct(Some("male, young adult, high pitch"), false)
            .unwrap()
            .unwrap();
        assert_eq!(result, "male, young adult, high pitch");
    }

    #[test]
    fn test_resolve_translate_to_zh() {
        let result = resolve_instruct(Some("female, elderly"), true)
            .unwrap()
            .unwrap();
        // 女，老年
        assert!(result.contains('\u{5973}'));
        assert!(result.contains("\u{8001}\u{5E74}"));
    }

    #[test]
    fn test_resolve_dialect_forces_zh() {
        // Even with use_zh=false, dialect forces Chinese output.
        let result = resolve_instruct(Some("male, \u{56DB}\u{5DDD}\u{8BDD}"), false)
            .unwrap()
            .unwrap();
        // Should translate "male" to "男"
        assert!(result.contains('\u{7537}'));
        assert!(result.contains("\u{56DB}\u{5DDD}\u{8BDD}"));
    }

    #[test]
    fn test_resolve_accent_forces_en() {
        let result = resolve_instruct(Some("\u{7537}, british accent"), true)
            .unwrap()
            .unwrap();
        // Should translate 男 to "male"
        assert!(result.contains("male"));
        assert!(result.contains("british accent"));
    }

    #[test]
    fn test_resolve_conflict_dialect_accent() {
        let result = resolve_instruct(Some("\u{56DB}\u{5DDD}\u{8BDD}, british accent"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_conflict_same_category() {
        let result = resolve_instruct(Some("male, female"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_unknown_item() {
        let result = resolve_instruct(Some("male, robotic"), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_fullwidth_comma() {
        let result = resolve_instruct(Some("male\u{FF0C}young adult"), false)
            .unwrap()
            .unwrap();
        assert!(result.contains("male"));
        assert!(result.contains("young adult"));
    }

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        assert_eq!(levenshtein("hello", "helo"), 1);
        assert_eq!(levenshtein("hello", "jello"), 1);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }
}
