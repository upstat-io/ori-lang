//! Re-escaping of cooked template-literal text back into canonical source.
//!
//! The lexer cooks template literals (`ori_lexer::cook_escape`), turning the
//! source escapes `\\` `` \` `` `{{` `}}` into their literal characters in the
//! interned value. The formatter holds that cooked value, so emitting it
//! verbatim would not round-trip: a literal `\` re-lexes as an escape start, a
//! literal `` ` `` ends the template, and a literal `{`/`}` re-lexes as an
//! interpolation boundary / brace de-doubling.
//!
//! [`escape_template_text`] is the inverse of the cooker for exactly the
//! structurally-significant characters. Literal control characters
//! (newline, tab, CR, NUL) are valid template content and stay literal, which
//! preserves an author's multi-line template layout.

/// Re-escape cooked template-literal text into canonical template source.
///
/// Reverses the lexer cooker for `\` -> `\\`, `` ` `` -> `` \` ``,
/// `{` -> `{{`, `}` -> `}}`. Every other character (including literal
/// newline/tab/CR/NUL) is emitted unchanged. Single-pass so an introduced
/// backslash is never re-escaped.
pub fn escape_template_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests;
