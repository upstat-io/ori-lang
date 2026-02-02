//! Token types for the Ori lexer.
//!
//! Provides token representation with all Salsa-required traits (Clone, Eq, Hash, Debug).

use super::{Name, Span};
use std::fmt;
use std::hash::Hash;

/// A token with its span in the source.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    #[inline]
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    /// Create a dummy token for testing/generated code.
    pub fn dummy(kind: TokenKind) -> Self {
        Token {
            kind,
            span: Span::DUMMY,
        }
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {}", self.kind, self.span)
    }
}

/// Token kinds for Ori.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
///
/// Float literals store bits as u64 for Hash compatibility.
/// String/Ident use interned Name for Hash compatibility.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum TokenKind {
    /// Integer literal: 42, `1_000` (stored as u64; negation folded in parser)
    Int(u64),
    /// Float literal: 3.14, 2.5e-8 (stored as bits for Eq/Hash)
    Float(u64),
    /// String literal (interned): "hello"
    String(Name),
    /// Char literal: 'a', '\n'
    Char(char),
    /// Duration literal: 100ms, 5s, 2h
    Duration(u64, DurationUnit),
    /// Size literal: 4kb, 10mb
    Size(u64, SizeUnit),

    /// Identifier (interned)
    Ident(Name),

    Async,
    Break,
    Continue,
    Return,
    Def,
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
    SelfLower, // self
    SelfUpper, // Self
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
    As,
    Dyn,
    Extend,
    Extension,
    Skip,

    IntType,   // int
    FloatType, // float
    BoolType,  // bool
    StrType,   // str
    CharType,  // char
    ByteType,  // byte
    NeverType, // Never

    Ok,
    Err,
    Some,
    None,

    Cache,
    Catch,
    Parallel,
    Spawn,
    Recurse,
    Run,
    Timeout,
    Try,
    By, // Context-sensitive: range step (0..10 by 2)

    Print,
    Panic,
    Todo,
    Unreachable,

    HashBracket,    // #[
    At,             // @
    Dollar,         // $
    Hash,           // #
    LParen,         // (
    RParen,         // )
    LBrace,         // {
    RBrace,         // }
    LBracket,       // [
    RBracket,       // ]
    Colon,          // :
    DoubleColon,    // ::
    Comma,          // ,
    Dot,            // .
    DotDot,         // ..
    DotDotEq,       // ..=
    Arrow,          // ->
    FatArrow,       // =>
    Pipe,           // |
    Question,       // ?
    DoubleQuestion, // ??
    Underscore,     // _
    Semicolon,      // ;

    Eq,       // =
    EqEq,     // ==
    NotEq,    // !=
    Lt,       // <
    LtEq,     // <=
    Shl,      // <<
    Gt,       // >
    GtEq,     // >=
    Shr,      // >>
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %
    Bang,     // !
    Tilde,    // ~
    Amp,      // &
    AmpAmp,   // &&
    PipePipe, // ||
    Caret,    // ^
    Div,      // div (floor division keyword)

    Newline,
    Eof,

    /// Generic error token for unrecognized input.
    Error,

    /// Float with duration suffix error (e.g., 1.5s, 2.5ms).
    /// Per spec: "Duration: no float prefix (`1500ms` not `1.5s`)"
    FloatDurationError,

    /// Float with size suffix error (e.g., 1.5kb, 2.5mb).
    /// Per spec: "Size: no float prefix"
    FloatSizeError,
}

impl TokenKind {
    /// Check if this token can start an expression.
    pub fn can_start_expr(&self) -> bool {
        matches!(
            self,
            TokenKind::Int(_)
                | TokenKind::Float(_)
                | TokenKind::String(_)
                | TokenKind::Char(_)
                | TokenKind::Duration(_, _)
                | TokenKind::Size(_, _)
                | TokenKind::Ident(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::If
                | TokenKind::For
                | TokenKind::Match
                | TokenKind::Loop
                | TokenKind::Let
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::At
                | TokenKind::Dollar
                | TokenKind::Minus
                | TokenKind::Bang
                | TokenKind::Tilde
                | TokenKind::Ok
                | TokenKind::Err
                | TokenKind::Some
                | TokenKind::None
                | TokenKind::Run
                | TokenKind::Try
                | TokenKind::Recurse
                | TokenKind::Parallel
                | TokenKind::Spawn
                | TokenKind::Timeout
                | TokenKind::Cache
                | TokenKind::Catch
                | TokenKind::With
                | TokenKind::Print
                | TokenKind::Panic
                | TokenKind::Todo
                | TokenKind::Unreachable
        )
    }

    /// Check if this is a pattern keyword.
    pub fn is_pattern_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Cache
                | TokenKind::Catch
                | TokenKind::Parallel
                | TokenKind::Spawn
                | TokenKind::Recurse
                | TokenKind::Run
                | TokenKind::Timeout
                | TokenKind::Try
                | TokenKind::With
                | TokenKind::Print
                | TokenKind::Panic
                | TokenKind::Todo
                | TokenKind::Unreachable
        )
    }

    /// Get a display name for the token.
    ///
    /// # Performance
    ///
    /// This uses a match statement rather than a lookup table because:
    /// 1. Some variants carry data (e.g., `Int(i64)`) and must be grouped
    /// 2. The Rust compiler optimizes exhaustive matches into efficient jump tables
    /// 3. All display names are static strings, so no allocation occurs
    ///
    /// The generated assembly is comparable to a direct array lookup.
    #[inline]
    pub fn display_name(&self) -> &'static str {
        match self {
            TokenKind::Int(_) => "integer",
            TokenKind::Float(_) | TokenKind::FloatType => "float",
            TokenKind::String(_) => "string",
            TokenKind::Char(_) | TokenKind::CharType => "char",
            TokenKind::Duration(_, _) => "duration",
            TokenKind::Size(_, _) => "size",
            TokenKind::Ident(_) => "identifier",
            TokenKind::Async => "async",
            TokenKind::Break => "break",
            TokenKind::Continue => "continue",
            TokenKind::Return => "return",
            TokenKind::Def => "def",
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
            TokenKind::As => "as",
            TokenKind::Dyn => "dyn",
            TokenKind::Extend => "extend",
            TokenKind::Extension => "extension",
            TokenKind::Skip => "skip",
            TokenKind::IntType => "int",
            TokenKind::BoolType => "bool",
            TokenKind::StrType => "str",
            TokenKind::ByteType => "byte",
            TokenKind::NeverType => "Never",
            TokenKind::Ok => "Ok",
            TokenKind::Err => "Err",
            TokenKind::Some => "Some",
            TokenKind::None => "None",
            TokenKind::Cache => "cache",
            TokenKind::Catch => "catch",
            TokenKind::Parallel => "parallel",
            TokenKind::Spawn => "spawn",
            TokenKind::Recurse => "recurse",
            TokenKind::Run => "run",
            TokenKind::Timeout => "timeout",
            TokenKind::Try => "try",
            TokenKind::By => "by",
            TokenKind::Print => "print",
            TokenKind::Panic => "panic",
            TokenKind::Todo => "todo",
            TokenKind::Unreachable => "unreachable",
            TokenKind::HashBracket => "#[",
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
            TokenKind::Question => "?",
            TokenKind::DoubleQuestion => "??",
            TokenKind::Underscore => "_",
            TokenKind::Semicolon => ";",
            TokenKind::Eq => "=",
            TokenKind::EqEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::LtEq => "<=",
            TokenKind::Shl => "<<",
            TokenKind::Gt => ">",
            TokenKind::GtEq => ">=",
            TokenKind::Shr => ">>",
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
            TokenKind::FloatDurationError => "invalid float duration literal",
            TokenKind::FloatSizeError => "invalid float size literal",
        }
    }
}

impl fmt::Debug for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(n) => write!(f, "Int({n})"),
            TokenKind::Float(bits) => write!(f, "Float({})", f64::from_bits(*bits)),
            TokenKind::String(name) => write!(f, "String({name:?})"),
            TokenKind::Char(c) => write!(f, "Char({c:?})"),
            TokenKind::Duration(n, unit) => write!(f, "Duration({n}{unit:?})"),
            TokenKind::Size(n, unit) => write!(f, "Size({n}{unit:?})"),
            TokenKind::Ident(name) => write!(f, "Ident({name:?})"),
            _ => write!(f, "{}", self.display_name()),
        }
    }
}

/// Duration unit for duration literals.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum DurationUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl DurationUnit {
    /// Convert value to nanoseconds.
    #[inline]
    pub fn to_nanos(self, value: u64) -> i64 {
        let ns = match self {
            DurationUnit::Nanoseconds => value,
            DurationUnit::Microseconds => value * 1_000,
            DurationUnit::Milliseconds => value * 1_000_000,
            DurationUnit::Seconds => value * 1_000_000_000,
            DurationUnit::Minutes => value * 60 * 1_000_000_000,
            DurationUnit::Hours => value * 60 * 60 * 1_000_000_000,
        };
        // Intentional wrap: literal values from lexer won't exceed i64::MAX
        ns.cast_signed()
    }

    /// Get the suffix string.
    #[inline]
    pub fn suffix(self) -> &'static str {
        match self {
            DurationUnit::Nanoseconds => "ns",
            DurationUnit::Microseconds => "us",
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
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl SizeUnit {
    /// Convert value to bytes.
    #[inline]
    pub fn to_bytes(self, value: u64) -> u64 {
        match self {
            SizeUnit::Bytes => value,
            SizeUnit::Kilobytes => value * 1024,
            SizeUnit::Megabytes => value * 1024 * 1024,
            SizeUnit::Gigabytes => value * 1024 * 1024 * 1024,
            SizeUnit::Terabytes => value * 1024 * 1024 * 1024 * 1024,
        }
    }

    /// Get the suffix string.
    #[inline]
    pub fn suffix(self) -> &'static str {
        match self {
            SizeUnit::Bytes => "b",
            SizeUnit::Kilobytes => "kb",
            SizeUnit::Megabytes => "mb",
            SizeUnit::Gigabytes => "gb",
            SizeUnit::Terabytes => "tb",
        }
    }
}

impl fmt::Debug for SizeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// A list of tokens with Salsa-compatible traits.
///
/// Wraps `Vec<Token>` with Clone, Eq, Hash support.
/// Uses the tokens' own Hash impl for content hashing.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug, Default
#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct TokenList {
    tokens: Vec<Token>,
}

impl TokenList {
    /// Create a new empty token list.
    #[inline]
    pub fn new() -> Self {
        TokenList { tokens: Vec::new() }
    }

    /// Create from a Vec of tokens.
    #[inline]
    pub fn from_vec(tokens: Vec<Token>) -> Self {
        TokenList { tokens }
    }

    /// Push a token.
    #[inline]
    pub fn push(&mut self, token: Token) {
        self.tokens.push(token);
    }

    /// Get the number of tokens.
    #[inline]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Check if empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Get token at index.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }

    /// Get a slice of all tokens.
    #[inline]
    pub fn as_slice(&self) -> &[Token] {
        &self.tokens
    }

    /// Iterate over tokens.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Token> {
        self.tokens.iter()
    }

    /// Consume into Vec.
    #[inline]
    pub fn into_vec(self) -> Vec<Token> {
        self.tokens
    }
}

impl fmt::Debug for TokenList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenList({} tokens)", self.tokens.len())
    }
}

impl std::ops::Index<usize> for TokenList {
    type Output = Token;

    fn index(&self, index: usize) -> &Self::Output {
        &self.tokens[index]
    }
}

impl IntoIterator for TokenList {
    type Item = Token;
    type IntoIter = std::vec::IntoIter<Token>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens.into_iter()
    }
}

impl<'a> IntoIterator for &'a TokenList {
    type Item = &'a Token;
    type IntoIter = std::slice::Iter<'a, Token>;

    fn into_iter(self) -> Self::IntoIter {
        self.tokens.iter()
    }
}

// Size assertions to prevent accidental regressions in frequently-allocated types.
// These are compile-time checks that will fail the build if sizes change.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{DurationUnit, SizeUnit, Token, TokenKind};
    // Token is frequently allocated in TokenList, keep it compact.
    // Contains: TokenKind (16 bytes) + Span (8 bytes) = 24 bytes
    crate::static_assert_size!(Token, 24);
    // TokenKind largest variant: Duration(u64, DurationUnit) or Int(u64)
    // 8 bytes payload + 8 bytes discriminant/padding = 16 bytes
    crate::static_assert_size!(TokenKind, 16);
    // Compact unit types
    crate::static_assert_size!(DurationUnit, 1);
    crate::static_assert_size!(SizeUnit, 1);
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_token_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let t1 = Token::new(TokenKind::Int(42), Span::new(0, 2));
        let t2 = Token::new(TokenKind::Int(42), Span::new(0, 2)); // same
        let t3 = Token::new(TokenKind::Int(43), Span::new(0, 2)); // different

        set.insert(t1);
        set.insert(t2);
        set.insert(t3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_token_kind_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(TokenKind::Int(1));
        set.insert(TokenKind::Int(1)); // duplicate
        set.insert(TokenKind::Int(2));
        set.insert(TokenKind::Plus);
        set.insert(TokenKind::Plus); // duplicate

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_token_list_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let list1 = TokenList::from_vec(vec![
            Token::new(TokenKind::Int(1), Span::new(0, 1)),
            Token::new(TokenKind::Plus, Span::new(2, 3)),
        ]);
        let list2 = TokenList::from_vec(vec![
            Token::new(TokenKind::Int(1), Span::new(0, 1)),
            Token::new(TokenKind::Plus, Span::new(2, 3)),
        ]);
        let list3 = TokenList::from_vec(vec![Token::new(TokenKind::Int(2), Span::new(0, 1))]);

        set.insert(list1);
        set.insert(list2); // same as list1
        set.insert(list3);

        assert_eq!(set.len(), 2);
    }

    #[test]
    #[expect(
        clippy::approx_constant,
        reason = "testing float literal parsing, not using PI"
    )]
    fn test_float_bits_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let bits1 = 3.14f64.to_bits();
        let bits2 = 3.14f64.to_bits();
        let bits3 = 2.71f64.to_bits();

        set.insert(TokenKind::Float(bits1));
        set.insert(TokenKind::Float(bits2)); // same
        set.insert(TokenKind::Float(bits3));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_duration_unit() {
        assert_eq!(DurationUnit::Nanoseconds.to_nanos(100), 100);
        assert_eq!(DurationUnit::Microseconds.to_nanos(50), 50_000);
        assert_eq!(DurationUnit::Milliseconds.to_nanos(100), 100_000_000);
        assert_eq!(DurationUnit::Seconds.to_nanos(5), 5_000_000_000);
        assert_eq!(DurationUnit::Minutes.to_nanos(1), 60_000_000_000);
        assert_eq!(DurationUnit::Hours.suffix(), "h");
        assert_eq!(DurationUnit::Nanoseconds.suffix(), "ns");
        assert_eq!(DurationUnit::Microseconds.suffix(), "us");
    }

    #[test]
    fn test_size_unit() {
        assert_eq!(SizeUnit::Kilobytes.to_bytes(4), 4096);
        assert_eq!(SizeUnit::Megabytes.to_bytes(1), 1024 * 1024);
        assert_eq!(SizeUnit::Gigabytes.to_bytes(1), 1024 * 1024 * 1024);
        assert_eq!(SizeUnit::Terabytes.to_bytes(1), 1024 * 1024 * 1024 * 1024);
        assert_eq!(SizeUnit::Bytes.suffix(), "b");
        assert_eq!(SizeUnit::Terabytes.suffix(), "tb");
    }

    #[test]
    fn test_token_list_operations() {
        let mut list = TokenList::new();
        assert!(list.is_empty());

        list.push(Token::new(TokenKind::Int(1), Span::new(0, 1)));
        list.push(Token::new(TokenKind::Plus, Span::new(2, 3)));

        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
        assert_eq!(list[0].kind, TokenKind::Int(1));
        assert_eq!(list.get(1).unwrap().kind, TokenKind::Plus);
    }
}
