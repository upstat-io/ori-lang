//! Lexer tests.
//!
//! Tests for the `ori_lexer` crate, validating:
//! - Basic tokenization (keywords, identifiers, literals)
//! - Duration and size literal parsing
//! - Integer formats (decimal, hex, binary with underscores)
//! - String and char escape sequences
//! - Comment handling and classification
//! - Edge cases (empty input, whitespace, errors)

use ori_ir::{CommentKind, DurationUnit, SizeUnit, StringInterner, TokenKind};
use ori_lexer::{lex, lex_with_comments};

fn test_interner() -> StringInterner {
    StringInterner::new()
}

#[test]
fn test_lex_basic() {
    let interner = test_interner();
    let tokens = lex("let x = 42", &interner);

    assert_eq!(tokens.len(), 5); // let, x, =, 42, EOF
    assert!(matches!(tokens[0].kind, TokenKind::Let));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[2].kind, TokenKind::Eq));
    assert!(matches!(tokens[3].kind, TokenKind::Int(42)));
    assert!(matches!(tokens[4].kind, TokenKind::Eof));
}

#[test]
fn test_lex_string() {
    let interner = test_interner();
    let tokens = lex(r#""hello\nworld""#, &interner);

    if let TokenKind::String(name) = tokens[0].kind {
        assert_eq!(interner.lookup(name), "hello\nworld");
    } else {
        panic!("Expected string token");
    }
}

#[test]
fn test_lex_duration() {
    let interner = test_interner();
    let tokens = lex("100ms 5s 2h", &interner);

    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(100, DurationUnit::Milliseconds)
    ));
    assert!(matches!(
        tokens[1].kind,
        TokenKind::Duration(5, DurationUnit::Seconds)
    ));
    assert!(matches!(
        tokens[2].kind,
        TokenKind::Duration(2, DurationUnit::Hours)
    ));
}

#[test]
fn test_lex_pattern_keywords() {
    let interner = test_interner();
    let tokens = lex("run try catch parallel", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Run));
    assert!(matches!(tokens[1].kind, TokenKind::Try));
    assert!(matches!(tokens[2].kind, TokenKind::Catch));
    assert!(matches!(tokens[3].kind, TokenKind::Parallel));
}

#[test]
fn test_lex_function_def() {
    let interner = test_interner();
    let tokens = lex("@main () -> int = 42", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::At));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[2].kind, TokenKind::LParen));
    assert!(matches!(tokens[3].kind, TokenKind::RParen));
    assert!(matches!(tokens[4].kind, TokenKind::Arrow));
    assert!(matches!(tokens[5].kind, TokenKind::IntType));
    assert!(matches!(tokens[6].kind, TokenKind::Eq));
    assert!(matches!(tokens[7].kind, TokenKind::Int(42)));
}

#[test]
fn test_lex_underscore() {
    let interner = test_interner();
    let tokens = lex("_ -> x", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Underscore));
    assert!(matches!(tokens[1].kind, TokenKind::Arrow));
}

#[test]
fn test_lex_hex_integers() {
    let interner = test_interner();
    let tokens = lex("0xFF 0x1_000", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Int(255)));
    assert!(matches!(tokens[1].kind, TokenKind::Int(4096)));
}

#[test]
fn test_lex_binary_integers() {
    let interner = test_interner();
    let tokens = lex("0b1010 0b1111_0000", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Int(10)));
    assert!(matches!(tokens[1].kind, TokenKind::Int(240)));
}

#[test]
fn test_lex_integers_with_underscores() {
    let interner = test_interner();
    let tokens = lex("1_000_000 123_456", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Int(1_000_000)));
    assert!(matches!(tokens[1].kind, TokenKind::Int(123_456)));
}

#[test]
fn test_lex_size_literals() {
    let interner = test_interner();
    let tokens = lex("100b 4kb 10mb 2gb", &interner);

    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(100, SizeUnit::Bytes)
    ));
    assert!(matches!(
        tokens[1].kind,
        TokenKind::Size(4, SizeUnit::Kilobytes)
    ));
    assert!(matches!(
        tokens[2].kind,
        TokenKind::Size(10, SizeUnit::Megabytes)
    ));
    assert!(matches!(
        tokens[3].kind,
        TokenKind::Size(2, SizeUnit::Gigabytes)
    ));
}

#[test]
fn test_lex_duration_minutes() {
    let interner = test_interner();
    let tokens = lex("30m", &interner);

    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(30, DurationUnit::Minutes)
    ));
}

#[test]
fn test_lex_empty_input() {
    let interner = test_interner();
    let tokens = lex("", &interner);

    assert_eq!(tokens.len(), 1);
    assert!(matches!(tokens[0].kind, TokenKind::Eof));
}

#[test]
fn test_lex_whitespace_only() {
    let interner = test_interner();
    let tokens = lex("   \t  ", &interner);

    assert_eq!(tokens.len(), 1);
    assert!(matches!(tokens[0].kind, TokenKind::Eof));
}

#[test]
fn test_lex_newlines() {
    let interner = test_interner();
    let tokens = lex("a\nb", &interner);

    assert_eq!(tokens.len(), 4); // a, newline, b, EOF
    assert!(matches!(tokens[1].kind, TokenKind::Newline));
}

#[test]
fn test_lex_error_tokens() {
    let interner = test_interner();
    // Backtick is not a valid token
    let tokens = lex("`invalid`", &interner);

    // Should have error tokens for the backticks
    assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Error)));
}

#[test]
fn test_lex_all_reserved_keywords() {
    let interner = test_interner();
    let source =
        "async break continue do else false for if impl in let loop match mut pub self Self then trait true type use uses void where with yield";
    let tokens = lex(source, &interner);

    let expected = [
        TokenKind::Async,
        TokenKind::Break,
        TokenKind::Continue,
        TokenKind::Do,
        TokenKind::Else,
        TokenKind::False,
        TokenKind::For,
        TokenKind::If,
        TokenKind::Impl,
        TokenKind::In,
        TokenKind::Let,
        TokenKind::Loop,
        TokenKind::Match,
        TokenKind::Mut,
        TokenKind::Pub,
        TokenKind::SelfLower,
        TokenKind::SelfUpper,
        TokenKind::Then,
        TokenKind::Trait,
        TokenKind::True,
        TokenKind::Type,
        TokenKind::Use,
        TokenKind::Uses,
        TokenKind::Void,
        TokenKind::Where,
        TokenKind::With,
        TokenKind::Yield,
    ];

    for (i, expected_kind) in expected.iter().enumerate() {
        assert_eq!(
            &tokens[i].kind, expected_kind,
            "Mismatch at index {i}: expected {expected_kind:?}, got {:?}",
            tokens[i].kind
        );
    }
}

#[test]
fn test_lex_type_keywords() {
    let interner = test_interner();
    let tokens = lex("int float bool str char byte Never", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::IntType));
    assert!(matches!(tokens[1].kind, TokenKind::FloatType));
    assert!(matches!(tokens[2].kind, TokenKind::BoolType));
    assert!(matches!(tokens[3].kind, TokenKind::StrType));
    assert!(matches!(tokens[4].kind, TokenKind::CharType));
    assert!(matches!(tokens[5].kind, TokenKind::ByteType));
    assert!(matches!(tokens[6].kind, TokenKind::NeverType));
}

#[test]
fn test_lex_constructors() {
    let interner = test_interner();
    let tokens = lex("Ok Err Some None", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Ok));
    assert!(matches!(tokens[1].kind, TokenKind::Err));
    assert!(matches!(tokens[2].kind, TokenKind::Some));
    assert!(matches!(tokens[3].kind, TokenKind::None));
}

#[test]
fn test_lex_char_literals() {
    let interner = test_interner();
    let tokens = lex(r"'a' '\n' '\\' '\''", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Char('a')));
    assert!(matches!(tokens[1].kind, TokenKind::Char('\n')));
    assert!(matches!(tokens[2].kind, TokenKind::Char('\\')));
    assert!(matches!(tokens[3].kind, TokenKind::Char('\'')));
}

#[test]
#[expect(
    clippy::approx_constant,
    reason = "testing float literal parsing, not using PI"
)]
#[allow(clippy::float_cmp)] // Exact bit-level comparison for lexer output
fn test_lex_float_literals() {
    let interner = test_interner();
    let tokens = lex("3.14 2.5e10 1_000.5", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Float(bits) if f64::from_bits(bits) == 3.14));
    assert!(matches!(tokens[1].kind, TokenKind::Float(bits) if f64::from_bits(bits) == 2.5e10));
    assert!(matches!(tokens[2].kind, TokenKind::Float(bits) if f64::from_bits(bits) == 1000.5));
}

#[test]
fn test_lex_line_comments() {
    let interner = test_interner();
    let tokens = lex("a // comment\nb", &interner);

    assert_eq!(tokens.len(), 4); // a, newline, b, EOF
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Newline));
    assert!(matches!(tokens[2].kind, TokenKind::Ident(_)));
}

#[test]
fn test_lex_line_continuation() {
    let interner = test_interner();
    let tokens = lex("a \\\nb", &interner);

    // Line continuation is skipped, no newline token
    assert_eq!(tokens.len(), 3); // a, b, EOF
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
}

#[test]
fn test_lex_with_comments_basic() {
    let interner = test_interner();
    let output = lex_with_comments("// comment\nlet x = 42", &interner);

    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.tokens.len(), 6); // newline, let, x, =, 42, EOF
    assert_eq!(output.comments[0].kind, CommentKind::Regular);
}

#[test]
fn test_lex_with_comments_multiple() {
    let interner = test_interner();
    let output = lex_with_comments("// first\n// second\nlet x = 42", &interner);

    assert_eq!(output.comments.len(), 2);
    assert_eq!(output.comments[0].kind, CommentKind::Regular);
    assert_eq!(output.comments[1].kind, CommentKind::Regular);
}

#[test]
fn test_lex_with_comments_doc_description() {
    let interner = test_interner();
    let output = lex_with_comments("// #Calculates the sum.", &interner);

    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.comments[0].kind, CommentKind::DocDescription);
    assert_eq!(
        interner.lookup(output.comments[0].content),
        " #Calculates the sum."
    );
}

#[test]
fn test_lex_with_comments_doc_param() {
    let interner = test_interner();
    let output = lex_with_comments("// @param x The value", &interner);

    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.comments[0].kind, CommentKind::DocParam);
    assert_eq!(
        interner.lookup(output.comments[0].content),
        " @param x The value"
    );
}

#[test]
fn test_lex_with_comments_doc_field() {
    let interner = test_interner();
    let output = lex_with_comments("// @field x The x coordinate", &interner);

    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.comments[0].kind, CommentKind::DocField);
    assert_eq!(
        interner.lookup(output.comments[0].content),
        " @field x The x coordinate"
    );
}

#[test]
fn test_lex_with_comments_doc_warning() {
    let interner = test_interner();
    let output = lex_with_comments("// !Panics if n is negative", &interner);

    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.comments[0].kind, CommentKind::DocWarning);
    assert_eq!(
        interner.lookup(output.comments[0].content),
        " !Panics if n is negative"
    );
}

#[test]
fn test_lex_with_comments_doc_example() {
    let interner = test_interner();
    let output = lex_with_comments("// >add(a: 1, b: 2) -> 3", &interner);

    assert_eq!(output.comments.len(), 1);
    assert_eq!(output.comments[0].kind, CommentKind::DocExample);
    // Preserve formatting after > exactly
    assert_eq!(
        interner.lookup(output.comments[0].content),
        " >add(a: 1, b: 2) -> 3"
    );
}

#[test]
fn test_lex_with_comments_normalize_spacing() {
    let interner = test_interner();

    // Missing space after //
    let output = lex_with_comments("//no space", &interner);
    assert_eq!(interner.lookup(output.comments[0].content), " no space");

    // Has space - preserved
    let output = lex_with_comments("// has space", &interner);
    assert_eq!(interner.lookup(output.comments[0].content), " has space");

    // Empty comment
    let output = lex_with_comments("//", &interner);
    assert_eq!(interner.lookup(output.comments[0].content), "");
}

#[test]
fn test_lex_with_comments_doc_normalize_spacing() {
    let interner = test_interner();

    // Doc with extra spaces normalized
    let output = lex_with_comments("//  #Description", &interner);
    assert_eq!(output.comments[0].kind, CommentKind::DocDescription);
    assert_eq!(interner.lookup(output.comments[0].content), " #Description");

    // Doc without space before marker
    let output = lex_with_comments("//#Description", &interner);
    assert_eq!(output.comments[0].kind, CommentKind::DocDescription);
    assert_eq!(interner.lookup(output.comments[0].content), " #Description");
}

#[test]
fn test_lex_with_comments_spans() {
    let interner = test_interner();
    let output = lex_with_comments("// comment\nlet x", &interner);

    // Comment span covers "// comment" (10 chars)
    assert_eq!(output.comments[0].span.start, 0);
    assert_eq!(output.comments[0].span.end, 10);
}

#[test]
fn test_lex_with_comments_mixed_doc_types() {
    let interner = test_interner();
    let source = r"// #Computes the sum.
// @param a First operand
// @param b Second operand
// !Panics on overflow
// >add(a: 1, b: 2) -> 3
@add (a: int, b: int) -> int = a + b";

    let output = lex_with_comments(source, &interner);

    assert_eq!(output.comments.len(), 5);
    assert_eq!(output.comments[0].kind, CommentKind::DocDescription);
    assert_eq!(output.comments[1].kind, CommentKind::DocParam);
    assert_eq!(output.comments[2].kind, CommentKind::DocParam);
    assert_eq!(output.comments[3].kind, CommentKind::DocWarning);
    assert_eq!(output.comments[4].kind, CommentKind::DocExample);
}

#[test]
fn test_lex_with_comments_no_comments() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42", &interner);

    assert!(output.comments.is_empty());
    assert_eq!(output.tokens.len(), 5); // let, x, =, 42, EOF
}
