//! Lexer for Ori using logos with string interning.
//!
//! Produces `TokenList` for Salsa queries.

use logos::Logos;
use ori_ir::{DurationUnit, SizeUnit, Span, StringInterner, Token, TokenKind, TokenList};

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
    #[token(">>")]
    Shr,
    #[token(">=")]
    GtEq,
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

    // Hex integer
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", |lex| {
        let s = lex.slice();
        u64::from_str_radix(&s[2..].replace('_', ""), 16).ok()
    })]
    HexInt(u64),

    // Binary integer
    #[regex(r"0b[01][01_]*", |lex| {
        let s = lex.slice();
        u64::from_str_radix(&s[2..].replace('_', ""), 2).ok()
    })]
    BinInt(u64),

    // Integer
    #[regex(r"[0-9][0-9_]*", |lex| {
        lex.slice().replace('_', "").parse::<u64>().ok()
    })]
    Int(u64),

    // Float
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?", |lex| {
        lex.slice().replace('_', "").parse::<f64>().ok()
    })]
    Float(f64),

    // Duration literals
    #[regex(r"[0-9]+ms", |lex| {
        let s = lex.slice();
        s[..s.len()-2].parse::<u64>().ok().map(|v| (v, DurationUnit::Milliseconds))
    })]
    DurationMs((u64, DurationUnit)),

    #[regex(r"[0-9]+s", |lex| {
        let s = lex.slice();
        s[..s.len()-1].parse::<u64>().ok().map(|v| (v, DurationUnit::Seconds))
    })]
    DurationS((u64, DurationUnit)),

    #[regex(r"[0-9]+m", |lex| {
        let s = lex.slice();
        s[..s.len()-1].parse::<u64>().ok().map(|v| (v, DurationUnit::Minutes))
    })]
    DurationM((u64, DurationUnit)),

    #[regex(r"[0-9]+h", |lex| {
        let s = lex.slice();
        s[..s.len()-1].parse::<u64>().ok().map(|v| (v, DurationUnit::Hours))
    })]
    DurationH((u64, DurationUnit)),

    // Size literals
    #[regex(r"[0-9]+b", |lex| {
        let s = lex.slice();
        s[..s.len()-1].parse::<u64>().ok().map(|v| (v, SizeUnit::Bytes))
    })]
    SizeB((u64, SizeUnit)),

    #[regex(r"[0-9]+kb", |lex| {
        let s = lex.slice();
        s[..s.len()-2].parse::<u64>().ok().map(|v| (v, SizeUnit::Kilobytes))
    })]
    SizeKb((u64, SizeUnit)),

    #[regex(r"[0-9]+mb", |lex| {
        let s = lex.slice();
        s[..s.len()-2].parse::<u64>().ok().map(|v| (v, SizeUnit::Megabytes))
    })]
    SizeMb((u64, SizeUnit)),

    #[regex(r"[0-9]+gb", |lex| {
        let s = lex.slice();
        s[..s.len()-2].parse::<u64>().ok().map(|v| (v, SizeUnit::Gigabytes))
    })]
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

/// Lex source code into a `TokenList`.
///
/// This is the core lexing function used by the `tokens` query.
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
    let eof_pos = u32::try_from(source.len())
        .unwrap_or_else(|_| panic!("source file exceeds {} bytes", u32::MAX));
    let eof_span = Span::point(eof_pos);
    result.push(Token::new(TokenKind::Eof, eof_span));

    result
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
            TokenKind::String(interner.intern(&unescaped))
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
        RawToken::GtEq => TokenKind::GtEq,
        RawToken::Shr => TokenKind::Shr,
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

/// Process string escape sequences.
fn unescape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') | None => result.push('\\'),
                Some('"') => result.push('"'),
                Some('0') => result.push('\0'),
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Process char escape sequences.
fn unescape_char(s: &str) -> char {
    let mut chars = s.chars();
    match chars.next() {
        Some('\\') => match chars.next() {
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('\\') | None => '\\',
            Some('\'') => '\'',
            Some('0') => '\0',
            Some(c) => c,
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
}
