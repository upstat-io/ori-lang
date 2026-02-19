//! Lexer tests.
//!
//! Tests for the `ori_lexer` crate, validating:
//! - Basic tokenization (keywords, identifiers, literals)
//! - Duration and size literal parsing
//! - Integer formats (decimal, hex, binary with underscores)
//! - String and char escape sequences
//! - Comment handling and classification
//! - Edge cases (empty input, whitespace, errors)

// Spec: 06-types.md § Duration, 06-types.md § Size
// Spec: duration-size-types-proposal.md § Numeric Prefix

use ori_ir::{CommentKind, DurationUnit, SizeUnit, StringInterner, TokenFlags, TokenKind};
use ori_lexer::lex_error::{LexErrorKind, LexSuggestion};
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

    // `run` and `try` are always-resolved keywords
    let tokens = lex("run try", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Run));
    assert!(matches!(tokens[1].kind, TokenKind::Try));

    // `catch` and `parallel` are soft keywords — only keyword when followed by `(`
    let tokens = lex("catch parallel", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));

    // With `(` lookahead, soft keywords become keyword tokens
    let tokens = lex("catch(e) parallel(tasks)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Catch));
    assert!(matches!(tokens[1].kind, TokenKind::LParen));
    assert!(matches!(tokens[4].kind, TokenKind::Parallel));
    assert!(matches!(tokens[5].kind, TokenKind::LParen));
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
    // Standalone backslash is not a valid token (produces Error)
    let tokens = lex("a \\ b", &interner);

    assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Error)));
}

#[test]
fn test_lex_template_literal() {
    let interner = test_interner();
    // Backticks are valid template literals, not errors
    let tokens = lex("`hello`", &interner);

    assert!(tokens
        .iter()
        .any(|t| matches!(t.kind, TokenKind::TemplateFull(_))));
}

#[test]
fn test_lex_template_full_content() {
    let interner = test_interner();
    let tokens = lex("`hello world`", &interner);
    if let TokenKind::TemplateFull(name) = tokens[0].kind {
        assert_eq!(interner.lookup(name), "hello world");
    } else {
        panic!("Expected TemplateFull, got {:?}", tokens[0].kind);
    }
}

#[test]
fn test_lex_template_interpolation() {
    let interner = test_interner();
    let tokens = lex("`hello {name}`", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::TemplateHead(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[2].kind, TokenKind::TemplateTail(_)));

    // Verify head text
    if let TokenKind::TemplateHead(name) = tokens[0].kind {
        assert_eq!(interner.lookup(name), "hello ");
    } else {
        panic!("Expected TemplateHead");
    }
}

#[test]
fn test_lex_template_multiple_interpolations() {
    let interner = test_interner();
    let tokens = lex("`{a} and {b}`", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::TemplateHead(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[2].kind, TokenKind::TemplateMiddle(_)));
    assert!(matches!(tokens[3].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[4].kind, TokenKind::TemplateTail(_)));
}

#[test]
fn test_lex_template_format_spec() {
    let interner = test_interner();
    let tokens = lex("`{value:x}`", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::TemplateHead(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[2].kind, TokenKind::FormatSpec(_)));
    assert!(matches!(tokens[3].kind, TokenKind::TemplateTail(_)));

    // Verify format spec content (stripped of leading ':')
    if let TokenKind::FormatSpec(name) = tokens[2].kind {
        assert_eq!(interner.lookup(name), "x");
    } else {
        panic!("Expected FormatSpec");
    }
}

#[test]
fn test_lex_template_format_spec_complex() {
    let interner = test_interner();
    let tokens = lex("`{value:>10.2f}`", &interner);
    if let TokenKind::FormatSpec(name) = tokens[2].kind {
        assert_eq!(interner.lookup(name), ">10.2f");
    } else {
        panic!("Expected FormatSpec, got {:?}", tokens[2].kind);
    }
}

#[test]
fn test_lex_all_reserved_keywords() {
    let interner = test_interner();
    let source =
        "async break continue do else false for if impl in let loop match pub self Self then trait true type use uses void where with yield";
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
#[allow(
    clippy::float_cmp,
    reason = "exact bit-level comparison of lexer float output"
)]
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
fn test_lex_standalone_backslash() {
    let interner = test_interner();
    let tokens = lex("a \\\nb", &interner);

    // Ori does not use backslash line continuation (unlike C).
    // `\` is an invalid token — produces Error.
    // Tokens: a, Error(\), Newline, b, Eof
    assert_eq!(tokens.len(), 5);
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Error));
    assert!(matches!(tokens[2].kind, TokenKind::Newline));
    assert!(matches!(tokens[3].kind, TokenKind::Ident(_)));
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
    assert_eq!(output.comments[0].kind, CommentKind::DocMember);
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
    assert_eq!(output.comments[0].kind, CommentKind::DocMember);
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
// * a: First operand
// * b: Second operand
// !Panics on overflow
// >add(a: 1, b: 2) -> 3
@add (a: int, b: int) -> int = a + b";

    let output = lex_with_comments(source, &interner);

    assert_eq!(output.comments.len(), 5);
    assert_eq!(output.comments[0].kind, CommentKind::DocDescription);
    assert_eq!(output.comments[1].kind, CommentKind::DocMember);
    assert_eq!(output.comments[2].kind, CommentKind::DocMember);
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

// Decimal duration/size literal tests
// Spec: decimal-duration-size-literals-proposal.md
// "Decimal syntax is compile-time sugar computed via integer arithmetic"

#[test]
fn test_lex_decimal_duration_seconds() {
    let interner = test_interner();
    let tokens = lex("1.5s", &interner);

    assert_eq!(tokens.len(), 2); // Duration, EOF
                                 // 1.5s = 1,500,000,000 nanoseconds
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(1_500_000_000, DurationUnit::Nanoseconds)
    ));
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 4);
}

#[test]
fn test_lex_decimal_duration_milliseconds() {
    let interner = test_interner();
    let tokens = lex("2.5ms", &interner);

    assert_eq!(tokens.len(), 2);
    // 2.5ms = 2,500,000 nanoseconds
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(2_500_000, DurationUnit::Nanoseconds)
    ));
}

#[test]
fn test_lex_decimal_duration_all_units() {
    let interner = test_interner();

    // Test decimal durations for units where 1.5 * multiplier is whole
    // 1.5ns = 1.5 nanoseconds → NOT whole → Error
    let tokens = lex("1.5ns", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Error),
        "Expected Error for 1.5ns (not whole), got {:?}",
        tokens[0].kind
    );

    // 1.5us = 1,500 nanoseconds → whole
    let tokens = lex("1.5us", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(1_500, DurationUnit::Nanoseconds)
    ));

    // 1.5ms = 1,500,000 nanoseconds → whole
    let tokens = lex("1.5ms", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(1_500_000, DurationUnit::Nanoseconds)
    ));

    // 1.5s = 1,500,000,000 nanoseconds → whole
    let tokens = lex("1.5s", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(1_500_000_000, DurationUnit::Nanoseconds)
    ));

    // 1.5m = 90,000,000,000 nanoseconds → whole
    let tokens = lex("1.5m", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(90_000_000_000, DurationUnit::Nanoseconds)
    ));

    // 1.5h = 5,400,000,000,000 nanoseconds → whole
    let tokens = lex("1.5h", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(5_400_000_000_000, DurationUnit::Nanoseconds)
    ));
}

#[test]
fn test_lex_decimal_size_kilobytes() {
    let interner = test_interner();
    let tokens = lex("2.5kb", &interner);

    assert_eq!(tokens.len(), 2); // Size, EOF
                                 // 2.5kb = 2,500 bytes (SI: 1kb = 1000 bytes)
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(2_500, SizeUnit::Bytes)
    ));
}

#[test]
fn test_lex_decimal_size_all_units() {
    let interner = test_interner();

    // 1.5b = 1.5 bytes → NOT whole → Error
    let tokens = lex("1.5b", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Error),
        "Expected Error for 1.5b (not whole), got {:?}",
        tokens[0].kind
    );

    // 1.5kb = 1,500 bytes → whole
    let tokens = lex("1.5kb", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(1_500, SizeUnit::Bytes)
    ));

    // 1.5mb = 1,500,000 bytes → whole
    let tokens = lex("1.5mb", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(1_500_000, SizeUnit::Bytes)
    ));

    // 1.5gb = 1,500,000,000 bytes → whole
    let tokens = lex("1.5gb", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(1_500_000_000, SizeUnit::Bytes)
    ));

    // 1.5tb = 1,500,000,000,000 bytes → whole
    let tokens = lex("1.5tb", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(1_500_000_000_000, SizeUnit::Bytes)
    ));
}

#[test]
fn test_lex_decimal_duration_many_digits() {
    let interner = test_interner();

    // 1.123456789s = 1,123,456,789 nanoseconds (9 decimal places, exact)
    let tokens = lex("1.123456789s", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(1_123_456_789, DurationUnit::Nanoseconds)
    ));
}

#[test]
fn test_lex_valid_integer_duration_still_works() {
    let interner = test_interner();

    // Ensure valid integer durations still work
    let tokens = lex("1500ms", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(1500, DurationUnit::Milliseconds)
    ));

    let tokens = lex("30m", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Duration(30, DurationUnit::Minutes)
    ));
}

#[test]
fn test_lex_valid_integer_size_still_works() {
    let interner = test_interner();

    // Ensure valid integer sizes still work
    let tokens = lex("1024kb", &interner);
    assert!(matches!(
        tokens[0].kind,
        TokenKind::Size(1024, SizeUnit::Kilobytes)
    ));
}

// ─── Section 07: Diagnostics & Error Recovery ─────────────────────────────

// ── Error accumulation: errors surfaced in LexOutput ──────────────────────

#[test]
fn test_errors_surfaced_in_lex_output() {
    let interner = test_interner();
    // Standalone backslash produces an error token AND an accumulated error
    let output = lex_with_comments("a \\ b", &interner);

    // Token stream still contains the error token for parser recovery
    assert!(output
        .tokens
        .iter()
        .any(|t| matches!(t.kind, TokenKind::Error)));

    // Error is also surfaced in the errors vector
    assert!(
        output.has_errors(),
        "Expected errors in LexOutput for standalone backslash"
    );
    assert!(
        !output.errors.is_empty(),
        "Expected at least one error in LexOutput.errors"
    );
}

#[test]
fn test_semicolon_produces_error() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42;", &interner);

    // Semicolon should produce both a Semicolon token AND an error
    assert!(output.has_errors(), "Expected error for semicolon");
    assert_eq!(output.errors.len(), 1);
    assert!(
        matches!(output.errors[0].kind, LexErrorKind::Semicolon),
        "Expected Semicolon error kind, got {:?}",
        output.errors[0].kind
    );

    // The error should carry a removal suggestion
    assert!(
        !output.errors[0].suggestions.is_empty(),
        "Semicolon error should have a removal suggestion"
    );
}

#[test]
fn test_semicolon_error_span() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42;", &interner);

    assert_eq!(output.errors.len(), 1);
    let err = &output.errors[0];
    // The semicolon is at byte offset 10 (0-indexed), length 1
    assert_eq!(err.span.start, 10);
    assert_eq!(err.span.end, 11);
}

#[test]
fn test_semicolon_recovery_tokens_correct() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42;\nlet y = 10", &interner);

    // After the semicolon error, the lexer should recover and produce
    // correct tokens for the next line
    let kinds: Vec<_> = output.tokens.iter().map(|t| &t.kind).collect();
    // Expected: let, x, =, 42, ;, newline, let, y, =, 10, EOF
    assert!(
        kinds.iter().any(|k| matches!(k, TokenKind::Semicolon)),
        "Semicolon should still produce a Semicolon token for parser"
    );

    // Count Int tokens — should have both 42 and 10
    let int_count = kinds
        .iter()
        .filter(|k| matches!(k, TokenKind::Int(_)))
        .count();
    assert_eq!(
        int_count, 2,
        "Both integer literals should be lexed after recovery"
    );
}

// ── Unicode confusable detection ──────────────────────────────────────────

#[test]
fn test_unicode_confusable_smart_quote() {
    let interner = test_interner();
    // \u{201C} = left double quotation mark (curly quote)
    let source = "\u{201C}hello\u{201D}";
    let output = lex_with_comments(source, &interner);

    assert!(
        output.has_errors(),
        "Expected confusable error for smart quotes"
    );

    // Should detect at least the left smart quote as a confusable
    let confusable_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| matches!(e.kind, LexErrorKind::UnicodeConfusable { .. }))
        .collect();
    assert!(
        !confusable_errors.is_empty(),
        "Expected UnicodeConfusable error for smart quote"
    );

    // Check that the confusable points to `"` as the replacement
    if let LexErrorKind::UnicodeConfusable {
        found, suggested, ..
    } = &confusable_errors[0].kind
    {
        assert_eq!(*found, '\u{201C}');
        assert_eq!(*suggested, '"');
    }
}

#[test]
fn test_unicode_confusable_en_dash() {
    let interner = test_interner();
    // \u{2013} = en dash, should suggest ASCII hyphen-minus
    let source = "a \u{2013} b";
    let output = lex_with_comments(source, &interner);

    assert!(output.has_errors(), "Expected confusable error for en dash");
    let confusable_errors: Vec<_> = output
        .errors
        .iter()
        .filter(|e| matches!(e.kind, LexErrorKind::UnicodeConfusable { .. }))
        .collect();
    assert!(!confusable_errors.is_empty());

    if let LexErrorKind::UnicodeConfusable {
        found, suggested, ..
    } = &confusable_errors[0].kind
    {
        assert_eq!(*found, '\u{2013}');
        assert_eq!(*suggested, '-');
    }
}

#[test]
fn test_unicode_confusable_fullwidth_plus() {
    let interner = test_interner();
    // \u{FF0B} = fullwidth plus sign
    let source = "a \u{FF0B} b";
    let output = lex_with_comments(source, &interner);

    assert!(output.has_errors());
    let confusable = output
        .errors
        .iter()
        .find(|e| matches!(e.kind, LexErrorKind::UnicodeConfusable { .. }));
    assert!(
        confusable.is_some(),
        "Expected confusable for fullwidth plus"
    );
}

// ── Multiple errors in one file ───────────────────────────────────────────

#[test]
fn test_multiple_errors_accumulated() {
    let interner = test_interner();
    // Two semicolons — should produce two errors
    let output = lex_with_comments("let x = 1;\nlet y = 2;", &interner);

    assert_eq!(
        output.errors.len(),
        2,
        "Expected two errors for two semicolons"
    );
    assert!(matches!(output.errors[0].kind, LexErrorKind::Semicolon));
    assert!(matches!(output.errors[1].kind, LexErrorKind::Semicolon));
}

#[test]
fn test_mixed_error_types() {
    let interner = test_interner();
    // Semicolon + standalone backslash = two different error types
    let output = lex_with_comments("a;\nb \\", &interner);

    assert!(
        output.errors.len() >= 2,
        "Expected at least two errors, got {}",
        output.errors.len()
    );

    let has_semicolon = output
        .errors
        .iter()
        .any(|e| matches!(e.kind, LexErrorKind::Semicolon));
    let has_backslash = output
        .errors
        .iter()
        .any(|e| matches!(e.kind, LexErrorKind::StandaloneBackslash));
    assert!(has_semicolon, "Expected Semicolon error");
    assert!(has_backslash, "Expected StandaloneBackslash error");
}

// ── Error structure: WHERE+WHAT+WHY+HOW ───────────────────────────────────

#[test]
fn test_error_has_span() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42;", &interner);

    for err in &output.errors {
        assert!(
            err.span.start < err.span.end,
            "Error span should be non-empty: {:?}",
            err.span
        );
    }
}

#[test]
fn test_error_has_suggestions() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42;", &interner);

    // Semicolon error should have a removal suggestion
    let semicolon_err = output
        .errors
        .iter()
        .find(|e| matches!(e.kind, LexErrorKind::Semicolon))
        .expect("Expected semicolon error");

    assert!(
        !semicolon_err.suggestions.is_empty(),
        "Semicolon error should have suggestions"
    );

    // The suggestion should be for removal (replacement with empty string)
    let suggestion = &semicolon_err.suggestions[0];
    assert!(
        suggestion.replacement.is_some(),
        "Semicolon suggestion should have a replacement"
    );
    if let Some(ref replacement) = suggestion.replacement {
        assert_eq!(
            replacement.text, "",
            "Removal suggestion should replace with empty string"
        );
    }
}

// ── render_lex_error → Diagnostic rendering ───────────────────────────────

#[test]
fn test_render_lex_error_semicolon_diagnostic() {
    use ori_lexer::lex_error::LexError;

    let err = LexError::semicolon(ori_ir::Span::new(10, 11));
    let diag = oric::problem::lex::render_lex_error(&err);

    assert!(diag.is_error());
    assert_eq!(diag.code, ori_diagnostic::ErrorCode::E0007);
    assert!(
        diag.message.contains("semicolon"),
        "Message should mention semicolons: {}",
        diag.message
    );
}

#[test]
fn test_render_lex_error_confusable_diagnostic() {
    use ori_lexer::lex_error::LexError;

    let err = LexError::unicode_confusable(
        ori_ir::Span::new(0, 3),
        '\u{201C}',
        '"',
        "Left Double Quotation Mark",
    );
    let diag = oric::problem::lex::render_lex_error(&err);

    assert!(diag.is_error());
    assert_eq!(diag.code, ori_diagnostic::ErrorCode::E0011);
    assert!(
        diag.message.contains("Left Double Quotation Mark"),
        "Message should name the confusable: {}",
        diag.message
    );
}

#[test]
fn test_render_lex_error_unterminated_string() {
    use ori_lexer::lex_error::LexError;

    let err = LexError::unterminated_string(ori_ir::Span::new(0, 10));
    let diag = oric::problem::lex::render_lex_error(&err);

    assert!(diag.is_error());
    assert_eq!(diag.code, ori_diagnostic::ErrorCode::E0001);
    assert!(diag.message.contains("unterminated"));
}

#[test]
fn test_render_lex_error_invalid_escape() {
    use ori_lexer::lex_error::LexError;

    let err = LexError::invalid_string_escape(ori_ir::Span::new(5, 7), 'z');
    let diag = oric::problem::lex::render_lex_error(&err);

    assert!(diag.is_error());
    assert_eq!(diag.code, ori_diagnostic::ErrorCode::E0005);
    assert!(diag.message.contains("\\z"));
}

// ── Detached doc comment warnings ─────────────────────────────────────────

#[test]
fn test_detached_doc_comment_warning() {
    let interner = test_interner();
    // Doc comment followed by an expression (not a declaration like @func, type, trait, etc.)
    let output = lex_with_comments("// #This is a doc comment\n42 + 10", &interner);

    // `42` is NOT a declaration keyword — so the doc comment should be flagged as detached
    assert!(
        !output.warnings.is_empty(),
        "Expected detached doc comment warning"
    );
}

#[test]
fn test_attached_doc_comment_no_warning() {
    let interner = test_interner();
    // Doc comment followed by a function declaration (@name)
    let output = lex_with_comments(
        "// #Adds two numbers\n@add (a: int, b: int) -> int = a + b",
        &interner,
    );

    assert!(
        output.warnings.is_empty(),
        "Expected no warnings for attached doc comment, got {} warnings",
        output.warnings.len()
    );
}

// ── No errors for clean input ─────────────────────────────────────────────

#[test]
fn test_no_errors_for_clean_input() {
    let interner = test_interner();
    let output = lex_with_comments("let x = 42\nlet y = 10", &interner);

    assert!(
        output.errors.is_empty(),
        "Clean input should produce no errors"
    );
    assert!(
        output.warnings.is_empty(),
        "Clean input should produce no warnings"
    );
}

#[test]
fn test_has_errors_helper() {
    let interner = test_interner();

    let clean = lex_with_comments("let x = 42", &interner);
    assert!(!clean.has_errors());

    let dirty = lex_with_comments("let x = 42;", &interner);
    assert!(dirty.has_errors());
}

// ── LexSuggestion structure ──────────────────────────────────────────────

#[test]
fn test_suggestion_text_only() {
    let suggestion = LexSuggestion::text("try using double quotes", 0);
    assert_eq!(suggestion.message, "try using double quotes");
    assert!(suggestion.replacement.is_none());
}

#[test]
fn test_suggestion_with_replacement() {
    let suggestion = LexSuggestion::replace("replace with `==`", ori_ir::Span::new(0, 3), "==");
    assert!(suggestion.replacement.is_some());
    let repl = suggestion.replacement.unwrap();
    assert_eq!(repl.text, "==");
    assert_eq!(repl.span, ori_ir::Span::new(0, 3));
}

#[test]
fn test_suggestion_removal() {
    let suggestion = LexSuggestion::removal("remove this", ori_ir::Span::new(5, 6));
    assert!(suggestion.replacement.is_some());
    let repl = suggestion.replacement.unwrap();
    assert_eq!(repl.text, "");
    assert_eq!(repl.span, ori_ir::Span::new(5, 6));
}

// ── Context-sensitive keyword tests ──────────────────────────────────────

// Section 03.3/03.10: Soft keywords resolve to keyword tokens only when
// followed by `(` (with horizontal whitespace allowed, but not newlines).
// The 6 soft keywords are: cache, catch, parallel, spawn, recurse, timeout.

#[test]
fn test_soft_keywords_as_identifiers_without_lparen() {
    let interner = test_interner();
    // Without `(` after them, soft keywords lex as plain identifiers
    let tokens = lex("cache catch parallel spawn recurse timeout", &interner);

    for i in 0..6 {
        assert!(
            matches!(tokens[i].kind, TokenKind::Ident(_)),
            "Soft keyword at index {i} should be Ident without `(`, got {:?}",
            tokens[i].kind
        );
    }
}

#[test]
fn test_soft_keywords_as_keywords_with_lparen() {
    let interner = test_interner();
    // With `(` immediately after, soft keywords resolve to keyword tokens
    let tokens = lex("cache(x)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Cache));

    let tokens = lex("catch(e)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Catch));

    let tokens = lex("parallel(tasks)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Parallel));

    let tokens = lex("spawn(task)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Spawn));

    let tokens = lex("recurse(n)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Recurse));

    let tokens = lex("timeout(5s, task)", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Timeout));
}

#[test]
fn test_soft_keywords_with_space_before_lparen() {
    let interner = test_interner();
    // Horizontal whitespace (space/tab) between keyword and `(` is allowed
    let tokens = lex("cache (x)", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Cache),
        "Space before `(` should still resolve to keyword, got {:?}",
        tokens[0].kind
    );

    let tokens = lex("parallel\t(tasks)", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Parallel),
        "Tab before `(` should still resolve to keyword, got {:?}",
        tokens[0].kind
    );

    let tokens = lex("catch  \t (e)", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Catch),
        "Mixed whitespace before `(` should still resolve to keyword, got {:?}",
        tokens[0].kind
    );
}

#[test]
fn test_soft_keywords_with_newline_before_lparen() {
    let interner = test_interner();
    // Newline between keyword and `(` blocks soft keyword resolution
    // (newlines are significant in Ori)
    let tokens = lex("cache\n(x)", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Ident(_)),
        "Newline before `(` should prevent keyword resolution, got {:?}",
        tokens[0].kind
    );

    let tokens = lex("spawn\r\n(task)", &interner);
    assert!(
        matches!(tokens[0].kind, TokenKind::Ident(_)),
        "CRLF before `(` should prevent keyword resolution, got {:?}",
        tokens[0].kind
    );
}

#[test]
fn test_soft_keywords_in_non_call_positions() {
    let interner = test_interner();
    // Soft keywords as variable names
    let tokens = lex("let cache = 42", &interner);
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));

    // Soft keywords as field access
    let tokens = lex("x.spawn", &interner);
    assert!(matches!(tokens[2].kind, TokenKind::Ident(_)));

    // Soft keywords in assignment
    let tokens = lex("timeout = 100", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));

    // Soft keywords as type annotation
    let tokens = lex("let x: parallel", &interner);
    assert!(matches!(tokens[3].kind, TokenKind::Ident(_)));
}

#[test]
fn test_contextual_kw_flag_set_on_soft_keywords() {
    let interner = test_interner();
    let tokens = lex("cache(x)", &interner);

    // The CONTEXTUAL_KW flag should be set on the soft keyword token
    let flags = tokens.flags();
    assert!(
        flags[0].contains(TokenFlags::CONTEXTUAL_KW),
        "CONTEXTUAL_KW flag should be set on soft keyword `cache`, flags: {:b}",
        flags[0].bits()
    );
}

#[test]
fn test_contextual_kw_flag_not_set_on_regular_ident() {
    let interner = test_interner();
    let tokens = lex("cache = 42", &interner);

    // When resolved as identifier, CONTEXTUAL_KW should NOT be set
    let flags = tokens.flags();
    assert!(
        !flags[0].contains(TokenFlags::CONTEXTUAL_KW),
        "CONTEXTUAL_KW flag should NOT be set on identifier `cache`, flags: {:b}",
        flags[0].bits()
    );
}

#[test]
fn test_contextual_kw_flag_not_set_on_reserved_keywords() {
    let interner = test_interner();
    let tokens = lex("if(x)", &interner);

    // Reserved keywords are always resolved — they don't get CONTEXTUAL_KW
    let flags = tokens.flags();
    assert!(
        !flags[0].contains(TokenFlags::CONTEXTUAL_KW),
        "CONTEXTUAL_KW flag should NOT be set on reserved keyword `if`, flags: {:b}",
        flags[0].bits()
    );
}

// ── Always-resolved pattern keywords ──────────────────────────────────────

// `run`, `try`, `by` are always reserved keywords (not context-sensitive).
// They resolve to keyword tokens regardless of whether `(` follows.

#[test]
fn test_always_keywords_without_lparen() {
    let interner = test_interner();
    let tokens = lex("run try by", &interner);

    assert!(
        matches!(tokens[0].kind, TokenKind::Run),
        "`run` should always be a keyword, got {:?}",
        tokens[0].kind
    );
    assert!(
        matches!(tokens[1].kind, TokenKind::Try),
        "`try` should always be a keyword, got {:?}",
        tokens[1].kind
    );
    assert!(
        matches!(tokens[2].kind, TokenKind::By),
        "`by` should always be a keyword, got {:?}",
        tokens[2].kind
    );
}

#[test]
fn test_always_keywords_with_lparen() {
    let interner = test_interner();
    // run(x) try(y) by(z)
    // [0]=run [1]=( [2]=x [3]=) [4]=try [5]=( [6]=y [7]=) [8]=by [9]=( [10]=z [11]=) [12]=EOF
    let tokens = lex("run(x) try(y) by(z)", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Run));
    assert!(matches!(tokens[4].kind, TokenKind::Try));
    assert!(matches!(tokens[8].kind, TokenKind::By));
}

// ── Type keywords are always resolved ─────────────────────────────────────

#[test]
fn test_type_keywords_always_resolved() {
    let interner = test_interner();
    // Type keywords are always resolved, not context-sensitive
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
fn test_type_keywords_not_context_sensitive() {
    let interner = test_interner();
    // Even without `(`, type keywords resolve as keywords (not identifiers)
    // let x: int = 42
    // [0]=let [1]=x [2]=: [3]=int [4]== [5]=42 [6]=EOF
    let tokens = lex("let x: int = 42", &interner);
    assert!(matches!(tokens[3].kind, TokenKind::IntType));

    let tokens = lex("int + float", &interner);
    assert!(matches!(tokens[0].kind, TokenKind::IntType));
    assert!(matches!(tokens[2].kind, TokenKind::FloatType));
}

// ── Built-in names are regular identifiers ────────────────────────────────

// The spec says built-in names (len, is_empty, assert, etc.) are
// "reserved in call position, usable as variables otherwise".
// But this is a semantic concern — the lexer emits them as Ident tokens.
// The type-checker/evaluator enforces call-position reservation.

#[test]
fn test_builtin_names_are_identifiers() {
    let interner = test_interner();
    let tokens = lex("len is_empty assert assert_eq compare min max", &interner);

    for i in 0..7 {
        assert!(
            matches!(tokens[i].kind, TokenKind::Ident(_)),
            "Built-in name at index {i} should be Ident, got {:?}",
            tokens[i].kind
        );
    }
}

#[test]
fn test_builtin_names_usable_as_variables() {
    let interner = test_interner();
    let tokens = lex("let len = 42", &interner);
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));

    let tokens = lex("let max = 100", &interner);
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
}

// ── print/panic are always-resolved keywords ─────────────────────────────

#[test]
fn test_print_panic_always_keywords() {
    let interner = test_interner();
    // Unlike other built-in names, print/panic/todo/unreachable are
    // always-resolved as keyword tokens (V1 compatibility)
    let tokens = lex("print panic todo unreachable", &interner);

    assert!(matches!(tokens[0].kind, TokenKind::Print));
    assert!(matches!(tokens[1].kind, TokenKind::Panic));
    assert!(matches!(tokens[2].kind, TokenKind::Todo));
    assert!(matches!(tokens[3].kind, TokenKind::Unreachable));
}

// ── `without` and `max` are parser-resolved (plain identifiers) ──────────

#[test]
fn test_parser_resolved_keywords_are_identifiers() {
    let interner = test_interner();
    // `without` and `max` (in type context) have no TokenKind variants.
    // The parser recognizes them contextually as identifiers.
    let tokens = lex("without max", &interner);

    assert!(
        matches!(tokens[0].kind, TokenKind::Ident(_)),
        "`without` should be Ident (parser-resolved), got {:?}",
        tokens[0].kind
    );
    assert!(
        matches!(tokens[1].kind, TokenKind::Ident(_)),
        "`max` should be Ident (parser-resolved), got {:?}",
        tokens[1].kind
    );
}

// ── Reserved-future keywords ─────────────────────────────────────────────

#[test]
fn test_reserved_future_keywords_lex_as_ident_with_error() {
    let interner = test_interner();
    // Reserved-future keywords (asm, inline, static, union, view) lex as
    // identifiers but produce a diagnostic error
    let output = lex_with_comments("let asm = 42", &interner);

    // Still lexes as Ident so parser can continue
    assert!(matches!(output.tokens[1].kind, TokenKind::Ident(_)));

    // But an error is produced
    assert!(
        output.has_errors(),
        "Expected error for reserved-future keyword `asm`"
    );
    assert!(output
        .errors
        .iter()
        .any(|e| matches!(e.kind, LexErrorKind::ReservedFutureKeyword { .. })));
}
