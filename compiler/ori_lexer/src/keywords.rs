//! Keyword resolution for the V2 cooking layer.
//!
//! Two-table keyword system:
//! 1. **Reserved keywords** — length-bucketed lookup, always resolved
//! 2. **Soft keywords** — context-sensitive pattern keywords resolved via `(` lookahead
//!
//! # Reserved Keywords
//!
//! These are always resolved as keyword tokens. The lookup function uses the
//! identifier's length as a first-pass filter (keywords range from 2-11 chars),
//! then matches against the specific keywords of that length.
//!
//! # Soft Keywords (Context-Sensitive)
//!
//! Six pattern keywords (`cache`, `catch`, `parallel`, `spawn`, `recurse`,
//! `timeout`) are only recognized as keywords when followed by `(`.
//! This allows them to be used as identifiers in non-keyword positions
//! (e.g., `let cache = 42`), eliminating parser compensation.
//!
//! The lookahead skips horizontal whitespace (space/tab) but NOT newlines,
//! so `cache\n(...)` parses as identifier `cache` followed by a parenthesized
//! expression, while `cache (...)` parses as a keyword call.

use ori_ir::TokenKind;

/// Look up a reserved keyword by text.
///
/// Returns the corresponding `TokenKind` if the text is a reserved keyword,
/// `None` if it's a regular identifier (or a soft keyword — those are handled
/// separately by [`soft_keyword_lookup`]).
///
/// Uses length-bucketing for fast rejection: identifiers whose length falls
/// outside the 2-11 range are immediately rejected without any comparison.
#[inline]
pub(crate) fn lookup(text: &str) -> Option<TokenKind> {
    let bytes = text.as_bytes();
    let len = bytes.len();

    // Guard: all keywords are 2-11 chars and start with ASCII alpha
    if !(2..=11).contains(&len) {
        return None;
    }
    let first = bytes[0];
    if !first.is_ascii_alphabetic() {
        return None;
    }

    match len {
        2 => match text {
            "as" => Some(TokenKind::As),
            "by" => Some(TokenKind::By),
            "do" => Some(TokenKind::Do),
            "if" => Some(TokenKind::If),
            "in" => Some(TokenKind::In),
            "Ok" => Some(TokenKind::Ok),
            _ => None,
        },
        3 => match text {
            "def" => Some(TokenKind::Def),
            "div" => Some(TokenKind::Div),
            "dyn" => Some(TokenKind::Dyn),
            "Err" => Some(TokenKind::Err),
            "for" => Some(TokenKind::For),
            "int" => Some(TokenKind::IntType),
            "let" => Some(TokenKind::Let),
            "mut" => Some(TokenKind::Mut),
            "pub" => Some(TokenKind::Pub),
            "run" => Some(TokenKind::Run),
            "str" => Some(TokenKind::StrType),
            "try" => Some(TokenKind::Try),
            "use" => Some(TokenKind::Use),
            _ => None,
        },
        4 => match text {
            "Self" => Some(TokenKind::SelfUpper),
            "None" => Some(TokenKind::None),
            "Some" => Some(TokenKind::Some),
            "bool" => Some(TokenKind::BoolType),
            "byte" => Some(TokenKind::ByteType),
            "char" => Some(TokenKind::CharType),
            "else" => Some(TokenKind::Else),
            "impl" => Some(TokenKind::Impl),
            "loop" => Some(TokenKind::Loop),
            "self" => Some(TokenKind::SelfLower),
            "skip" => Some(TokenKind::Skip),
            "then" => Some(TokenKind::Then),
            "todo" => Some(TokenKind::Todo),
            "true" => Some(TokenKind::True),
            "type" => Some(TokenKind::Type),
            "uses" => Some(TokenKind::Uses),
            "void" => Some(TokenKind::Void),
            "with" => Some(TokenKind::With),
            _ => None,
        },
        5 => match text {
            "Never" => Some(TokenKind::NeverType),
            "async" => Some(TokenKind::Async),
            "break" => Some(TokenKind::Break),
            "false" => Some(TokenKind::False),
            "float" => Some(TokenKind::FloatType),
            "match" => Some(TokenKind::Match),
            "panic" => Some(TokenKind::Panic),
            "print" => Some(TokenKind::Print),
            "tests" => Some(TokenKind::Tests),
            "trait" => Some(TokenKind::Trait),
            "where" => Some(TokenKind::Where),
            "yield" => Some(TokenKind::Yield),
            _ => None,
        },
        6 => match text {
            "extend" => Some(TokenKind::Extend),
            "extern" => Some(TokenKind::Extern),
            "return" => Some(TokenKind::Return),
            "unsafe" => Some(TokenKind::Unsafe),
            _ => None,
        },
        7 => match text {
            "suspend" => Some(TokenKind::Suspend),
            _ => None,
        },
        8 => match text {
            "continue" => Some(TokenKind::Continue),
            _ => None,
        },
        9 => match text {
            "extension" => Some(TokenKind::Extension),
            _ => None,
        },
        11 => match text {
            "unreachable" => Some(TokenKind::Unreachable),
            _ => None,
        },
        _ => None,
    }
}

/// Check if a keyword is reserved for future use.
///
/// Returns the static keyword string if it matches, `None` otherwise.
/// These keywords are not yet implemented but are reserved to prevent
/// user code from depending on them as identifiers.
///
/// Reserved-future: `asm`, `inline`, `static`, `union`, `view`
pub(crate) fn reserved_future_lookup(text: &str) -> Option<&'static str> {
    match text {
        "asm" => Some("asm"),
        "inline" => Some("inline"),
        "static" => Some("static"),
        "union" => Some("union"),
        "view" => Some("view"),
        _ => None,
    }
}

/// Fast pre-filter: can this identifier possibly be a soft keyword?
///
/// Checks length (5, 7, or 8) and first byte (`c`, `p`, `r`, `s`, `t`).
/// Only 6 soft keywords exist: `cache`(5), `catch`(5), `parallel`(8),
/// `recurse`(7), `spawn`(5), `timeout`(7).
///
/// Rejects >99% of identifiers before the binary search in [`soft_keyword_lookup`].
#[inline]
pub(crate) fn could_be_soft_keyword(text: &str) -> bool {
    let bytes = text.as_bytes();
    matches!(bytes.len(), 5 | 7 | 8) && matches!(bytes[0], b'c' | b'p' | b'r' | b's' | b't')
}

/// Fast pre-filter: can this identifier possibly be a reserved-future keyword?
///
/// Checks length (3–6) and first byte (`a`, `i`, `s`, `u`, `v`).
/// Reserved-future: `asm`(3), `inline`(6), `static`(6), `union`(5), `view`(4).
///
/// Rejects >99% of identifiers before the match in [`reserved_future_lookup`].
#[inline]
pub(crate) fn could_be_reserved_future(text: &str) -> bool {
    let bytes = text.as_bytes();
    matches!(bytes.len(), 3..=6) && matches!(bytes[0], b'a' | b'i' | b's' | b'u' | b'v')
}

/// Sorted list of soft keywords and their corresponding `TokenKind`.
///
/// These 6 pattern keywords are only recognized when followed by `(`.
/// Sorted alphabetically for binary search.
const SOFT_KEYWORDS: [(&str, TokenKind); 6] = [
    ("cache", TokenKind::Cache),
    ("catch", TokenKind::Catch),
    ("parallel", TokenKind::Parallel),
    ("recurse", TokenKind::Recurse),
    ("spawn", TokenKind::Spawn),
    ("timeout", TokenKind::Timeout),
];

/// Look up a soft (context-sensitive) keyword.
///
/// Returns the corresponding `TokenKind` if the text is a soft keyword AND
/// the bytes following the token (in `rest`) contain `(` as the next
/// non-horizontal-whitespace character. Returns `None` otherwise, letting
/// the identifier be interned as a regular `Ident`.
///
/// Lookahead rules:
/// - Skips ASCII horizontal whitespace: `' '` (space) and `'\t'` (tab)
/// - Does NOT skip newlines — `cache\n(...)` is identifier + paren expression
/// - Does NOT skip comments — `cache // foo\n(...)` is identifier
pub(crate) fn soft_keyword_lookup(text: &str, rest: &[u8]) -> Option<TokenKind> {
    // Binary search the sorted soft keyword table
    let idx = SOFT_KEYWORDS
        .binary_search_by_key(&text, |(kw, _)| kw)
        .ok()?;

    // Check lookahead: next non-horizontal-whitespace byte must be `(`
    if has_lparen_lookahead(rest) {
        Some(SOFT_KEYWORDS[idx].1.clone())
    } else {
        None
    }
}

/// Check if the next non-horizontal-whitespace byte is `(`.
///
/// Skips only `' '` and `'\t'` — newlines, comments, and other bytes stop the scan.
#[inline]
fn has_lparen_lookahead(rest: &[u8]) -> bool {
    for &b in rest {
        match b {
            b' ' | b'\t' => {}
            b'(' => return true,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Reserved keyword tests ===

    #[test]
    fn control_flow_keywords() {
        assert_eq!(lookup("if"), Some(TokenKind::If));
        assert_eq!(lookup("else"), Some(TokenKind::Else));
        assert_eq!(lookup("for"), Some(TokenKind::For));
        assert_eq!(lookup("in"), Some(TokenKind::In));
        assert_eq!(lookup("match"), Some(TokenKind::Match));
        assert_eq!(lookup("loop"), Some(TokenKind::Loop));
        assert_eq!(lookup("break"), Some(TokenKind::Break));
        assert_eq!(lookup("continue"), Some(TokenKind::Continue));
        assert_eq!(lookup("return"), Some(TokenKind::Return));
    }

    #[test]
    fn declaration_keywords() {
        assert_eq!(lookup("let"), Some(TokenKind::Let));
        assert_eq!(lookup("def"), Some(TokenKind::Def));
        assert_eq!(lookup("type"), Some(TokenKind::Type));
        assert_eq!(lookup("trait"), Some(TokenKind::Trait));
        assert_eq!(lookup("impl"), Some(TokenKind::Impl));
        assert_eq!(lookup("pub"), Some(TokenKind::Pub));
        assert_eq!(lookup("mut"), Some(TokenKind::Mut));
    }

    #[test]
    fn value_keywords() {
        assert_eq!(lookup("true"), Some(TokenKind::True));
        assert_eq!(lookup("false"), Some(TokenKind::False));
        assert_eq!(lookup("void"), Some(TokenKind::Void));
    }

    #[test]
    fn type_keywords() {
        assert_eq!(lookup("int"), Some(TokenKind::IntType));
        assert_eq!(lookup("float"), Some(TokenKind::FloatType));
        assert_eq!(lookup("bool"), Some(TokenKind::BoolType));
        assert_eq!(lookup("str"), Some(TokenKind::StrType));
        assert_eq!(lookup("char"), Some(TokenKind::CharType));
        assert_eq!(lookup("byte"), Some(TokenKind::ByteType));
        assert_eq!(lookup("Never"), Some(TokenKind::NeverType));
    }

    #[test]
    fn constructor_keywords() {
        assert_eq!(lookup("Ok"), Some(TokenKind::Ok));
        assert_eq!(lookup("Err"), Some(TokenKind::Err));
        assert_eq!(lookup("Some"), Some(TokenKind::Some));
        assert_eq!(lookup("None"), Some(TokenKind::None));
    }

    #[test]
    fn always_resolved_pattern_keywords() {
        // run and try are always keywords (not soft)
        assert_eq!(lookup("run"), Some(TokenKind::Run));
        assert_eq!(lookup("try"), Some(TokenKind::Try));
        assert_eq!(lookup("by"), Some(TokenKind::By));
    }

    #[test]
    fn builtin_keywords() {
        assert_eq!(lookup("print"), Some(TokenKind::Print));
        assert_eq!(lookup("panic"), Some(TokenKind::Panic));
        assert_eq!(lookup("todo"), Some(TokenKind::Todo));
        assert_eq!(lookup("unreachable"), Some(TokenKind::Unreachable));
    }

    #[test]
    fn misc_keywords() {
        assert_eq!(lookup("async"), Some(TokenKind::Async));
        assert_eq!(lookup("do"), Some(TokenKind::Do));
        assert_eq!(lookup("then"), Some(TokenKind::Then));
        assert_eq!(lookup("yield"), Some(TokenKind::Yield));
        assert_eq!(lookup("tests"), Some(TokenKind::Tests));
        assert_eq!(lookup("dyn"), Some(TokenKind::Dyn));
        assert_eq!(lookup("extend"), Some(TokenKind::Extend));
        assert_eq!(lookup("extension"), Some(TokenKind::Extension));
        assert_eq!(lookup("skip"), Some(TokenKind::Skip));
        assert_eq!(lookup("div"), Some(TokenKind::Div));
        assert_eq!(lookup("self"), Some(TokenKind::SelfLower));
        assert_eq!(lookup("Self"), Some(TokenKind::SelfUpper));
        assert_eq!(lookup("use"), Some(TokenKind::Use));
        assert_eq!(lookup("uses"), Some(TokenKind::Uses));
        assert_eq!(lookup("as"), Some(TokenKind::As));
        assert_eq!(lookup("where"), Some(TokenKind::Where));
        assert_eq!(lookup("with"), Some(TokenKind::With));
        assert_eq!(lookup("suspend"), Some(TokenKind::Suspend));
        assert_eq!(lookup("unsafe"), Some(TokenKind::Unsafe));
        assert_eq!(lookup("extern"), Some(TokenKind::Extern));
    }

    // === Soft keywords are NOT in the reserved table ===

    #[test]
    fn soft_keywords_not_in_reserved_table() {
        assert_eq!(lookup("cache"), None);
        assert_eq!(lookup("catch"), None);
        assert_eq!(lookup("parallel"), None);
        assert_eq!(lookup("spawn"), None);
        assert_eq!(lookup("recurse"), None);
        assert_eq!(lookup("timeout"), None);
    }

    // === Soft keyword lookup tests ===

    #[test]
    fn soft_keyword_with_lparen() {
        assert_eq!(soft_keyword_lookup("cache", b"(x)"), Some(TokenKind::Cache));
        assert_eq!(
            soft_keyword_lookup("catch", b"(err)"),
            Some(TokenKind::Catch)
        );
        assert_eq!(
            soft_keyword_lookup("parallel", b"(tasks)"),
            Some(TokenKind::Parallel)
        );
        assert_eq!(
            soft_keyword_lookup("spawn", b"(task)"),
            Some(TokenKind::Spawn)
        );
        assert_eq!(
            soft_keyword_lookup("recurse", b"(n)"),
            Some(TokenKind::Recurse)
        );
        assert_eq!(
            soft_keyword_lookup("timeout", b"(5s, task)"),
            Some(TokenKind::Timeout)
        );
    }

    #[test]
    fn soft_keyword_without_lparen() {
        // No `(` follows → identifier
        assert_eq!(soft_keyword_lookup("cache", b" = 42"), None);
        assert_eq!(soft_keyword_lookup("catch", b".field"), None);
        assert_eq!(soft_keyword_lookup("parallel", b""), None);
        assert_eq!(soft_keyword_lookup("spawn", b"\n(x)"), None);
        assert_eq!(soft_keyword_lookup("recurse", b" + 1"), None);
        assert_eq!(soft_keyword_lookup("timeout", b": int"), None);
    }

    #[test]
    fn soft_keyword_with_space_before_lparen() {
        // Space before `(` → still keyword
        assert_eq!(
            soft_keyword_lookup("cache", b" (x)"),
            Some(TokenKind::Cache)
        );
        assert_eq!(
            soft_keyword_lookup("catch", b"  (err)"),
            Some(TokenKind::Catch)
        );
    }

    #[test]
    fn soft_keyword_with_tab_before_lparen() {
        // Tab before `(` → still keyword
        assert_eq!(
            soft_keyword_lookup("cache", b"\t(x)"),
            Some(TokenKind::Cache)
        );
        assert_eq!(
            soft_keyword_lookup("parallel", b"\t\t(tasks)"),
            Some(TokenKind::Parallel)
        );
    }

    #[test]
    fn soft_keyword_with_newline_before_lparen() {
        // Newline before `(` → identifier (not keyword)
        assert_eq!(soft_keyword_lookup("cache", b"\n(x)"), None);
        assert_eq!(soft_keyword_lookup("spawn", b"\r\n(x)"), None);
    }

    #[test]
    fn soft_keyword_non_keyword_text() {
        // Text that isn't a soft keyword at all
        assert_eq!(soft_keyword_lookup("foo", b"(x)"), None);
        assert_eq!(soft_keyword_lookup("let", b"(x)"), None);
        assert_eq!(soft_keyword_lookup("if", b"(x)"), None);
    }

    // === Edge cases ===

    #[test]
    fn non_keywords_return_none() {
        assert_eq!(lookup("foo"), None);
        assert_eq!(lookup("bar"), None);
        assert_eq!(lookup("x"), None);
        assert_eq!(lookup("my_var"), None);
    }

    #[test]
    fn case_sensitivity() {
        // Keywords are case-sensitive
        assert_eq!(lookup("If"), None);
        assert_eq!(lookup("IF"), None);
        assert_eq!(lookup("TRUE"), None);
        assert_eq!(lookup("False"), None);

        // But Self is uppercase
        assert_eq!(lookup("Self"), Some(TokenKind::SelfUpper));
        assert_eq!(lookup("self"), Some(TokenKind::SelfLower));

        // Never is uppercase
        assert_eq!(lookup("Never"), Some(TokenKind::NeverType));
        assert_eq!(lookup("never"), None);
    }

    #[test]
    fn reserved_keywords_recognized() {
        assert_eq!(lookup("extern"), Some(TokenKind::Extern));
        assert_eq!(lookup("suspend"), Some(TokenKind::Suspend));
        assert_eq!(lookup("unsafe"), Some(TokenKind::Unsafe));
    }

    #[test]
    fn empty_string_is_not_keyword() {
        assert_eq!(lookup(""), None);
    }

    #[test]
    fn single_char_is_not_keyword() {
        assert_eq!(lookup("a"), None);
        assert_eq!(lookup("i"), None);
        assert_eq!(lookup("x"), None);
    }

    #[test]
    fn length_boundary_rejection() {
        // Strings longer than 11 chars are rejected immediately
        assert_eq!(lookup("unreachable_"), None);
        assert_eq!(lookup("unreachables"), None);
    }

    #[test]
    fn non_alpha_start_rejection() {
        // Keywords must start with ASCII alpha
        assert_eq!(lookup("_if"), None);
        assert_eq!(lookup("1let"), None);
    }

    // === has_lparen_lookahead edge cases ===

    #[test]
    fn lparen_lookahead_empty_rest() {
        assert!(!has_lparen_lookahead(b""));
    }

    #[test]
    fn lparen_lookahead_immediate() {
        assert!(has_lparen_lookahead(b"("));
    }

    #[test]
    fn lparen_lookahead_with_mixed_whitespace() {
        assert!(has_lparen_lookahead(b" \t (x)"));
    }

    #[test]
    fn lparen_lookahead_stops_at_non_whitespace() {
        assert!(!has_lparen_lookahead(b"x("));
        assert!(!has_lparen_lookahead(b"// comment\n("));
    }

    // === Reserved-future keyword tests ===

    #[test]
    fn reserved_future_keywords_detected() {
        assert_eq!(reserved_future_lookup("asm"), Some("asm"));
        assert_eq!(reserved_future_lookup("inline"), Some("inline"));
        assert_eq!(reserved_future_lookup("static"), Some("static"));
        assert_eq!(reserved_future_lookup("union"), Some("union"));
        assert_eq!(reserved_future_lookup("view"), Some("view"));
    }

    #[test]
    fn non_reserved_future_returns_none() {
        assert_eq!(reserved_future_lookup("let"), None);
        assert_eq!(reserved_future_lookup("foo"), None);
        assert_eq!(reserved_future_lookup(""), None);
        assert_eq!(reserved_future_lookup("Static"), None); // case-sensitive
    }

    // === Pre-filter tests ===

    #[test]
    fn could_be_soft_keyword_accepts_all_soft_keywords() {
        assert!(could_be_soft_keyword("cache")); // len=5, starts with 'c'
        assert!(could_be_soft_keyword("catch")); // len=5, starts with 'c'
        assert!(could_be_soft_keyword("spawn")); // len=5, starts with 's'
        assert!(could_be_soft_keyword("recurse")); // len=7, starts with 'r'
        assert!(could_be_soft_keyword("timeout")); // len=7, starts with 't'
        assert!(could_be_soft_keyword("parallel")); // len=8, starts with 'p'
    }

    #[test]
    fn could_be_soft_keyword_rejects_wrong_length() {
        assert!(!could_be_soft_keyword("if")); // len=2
        assert!(!could_be_soft_keyword("let")); // len=3
        assert!(!could_be_soft_keyword("self")); // len=4
        assert!(!could_be_soft_keyword("return")); // len=6
        assert!(!could_be_soft_keyword("extension")); // len=9
    }

    #[test]
    fn could_be_soft_keyword_rejects_wrong_first_byte() {
        assert!(!could_be_soft_keyword("match")); // len=5, starts with 'm'
        assert!(!could_be_soft_keyword("break")); // len=5, starts with 'b'
        assert!(!could_be_soft_keyword("async")); // len=5, starts with 'a'
    }

    #[test]
    fn could_be_reserved_future_accepts_all_reserved_future() {
        assert!(could_be_reserved_future("asm")); // len=3, starts with 'a'
        assert!(could_be_reserved_future("view")); // len=4, starts with 'v'
        assert!(could_be_reserved_future("union")); // len=5, starts with 'u'
        assert!(could_be_reserved_future("inline")); // len=6, starts with 'i'
        assert!(could_be_reserved_future("static")); // len=6, starts with 's'
    }

    #[test]
    fn could_be_reserved_future_rejects_wrong_length() {
        assert!(!could_be_reserved_future("if")); // len=2
        assert!(!could_be_reserved_future("suspend")); // len=7
        assert!(!could_be_reserved_future("parallel")); // len=8
    }

    #[test]
    fn could_be_reserved_future_rejects_wrong_first_byte() {
        assert!(!could_be_reserved_future("def")); // len=3, starts with 'd'
        assert!(!could_be_reserved_future("loop")); // len=4, starts with 'l'
        assert!(!could_be_reserved_future("match")); // len=5, starts with 'm'
    }
}
