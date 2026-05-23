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

//! Text duration estimation for TTS generation.
//!
//! Provides [`RuleDurationEstimator`], which estimates audio duration from text
//! using character phonetic weights across 600+ languages. Used by
//! `OmniVoice::generate()` to determine output length when no duration is
//! specified.

use std::collections::HashMap;

/// Script type identifier used as key into the weight table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Script {
    Cjk,
    Hangul,
    Kana,
    Ethiopic,
    Yi,
    Indic,
    ThaiLao,
    KhmerMyanmar,
    Arabic,
    Hebrew,
    Latin,
    Cyrillic,
    Greek,
    Armenian,
    Georgian,
    Punctuation,
    Space,
    Digit,
    Mark,
    Default,
}

/// Estimates speech duration from text using per-character phonetic weights.
///
/// Each Unicode character is mapped to a script category with a relative
/// speaking time weight (baseline: 1.0 = one Latin letter, roughly 40-50 ms).
///
/// # Example
///
/// ```
/// use omnivoice_rs::utils::duration::RuleDurationEstimator;
///
/// let estimator = RuleDurationEstimator::new();
/// let dur = estimator.estimate_duration("Hello, world!", "Nice to meet you.", 25.0);
/// assert!(dur > 0.0);
/// ```
pub struct RuleDurationEstimator {
    weights: HashMap<Script, f64>,
    /// Sorted list of `(end_codepoint, script)` for binary search.
    ranges: Vec<(u32, Script)>,
    /// Breakpoints extracted from `ranges` for binary search.
    breakpoints: Vec<u32>,
}

impl RuleDurationEstimator {
    /// Create a new estimator with the default weight table and Unicode range
    /// mapping.
    pub fn new() -> Self {
        let weights = HashMap::from([
            (Script::Cjk, 3.0),
            (Script::Hangul, 2.5),
            (Script::Kana, 2.2),
            (Script::Ethiopic, 3.0),
            (Script::Yi, 3.0),
            (Script::Indic, 1.8),
            (Script::ThaiLao, 1.5),
            (Script::KhmerMyanmar, 1.8),
            (Script::Arabic, 1.5),
            (Script::Hebrew, 1.5),
            (Script::Latin, 1.0),
            (Script::Cyrillic, 1.0),
            (Script::Greek, 1.0),
            (Script::Armenian, 1.0),
            (Script::Georgian, 1.0),
            (Script::Punctuation, 0.5),
            (Script::Space, 0.2),
            (Script::Digit, 3.5),
            (Script::Mark, 0.0),
            (Script::Default, 1.0),
        ]);

        let ranges = vec![
            (0x02AF, Script::Latin),
            (0x03FF, Script::Greek),
            (0x052F, Script::Cyrillic),
            (0x058F, Script::Armenian),
            (0x05FF, Script::Hebrew),
            (0x077F, Script::Arabic),
            (0x089F, Script::Arabic),
            (0x08FF, Script::Arabic),
            (0x097F, Script::Indic),
            (0x09FF, Script::Indic),
            (0x0A7F, Script::Indic),
            (0x0AFF, Script::Indic),
            (0x0B7F, Script::Indic),
            (0x0BFF, Script::Indic),
            (0x0C7F, Script::Indic),
            (0x0CFF, Script::Indic),
            (0x0D7F, Script::Indic),
            (0x0DFF, Script::Indic),
            (0x0EFF, Script::ThaiLao),
            (0x0FFF, Script::Indic),
            (0x109F, Script::KhmerMyanmar),
            (0x10FF, Script::Georgian),
            (0x11FF, Script::Hangul),
            (0x137F, Script::Ethiopic),
            (0x139F, Script::Ethiopic),
            (0x13FF, Script::Default),
            (0x167F, Script::Default),
            (0x169F, Script::Default),
            (0x16FF, Script::Default),
            (0x171F, Script::Default),
            (0x173F, Script::Default),
            (0x175F, Script::Default),
            (0x177F, Script::Default),
            (0x17FF, Script::KhmerMyanmar),
            (0x18AF, Script::Default),
            (0x18FF, Script::Default),
            (0x194F, Script::Indic),
            (0x19DF, Script::Indic),
            (0x19FF, Script::KhmerMyanmar),
            (0x1A1F, Script::Indic),
            (0x1AAF, Script::Indic),
            (0x1B7F, Script::Indic),
            (0x1BBF, Script::Indic),
            (0x1BFF, Script::Indic),
            (0x1C4F, Script::Indic),
            (0x1C7F, Script::Indic),
            (0x1C8F, Script::Cyrillic),
            (0x1CBF, Script::Georgian),
            (0x1CCF, Script::Indic),
            (0x1CFF, Script::Indic),
            (0x1D7F, Script::Latin),
            (0x1DBF, Script::Latin),
            (0x1DFF, Script::Default),
            (0x1EFF, Script::Latin),
            (0x309F, Script::Kana),
            (0x30FF, Script::Kana),
            (0x312F, Script::Cjk),
            (0x318F, Script::Hangul),
            (0x9FFF, Script::Cjk),
            (0xA4CF, Script::Yi),
            (0xA4FF, Script::Default),
            (0xA63F, Script::Default),
            (0xA69F, Script::Cyrillic),
            (0xA6FF, Script::Default),
            (0xA7FF, Script::Latin),
            (0xA82F, Script::Indic),
            (0xA87F, Script::Default),
            (0xA8DF, Script::Indic),
            (0xA8FF, Script::Indic),
            (0xA92F, Script::Indic),
            (0xA95F, Script::Indic),
            (0xA97F, Script::Hangul),
            (0xA9DF, Script::Indic),
            (0xA9FF, Script::KhmerMyanmar),
            (0xAA5F, Script::Indic),
            (0xAA7F, Script::KhmerMyanmar),
            (0xAADF, Script::Indic),
            (0xAAFF, Script::Indic),
            (0xAB2F, Script::Ethiopic),
            (0xAB6F, Script::Latin),
            (0xABBF, Script::Default),
            (0xABFF, Script::Indic),
            (0xD7AF, Script::Hangul),
            (0xFAFF, Script::Cjk),
            (0xFDFF, Script::Arabic),
            (0xFE6F, Script::Default),
            (0xFEFF, Script::Arabic),
            (0xFFEF, Script::Latin),
        ];

        let breakpoints = ranges.iter().map(|&(bp, _)| bp).collect();

        Self {
            weights,
            ranges,
            breakpoints,
        }
    }

    /// Determine the phonetic weight of a single character.
    fn get_char_weight(&self, ch: char) -> f64 {
        let code = ch as u32;

        // Fast path: ASCII letters.
        if (65..=90).contains(&code) || (97..=122).contains(&code) {
            return self.weights[&Script::Latin];
        }

        // Space.
        if code == 0x20 {
            return self.weights[&Script::Space];
        }

        // Arabic Tatweel (kashida) is a spacing mark, not phonetic.
        if code == 0x0640 {
            return self.weights[&Script::Mark];
        }

        // Use Unicode general category to classify special characters.
        // We check a few categories manually to avoid pulling in a full
        // Unicode data crate. This mirrors the Python unicodedata.category()
        // checks.
        if is_unicode_mark(ch) {
            return self.weights[&Script::Mark];
        }
        if is_unicode_punctuation(ch) || is_unicode_symbol(ch) {
            return self.weights[&Script::Punctuation];
        }
        if is_unicode_separator(ch) {
            return self.weights[&Script::Space];
        }
        if ch.is_ascii_digit() || is_unicode_number(ch) {
            return self.weights[&Script::Digit];
        }

        // Binary search for Unicode block.
        let idx = self.breakpoints.partition_point(|&bp| bp < code);
        if idx < self.ranges.len() {
            let script = self.ranges[idx].1;
            return *self
                .weights
                .get(&script)
                .unwrap_or(&self.weights[&Script::Default]);
        }

        // Upper planes (CJK Extension B/C/D, etc.).
        if code > 0x20000 {
            return self.weights[&Script::Cjk];
        }

        self.weights[&Script::Default]
    }

    /// Sum the phonetic weights for all characters in a string.
    pub fn calculate_total_weight(&self, text: &str) -> f64 {
        text.chars().map(|c| self.get_char_weight(c)).sum()
    }

    /// Estimate the number of audio tokens (or duration in frames) for
    /// `target_text`, given a reference text and its known duration.
    ///
    /// When the estimated duration falls below `low_threshold` (default 50),
    /// a power-curve boost is applied to avoid unrealistically short outputs.
    ///
    /// # Arguments
    ///
    /// * `target_text` -- the text to estimate duration for.
    /// * `ref_text` -- the reference text whose duration is known.
    /// * `ref_duration` -- the measured duration of `ref_text` (in the same
    ///   unit as the desired output, e.g. number of audio frames).
    pub fn estimate_duration(&self, target_text: &str, ref_text: &str, ref_duration: f64) -> f64 {
        self.estimate_duration_with_params(target_text, ref_text, ref_duration, Some(50.0), 3.0)
    }

    /// Full-parameter variant of [`Self::estimate_duration`].
    ///
    /// * `low_threshold` -- minimum duration below which a power boost is
    ///   applied. Pass `None` to disable.
    /// * `boost_strength` -- exponent denominator for the power boost
    ///   (higher = more aggressive boost for short texts).
    pub fn estimate_duration_with_params(
        &self,
        target_text: &str,
        ref_text: &str,
        ref_duration: f64,
        low_threshold: Option<f64>,
        boost_strength: f64,
    ) -> f64 {
        if ref_duration <= 0.0 || ref_text.is_empty() {
            return 0.0;
        }

        let ref_weight = self.calculate_total_weight(ref_text);
        if ref_weight == 0.0 {
            return 0.0;
        }

        let speed_factor = ref_weight / ref_duration;
        let target_weight = self.calculate_total_weight(target_text);
        let estimated = target_weight / speed_factor;

        if let Some(threshold) = low_threshold {
            if estimated < threshold {
                let alpha = 1.0 / boost_strength;
                return threshold * (estimated / threshold).powf(alpha);
            }
        }

        estimated
    }
}

impl Default for RuleDurationEstimator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Lightweight Unicode category helpers
// ---------------------------------------------------------------------------
// These cover the most common ranges without pulling in a full Unicode data
// crate. They mirror the Python `unicodedata.category()` checks used in the
// reference implementation.

/// Check if a character is a Unicode combining mark (category M*).
fn is_unicode_mark(c: char) -> bool {
    let code = c as u32;
    // Combining Diacritical Marks (0300-036F)
    // Combining Diacritical Marks Extended (1AB0-1AFF)
    // Combining Diacritical Marks Supplement (1DC0-1DFF)
    // Combining Half Marks (FE20-FE2F)
    // Various Indic/Arabic combining marks are within script blocks and
    // handled by the Unicode range lookup, so we only need the dedicated
    // combining mark blocks here.
    matches!(
        code,
        0x0300..=0x036F
        | 0x0483..=0x0489
        | 0x0591..=0x05BD
        | 0x05BF
        | 0x05C1..=0x05C2
        | 0x05C4..=0x05C5
        | 0x05C7
        | 0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x0670
        | 0x06D6..=0x06DC
        | 0x06DF..=0x06E4
        | 0x06E7..=0x06E8
        | 0x06EA..=0x06ED
        | 0x0711
        | 0x0730..=0x074A
        | 0x07A6..=0x07B0
        | 0x0900..=0x0903
        | 0x093A..=0x094F
        | 0x0951..=0x0957
        | 0x0962..=0x0963
        | 0x0981..=0x0983
        | 0x09BC
        | 0x09BE..=0x09CD
        | 0x09D7
        | 0x09E2..=0x09E3
        | 0x0A01..=0x0A03
        | 0x0A3C
        | 0x0A3E..=0x0A4D
        | 0x0A51
        | 0x0A70..=0x0A71
        | 0x0A75
        | 0x0A81..=0x0A83
        | 0x0ABC
        | 0x0ABE..=0x0ACD
        | 0x0AE2..=0x0AE3
        | 0x0B01..=0x0B03
        | 0x0B3C
        | 0x0B3E..=0x0B4D
        | 0x0B56..=0x0B57
        | 0x0B62..=0x0B63
        | 0x0B82
        | 0x0BBE..=0x0BCD
        | 0x0BD7
        | 0x0C00..=0x0C04
        | 0x0C3C
        | 0x0C3E..=0x0C4D
        | 0x0C55..=0x0C56
        | 0x0C62..=0x0C63
        | 0x0C81..=0x0C83
        | 0x0CBC
        | 0x0CBE..=0x0CCD
        | 0x0CD5..=0x0CD6
        | 0x0CE2..=0x0CE3
        | 0x0D00..=0x0D03
        | 0x0D3B..=0x0D3C
        | 0x0D3E..=0x0D4D
        | 0x0D57
        | 0x0D62..=0x0D63
        | 0x0E31
        | 0x0E34..=0x0E3A
        | 0x0E47..=0x0E4E
        | 0x0EB1
        | 0x0EB4..=0x0EBC
        | 0x0EC8..=0x0ECE
        | 0x0F18..=0x0F19
        | 0x0F35
        | 0x0F37
        | 0x0F39
        | 0x0F3E..=0x0F3F
        | 0x0F71..=0x0F84
        | 0x0F86..=0x0F87
        | 0x0F8D..=0x0FBC
        | 0x0FC6
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20FF
        | 0xFE20..=0xFE2F
    )
}

/// Check if a character is in a Unicode punctuation category (P*).
fn is_unicode_punctuation(c: char) -> bool {
    if c.is_ascii_punctuation() {
        return true;
    }
    let code = c as u32;
    matches!(
        code,
        0x00A1..=0x00BF  // Latin-1 punctuation
        | 0x2000..=0x206F // General Punctuation
        | 0x2E00..=0x2E7F // Supplemental Punctuation
        | 0x3000..=0x303F // CJK Symbols and Punctuation
        | 0xFE30..=0xFE4F // CJK Compatibility Forms
        | 0xFE50..=0xFE6B // Small Form Variants
        | 0xFF01..=0xFF0F // Fullwidth punctuation
        | 0xFF1A..=0xFF20 // Fullwidth punctuation
        | 0xFF3B..=0xFF40 // Fullwidth brackets
        | 0xFF5B..=0xFF65 // Fullwidth punctuation
    )
}

/// Check if a character is a Unicode symbol (category S*).
fn is_unicode_symbol(c: char) -> bool {
    let code = c as u32;
    matches!(
        code,
        0x00A2..=0x00A9
        | 0x00AC
        | 0x00AE..=0x00B1
        | 0x00B4
        | 0x00B6
        | 0x00B8
        | 0x00D7
        | 0x00F7
        | 0x02C2..=0x02C5
        | 0x02D2..=0x02DF
        | 0x02E5..=0x02EB
        | 0x02ED
        | 0x02EF..=0x02FF
        | 0x2100..=0x214F // Letterlike Symbols
        | 0x2190..=0x21FF // Arrows
        | 0x2200..=0x22FF // Mathematical Operators
        | 0x2300..=0x23FF // Miscellaneous Technical
        | 0x2400..=0x243F // Control Pictures
        | 0x2440..=0x245F // OCR
        | 0x2460..=0x24FF // Enclosed Alphanumerics
        | 0x2500..=0x257F // Box Drawing
        | 0x2580..=0x259F // Block Elements
        | 0x25A0..=0x25FF // Geometric Shapes
        | 0x2600..=0x26FF // Miscellaneous Symbols
        | 0x2700..=0x27BF // Dingbats
        | 0x27C0..=0x27EF // Misc Mathematical Symbols-A
        | 0x27F0..=0x27FF // Supplemental Arrows-A
        | 0x2900..=0x297F // Supplemental Arrows-B
        | 0x2980..=0x29FF // Misc Mathematical Symbols-B
        | 0x2A00..=0x2AFF // Supplemental Mathematical Operators
        | 0x2B00..=0x2BFF // Misc Symbols and Arrows
    )
}

/// Check if a character is a Unicode separator (category Z*).
fn is_unicode_separator(c: char) -> bool {
    let code = c as u32;
    matches!(
        code,
        0x00A0          // No-Break Space
        | 0x1680        // Ogham Space Mark
        | 0x2000
            ..=0x200A // Various spaces
        | 0x2028        // Line Separator
        | 0x2029        // Paragraph Separator
        | 0x202F        // Narrow No-Break Space
        | 0x205F        // Medium Mathematical Space
        | 0x3000 // Ideographic Space
    )
}

/// Check if a character is a Unicode number (category N*) beyond ASCII digits.
fn is_unicode_number(c: char) -> bool {
    if c.is_ascii_digit() {
        return true;
    }
    let code = c as u32;
    matches!(
        code,
        0x0660..=0x0669 // Arabic-Indic digits
        | 0x06F0..=0x06F9 // Extended Arabic-Indic digits
        | 0x07C0..=0x07C9 // NKo digits
        | 0x0966..=0x096F // Devanagari digits
        | 0x09E6..=0x09EF // Bengali digits
        | 0x0A66..=0x0A6F // Gurmukhi digits
        | 0x0AE6..=0x0AEF // Gujarati digits
        | 0x0B66..=0x0B6F // Oriya digits
        | 0x0BE6..=0x0BEF // Tamil digits
        | 0x0C66..=0x0C6F // Telugu digits
        | 0x0CE6..=0x0CEF // Kannada digits
        | 0x0D66..=0x0D6F // Malayalam digits
        | 0x0DE6..=0x0DEF // Sinhala digits
        | 0x0E50..=0x0E59 // Thai digits
        | 0x0ED0..=0x0ED9 // Lao digits
        | 0x0F20..=0x0F33 // Tibetan digits
        | 0x1040..=0x1049 // Myanmar digits
        | 0x17E0..=0x17E9 // Khmer digits
        | 0xFF10..=0xFF19 // Fullwidth digits
        | 0x2070..=0x2079 // Superscripts
        | 0x2080..=0x2089 // Subscripts
        | 0x2150..=0x218F // Number Forms
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn estimator() -> RuleDurationEstimator {
        RuleDurationEstimator::new()
    }

    #[test]
    fn test_latin_weight() {
        let e = estimator();
        let w = e.get_char_weight('A');
        assert!((w - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cjk_weight() {
        let e = estimator();
        let w = e.get_char_weight('\u{4F60}'); // 你
        assert!((w - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_space_weight() {
        let e = estimator();
        let w = e.get_char_weight(' ');
        assert!((w - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_digit_weight() {
        let e = estimator();
        let w = e.get_char_weight('5');
        assert!((w - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_weight_hello() {
        let e = estimator();
        // "Hello" = 5 Latin chars => 5.0
        let w = e.calculate_total_weight("Hello");
        assert!((w - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_duration_basic() {
        let e = estimator();
        // ref_text = "Hello" (weight 5.0), ref_duration = 5.0
        // => speed_factor = 1.0
        // target = "Hi" (weight 2.0) => estimated = 2.0
        // 2.0 < 50 => boosted: 50 * (2/50)^(1/3)
        let dur = e.estimate_duration("Hi", "Hello", 5.0);
        assert!(dur > 0.0);
    }

    #[test]
    fn test_estimate_duration_no_boost() {
        let e = estimator();
        // Use a long enough target to exceed the 50 threshold.
        // "Hello" weight = 5.0, duration = 1.0 => speed = 5.0
        // A 300-char Latin string weight = 300.0 => est = 300/5 = 60.0 > 50
        let long_text: String = std::iter::repeat('a').take(300).collect();
        let dur = e.estimate_duration(&long_text, "Hello", 1.0);
        assert!((dur - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_duration_zero_ref() {
        let e = estimator();
        assert_eq!(e.estimate_duration("Hi", "", 10.0), 0.0);
        assert_eq!(e.estimate_duration("Hi", "Hello", 0.0), 0.0);
        assert_eq!(e.estimate_duration("Hi", "Hello", -1.0), 0.0);
    }

    #[test]
    fn test_chinese_weight() {
        let e = estimator();
        // "你好" = 2 CJK chars => 6.0
        let w = e.calculate_total_weight("你好");
        assert!((w - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mixed_script() {
        let e = estimator();
        // "Hello 你好" = 5 Latin + 1 space + 2 CJK = 5 + 0.2 + 6 = 11.2
        let w = e.calculate_total_weight("Hello 你好");
        assert!((w - 11.2).abs() < f64::EPSILON);
    }
}
