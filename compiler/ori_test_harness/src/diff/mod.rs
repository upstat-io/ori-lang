//! Diff generation using the `similar` crate.
//!
//! Produces readable unified diffs for test failure output.

use similar::TextDiff;

/// Generate a unified diff between expected and actual text.
///
/// Uses standard unified diff format with 3 lines of context,
/// line numbers, and +/- prefixes. Designed for terminal readability.
pub fn generate_diff(expected: &str, actual: &str) -> String {
    let diff = TextDiff::from_lines(expected, actual);
    diff.unified_diff()
        .context_radius(3)
        .header("expected", "actual")
        .to_string()
}

#[cfg(test)]
mod tests;
