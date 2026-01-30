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

use logos::Logos;
use ori_ir::{
    Comment, CommentKind, CommentList, DurationUnit, SizeUnit, Span, StringInterner, Token,
    TokenKind, TokenList,
};

/// Parse integer skipping underscores without allocation.
#[inline]
fn parse_int_skip_underscores(s: &str, radix: u32) -> Option<u64> {
    let mut result: u64 = 0;
    for c in s.chars() {
        if c == '_' {
            continue;
        }
        let digit = c.to_digit(radix)?;
        result = result.checked_mul(u64::from(radix))?;
        result = result.checked_add(u64::from(digit))?;
    }
    Some(result)
}

/// Parse float - only allocate if underscores present.
#[inline]
fn parse_float_skip_underscores(s: &str) -> Option<f64> {
    if s.contains('_') {
        s.replace('_', "").parse().ok()
    } else {
        s.parse().ok()
    }
}

/// Parse numeric value with suffix, returning (value, unit).
#[inline]
fn parse_with_suffix<T: Copy>(s: &str, suffix_len: usize, unit: T) -> Option<(u64, T)> {
    s[..s.len() - suffix_len]
        .parse::<u64>()
        .ok()
        .map(|v| (v, unit))
}

/// Raw token from logos (before interning).
#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\r]+")] // Skip horizontal whitespace
enum RawToken {
    #[regex(r"//[^\n]*")]
    LineComment,

    #[token("\n")]
    Newline,

    #[regex(r"\\[ \t]*\n")]
    LineContinuation,

    #[token("async")]
    Async,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("do")]
    Do,
    #[token("else")]
    Else,
    #[token("false")]
    False,
    #[token("for")]
    For,
    #[token("if")]
    If,
    #[token("impl")]
    Impl,
    #[token("in")]
    In,
    #[token("let")]
    Let,
    #[token("loop")]
    Loop,
    #[token("match")]
    Match,
    #[token("mut")]
    Mut,
    #[token("pub")]
    Pub,
    #[token("self")]
    SelfLower,
    #[token("Self")]
    SelfUpper,
    #[token("then")]
    Then,
    #[token("trait")]
    Trait,
    #[token("true")]
    True,
    #[token("type")]
    Type,
    #[token("use")]
    Use,
    #[token("uses")]
    Uses,
    #[token("void")]
    Void,
    #[token("where")]
    Where,
    #[token("with")]
    With,
    #[token("yield")]
    Yield,

    // Additional keywords
    #[token("tests")]
    Tests,
    #[token("as")]
    As,
    #[token("dyn")]
    Dyn,
    #[token("extend")]
    Extend,
    #[token("extension")]
    Extension,
    #[token("skip")]
    Skip,

    #[token("int")]
    IntType,
    #[token("float")]
    FloatType,
    #[token("bool")]
    BoolType,
    #[token("str")]
    StrType,
    #[token("char")]
    CharType,
    #[token("byte")]
    ByteType,
    #[token("Never")]
    NeverType,

    #[token("Ok")]
    Ok,
    #[token("Err")]
    Err,
    #[token("Some")]
    Some,
    #[token("None")]
    None,

    #[token("cache")]
    Cache,
    #[token("catch")]
    Catch,
    #[token("parallel")]
    Parallel,
    #[token("spawn")]
    Spawn,
    #[token("recurse")]
    Recurse,
    #[token("run")]
    Run,
    #[token("timeout")]
    Timeout,
    #[token("try")]
    Try,

    #[token("print")]
    Print,
    #[token("panic")]
    Panic,

    #[token("#[")]
    HashBracket,
    #[token("@")]
    At,
    #[token("$")]
    Dollar,
    #[token("#")]
    Hash,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("::")]
    DoubleColon,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("|")]
    Pipe,
    #[token("??")]
    DoubleQuestion,
    #[token("?")]
    Question,
    #[token("_", priority = 3)]
    Underscore,
    #[token(";")]
    Semicolon,

    #[token("==")]
    EqEq,
    #[token("=")]
    Eq,
    #[token("!=")]
    NotEq,
    #[token("<<")]
    Shl,
    #[token("<=")]
    LtEq,
    #[token("<")]
    Lt,
    // Note: `>>` and `>=` are NOT lexed as single tokens.
    // The lexer always produces individual `>` tokens.
    // The parser combines adjacent `>` tokens into `>>` or `>=` operators
    // in expression context. This enables parsing nested generics like
    // `Result<Result<T, E>, E>` where the `>>` at the end should be two
    // separate `>` tokens closing the generic parameter lists.
    #[token(">")]
    Gt,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("!")]
    Bang,
    #[token("~")]
    Tilde,
    #[token("&&")]
    AmpAmp,
    #[token("&")]
    Amp,
    #[token("||")]
    PipePipe,
    #[token("^")]
    Caret,
    #[token("div")]
    Div,

    // Hex integer (zero-allocation parsing)
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", |lex| {
        parse_int_skip_underscores(&lex.slice()[2..], 16)
    })]
    HexInt(u64),

    // Binary integer (zero-allocation parsing)
    #[regex(r"0b[01][01_]*", |lex| {
        parse_int_skip_underscores(&lex.slice()[2..], 2)
    })]
    BinInt(u64),

    // Integer (zero-allocation parsing)
    #[regex(r"[0-9][0-9_]*", |lex| {
        parse_int_skip_underscores(lex.slice(), 10)
    })]
    Int(u64),

    // Float (only allocates if underscores present)
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?", |lex| {
        parse_float_skip_underscores(lex.slice())
    })]
    Float(f64),

    // Duration literals (using shared helper)
    #[regex(r"[0-9]+ms", |lex| parse_with_suffix(lex.slice(), 2, DurationUnit::Milliseconds))]
    DurationMs((u64, DurationUnit)),

    #[regex(r"[0-9]+s", |lex| parse_with_suffix(lex.slice(), 1, DurationUnit::Seconds))]
    DurationS((u64, DurationUnit)),

    #[regex(r"[0-9]+m", |lex| parse_with_suffix(lex.slice(), 1, DurationUnit::Minutes))]
    DurationM((u64, DurationUnit)),

    #[regex(r"[0-9]+h", |lex| parse_with_suffix(lex.slice(), 1, DurationUnit::Hours))]
    DurationH((u64, DurationUnit)),

    // Size literals (using shared helper)
    #[regex(r"[0-9]+b", |lex| parse_with_suffix(lex.slice(), 1, SizeUnit::Bytes))]
    SizeB((u64, SizeUnit)),

    #[regex(r"[0-9]+kb", |lex| parse_with_suffix(lex.slice(), 2, SizeUnit::Kilobytes))]
    SizeKb((u64, SizeUnit)),

    #[regex(r"[0-9]+mb", |lex| parse_with_suffix(lex.slice(), 2, SizeUnit::Megabytes))]
    SizeMb((u64, SizeUnit)),

    #[regex(r"[0-9]+gb", |lex| parse_with_suffix(lex.slice(), 2, SizeUnit::Gigabytes))]
    SizeGb((u64, SizeUnit)),

    // String literal (no unescaped newlines allowed)
    #[regex(r#""([^"\\\n\r]|\\.)*""#)]
    String,

    // Char literal (no unescaped newlines allowed)
    #[regex(r"'([^'\\\n\r]|\\.)'")]
    Char,

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
}

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
        let span = Span::from_range(logos.span());
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
        let span = Span::from_range(logos.span());
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

/// Classify a comment by its content and return the normalized content.
///
/// Normalizes spacing: adds a space after `//` if missing, removes extra space
/// after doc markers.
///
/// Returns (`CommentKind`, `normalized_content`).
fn classify_and_normalize_comment(content: &str) -> (CommentKind, String) {
    // Trim leading whitespace to check for markers
    let trimmed = content.trim_start();

    // Check for doc comment markers
    if let Some(rest) = trimmed.strip_prefix('#') {
        // Description: `// #Text` -> ` #Text`
        let text = rest.trim_start();
        return (CommentKind::DocDescription, format!(" #{text}"));
    }

    if let Some(rest) = trimmed.strip_prefix("@param") {
        // Parameter: `// @param name desc` -> ` @param name desc`
        // Keep the space or lack thereof after @param
        let text = if rest.starts_with(char::is_whitespace) {
            rest.trim_start()
        } else {
            rest
        };
        return (CommentKind::DocParam, format!(" @param {text}"));
    }

    if let Some(rest) = trimmed.strip_prefix("@field") {
        // Field: `// @field name desc` -> ` @field name desc`
        let text = if rest.starts_with(char::is_whitespace) {
            rest.trim_start()
        } else {
            rest
        };
        return (CommentKind::DocField, format!(" @field {text}"));
    }

    if let Some(rest) = trimmed.strip_prefix('!') {
        // Warning: `// !Text` -> ` !Text`
        let text = rest.trim_start();
        return (CommentKind::DocWarning, format!(" !{text}"));
    }

    if let Some(rest) = trimmed.strip_prefix('>') {
        // Example: `// >example()` -> ` >example()`
        // Don't trim after > to preserve example formatting
        return (CommentKind::DocExample, format!(" >{rest}"));
    }

    // Regular comment - ensure space after //
    if content.is_empty() {
        // Empty comment: just "//"
        (CommentKind::Regular, String::new())
    } else if content.starts_with(' ') {
        // Already has space: preserve as-is
        (CommentKind::Regular, content.to_string())
    } else {
        // Missing space: add one
        (CommentKind::Regular, format!(" {content}"))
    }
}

/// Convert a raw token to a `TokenKind`, interning strings.
fn convert_token(raw: RawToken, slice: &str, interner: &StringInterner) -> TokenKind {
    match raw {
        // Literals
        RawToken::Int(n) | RawToken::HexInt(n) | RawToken::BinInt(n) => TokenKind::Int(n),
        RawToken::Float(f) => TokenKind::Float(f.to_bits()),
        RawToken::String => {
            let content = &slice[1..slice.len() - 1];
            let unescaped = unescape_string(content);
            // Use intern_owned to avoid double allocation
            TokenKind::String(interner.intern_owned(unescaped))
        }
        RawToken::Char => {
            let content = &slice[1..slice.len() - 1];
            let c = unescape_char(content);
            TokenKind::Char(c)
        }
        RawToken::Ident => TokenKind::Ident(interner.intern(slice)),

        // Duration
        RawToken::DurationMs((v, u))
        | RawToken::DurationS((v, u))
        | RawToken::DurationM((v, u))
        | RawToken::DurationH((v, u)) => TokenKind::Duration(v, u),

        // Size
        RawToken::SizeB((v, u))
        | RawToken::SizeKb((v, u))
        | RawToken::SizeMb((v, u))
        | RawToken::SizeGb((v, u)) => TokenKind::Size(v, u),

        // Keywords
        RawToken::Async => TokenKind::Async,
        RawToken::Break => TokenKind::Break,
        RawToken::Continue => TokenKind::Continue,
        RawToken::Do => TokenKind::Do,
        RawToken::Else => TokenKind::Else,
        RawToken::False => TokenKind::False,
        RawToken::For => TokenKind::For,
        RawToken::If => TokenKind::If,
        RawToken::Impl => TokenKind::Impl,
        RawToken::In => TokenKind::In,
        RawToken::Let => TokenKind::Let,
        RawToken::Loop => TokenKind::Loop,
        RawToken::Match => TokenKind::Match,
        RawToken::Mut => TokenKind::Mut,
        RawToken::Pub => TokenKind::Pub,
        RawToken::SelfLower => TokenKind::SelfLower,
        RawToken::SelfUpper => TokenKind::SelfUpper,
        RawToken::Then => TokenKind::Then,
        RawToken::Trait => TokenKind::Trait,
        RawToken::True => TokenKind::True,
        RawToken::Type => TokenKind::Type,
        RawToken::Use => TokenKind::Use,
        RawToken::Uses => TokenKind::Uses,
        RawToken::Void => TokenKind::Void,
        RawToken::Where => TokenKind::Where,
        RawToken::With => TokenKind::With,
        RawToken::Yield => TokenKind::Yield,
        RawToken::Tests => TokenKind::Tests,
        RawToken::As => TokenKind::As,
        RawToken::Dyn => TokenKind::Dyn,
        RawToken::Extend => TokenKind::Extend,
        RawToken::Extension => TokenKind::Extension,
        RawToken::Skip => TokenKind::Skip,

        // Type keywords
        RawToken::IntType => TokenKind::IntType,
        RawToken::FloatType => TokenKind::FloatType,
        RawToken::BoolType => TokenKind::BoolType,
        RawToken::StrType => TokenKind::StrType,
        RawToken::CharType => TokenKind::CharType,
        RawToken::ByteType => TokenKind::ByteType,
        RawToken::NeverType => TokenKind::NeverType,

        // Constructors
        RawToken::Ok => TokenKind::Ok,
        RawToken::Err => TokenKind::Err,
        RawToken::Some => TokenKind::Some,
        RawToken::None => TokenKind::None,

        // Pattern keywords (compiler-supported only)
        RawToken::Cache => TokenKind::Cache,
        RawToken::Catch => TokenKind::Catch,
        RawToken::Parallel => TokenKind::Parallel,
        RawToken::Spawn => TokenKind::Spawn,
        RawToken::Recurse => TokenKind::Recurse,
        RawToken::Run => TokenKind::Run,
        RawToken::Timeout => TokenKind::Timeout,
        RawToken::Try => TokenKind::Try,

        // Built-in I/O primitives
        RawToken::Print => TokenKind::Print,
        RawToken::Panic => TokenKind::Panic,

        // Symbols
        RawToken::HashBracket => TokenKind::HashBracket,
        RawToken::At => TokenKind::At,
        RawToken::Dollar => TokenKind::Dollar,
        RawToken::Hash => TokenKind::Hash,
        RawToken::LParen => TokenKind::LParen,
        RawToken::RParen => TokenKind::RParen,
        RawToken::LBrace => TokenKind::LBrace,
        RawToken::RBrace => TokenKind::RBrace,
        RawToken::LBracket => TokenKind::LBracket,
        RawToken::RBracket => TokenKind::RBracket,
        RawToken::Colon => TokenKind::Colon,
        RawToken::DoubleColon => TokenKind::DoubleColon,
        RawToken::Comma => TokenKind::Comma,
        RawToken::Dot => TokenKind::Dot,
        RawToken::DotDot => TokenKind::DotDot,
        RawToken::DotDotEq => TokenKind::DotDotEq,
        RawToken::Arrow => TokenKind::Arrow,
        RawToken::FatArrow => TokenKind::FatArrow,
        RawToken::Pipe => TokenKind::Pipe,
        RawToken::Question => TokenKind::Question,
        RawToken::DoubleQuestion => TokenKind::DoubleQuestion,
        RawToken::Underscore => TokenKind::Underscore,
        RawToken::Semicolon => TokenKind::Semicolon,

        // Operators
        RawToken::Eq => TokenKind::Eq,
        RawToken::EqEq => TokenKind::EqEq,
        RawToken::NotEq => TokenKind::NotEq,
        RawToken::Lt => TokenKind::Lt,
        RawToken::LtEq => TokenKind::LtEq,
        RawToken::Shl => TokenKind::Shl,
        RawToken::Gt => TokenKind::Gt,
        // Note: GtEq (>=) and Shr (>>) are synthesized by the parser from
        // adjacent Gt tokens. The lexer only produces individual Gt tokens.
        RawToken::Plus => TokenKind::Plus,
        RawToken::Minus => TokenKind::Minus,
        RawToken::Star => TokenKind::Star,
        RawToken::Slash => TokenKind::Slash,
        RawToken::Percent => TokenKind::Percent,
        RawToken::Bang => TokenKind::Bang,
        RawToken::Tilde => TokenKind::Tilde,
        RawToken::Amp => TokenKind::Amp,
        RawToken::AmpAmp => TokenKind::AmpAmp,
        RawToken::PipePipe => TokenKind::PipePipe,
        RawToken::Caret => TokenKind::Caret,
        RawToken::Div => TokenKind::Div,

        // Trivia (shouldn't reach here)
        RawToken::LineComment | RawToken::Newline | RawToken::LineContinuation => {
            unreachable!("Trivia should be handled separately")
        }
    }
}

/// Resolve a single escape character to its replacement.
///
/// Returns `Some(char)` for recognized escapes, `None` for unrecognized ones.
/// Recognized escapes: `\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`
#[inline]
fn resolve_escape(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '0' => Some('\0'),
        _ => None,
    }
}

/// Process string escape sequences.
///
/// Uses `char_indices()` directly to avoid `Peekable` iterator overhead.
/// Invalid escapes are preserved literally (e.g., `\q` becomes `\q`).
#[inline]
fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.char_indices();

    while let Some((_, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some((_, esc)) => {
                    if let Some(resolved) = resolve_escape(esc) {
                        result.push(resolved);
                    } else {
                        result.push('\\');
                        result.push(esc);
                    }
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Process char escape sequences.
///
/// Returns the unescaped character. Invalid escapes return the escaped character
/// (e.g., `\q` returns `q`). Empty input returns `\0`.
#[inline]
fn unescape_char(s: &str) -> char {
    let mut chars = s.chars();
    match chars.next() {
        Some('\\') => match chars.next() {
            Some(esc) => resolve_escape(esc).unwrap_or(esc),
            None => '\\',
        },
        Some(c) => c,
        None => '\0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // === Escape sequence tests ===

    #[test]
    fn test_resolve_escape_valid() {
        assert_eq!(resolve_escape('n'), Some('\n'));
        assert_eq!(resolve_escape('r'), Some('\r'));
        assert_eq!(resolve_escape('t'), Some('\t'));
        assert_eq!(resolve_escape('\\'), Some('\\'));
        assert_eq!(resolve_escape('"'), Some('"'));
        assert_eq!(resolve_escape('\''), Some('\''));
        assert_eq!(resolve_escape('0'), Some('\0'));
    }

    #[test]
    fn test_resolve_escape_invalid() {
        assert_eq!(resolve_escape('q'), None);
        assert_eq!(resolve_escape('x'), None);
        assert_eq!(resolve_escape('a'), None);
        assert_eq!(resolve_escape(' '), None);
    }

    #[test]
    fn test_unescape_string_no_escapes() {
        assert_eq!(unescape_string("hello world"), "hello world");
        assert_eq!(unescape_string(""), "");
        assert_eq!(unescape_string("abc123"), "abc123");
    }

    #[test]
    fn test_unescape_string_valid_escapes() {
        assert_eq!(unescape_string(r"hello\nworld"), "hello\nworld");
        assert_eq!(unescape_string(r"tab\there"), "tab\there");
        assert_eq!(unescape_string(r#"quote\"test"#), "quote\"test");
        assert_eq!(unescape_string(r"back\\slash"), "back\\slash");
        assert_eq!(unescape_string(r"null\0char"), "null\0char");
        assert_eq!(unescape_string(r"\n\r\t"), "\n\r\t");
    }

    #[test]
    fn test_unescape_string_invalid_escapes() {
        // Invalid escapes are preserved literally
        assert_eq!(unescape_string(r"\q"), "\\q");
        assert_eq!(unescape_string(r"\x"), "\\x");
        assert_eq!(unescape_string(r"test\qvalue"), "test\\qvalue");
    }

    #[test]
    fn test_unescape_string_trailing_backslash() {
        assert_eq!(unescape_string(r"test\"), "test\\");
    }

    #[test]
    fn test_unescape_char_simple() {
        assert_eq!(unescape_char("a"), 'a');
        assert_eq!(unescape_char("λ"), 'λ');
        assert_eq!(unescape_char("0"), '0');
    }

    #[test]
    fn test_unescape_char_escapes() {
        assert_eq!(unescape_char(r"\n"), '\n');
        assert_eq!(unescape_char(r"\t"), '\t');
        assert_eq!(unescape_char(r"\\"), '\\');
        assert_eq!(unescape_char(r"\'"), '\'');
    }

    #[test]
    fn test_unescape_char_invalid_escape() {
        // Invalid escape returns the escaped character
        assert_eq!(unescape_char(r"\q"), 'q');
    }

    #[test]
    fn test_unescape_char_empty() {
        assert_eq!(unescape_char(""), '\0');
    }

    #[test]
    fn test_unescape_char_lone_backslash() {
        assert_eq!(unescape_char("\\"), '\\');
    }

    // === Numeric parsing tests ===

    #[test]
    fn test_parse_int_skip_underscores() {
        assert_eq!(parse_int_skip_underscores("123", 10), Some(123));
        assert_eq!(parse_int_skip_underscores("1_000_000", 10), Some(1_000_000));
        assert_eq!(parse_int_skip_underscores("1_2_3", 10), Some(123));
        assert_eq!(parse_int_skip_underscores("___1___", 10), Some(1));
    }

    #[test]
    fn test_parse_int_hex_with_underscores() {
        assert_eq!(parse_int_skip_underscores("FF", 16), Some(255));
        assert_eq!(parse_int_skip_underscores("F_F", 16), Some(255));
        assert_eq!(
            parse_int_skip_underscores("dead_beef", 16),
            Some(0xdead_beef)
        );
    }

    #[test]
    fn test_parse_int_binary_with_underscores() {
        assert_eq!(parse_int_skip_underscores("1010", 2), Some(10));
        assert_eq!(parse_int_skip_underscores("1_0_1_0", 2), Some(10));
        assert_eq!(parse_int_skip_underscores("1111_0000", 2), Some(240));
    }

    #[test]
    fn test_parse_int_overflow() {
        // Should return None on overflow
        assert_eq!(
            parse_int_skip_underscores("99999999999999999999999", 10),
            None
        );
    }

    #[test]
    #[allow(clippy::approx_constant)] // Testing float parsing, not using mathematical constants
    fn test_parse_float_skip_underscores() {
        assert_eq!(parse_float_skip_underscores("3.14"), Some(3.14));
        assert_eq!(parse_float_skip_underscores("1_000.5"), Some(1000.5));
        assert_eq!(parse_float_skip_underscores("1.5e10"), Some(1.5e10));
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

    // === Size literal tests ===

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

    // === Duration literal tests ===

    #[test]
    fn test_lex_duration_minutes() {
        let interner = test_interner();
        let tokens = lex("30m", &interner);

        assert!(matches!(
            tokens[0].kind,
            TokenKind::Duration(30, DurationUnit::Minutes)
        ));
    }

    // === Edge case tests ===

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

    // === Keyword tests ===

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

    // === Char literal tests ===

    #[test]
    fn test_lex_char_literals() {
        let interner = test_interner();
        let tokens = lex(r"'a' '\n' '\\' '\''", &interner);

        assert!(matches!(tokens[0].kind, TokenKind::Char('a')));
        assert!(matches!(tokens[1].kind, TokenKind::Char('\n')));
        assert!(matches!(tokens[2].kind, TokenKind::Char('\\')));
        assert!(matches!(tokens[3].kind, TokenKind::Char('\'')));
    }

    // === Float literal tests ===

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

    // === Comments and line continuations ===

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

    // === Comment capture tests (lex_with_comments) ===

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

    // === classify_and_normalize_comment tests ===

    #[test]
    fn test_classify_regular_comment() {
        let (kind, content) = classify_and_normalize_comment(" regular text");
        assert_eq!(kind, CommentKind::Regular);
        assert_eq!(content, " regular text");
    }

    #[test]
    fn test_classify_doc_description() {
        let (kind, content) = classify_and_normalize_comment(" #Description");
        assert_eq!(kind, CommentKind::DocDescription);
        assert_eq!(content, " #Description");

        // With extra spaces
        let (kind, content) = classify_and_normalize_comment("  #Description");
        assert_eq!(kind, CommentKind::DocDescription);
        assert_eq!(content, " #Description");
    }

    #[test]
    fn test_classify_doc_param() {
        let (kind, content) = classify_and_normalize_comment(" @param x value");
        assert_eq!(kind, CommentKind::DocParam);
        assert_eq!(content, " @param x value");
    }

    #[test]
    fn test_classify_doc_field() {
        let (kind, content) = classify_and_normalize_comment(" @field x coord");
        assert_eq!(kind, CommentKind::DocField);
        assert_eq!(content, " @field x coord");
    }

    #[test]
    fn test_classify_doc_warning() {
        let (kind, content) = classify_and_normalize_comment(" !Panics");
        assert_eq!(kind, CommentKind::DocWarning);
        assert_eq!(content, " !Panics");
    }

    #[test]
    fn test_classify_doc_example() {
        let (kind, content) = classify_and_normalize_comment(" >foo() -> 1");
        assert_eq!(kind, CommentKind::DocExample);
        // Preserve spacing after > exactly
        assert_eq!(content, " >foo() -> 1");
    }

    #[test]
    fn test_classify_empty_comment() {
        let (kind, content) = classify_and_normalize_comment("");
        assert_eq!(kind, CommentKind::Regular);
        assert_eq!(content, "");
    }

    #[test]
    fn test_classify_no_space_adds_space() {
        let (kind, content) = classify_and_normalize_comment("no space");
        assert_eq!(kind, CommentKind::Regular);
        assert_eq!(content, " no space");
    }
}
