//! Tests for the trivial token fast path.

use ori_ir::{StringInterner, TokenKind};
use ori_lexer_core::RawTag;

use super::try_trivial;
use crate::cooker::TokenCooker;

/// Verify that every trivial entry produces the same `TokenKind` as `cook()`,
/// and that the pre-computed tag matches `discriminant_index()`.
#[test]
fn trivial_matches_cook() {
    let interner = StringInterner::new();
    // Trivial tokens don't read source bytes, so any dummy source works.
    let source = b"+";
    let mut cooker = TokenCooker::new(source, &interner);

    let trivial_tags = [
        // Single operators
        RawTag::Plus,
        RawTag::Minus,
        RawTag::Star,
        RawTag::Slash,
        RawTag::Percent,
        RawTag::Caret,
        RawTag::Ampersand,
        RawTag::Pipe,
        RawTag::Tilde,
        RawTag::Bang,
        RawTag::Equal,
        RawTag::Less,
        RawTag::Greater,
        RawTag::Dot,
        RawTag::Question,
        // Compound operators
        RawTag::EqualEqual,
        RawTag::BangEqual,
        RawTag::LessEqual,
        RawTag::AmpersandAmpersand,
        RawTag::PipePipe,
        RawTag::Arrow,
        RawTag::FatArrow,
        RawTag::DotDot,
        RawTag::DotDotEqual,
        RawTag::DotDotDot,
        RawTag::ColonColon,
        RawTag::Shl,
        RawTag::QuestionQuestion,
        // Compound assignment
        RawTag::PlusEq,
        RawTag::MinusEq,
        RawTag::StarEq,
        RawTag::SlashEq,
        RawTag::PercentEq,
        RawTag::AtEq,
        RawTag::AmpersandEq,
        RawTag::PipeEq,
        RawTag::CaretEq,
        RawTag::ShlEq,
        RawTag::AmpersandAmpersandEq,
        RawTag::PipePipeEq,
        // Delimiters
        RawTag::LeftParen,
        RawTag::RightParen,
        RawTag::LeftBracket,
        RawTag::RightBracket,
        RawTag::LeftBrace,
        RawTag::RightBrace,
        RawTag::Comma,
        RawTag::Colon,
        RawTag::At,
        RawTag::Hash,
        RawTag::Underscore,
        RawTag::Dollar,
        RawTag::HashBracket,
        RawTag::HashBang,
    ];

    for raw in trivial_tags {
        let (kind, tag) =
            try_trivial(raw).unwrap_or_else(|| panic!("try_trivial returned None for {raw:?}"));

        let cooked = cooker.cook(raw, 0, 1);
        assert_eq!(kind, cooked.kind, "kind mismatch for {raw:?}");
        assert_eq!(tag, cooked.tag, "tag mismatch for {raw:?}");
        assert_eq!(
            tag,
            kind.discriminant_index(),
            "tag vs discriminant for {raw:?}"
        );
    }
}

/// Verify that non-trivial tokens (identifiers, literals, errors, etc.)
/// correctly return `None`.
#[test]
fn non_trivial_returns_none() {
    let non_trivial = [
        RawTag::Ident,
        RawTag::Int,
        RawTag::Float,
        RawTag::HexInt,
        RawTag::BinInt,
        RawTag::String,
        RawTag::Char,
        RawTag::Duration,
        RawTag::Size,
        RawTag::TemplateHead,
        RawTag::TemplateMiddle,
        RawTag::TemplateTail,
        RawTag::TemplateComplete,
        RawTag::FormatSpec,
        RawTag::Semicolon,
        RawTag::Backslash,
        RawTag::InvalidByte,
        RawTag::UnterminatedString,
        RawTag::UnterminatedChar,
        RawTag::InvalidEscape,
        RawTag::UnterminatedTemplate,
        RawTag::InteriorNull,
        RawTag::Whitespace,
        RawTag::Newline,
        RawTag::LineComment,
        RawTag::Eof,
    ];

    for raw in non_trivial {
        assert!(try_trivial(raw).is_none(), "expected None for {raw:?}");
    }
}

/// Verify the trivial count matches expectations by counting all known
/// `RawTag` variants that return `Some`.
#[test]
fn trivial_count() {
    // All defined RawTag variants — test each one
    let all_raw_tags = [
        // Identifiers & Literals
        RawTag::Ident,
        RawTag::Int,
        RawTag::Float,
        RawTag::HexInt,
        RawTag::String,
        RawTag::Char,
        RawTag::Duration,
        RawTag::Size,
        RawTag::BinInt,
        // Template literals
        RawTag::TemplateHead,
        RawTag::TemplateMiddle,
        RawTag::TemplateTail,
        RawTag::TemplateComplete,
        RawTag::FormatSpec,
        // Single operators
        RawTag::Plus,
        RawTag::Minus,
        RawTag::Star,
        RawTag::Slash,
        RawTag::Percent,
        RawTag::Caret,
        RawTag::Ampersand,
        RawTag::Pipe,
        RawTag::Tilde,
        RawTag::Bang,
        RawTag::Equal,
        RawTag::Less,
        RawTag::Greater,
        RawTag::Dot,
        RawTag::Question,
        // Compound operators
        RawTag::EqualEqual,
        RawTag::BangEqual,
        RawTag::LessEqual,
        RawTag::AmpersandAmpersand,
        RawTag::PipePipe,
        RawTag::Arrow,
        RawTag::FatArrow,
        RawTag::DotDot,
        RawTag::DotDotEqual,
        RawTag::DotDotDot,
        RawTag::ColonColon,
        RawTag::Shl,
        RawTag::QuestionQuestion,
        // Compound assignment
        RawTag::PlusEq,
        RawTag::MinusEq,
        RawTag::StarEq,
        RawTag::SlashEq,
        RawTag::PercentEq,
        RawTag::AtEq,
        RawTag::AmpersandEq,
        RawTag::PipeEq,
        RawTag::CaretEq,
        RawTag::ShlEq,
        RawTag::AmpersandAmpersandEq,
        RawTag::PipePipeEq,
        // Delimiters
        RawTag::LeftParen,
        RawTag::RightParen,
        RawTag::LeftBracket,
        RawTag::RightBracket,
        RawTag::LeftBrace,
        RawTag::RightBrace,
        RawTag::Comma,
        RawTag::Colon,
        RawTag::Semicolon,
        RawTag::At,
        RawTag::Hash,
        RawTag::Underscore,
        RawTag::Backslash,
        RawTag::Dollar,
        RawTag::HashBracket,
        RawTag::HashBang,
        // Trivia
        RawTag::Whitespace,
        RawTag::Newline,
        RawTag::LineComment,
        // Errors
        RawTag::InvalidByte,
        RawTag::UnterminatedString,
        RawTag::UnterminatedChar,
        RawTag::InvalidEscape,
        RawTag::UnterminatedTemplate,
        RawTag::InteriorNull,
        // Control
        RawTag::Eof,
    ];

    let count = all_raw_tags
        .iter()
        .filter(|&&raw| try_trivial(raw).is_some())
        .count();
    assert_eq!(count, 54, "expected 54 trivial entries");
}

/// Verify `TokenKind::At` is included as trivial (it IS a declaration start,
/// but the doc comment check in the driver handles this correctly).
#[test]
fn at_is_trivial() {
    let result = try_trivial(RawTag::At);
    assert!(result.is_some(), "@ must be trivial");
    let (kind, tag) = result.unwrap_or_else(|| unreachable!());
    assert_eq!(kind, TokenKind::At);
    assert_eq!(tag, TokenKind::TAG_AT);
}
