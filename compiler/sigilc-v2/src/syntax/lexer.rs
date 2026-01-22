//! Lexer for Sigil using logos with string interning.
//!
//! This lexer:
//! - Interns all identifiers and string literals
//! - Preserves trivia (whitespace, comments) for formatting
//! - Handles line continuation with `_` at end of line

use logos::Logos;
use crate::intern::StringInterner;
use super::{
    Span, Token, TokenKind, Trivia, TriviaKind,
    token::{DurationUnit, SizeUnit},
};

/// Raw token from logos (before interning).
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r]+")] // Skip horizontal whitespace
enum RawToken {
    // === Comments ===
    #[regex(r"//[^\n]*")]
    LineComment,

    // === Newlines ===
    #[token("\n")]
    Newline,

    // === Line continuation ===
    // Note: Uses \\ instead of _ for line continuation to avoid conflict with wildcard
    #[regex(r"\\[ \t]*\n")]
    LineContinuation,

    // === Keywords ===
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
    #[token("assert")]
    Assert,
    #[token("dyn")]
    Dyn,
    #[token("extend")]
    Extend,
    #[token("extension")]
    Extension,

    // === Type keywords ===
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

    // === Constructors ===
    #[token("Ok")]
    Ok,
    #[token("Err")]
    Err,
    #[token("Some")]
    Some,
    #[token("None")]
    None,

    // === Pattern keywords ===
    #[token("cache")]
    Cache,
    #[token("collect")]
    Collect,
    #[token("filter")]
    Filter,
    #[token("find")]
    Find,
    #[token("fold")]
    Fold,
    #[token("map")]
    Map,
    #[token("parallel")]
    Parallel,
    #[token("recurse")]
    Recurse,
    #[token("retry")]
    Retry,
    #[token("run")]
    Run,
    #[token("timeout")]
    Timeout,
    #[token("try")]
    Try,
    #[token("validate")]
    Validate,

    // === Attribute keywords ===
    #[token("skip")]
    Skip,

    // === Symbols ===
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
    #[token("|>")]
    PipeArrow,
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

    // === Operators ===
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

    // === Literals ===

    // Hex integer with underscores
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", |lex| {
        let s = lex.slice();
        // Skip "0x" prefix and remove underscores
        i64::from_str_radix(&s[2..].replace('_', ""), 16).ok()
    })]
    HexInt(i64),

    // Binary integer with underscores
    #[regex(r"0b[01][01_]*", |lex| {
        let s = lex.slice();
        // Skip "0b" prefix and remove underscores
        i64::from_str_radix(&s[2..].replace('_', ""), 2).ok()
    })]
    BinInt(i64),

    // Integer with underscores
    #[regex(r"[0-9][0-9_]*", |lex| {
        lex.slice().replace('_', "").parse::<i64>().ok()
    })]
    Int(i64),

    // Float with optional exponent
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

    // String literal (double-quoted)
    #[regex(r#""([^"\\]|\\.)*""#)]
    String,

    // Char literal (single-quoted)
    #[regex(r"'([^'\\]|\\.)'")]
    Char,

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
}

/// Token list with trivia.
#[derive(Clone, Debug, Default)]
pub struct TokenList {
    /// Tokens (without trivia).
    pub tokens: Vec<Token>,
    /// Leading trivia for each token (indexed by token position).
    pub leading_trivia: Vec<Vec<Trivia>>,
    /// Trailing trivia for the last token.
    pub trailing_trivia: Vec<Trivia>,
}

impl TokenList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get number of tokens.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

/// Lexer that produces interned tokens.
pub struct Lexer<'src, 'i> {
    source: &'src str,
    interner: &'i StringInterner,
}

impl<'src, 'i> Lexer<'src, 'i> {
    /// Create a new lexer.
    pub fn new(source: &'src str, interner: &'i StringInterner) -> Self {
        Lexer { source, interner }
    }

    /// Lex all tokens from the source.
    pub fn lex_all(&self) -> TokenList {
        let mut result = TokenList::new();
        let mut logos = RawToken::lexer(self.source);
        let mut current_trivia: Vec<Trivia> = Vec::new();

        while let Some(token_result) = logos.next() {
            let span = Span::from_range(logos.span());
            let slice = logos.slice();

            match token_result {
                Ok(raw) => {
                    // Handle trivia vs real tokens
                    match raw {
                        RawToken::LineComment => {
                            let kind = if slice.starts_with("// #") || slice.starts_with("//#") {
                                TriviaKind::DocComment
                            } else {
                                TriviaKind::LineComment
                            };
                            current_trivia.push(Trivia { kind, span });
                        }
                        RawToken::Newline => {
                            current_trivia.push(Trivia {
                                kind: TriviaKind::Newline,
                                span,
                            });
                        }
                        RawToken::LineContinuation => {
                            current_trivia.push(Trivia {
                                kind: TriviaKind::LineContinuation,
                                span,
                            });
                        }
                        _ => {
                            // Real token - convert and add
                            let kind = self.convert_token(raw, slice);
                            result.tokens.push(Token::new(kind, span));
                            result.leading_trivia.push(std::mem::take(&mut current_trivia));
                        }
                    }
                }
                Err(_) => {
                    // Error token
                    result.tokens.push(Token::new(TokenKind::Error, span));
                    result.leading_trivia.push(std::mem::take(&mut current_trivia));
                }
            }
        }

        // Add EOF token
        let eof_span = Span::point(self.source.len() as u32);
        result.tokens.push(Token::new(TokenKind::Eof, eof_span));
        result.leading_trivia.push(std::mem::take(&mut current_trivia));

        // Any remaining trivia is trailing
        result.trailing_trivia = current_trivia;

        result
    }

    /// Convert a raw token to a TokenKind, interning strings.
    fn convert_token(&self, raw: RawToken, slice: &str) -> TokenKind {
        match raw {
            // Literals
            RawToken::Int(n) => TokenKind::Int(n),
            RawToken::HexInt(n) => TokenKind::Int(n),
            RawToken::BinInt(n) => TokenKind::Int(n),
            RawToken::Float(f) => TokenKind::Float(f.to_bits()),
            RawToken::String => {
                // Remove quotes and process escapes
                let content = &slice[1..slice.len()-1];
                let unescaped = unescape_string(content);
                TokenKind::String(self.interner.intern(&unescaped))
            }
            RawToken::Char => {
                let content = &slice[1..slice.len()-1];
                let c = unescape_char(content);
                TokenKind::Char(c)
            }
            RawToken::Ident => {
                TokenKind::Ident(self.interner.intern(slice))
            }

            // Duration
            RawToken::DurationMs((v, u)) |
            RawToken::DurationS((v, u)) |
            RawToken::DurationM((v, u)) |
            RawToken::DurationH((v, u)) => TokenKind::Duration(v, u),

            // Size
            RawToken::SizeB((v, u)) |
            RawToken::SizeKb((v, u)) |
            RawToken::SizeMb((v, u)) |
            RawToken::SizeGb((v, u)) => TokenKind::Size(v, u),

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
            RawToken::Assert => TokenKind::Assert,
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

            // Pattern keywords
            RawToken::Cache => TokenKind::Cache,
            RawToken::Collect => TokenKind::Collect,
            RawToken::Filter => TokenKind::Filter,
            RawToken::Find => TokenKind::Find,
            RawToken::Fold => TokenKind::Fold,
            RawToken::Map => TokenKind::Map,
            RawToken::Parallel => TokenKind::Parallel,
            RawToken::Recurse => TokenKind::Recurse,
            RawToken::Retry => TokenKind::Retry,
            RawToken::Run => TokenKind::Run,
            RawToken::Timeout => TokenKind::Timeout,
            RawToken::Try => TokenKind::Try,
            RawToken::Validate => TokenKind::Validate,

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
            RawToken::PipeArrow => TokenKind::PipeArrow,
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
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('0') => result.push('\0'),
                Some(c) => {
                    // Unknown escape - keep as-is
                    result.push('\\');
                    result.push(c);
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
fn unescape_char(s: &str) -> char {
    let mut chars = s.chars();
    match chars.next() {
        Some('\\') => match chars.next() {
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('\\') => '\\',
            Some('\'') => '\'',
            Some('0') => '\0',
            Some(c) => c,
            None => '\\',
        },
        Some(c) => c,
        None => '\0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_basic() {
        let interner = StringInterner::new();
        let lexer = Lexer::new("let x = 42", &interner);
        let tokens = lexer.lex_all();

        assert_eq!(tokens.tokens.len(), 5); // let, x, =, 42, EOF
        assert!(matches!(tokens.tokens[0].kind, TokenKind::Let));
        assert!(matches!(tokens.tokens[1].kind, TokenKind::Ident(_)));
        assert!(matches!(tokens.tokens[2].kind, TokenKind::Eq));
        assert!(matches!(tokens.tokens[3].kind, TokenKind::Int(42)));
        assert!(matches!(tokens.tokens[4].kind, TokenKind::Eof));
    }

    #[test]
    fn test_lex_string() {
        let interner = StringInterner::new();
        let lexer = Lexer::new(r#""hello\nworld""#, &interner);
        let tokens = lexer.lex_all();

        if let TokenKind::String(name) = tokens.tokens[0].kind {
            assert_eq!(interner.lookup(name), "hello\nworld");
        } else {
            panic!("Expected string token");
        }
    }

    #[test]
    fn test_lex_duration() {
        let interner = StringInterner::new();
        let lexer = Lexer::new("100ms 5s 2h", &interner);
        let tokens = lexer.lex_all();

        assert!(matches!(
            tokens.tokens[0].kind,
            TokenKind::Duration(100, DurationUnit::Milliseconds)
        ));
        assert!(matches!(
            tokens.tokens[1].kind,
            TokenKind::Duration(5, DurationUnit::Seconds)
        ));
        assert!(matches!(
            tokens.tokens[2].kind,
            TokenKind::Duration(2, DurationUnit::Hours)
        ));
    }

    #[test]
    fn test_lex_pattern_keywords() {
        let interner = StringInterner::new();
        let lexer = Lexer::new("map filter fold run try", &interner);
        let tokens = lexer.lex_all();

        assert!(matches!(tokens.tokens[0].kind, TokenKind::Map));
        assert!(matches!(tokens.tokens[1].kind, TokenKind::Filter));
        assert!(matches!(tokens.tokens[2].kind, TokenKind::Fold));
        assert!(matches!(tokens.tokens[3].kind, TokenKind::Run));
        assert!(matches!(tokens.tokens[4].kind, TokenKind::Try));
    }

    #[test]
    fn test_lex_with_comments() {
        let interner = StringInterner::new();
        let lexer = Lexer::new("let x = 42 // comment\nlet y = 10", &interner);
        let tokens = lexer.lex_all();

        // Should have: let, x, =, 42, let, y, =, 10, EOF
        assert_eq!(tokens.tokens.len(), 9);

        // Check that trivia is captured
        // The comment and newline should be leading trivia for the second 'let'
        assert!(!tokens.leading_trivia[4].is_empty());
    }
}

    #[test]
    fn test_lex_underscore() {
        let interner = StringInterner::new();
        let lexer = Lexer::new("_", &interner);
        let tokens = lexer.lex_all();
        
        println!("Token for '_': {:?}", tokens.tokens[0].kind);
        assert!(matches!(tokens.tokens[0].kind, TokenKind::Underscore));
    }

    #[test]
    fn test_lex_underscore_in_context() {
        let interner = StringInterner::new();
        let lexer = Lexer::new("_ -> x", &interner);
        let tokens = lexer.lex_all();
        
        println!("Tokens:");
        for (i, t) in tokens.tokens.iter().enumerate() {
            println!("  {}: {:?} at {:?}", i, t.kind, t.span);
        }
        
        assert!(matches!(tokens.tokens[0].kind, TokenKind::Underscore), 
            "Expected Underscore, got {:?}", tokens.tokens[0].kind);
    }
