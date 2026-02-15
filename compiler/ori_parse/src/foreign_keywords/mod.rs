//! Foreign keyword detection for cross-language habit messages.
//!
//! Known keywords from other languages that are NOT Ori keywords.
//! When encountered at declaration position, the parser emits a helpful
//! error suggesting the Ori equivalent (e.g., `fn` → `@name (params) -> type = body`).
//!
//! These are valid identifiers in Ori — the error is only emitted when
//! they appear where a declaration is expected.

/// Known keywords from other languages with their Ori equivalents.
///
/// Sorted by keyword for binary search.
const FOREIGN_KEYWORDS: &[(&str, &str)] = &[
    ("class", "use `type` for type definitions in Ori"),
    ("const", "use `let` for variable bindings in Ori"),
    ("enum", "use `type` with variants for enums in Ori"),
    (
        "fn",
        "use `@name (params) -> type = body` to declare functions in Ori",
    ),
    (
        "func",
        "use `@name (params) -> type = body` to declare functions in Ori",
    ),
    (
        "function",
        "use `@name (params) -> type = body` to declare functions in Ori",
    ),
    ("interface", "use `trait` for interfaces in Ori"),
    ("nil", "use `void` for the absence of a value in Ori"),
    ("null", "use `void` for the absence of a value in Ori"),
    (
        "return",
        "Ori is expression-based — the last expression in a block is its value",
    ),
    (
        "struct",
        "use `type Name = { fields }` for record types in Ori",
    ),
    ("switch", "use `match` for pattern matching in Ori"),
    ("var", "use `let` for variable bindings in Ori"),
    ("while", "use `loop` with `if`/`break` in Ori"),
];

/// Look up a foreign keyword and return its Ori-specific guidance message.
///
/// Returns `None` if the identifier is not a known foreign keyword.
pub fn lookup_foreign_keyword(ident: &str) -> Option<&'static str> {
    FOREIGN_KEYWORDS
        .binary_search_by_key(&ident, |&(kw, _)| kw)
        .ok()
        .map(|idx| FOREIGN_KEYWORDS[idx].1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "test assertions")]
mod tests;
