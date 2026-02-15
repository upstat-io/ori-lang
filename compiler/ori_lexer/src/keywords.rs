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
mod tests;
