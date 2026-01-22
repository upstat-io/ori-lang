//! Token types for the Sigil lexer.

use crate::intern::Name;
use super::Span;
use std::fmt;

/// A token with its span in the source.
#[derive(Clone, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {}", self.kind, self.span)
    }
}

/// Token kinds for Sigil.
#[derive(Clone, Eq, PartialEq)]
pub enum TokenKind {
    // === Literals ===
    /// Integer literal: 42, 1_000
    Int(i64),
    /// Float literal: 3.14, 2.5e-8
    Float(u64), // Store as bits for Eq/Hash
    /// String literal (interned): "hello"
    String(Name),
    /// Char literal: 'a', '\n'
    Char(char),
    /// Duration literal: 100ms, 5s, 2h
    Duration(u64, DurationUnit),
    /// Size literal: 4kb, 10mb
    Size(u64, SizeUnit),

    // === Identifiers ===
    /// Identifier (interned)
    Ident(Name),

    // === Keywords (Reserved) ===
    Async,
    Break,
    Continue,
    Do,
    Else,
    False,
    For,
    If,
    Impl,
    In,
    Let,
    Loop,
    Match,
    Mut,
    Pub,
    SelfLower,  // self
    SelfUpper,  // Self
    Then,
    Trait,
    True,
    Type,
    Use,
    Uses,
    Void,
    Where,
    With,
    Yield,

    // Additional keywords
    Tests,
    Assert,
    Dyn,
    Extend,
    Extension,

    // === Type Keywords ===
    IntType,     // int
    FloatType,   // float
    BoolType,    // bool
    StrType,     // str
    CharType,    // char
    ByteType,    // byte
    NeverType,   // Never

    // === Constructors/Prelude ===
    Ok,
    Err,
    Some,
    None,

    // === Pattern Keywords (Context-Sensitive) ===
    // These can also be identifiers outside pattern context
    Cache,
    Collect,
    Filter,
    Find,
    Fold,
    Map,
    Parallel,
    Recurse,
    Retry,
    Run,
    Timeout,
    Try,
    Validate,

    // === Symbols ===
    At,           // @
    Dollar,       // $
    Hash,         // #
    LParen,       // (
    RParen,       // )
    LBrace,       // {
    RBrace,       // }
    LBracket,     // [
    RBracket,     // ]
    Colon,        // :
    DoubleColon,  // ::
    Comma,        // ,
    Dot,          // .
    DotDot,       // ..
    DotDotEq,     // ..=
    Arrow,        // ->
    FatArrow,     // =>
    Pipe,         // |
    PipeArrow,    // |>
    Question,     // ?
    DoubleQuestion, // ??
    Underscore,   // _
    Semicolon,    // ; (for explicit statement separation if needed)

    // === Operators ===
    Eq,           // =
    EqEq,         // ==
    NotEq,        // !=
    Lt,           // <
    LtEq,         // <=
    Gt,           // >
    GtEq,         // >=
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Percent,      // %
    Bang,         // !
    Tilde,        // ~
    Amp,          // &
    AmpAmp,       // &&
    PipePipe,     // ||
    Caret,        // ^
    Div,          // div (floor division keyword)

    // === Whitespace/Trivia (stored separately) ===
    Newline,
    Eof,

    // === Error ===
    Error,
}

impl TokenKind {
    /// Check if this token can start an expression.
    pub fn can_start_expr(&self) -> bool {
        matches!(
            self,
            TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::String(_) |
            TokenKind::Char(_) | TokenKind::Duration(_, _) | TokenKind::Size(_, _) |
            TokenKind::Ident(_) | TokenKind::True | TokenKind::False |
            TokenKind::If | TokenKind::For | TokenKind::Match | TokenKind::Loop |
            TokenKind::Let | TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace |
            TokenKind::At | TokenKind::Dollar | TokenKind::Minus | TokenKind::Bang |
            TokenKind::Tilde | TokenKind::Ok | TokenKind::Err | TokenKind::Some | TokenKind::None |
            // Pattern keywords
            TokenKind::Run | TokenKind::Try | TokenKind::Map | TokenKind::Filter |
            TokenKind::Fold | TokenKind::Find | TokenKind::Collect | TokenKind::Recurse |
            TokenKind::Parallel | TokenKind::Timeout | TokenKind::Retry |
            TokenKind::Cache | TokenKind::Validate
        )
    }

    /// Check if this is a pattern keyword.
    pub fn is_pattern_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Cache | TokenKind::Collect | TokenKind::Filter |
            TokenKind::Find | TokenKind::Fold | TokenKind::Map |
            TokenKind::Parallel | TokenKind::Recurse | TokenKind::Retry |
            TokenKind::Run | TokenKind::Timeout | TokenKind::Try |
            TokenKind::Validate
        )
    }

    /// Check if this token can be used as an identifier in non-pattern context.
    pub fn as_contextual_ident(&self) -> Option<Name> {
        // In Sigil, pattern keywords can be used as identifiers outside patterns
        // This would need access to the interner to return the Name
        None // Caller should handle pattern keyword -> ident conversion
    }

    /// Get a display name for the token.
    pub fn display_name(&self) -> &'static str {
        match self {
            TokenKind::Int(_) => "integer",
            TokenKind::Float(_) => "float",
            TokenKind::String(_) => "string",
            TokenKind::Char(_) => "char",
            TokenKind::Duration(_, _) => "duration",
            TokenKind::Size(_, _) => "size",
            TokenKind::Ident(_) => "identifier",
            TokenKind::Async => "async",
            TokenKind::Break => "break",
            TokenKind::Continue => "continue",
            TokenKind::Do => "do",
            TokenKind::Else => "else",
            TokenKind::False => "false",
            TokenKind::For => "for",
            TokenKind::If => "if",
            TokenKind::Impl => "impl",
            TokenKind::In => "in",
            TokenKind::Let => "let",
            TokenKind::Loop => "loop",
            TokenKind::Match => "match",
            TokenKind::Mut => "mut",
            TokenKind::Pub => "pub",
            TokenKind::SelfLower => "self",
            TokenKind::SelfUpper => "Self",
            TokenKind::Then => "then",
            TokenKind::Trait => "trait",
            TokenKind::True => "true",
            TokenKind::Type => "type",
            TokenKind::Use => "use",
            TokenKind::Uses => "uses",
            TokenKind::Void => "void",
            TokenKind::Where => "where",
            TokenKind::With => "with",
            TokenKind::Yield => "yield",
            TokenKind::Tests => "tests",
            TokenKind::Assert => "assert",
            TokenKind::Dyn => "dyn",
            TokenKind::Extend => "extend",
            TokenKind::Extension => "extension",
            TokenKind::IntType => "int",
            TokenKind::FloatType => "float",
            TokenKind::BoolType => "bool",
            TokenKind::StrType => "str",
            TokenKind::CharType => "char",
            TokenKind::ByteType => "byte",
            TokenKind::NeverType => "Never",
            TokenKind::Ok => "Ok",
            TokenKind::Err => "Err",
            TokenKind::Some => "Some",
            TokenKind::None => "None",
            TokenKind::Cache => "cache",
            TokenKind::Collect => "collect",
            TokenKind::Filter => "filter",
            TokenKind::Find => "find",
            TokenKind::Fold => "fold",
            TokenKind::Map => "map",
            TokenKind::Parallel => "parallel",
            TokenKind::Recurse => "recurse",
            TokenKind::Retry => "retry",
            TokenKind::Run => "run",
            TokenKind::Timeout => "timeout",
            TokenKind::Try => "try",
            TokenKind::Validate => "validate",
            TokenKind::At => "@",
            TokenKind::Dollar => "$",
            TokenKind::Hash => "#",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Colon => ":",
            TokenKind::DoubleColon => "::",
            TokenKind::Comma => ",",
            TokenKind::Dot => ".",
            TokenKind::DotDot => "..",
            TokenKind::DotDotEq => "..=",
            TokenKind::Arrow => "->",
            TokenKind::FatArrow => "=>",
            TokenKind::Pipe => "|",
            TokenKind::PipeArrow => "|>",
            TokenKind::Question => "?",
            TokenKind::DoubleQuestion => "??",
            TokenKind::Underscore => "_",
            TokenKind::Semicolon => ";",
            TokenKind::Eq => "=",
            TokenKind::EqEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::LtEq => "<=",
            TokenKind::Gt => ">",
            TokenKind::GtEq => ">=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Bang => "!",
            TokenKind::Tilde => "~",
            TokenKind::Amp => "&",
            TokenKind::AmpAmp => "&&",
            TokenKind::PipePipe => "||",
            TokenKind::Caret => "^",
            TokenKind::Div => "div",
            TokenKind::Newline => "newline",
            TokenKind::Eof => "end of file",
            TokenKind::Error => "error",
        }
    }
}

impl fmt::Debug for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(n) => write!(f, "Int({})", n),
            TokenKind::Float(bits) => write!(f, "Float({})", f64::from_bits(*bits)),
            TokenKind::String(name) => write!(f, "String({:?})", name),
            TokenKind::Char(c) => write!(f, "Char({:?})", c),
            TokenKind::Duration(n, unit) => write!(f, "Duration({}{:?})", n, unit),
            TokenKind::Size(n, unit) => write!(f, "Size({}{:?})", n, unit),
            TokenKind::Ident(name) => write!(f, "Ident({:?})", name),
            _ => write!(f, "{}", self.display_name()),
        }
    }
}

/// Duration unit for duration literals.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl DurationUnit {
    pub fn to_millis(self, value: u64) -> u64 {
        match self {
            DurationUnit::Milliseconds => value,
            DurationUnit::Seconds => value * 1000,
            DurationUnit::Minutes => value * 60 * 1000,
            DurationUnit::Hours => value * 60 * 60 * 1000,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            DurationUnit::Milliseconds => "ms",
            DurationUnit::Seconds => "s",
            DurationUnit::Minutes => "m",
            DurationUnit::Hours => "h",
        }
    }
}

impl fmt::Debug for DurationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// Size unit for size literals.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
}

impl SizeUnit {
    pub fn to_bytes(self, value: u64) -> u64 {
        match self {
            SizeUnit::Bytes => value,
            SizeUnit::Kilobytes => value * 1024,
            SizeUnit::Megabytes => value * 1024 * 1024,
            SizeUnit::Gigabytes => value * 1024 * 1024 * 1024,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            SizeUnit::Bytes => "b",
            SizeUnit::Kilobytes => "kb",
            SizeUnit::Megabytes => "mb",
            SizeUnit::Gigabytes => "gb",
        }
    }
}

impl fmt::Debug for SizeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// Trivia (whitespace, comments) between tokens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub span: Span,
}

/// Trivia kinds.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TriviaKind {
    /// Whitespace (spaces, tabs)
    Whitespace,
    /// Line comment: // ...
    LineComment,
    /// Doc comment: // #...
    DocComment,
    /// Newline
    Newline,
    /// Line continuation: _\n
    LineContinuation,
}

impl fmt::Debug for TriviaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriviaKind::Whitespace => write!(f, "Whitespace"),
            TriviaKind::LineComment => write!(f, "LineComment"),
            TriviaKind::DocComment => write!(f, "DocComment"),
            TriviaKind::Newline => write!(f, "Newline"),
            TriviaKind::LineContinuation => write!(f, "LineContinuation"),
        }
    }
}
