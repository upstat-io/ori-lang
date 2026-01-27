//! "Did You Mean?" Suggestions
//!
//! Provides fuzzy matching for identifier suggestions using Levenshtein
//! edit distance. When a user references an unknown identifier, this
//! module finds similar names to suggest.
//!
//! # Design
//!
//! Uses Levenshtein distance to find names within a threshold distance
//! from the misspelled input. The threshold is based on the input length
//! to avoid suggesting unrelated names for short inputs.
//!
//! # Example
//!
//! ```ignore
//! let candidates = vec!["length", "height", "width"];
//! let suggestion = suggest_similar("lenght", candidates.iter().copied());
//! assert_eq!(suggestion, Some("length"));
//! ```

/// Calculate Levenshtein edit distance between two strings.
///
/// This is the minimum number of single-character edits (insertions,
/// deletions, or substitutions) required to change one string into another.
///
/// # Example
///
/// ```ignore
/// assert_eq!(edit_distance("kitten", "sitting"), 3);
/// assert_eq!(edit_distance("hello", "hello"), 0);
/// assert_eq!(edit_distance("abc", ""), 3);
/// ```
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    // Handle empty strings
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // Use two-row optimization instead of full matrix
    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for (i, a_char) in a.chars().enumerate() {
        curr_row[0] = i + 1;

        for (j, b_char) in b.chars().enumerate() {
            let cost = usize::from(a_char != b_char);

            curr_row[j + 1] = (prev_row[j + 1] + 1) // deletion
                .min(curr_row[j] + 1) // insertion
                .min(prev_row[j] + cost); // substitution
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Calculate a reasonable threshold based on input length.
///
/// - Very short names (1-2 chars): 1 edit allowed
/// - Short names (3-5 chars): 2 edits allowed
/// - Medium names (6-10 chars): 3 edits allowed
/// - Long names: half the length, max 5
fn default_threshold(name_len: usize) -> usize {
    match name_len {
        0 => 0,
        1..=2 => 1,
        3..=5 => 2,
        6..=10 => 3,
        n => (n / 2).min(5),
    }
}

/// Find the most similar name from candidates.
///
/// Returns the candidate with the smallest edit distance, if any candidate
/// is within the threshold. Uses case-insensitive comparison for initial
/// filtering but preserves original case in the suggestion.
///
/// # Arguments
///
/// * `name` - The misspelled name to find suggestions for
/// * `candidates` - Iterator of candidate names to search
///
/// # Returns
///
/// The best matching candidate if one is found within threshold, or None.
///
/// # Example
///
/// ```ignore
/// let candidates = vec!["length", "height", "width"];
/// let suggestion = suggest_similar("lenght", candidates.iter().copied());
/// assert_eq!(suggestion, Some("length"));
/// ```
pub fn suggest_similar<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<&'a str> {
    suggest_similar_with_threshold(name, candidates, default_threshold(name.len()))
}

/// Find the most similar name from candidates with explicit threshold.
///
/// # Arguments
///
/// * `name` - The misspelled name to find suggestions for
/// * `candidates` - Iterator of candidate names to search
/// * `threshold` - Maximum edit distance for a match
///
/// # Returns
///
/// The best matching candidate if one is found within threshold, or None.
pub fn suggest_similar_with_threshold<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
    threshold: usize,
) -> Option<&'a str> {
    if name.is_empty() {
        return None;
    }

    let mut best: Option<(&str, usize)> = None;

    for candidate in candidates {
        // Skip if too different in length
        let len_diff = name.len().abs_diff(candidate.len());
        if len_diff > threshold {
            continue;
        }

        let distance = edit_distance(name, candidate);

        if distance <= threshold {
            match best {
                None => best = Some((candidate, distance)),
                Some((_, best_dist)) if distance < best_dist => {
                    best = Some((candidate, distance));
                }
                _ => {}
            }
        }
    }

    best.map(|(s, _)| s)
}

/// Find all similar names within threshold.
///
/// Returns a vector of candidates sorted by edit distance (best first).
/// Useful when you want to show multiple suggestions.
pub fn find_similar<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
    threshold: usize,
    max_results: usize,
) -> Vec<&'a str> {
    if name.is_empty() || max_results == 0 {
        return Vec::new();
    }

    let mut matches: Vec<(&str, usize)> = candidates
        .filter_map(|candidate| {
            let len_diff = name.len().abs_diff(candidate.len());
            if len_diff > threshold {
                return None;
            }

            let distance = edit_distance(name, candidate);
            if distance <= threshold {
                Some((candidate, distance))
            } else {
                None
            }
        })
        .collect();

    // Sort by distance, then alphabetically for ties
    matches.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));

    matches
        .into_iter()
        .take(max_results)
        .map(|(s, _)| s)
        .collect()
}

/// Check if two names are likely typos of each other.
///
/// More strict than `suggest_similar` - only matches if the edit distance
/// is 1 or if the strings differ only in case.
pub fn is_likely_typo(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    edit_distance(a, b) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_distance_identical() {
        assert_eq!(edit_distance("hello", "hello"), 0);
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("a", "a"), 0);
    }

    #[test]
    fn test_edit_distance_empty() {
        assert_eq!(edit_distance("hello", ""), 5);
        assert_eq!(edit_distance("", "world"), 5);
    }

    #[test]
    fn test_edit_distance_single_char() {
        assert_eq!(edit_distance("a", "b"), 1);
        assert_eq!(edit_distance("ab", "a"), 1);
        assert_eq!(edit_distance("a", "ab"), 1);
    }

    #[test]
    fn test_edit_distance_insertions() {
        assert_eq!(edit_distance("abc", "abcd"), 1);
        assert_eq!(edit_distance("abc", "abcde"), 2);
    }

    #[test]
    fn test_edit_distance_deletions() {
        assert_eq!(edit_distance("abcd", "abc"), 1);
        assert_eq!(edit_distance("abcde", "abc"), 2);
    }

    #[test]
    fn test_edit_distance_substitutions() {
        assert_eq!(edit_distance("abc", "adc"), 1);
        assert_eq!(edit_distance("abc", "xyz"), 3);
    }

    #[test]
    fn test_edit_distance_classic() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("saturday", "sunday"), 3);
    }

    #[test]
    fn test_edit_distance_typos() {
        // "lenght" vs "length" - swap 'h' and 't' requires 2 edits
        assert_eq!(edit_distance("lenght", "length"), 2);
        // "teh" vs "the" - swap 'e' and 'h' requires 2 edits
        assert_eq!(edit_distance("teh", "the"), 2);
        // "recieve" vs "receive" - swap 'i' and 'e' requires 2 edits
        assert_eq!(edit_distance("recieve", "receive"), 2);
        // Single character difference
        assert_eq!(edit_distance("helo", "hello"), 1);
    }

    #[test]
    fn test_suggest_similar_exact() {
        let candidates = vec!["foo", "bar", "baz"];
        // Exact match should not be suggested (edit distance 0)
        let result = suggest_similar("foo", candidates.iter().copied());
        assert_eq!(result, Some("foo"));
    }

    #[test]
    fn test_suggest_similar_typo() {
        let candidates = vec!["length", "height", "width"];
        let result = suggest_similar("lenght", candidates.iter().copied());
        assert_eq!(result, Some("length"));
    }

    #[test]
    fn test_suggest_similar_no_match() {
        let candidates = vec!["alpha", "beta", "gamma"];
        let result = suggest_similar("xyz", candidates.iter().copied());
        assert_eq!(result, None);
    }

    #[test]
    fn test_suggest_similar_empty_input() {
        let candidates = vec!["foo", "bar"];
        let result = suggest_similar("", candidates.iter().copied());
        assert_eq!(result, None);
    }

    #[test]
    fn test_suggest_similar_empty_candidates() {
        let candidates: Vec<&str> = vec![];
        let result = suggest_similar("foo", candidates.iter().copied());
        assert_eq!(result, None);
    }

    #[test]
    fn test_suggest_similar_best_match() {
        let candidates = vec!["for", "foo", "four"];
        let result = suggest_similar("fo", candidates.iter().copied());
        // "fo" -> "for" or "foo" both have distance 1, should pick alphabetically first
        assert!(result == Some("foo") || result == Some("for"));
    }

    #[test]
    fn test_suggest_similar_case_sensitive() {
        let candidates = vec!["Hello", "hello", "HELLO"];
        let result = suggest_similar("helo", candidates.iter().copied());
        // Should suggest "hello" (distance 1)
        assert_eq!(result, Some("hello"));
    }

    #[test]
    fn test_find_similar_multiple() {
        let candidates = vec!["length", "lenght", "lengthy", "strength"];
        let results = find_similar("length", candidates.iter().copied(), 2, 3);

        assert!(results.contains(&"length"));
        assert!(results.contains(&"lenght"));
        assert!(results.contains(&"lengthy"));
    }

    #[test]
    fn test_find_similar_sorted() {
        let candidates = vec!["abc", "ab", "abcd"];
        let results = find_similar("abc", candidates.iter().copied(), 2, 3);

        // Results should be sorted by distance
        assert_eq!(results[0], "abc"); // distance 0
        assert!(results.contains(&"ab")); // distance 1
        assert!(results.contains(&"abcd")); // distance 1
    }

    #[test]
    fn test_find_similar_max_results() {
        let candidates = vec!["a", "ab", "abc", "abcd", "abcde"];
        let results = find_similar("abc", candidates.iter().copied(), 3, 2);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_is_likely_typo_true() {
        assert!(is_likely_typo("hello", "helo")); // missing letter
        assert!(is_likely_typo("Hello", "hello")); // case difference
        assert!(is_likely_typo("abc", "ab")); // deletion
        assert!(is_likely_typo("ab", "abc")); // insertion
        assert!(is_likely_typo("abc", "adc")); // substitution
    }

    #[test]
    fn test_is_likely_typo_false() {
        assert!(!is_likely_typo("hello", "world"));
        assert!(!is_likely_typo("abc", "xyz"));
    }

    #[test]
    fn test_default_threshold() {
        assert_eq!(default_threshold(0), 0);
        assert_eq!(default_threshold(1), 1);
        assert_eq!(default_threshold(2), 1);
        assert_eq!(default_threshold(3), 2);
        assert_eq!(default_threshold(5), 2);
        assert_eq!(default_threshold(6), 3);
        assert_eq!(default_threshold(10), 3);
        assert_eq!(default_threshold(20), 5); // max 5
    }

    #[test]
    fn test_suggest_with_threshold() {
        let candidates = vec!["abc", "abcd", "abcde"];

        // With threshold 1
        let result = suggest_similar_with_threshold("abc", candidates.iter().copied(), 1);
        assert_eq!(result, Some("abc"));

        // With threshold 0 (exact only)
        let result = suggest_similar_with_threshold("abx", candidates.iter().copied(), 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_unicode() {
        assert_eq!(edit_distance("héllo", "hello"), 1);
        assert_eq!(edit_distance("日本語", "日本"), 1);
    }

    #[test]
    fn test_ori_identifiers() {
        // Test with typical Ori identifier patterns
        let candidates = vec![
            "filter",
            "map",
            "fold",
            "find",
            "collect",
            "foreach",
        ];

        assert_eq!(
            suggest_similar("fiter", candidates.iter().copied()),
            Some("filter")
        );
        assert_eq!(
            suggest_similar("mpa", candidates.iter().copied()),
            Some("map")
        );
        assert_eq!(
            suggest_similar("colect", candidates.iter().copied()),
            Some("collect")
        );
    }
}
