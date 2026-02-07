//! Foreign keyword detection for cross-language habit messages.
//!
//! Known keywords from other languages that are NOT Ori keywords.
//! When encountered, the cooker can produce a helpful note suggesting
//! the Ori equivalent. This is advisory only — these are valid identifiers
//! in Ori and do not produce errors.
//!
//! Detection deferred to later phases since identifiers don't error
//! in the lexer today. This module exists as a lookup table for
//! future use by the parser or IDE.

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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted() {
        for window in FOREIGN_KEYWORDS.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "table not sorted: {:?} >= {:?}",
                window[0].0,
                window[1].0
            );
        }
    }

    #[test]
    fn lookup_return() {
        let msg = lookup_foreign_keyword("return").unwrap();
        assert!(msg.contains("expression-based"));
    }

    #[test]
    fn lookup_null() {
        let msg = lookup_foreign_keyword("null").unwrap();
        assert!(msg.contains("void"));
    }

    #[test]
    fn lookup_class() {
        let msg = lookup_foreign_keyword("class").unwrap();
        assert!(msg.contains("type"));
    }

    #[test]
    fn lookup_unknown() {
        assert!(lookup_foreign_keyword("foo").is_none());
        assert!(lookup_foreign_keyword("let").is_none()); // Ori keyword, not foreign
    }
}
