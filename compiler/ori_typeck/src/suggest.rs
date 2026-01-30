//! "Did you mean?" suggestions for the type checker.
//!
//! This module provides functions to suggest similar names when an unknown
//! identifier or function is referenced. It leverages the Levenshtein distance
//! implementation from `oric::suggest`.

use crate::checker::TypeChecker;
use ori_ir::Name;

/// Suggest a similar identifier name.
///
/// Searches the type environment for names similar to the given unknown name
/// using edit distance.
///
/// # Arguments
///
/// * `checker` - The type checker (provides access to environment and interner)
/// * `unknown_name` - The unknown identifier name
///
/// # Returns
///
/// An optional suggestion string if a similar name is found.
pub fn suggest_identifier(checker: &TypeChecker<'_>, unknown_name: Name) -> Option<String> {
    let unknown_str = checker.context.interner.lookup(unknown_name);

    // Pass iterator directly to avoid intermediate Vec allocation
    let candidates = checker
        .inference
        .env
        .names()
        .map(|name| checker.context.interner.lookup(name));

    suggest_similar(unknown_str, candidates)
}

/// Suggest a similar function name.
///
/// Searches for function names similar to the given unknown function reference.
///
/// # Arguments
///
/// * `checker` - The type checker (provides access to environment and interner)
/// * `unknown_name` - The unknown function name (without `@` prefix)
///
/// # Returns
///
/// An optional suggestion string if a similar name is found.
pub fn suggest_function(checker: &TypeChecker<'_>, unknown_name: Name) -> Option<String> {
    let unknown_str = checker.context.interner.lookup(unknown_name);

    // Pass iterator directly to avoid intermediate Vec allocation
    // Functions are stored as regular bindings with function types
    let candidates = checker
        .inference
        .env
        .names()
        .map(|name| checker.context.interner.lookup(name));

    suggest_similar(unknown_str, candidates)
}

/// Suggest a similar type name.
///
/// Searches the type registry for type names similar to the given unknown type.
///
/// # Arguments
///
/// * `checker` - The type checker (provides access to interner and type registry)
/// * `unknown_type` - The unknown type name
///
/// # Returns
///
/// An optional suggestion string if a similar type name is found.
pub fn suggest_type(checker: &TypeChecker<'_>, unknown_type: Name) -> Option<String> {
    let unknown_str = checker.context.interner.lookup(unknown_type);

    // Get all registered type names
    let candidates = checker
        .registries
        .types
        .names()
        .map(|name| checker.context.interner.lookup(name));

    suggest_similar(unknown_str, candidates)
}

/// Suggest a similar struct field name.
///
/// Searches for field names similar to the given unknown field within a struct type.
///
/// # Arguments
///
/// * `checker` - The type checker (provides access to interner and type registry)
/// * `type_name` - The name of the struct type
/// * `unknown_field` - The unknown field name
///
/// # Returns
///
/// An optional suggestion string if a similar field name is found.
pub fn suggest_field(
    checker: &TypeChecker<'_>,
    type_name: Name,
    unknown_field: Name,
) -> Option<String> {
    let unknown_str = checker.context.interner.lookup(unknown_field);

    let entry = checker.registries.types.get_by_name(type_name)?;

    if let crate::registry::TypeKind::Struct { fields } = &entry.kind {
        let candidates = fields
            .iter()
            .map(|(name, _)| checker.context.interner.lookup(*name));

        suggest_similar(unknown_str, candidates)
    } else {
        None
    }
}

/// Calculate Levenshtein edit distance between two strings.
///
/// This is the minimum number of single-character edits (insertions,
/// deletions, or substitutions) required to change one string into another.
fn edit_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

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
/// is within the threshold.
fn suggest_similar<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    if name.is_empty() {
        return None;
    }

    let threshold = default_threshold(name.len());
    let mut best: Option<(&str, usize)> = None;

    for candidate in candidates {
        // Skip if too different in length
        let len_diff = name.len().abs_diff(candidate.len());
        if len_diff > threshold {
            continue;
        }

        // Skip exact matches (they wouldn't be "unknown")
        if candidate == name {
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

    best.map(|(s, _)| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_distance() {
        assert_eq!(edit_distance("hello", "hello"), 0);
        assert_eq!(edit_distance("hello", "helo"), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_suggest_similar() {
        let candidates = vec!["length", "height", "width"];
        let result = suggest_similar("lenght", candidates.into_iter());
        assert_eq!(result, Some("length".to_string()));
    }

    #[test]
    fn test_suggest_similar_no_match() {
        let candidates = vec!["alpha", "beta", "gamma"];
        let result = suggest_similar("xyz", candidates.into_iter());
        assert_eq!(result, None);
    }

    #[test]
    fn test_suggest_similar_skips_exact() {
        let candidates = vec!["foo", "bar"];
        let result = suggest_similar("foo", candidates.into_iter());
        // Exact match should not be suggested
        assert_eq!(result, None);
    }
}
