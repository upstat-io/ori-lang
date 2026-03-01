//! Fast path for trivial tokens: operators and delimiters.
//!
//! Trivial tokens have a 1:1 mapping from `RawTag` to `TokenKind` with no data
//! payload, no errors, and no side effects. For these tokens, we bypass both
//! `cook()` and `discriminant_index()` entirely.

use ori_ir::TokenKind;
use ori_lexer_core::RawTag;

/// Try to resolve a trivial `RawTag` directly to `(TokenKind, tag)`.
///
/// Returns `Some((kind, tag))` for operator/delimiter tokens that need no
/// cooking — bypassing both `cook()` (no state save/restore, no big match)
/// and `discriminant_index()` (tag is pre-computed).
///
/// Returns `None` for tokens that require full cooking: identifiers, literals,
/// strings, templates, error tags, semicolons, and backslashes.
#[inline]
pub(crate) fn try_trivial(raw: RawTag) -> Option<(TokenKind, u8)> {
    Some(match raw {
        // Single operators
        RawTag::Plus => (TokenKind::Plus, TokenKind::TAG_PLUS),
        RawTag::Minus => (TokenKind::Minus, TokenKind::TAG_MINUS),
        RawTag::Star => (TokenKind::Star, TokenKind::TAG_STAR),
        RawTag::Slash => (TokenKind::Slash, TokenKind::TAG_SLASH),
        RawTag::Percent => (TokenKind::Percent, TokenKind::TAG_PERCENT),
        RawTag::Caret => (TokenKind::Caret, TokenKind::TAG_CARET),
        RawTag::Ampersand => (TokenKind::Amp, TokenKind::TAG_AMP),
        RawTag::Pipe => (TokenKind::Pipe, TokenKind::TAG_PIPE),
        RawTag::Tilde => (TokenKind::Tilde, TokenKind::TAG_TILDE),
        RawTag::Bang => (TokenKind::Bang, TokenKind::TAG_BANG),
        RawTag::Equal => (TokenKind::Eq, TokenKind::TAG_EQ),
        RawTag::Less => (TokenKind::Lt, TokenKind::TAG_LT),
        RawTag::Greater => (TokenKind::Gt, TokenKind::TAG_GT),
        RawTag::Dot => (TokenKind::Dot, TokenKind::TAG_DOT),
        RawTag::Question => (TokenKind::Question, TokenKind::TAG_QUESTION),

        // Compound operators
        RawTag::EqualEqual => (TokenKind::EqEq, TokenKind::TAG_EQEQ),
        RawTag::BangEqual => (TokenKind::NotEq, TokenKind::TAG_NOTEQ),
        RawTag::LessEqual => (TokenKind::LtEq, TokenKind::TAG_LTEQ),
        RawTag::AmpersandAmpersand => (TokenKind::AmpAmp, TokenKind::TAG_AMPAMP),
        RawTag::PipePipe => (TokenKind::PipePipe, TokenKind::TAG_PIPEPIPE),
        RawTag::Arrow => (TokenKind::Arrow, TokenKind::TAG_ARROW),
        RawTag::FatArrow => (TokenKind::FatArrow, TokenKind::TAG_FAT_ARROW),
        RawTag::DotDot => (TokenKind::DotDot, TokenKind::TAG_DOTDOT),
        RawTag::DotDotEqual => (TokenKind::DotDotEq, TokenKind::TAG_DOTDOTEQ),
        RawTag::DotDotDot => (TokenKind::DotDotDot, TokenKind::TAG_DOTDOTDOT),
        RawTag::ColonColon => (TokenKind::DoubleColon, TokenKind::TAG_DOUBLE_COLON),
        RawTag::Shl => (TokenKind::Shl, TokenKind::TAG_SHL),
        RawTag::QuestionQuestion => (TokenKind::DoubleQuestion, TokenKind::TAG_DOUBLE_QUESTION),

        // Compound assignment
        RawTag::PlusEq => (TokenKind::PlusEq, TokenKind::TAG_PLUS_EQ),
        RawTag::MinusEq => (TokenKind::MinusEq, TokenKind::TAG_MINUS_EQ),
        RawTag::StarEq => (TokenKind::StarEq, TokenKind::TAG_STAR_EQ),
        RawTag::SlashEq => (TokenKind::SlashEq, TokenKind::TAG_SLASH_EQ),
        RawTag::PercentEq => (TokenKind::PercentEq, TokenKind::TAG_PERCENT_EQ),
        RawTag::AtEq => (TokenKind::AtEq, TokenKind::TAG_AT_EQ),
        RawTag::AmpersandEq => (TokenKind::AmpEq, TokenKind::TAG_AMP_EQ),
        RawTag::PipeEq => (TokenKind::PipeEq, TokenKind::TAG_PIPE_EQ),
        RawTag::CaretEq => (TokenKind::CaretEq, TokenKind::TAG_CARET_EQ),
        RawTag::ShlEq => (TokenKind::ShlEq, TokenKind::TAG_SHL_EQ),
        RawTag::AmpersandAmpersandEq => (TokenKind::AmpAmpEq, TokenKind::TAG_AMPAMP_EQ),
        RawTag::PipePipeEq => (TokenKind::PipePipeEq, TokenKind::TAG_PIPEPIPE_EQ),

        // Delimiters
        RawTag::LeftParen => (TokenKind::LParen, TokenKind::TAG_LPAREN),
        RawTag::RightParen => (TokenKind::RParen, TokenKind::TAG_RPAREN),
        RawTag::LeftBracket => (TokenKind::LBracket, TokenKind::TAG_LBRACKET),
        RawTag::RightBracket => (TokenKind::RBracket, TokenKind::TAG_RBRACKET),
        RawTag::LeftBrace => (TokenKind::LBrace, TokenKind::TAG_LBRACE),
        RawTag::RightBrace => (TokenKind::RBrace, TokenKind::TAG_RBRACE),
        RawTag::Comma => (TokenKind::Comma, TokenKind::TAG_COMMA),
        RawTag::Colon => (TokenKind::Colon, TokenKind::TAG_COLON),
        RawTag::At => (TokenKind::At, TokenKind::TAG_AT),
        RawTag::Hash => (TokenKind::Hash, TokenKind::TAG_HASH),
        RawTag::Underscore => (TokenKind::Underscore, TokenKind::TAG_UNDERSCORE),
        RawTag::Dollar => (TokenKind::Dollar, TokenKind::TAG_DOLLAR),
        RawTag::HashBracket => (TokenKind::HashBracket, TokenKind::TAG_HASH_BRACKET),
        RawTag::HashBang => (TokenKind::HashBang, TokenKind::TAG_HASH_BANG),

        // Everything else requires full cooking
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
