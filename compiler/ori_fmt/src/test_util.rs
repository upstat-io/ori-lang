//! Shared test-only helpers used across `formatter/tests.rs` and `width/tests.rs`.

/// Check that a source file has no wildcard match arms in non-comment lines.
///
/// Shared exhaustiveness guard for the three parallel `ExprKind` dispatch
/// tables (`emit_broken`, `emit_stacked`, `emit_inline`, `calculate_width`):
/// each match is already compile-time exhaustive (no `_` arm compiles today),
/// but this textual scan additionally guards against a future edit collapsing
/// several arms into a wildcard, which would still compile yet silently
/// swallow any variant added afterward.
pub(crate) fn has_wildcard_match_arm(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        // Skip comments and doc comments
        !trimmed.starts_with("//") && !trimmed.starts_with("///") && trimmed.contains("_ =>")
    })
}
