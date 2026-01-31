//! Lexer for Ori using logos with string interning.
//!
//! Produces `TokenList` for Salsa queries.
//!
//! # Lexing
//!
//! The main entry point is [`lex()`], which converts source code into a [`TokenList`].
//!
//! # Token Types
//!
//! - **Literals**: integers (decimal, hex, binary), floats, strings, chars, durations, sizes
//! - **Keywords**: reserved words (`if`, `else`, `let`, etc.), type names, pattern keywords
//! - **Symbols**: operators, delimiters, punctuation
//! - **Identifiers**: user-defined names (interned for efficient comparison)
//!
//! # Escape Sequences
//!
//! String and char literals support: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`
//! Invalid escapes are preserved literally (e.g., `\q` becomes `\q`).
//!
//! # Error Handling
//!
//! Invalid tokens produce `TokenKind::Error`. The lexer continues processing after errors.
//!
//! # File Size Limits
//!
//! Source files larger than `u32::MAX` bytes (~4GB) will emit an error token.
//! Spans use `u32` for positions to keep tokens compact.
//!
//! # Modules
//!
//! - [`raw_token`]: Logos-derived tokenizer definition
//! - [`convert`]: Token conversion with string interning
//! - [`escape`]: Escape sequence processing for strings and chars
//! - [`comments`]: Comment classification and normalization
//! - [`parse_helpers`]: Numeric literal parsing utilities

mod comments;
mod convert;
mod escape;
mod parse_helpers;
mod raw_token;

use comments::classify_and_normalize_comment;
use convert::convert_token;
use logos::Logos;
use ori_ir::{Comment, CommentList, Span, StringInterner, Token, TokenKind, TokenList};
use raw_token::RawToken;

/// Lex source code into a [`TokenList`].
///
/// This is the core lexing function used by the `tokens` query.
///
/// # Token Types Produced
///
/// - **Literals**: `Int`, `Float`, `String`, `Char`, `Duration`, `Size`
/// - **Keywords**: `If`, `Else`, `Let`, `For`, etc. (see [`TokenKind`])
/// - **Identifiers**: User-defined names (interned via `interner`)
/// - **Symbols**: Operators, delimiters, punctuation
/// - **Trivia**: `Newline` tokens (comments and line continuations are skipped)
/// - **Special**: `Eof` at end, `Error` for invalid tokens
///
/// # String/Char Escape Handling
///
/// String and char literals support escape sequences: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`.
/// Invalid escape sequences are preserved literally (e.g., `\q` becomes `\q`).
///
/// # Error Tokens
///
/// Invalid input produces `TokenKind::Error` tokens. The lexer continues past errors,
/// allowing partial parsing of malformed source code.
///
/// # File Size Limits
///
/// Source files larger than `u32::MAX` bytes (~4GB) will produce an error token.
/// Positions are stored as `u32` to keep tokens compact (24 bytes each).
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    let mut result = TokenList::new();
    let mut logos = RawToken::lexer(source);

    while let Some(token_result) = logos.next() {
        // Use try_from_range with saturated fallback to avoid panic on huge files
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            // File exceeds u32::MAX - use saturated position
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        match token_result {
            Ok(raw) => {
                // Skip trivia (comments, newlines, continuations)
                match raw {
                    RawToken::LineComment | RawToken::LineContinuation => {}
                    RawToken::Newline => {
                        result.push(Token::new(TokenKind::Newline, span));
                    }
                    _ => {
                        let kind = convert_token(raw, slice, interner);
                        result.push(Token::new(kind, span));
                    }
                }
            }
            Err(()) => {
                result.push(Token::new(TokenKind::Error, span));
            }
        }
    }

    // Add EOF token
    // If source exceeds u32::MAX bytes, emit error token and use max position
    let eof_pos = u32::try_from(source.len()).unwrap_or_else(|_| {
        // File too large - emit error token at end
        let error_span = Span::new(u32::MAX - 1, u32::MAX);
        result.push(Token::new(TokenKind::Error, error_span));
        u32::MAX
    });
    let eof_span = Span::point(eof_pos);
    result.push(Token::new(TokenKind::Eof, eof_span));

    result
}

/// Output from lexing with comment capture.
///
/// Contains both the token stream (for parsing) and the comment list (for formatting).
#[derive(Clone, Default)]
pub struct LexOutput {
    /// The token stream for parsing.
    pub tokens: TokenList,
    /// Comments captured during lexing.
    pub comments: CommentList,
}

impl LexOutput {
    /// Create a new empty lex output.
    pub fn new() -> Self {
        LexOutput {
            tokens: TokenList::new(),
            comments: CommentList::new(),
        }
    }
}

/// Lex source code into tokens and comments.
///
/// This is the comment-preserving lexer entry point used by the formatter.
/// Returns both the token stream and a list of all comments in source order.
///
/// # Comment Classification
///
/// Comments are classified by their content:
/// - `// #...` → `DocDescription`
/// - `// @param ...` → `DocParam`
/// - `// @field ...` → `DocField`
/// - `// !...` → `DocWarning`
/// - `// >...` → `DocExample`
/// - `// ...` (anything else) → `Regular`
///
/// # Example
///
/// ```
/// use ori_lexer::lex_with_comments;
/// use ori_ir::StringInterner;
///
/// let interner = StringInterner::new();
/// let output = lex_with_comments("// comment\nlet x = 42", &interner);
/// assert_eq!(output.comments.len(), 1);
/// assert_eq!(output.tokens.len(), 6); // newline, let, x, =, 42, EOF
/// ```
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    let mut output = LexOutput::new();
    let mut logos = RawToken::lexer(source);

    while let Some(token_result) = logos.next() {
        // Use try_from_range with saturated fallback to avoid panic on huge files
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            // File exceeds u32::MAX - use saturated position
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        match token_result {
            Ok(raw) => {
                match raw {
                    RawToken::LineComment => {
                        // Capture comment - strip the leading "//"
                        let content_str = if slice.len() > 2 { &slice[2..] } else { "" };
                        let (kind, normalized) = classify_and_normalize_comment(content_str);
                        let content = interner.intern(&normalized);
                        output.comments.push(Comment::new(content, span, kind));
                    }
                    RawToken::LineContinuation => {}
                    RawToken::Newline => {
                        output.tokens.push(Token::new(TokenKind::Newline, span));
                    }
                    _ => {
                        let kind = convert_token(raw, slice, interner);
                        output.tokens.push(Token::new(kind, span));
                    }
                }
            }
            Err(()) => {
                output.tokens.push(Token::new(TokenKind::Error, span));
            }
        }
    }

    // Add EOF token
    let eof_pos = u32::try_from(source.len()).unwrap_or_else(|_| {
        let error_span = Span::new(u32::MAX - 1, u32::MAX);
        output.tokens.push(Token::new(TokenKind::Error, error_span));
        u32::MAX
    });
    let eof_span = Span::point(eof_pos);
    output.tokens.push(Token::new(TokenKind::Eof, eof_span));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::{CommentKind, DurationUnit, SizeUnit};

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
}
