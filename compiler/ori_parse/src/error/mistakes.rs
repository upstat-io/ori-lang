//! Delimiter-matching helper for error classification.

use ori_ir::TokenKind;

/// Get the closing delimiter for an opening delimiter.
pub(crate) fn closing_delimiter(open: &TokenKind) -> TokenKind {
    match open {
        TokenKind::LParen => TokenKind::RParen,
        TokenKind::LBracket => TokenKind::RBracket,
        TokenKind::LBrace => TokenKind::RBrace,
        TokenKind::Lt => TokenKind::Gt,
        _ => TokenKind::Eof, // fallback
    }
}
