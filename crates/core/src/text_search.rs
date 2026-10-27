//! Bounded, Unicode-safe composer search. Positions are character offsets, as in GTK.
use std::ops::Range;
pub const MAX_MATCHES: usize = 10_000;
pub fn find(text: &str, needle: &str, match_case: bool, whole_word: bool) -> Vec<Range<i32>> {
    if text.len() > 512 * 1024 || needle.is_empty() || needle.len() > 512 {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut folded = String::with_capacity(text.len());
    let mut offsets = Vec::with_capacity(text.len());
    for (index, ch) in chars.iter().enumerate() {
        let start = folded.len();
        if match_case {
            folded.push(*ch);
        } else {
            folded.extend(ch.to_lowercase());
        }
        offsets.resize(folded.len(), index as i32);
        debug_assert!(folded.len() > start);
    }
    let query = if match_case {
        needle.to_owned()
    } else {
        needle.to_lowercase()
    };
    let word = |c: &char| c.is_alphanumeric() || *c == '_';
    let mut result = Vec::new();
    for (byte, value) in folded.match_indices(&query) {
        let range = offsets[byte]..offsets[byte + value.len() - 1] + 1;
        if whole_word
            && (range.start > 0 && word(&chars[range.start as usize - 1])
                || chars.get(range.end as usize).is_some_and(word))
        {
            continue;
        }
        if result
            .last()
            .is_some_and(|r: &Range<i32>| r.end > range.start)
        {
            continue;
        }
        result.push(range);
        if result.len() == MAX_MATCHES {
            break;
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_offsets_words_case_and_bounds() {
        assert_eq!(
            find("🙂 Café CAFE caféine", "CAFÉ", false, true),
            vec![2..6]
        );
        assert_eq!(find("İstanbul İ", "i", false, false), vec![0..1, 9..10]);
        assert!(find("cat_cat cats", "cat", false, true).is_empty());
        assert_eq!(find("Cat cat", "cat", true, true), vec![4..7]);
        assert_eq!(
            find(&"x ".repeat(20_000), "x", true, false).len(),
            MAX_MATCHES
        );
        assert!(find(&"x".repeat(512 * 1024 + 1), "x", true, false).is_empty());
    }
}
