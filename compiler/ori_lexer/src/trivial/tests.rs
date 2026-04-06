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
///
/// Uses `RawTag::ALL` as the single source of truth for variant enumeration.
/// If a new variant is added to `ori_lexer_core` without updating the trivial
/// mapping here, this test will catch the drift.
#[test]
fn trivial_count() {
    let count = RawTag::ALL
        .iter()
        .filter(|&&raw| try_trivial(raw).is_some())
        .count();
    assert_eq!(count, 54, "expected 54 trivial entries");
}

/// Drift guard: every `RawTag` variant with a fixed `lexeme()` (operators and
/// delimiters) must be routed through `try_trivial()` — UNLESS it's explicitly
/// excluded for semantic reasons (e.g. `Semicolon` needs error emission,
/// `Backslash` needs error cooking, `Newline` is handled by the driver).
///
/// If a new operator/delimiter variant is added to `RawTag` but not to
/// `try_trivial()`, this test will fail, forcing an explicit routing decision.
#[test]
fn fixed_lexeme_variants_are_routed() {
    // Variants with a fixed lexeme that are explicitly NOT in try_trivial().
    // Each exclusion must have a documented reason.
    let excluded_from_trivial: &[RawTag] = &[
        RawTag::Semicolon, // needs error emission (semicolons are invalid Ori syntax)
        RawTag::Backslash, // needs error cooking (standalone backslash)
        RawTag::Newline,   // handled by driver loop (significant for statement separation)
    ];

    for &tag in &RawTag::ALL {
        let has_fixed_lexeme = tag.lexeme().is_some();
        let is_trivial = try_trivial(tag).is_some();
        let is_excluded = excluded_from_trivial.contains(&tag);

        assert!(
            !(has_fixed_lexeme && !is_trivial && !is_excluded),
            "{tag:?} has a fixed lexeme but is not in try_trivial() \
             and not in the explicit exclusion list. Either add it to \
             try_trivial() or add it to excluded_from_trivial with a reason."
        );
    }
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

/// Drift guard: every `RawTag` variant must be accounted for in either
/// `try_trivial()` (direct operator/delimiter mapping) or `cook()` (explicit
/// match arm for non-trivial tokens).
///
/// If a new `RawTag` variant is added to `ori_lexer_core` but not handled in
/// either location, this test fails — forcing an explicit routing decision
/// instead of silent fallthrough to the `_ =>` catch-all.
///
/// Uses `RawTag::ALL` as the single source of truth for variant enumeration.
#[test]
fn every_raw_tag_has_explicit_routing() {
    // Tags with explicit match arms in cook() — non-trivial tokens that
    // require source text processing, error handling, or special routing.
    // This list MUST be updated when adding a new RawTag that needs cooking.
    let cooked_tags: &[RawTag] = &[
        // Identifiers & Literals
        RawTag::Ident,
        RawTag::Int,
        RawTag::Float,
        RawTag::HexInt,
        RawTag::BinInt,
        RawTag::String,
        RawTag::Char,
        RawTag::Duration,
        RawTag::Size,
        // Template Literals
        RawTag::TemplateHead,
        RawTag::TemplateMiddle,
        RawTag::TemplateTail,
        RawTag::TemplateComplete,
        RawTag::FormatSpec,
        // Delimiter with special handling
        RawTag::Semicolon,
        // Error tags
        RawTag::InvalidByte,
        RawTag::UnterminatedString,
        RawTag::UnterminatedChar,
        RawTag::UnterminatedTemplate,
        RawTag::Backslash,
        RawTag::InvalidEscape,
        // Trivia (debug_assert in cook — handled by driver, not cook)
        RawTag::Whitespace,
        RawTag::Newline,
        RawTag::LineComment,
        RawTag::InteriorNull,
        // Control
        RawTag::Eof,
    ];

    for &tag in &RawTag::ALL {
        let is_trivial = try_trivial(tag).is_some();
        let is_cooked = cooked_tags.contains(&tag);

        assert!(
            is_trivial || is_cooked,
            "{tag:?} is not handled by try_trivial() or listed in cook()'s explicit arms. \
             Add it to try_trivial() (if it's a direct mapping) or to cook()'s match \
             and update this test's cooked_tags list."
        );

        assert!(
            !(is_trivial && is_cooked),
            "{tag:?} is in BOTH try_trivial() AND cook()'s cooked_tags. \
             Each variant should be in exactly one routing path."
        );
    }
}
