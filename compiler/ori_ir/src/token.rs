//! Token types for the Ori lexer.
//!
//! Provides token representation with all Salsa-required traits (Clone, Eq, Hash, Debug).
//!
//! # Specification
//!
//! - Lexical grammar: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § LEXICAL GRAMMAR
//! - Prose: `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md`

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
    DotDotDot,      // ...
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

/// Number of [`TokenKind`] variants. Used for bitset sizing and test verification.
#[cfg(test)]
pub(crate) const TOKEN_KIND_COUNT: usize = 116;

impl TokenKind {
    // ─────────────────────────────────────────────────────────────────────────
    // Discriminant tag constants for O(1) tag-based dispatch.
    //
    // These are the values returned by `discriminant_index()` and stored in
    // `TokenList::tags`. Use these instead of magic numbers in match arms.
    // ─────────────────────────────────────────────────────────────────────────

    // Data-carrying variants
    pub const TAG_INT: u8 = 0;
    pub const TAG_FLOAT: u8 = 1;
    pub const TAG_STRING: u8 = 2;
    pub const TAG_CHAR: u8 = 3;
    pub const TAG_DURATION: u8 = 4;
    pub const TAG_SIZE: u8 = 5;
    pub const TAG_IDENT: u8 = 6;

    // Keywords
    pub const TAG_BREAK: u8 = 8;
    pub const TAG_CONTINUE: u8 = 9;
    pub const TAG_RETURN: u8 = 10;
    pub const TAG_FALSE: u8 = 14;
    pub const TAG_FOR: u8 = 15;
    pub const TAG_IF: u8 = 16;
    pub const TAG_IN: u8 = 18;
    pub const TAG_LET: u8 = 19;
    pub const TAG_LOOP: u8 = 20;
    pub const TAG_MATCH: u8 = 21;
    pub const TAG_SELF_LOWER: u8 = 24;
    pub const TAG_SELF_UPPER: u8 = 25;
    pub const TAG_TRUE: u8 = 28;
    pub const TAG_VOID: u8 = 32;
    pub const TAG_WITH: u8 = 34;
    pub const TAG_TESTS: u8 = 36;
    pub const TAG_AS: u8 = 37;

    // Type keywords
    pub const TAG_INT_TYPE: u8 = 42;
    pub const TAG_FLOAT_TYPE: u8 = 43;
    pub const TAG_BOOL_TYPE: u8 = 44;
    pub const TAG_STR_TYPE: u8 = 45;
    pub const TAG_CHAR_TYPE: u8 = 46;
    pub const TAG_BYTE_TYPE: u8 = 47;
    pub const TAG_NEVER_TYPE: u8 = 48;

    // Result/Option constructors
    pub const TAG_OK: u8 = 49;
    pub const TAG_ERR: u8 = 50;
    pub const TAG_SOME: u8 = 51;
    pub const TAG_NONE: u8 = 52;

    // Pattern keywords
    pub const TAG_CACHE: u8 = 53;
    pub const TAG_CATCH: u8 = 54;
    pub const TAG_PARALLEL: u8 = 55;
    pub const TAG_SPAWN: u8 = 56;
    pub const TAG_RECURSE: u8 = 57;
    pub const TAG_RUN: u8 = 58;
    pub const TAG_TIMEOUT: u8 = 59;
    pub const TAG_TRY: u8 = 60;
    pub const TAG_PRINT: u8 = 62;
    pub const TAG_PANIC: u8 = 63;
    pub const TAG_TODO: u8 = 64;
    pub const TAG_UNREACHABLE: u8 = 65;

    // Punctuation
    pub const TAG_AT: u8 = 67;
    pub const TAG_DOLLAR: u8 = 68;
    pub const TAG_HASH: u8 = 69;
    pub const TAG_LPAREN: u8 = 70;
    pub const TAG_RPAREN: u8 = 71;
    pub const TAG_LBRACE: u8 = 72;
    pub const TAG_RBRACE: u8 = 73;
    pub const TAG_LBRACKET: u8 = 74;
    pub const TAG_RBRACKET: u8 = 75;
    pub const TAG_COLON: u8 = 76;
    pub const TAG_DOT: u8 = 79;
    pub const TAG_DOTDOT: u8 = 80;
    pub const TAG_DOTDOTEQ: u8 = 81;
    pub const TAG_DOTDOTDOT: u8 = 82;
    pub const TAG_ARROW: u8 = 83;
    pub const TAG_PIPE: u8 = 85;
    pub const TAG_QUESTION: u8 = 86;
    pub const TAG_DOUBLE_QUESTION: u8 = 87;

    // Operators
    pub const TAG_EQ: u8 = 90;
    pub const TAG_EQEQ: u8 = 91;
    pub const TAG_NOTEQ: u8 = 92;
    pub const TAG_LT: u8 = 93;
    pub const TAG_LTEQ: u8 = 94;
    pub const TAG_SHL: u8 = 95;
    pub const TAG_GT: u8 = 96;
    pub const TAG_PLUS: u8 = 99;
    pub const TAG_MINUS: u8 = 100;
    pub const TAG_STAR: u8 = 101;
    pub const TAG_SLASH: u8 = 102;
    pub const TAG_PERCENT: u8 = 103;
    pub const TAG_BANG: u8 = 104;
    pub const TAG_TILDE: u8 = 105;
    pub const TAG_AMP: u8 = 106;
    pub const TAG_AMPAMP: u8 = 107;
    pub const TAG_PIPEPIPE: u8 = 108;
    pub const TAG_CARET: u8 = 109;
    pub const TAG_DIV: u8 = 110;

    // Special tokens
    pub const TAG_NEWLINE: u8 = 111;
    pub const TAG_EOF: u8 = 112;
    pub const TAG_FLOAT_DURATION_ERROR: u8 = 114;
    pub const TAG_FLOAT_SIZE_ERROR: u8 = 115;

    /// Get a unique index for this token's discriminant (0-115).
    ///
    /// This is used for O(1) bitset membership testing in `TokenSet`.
    /// The index is stable across calls but may change between compiler versions.
    ///
    /// # Performance
    /// This is a simple match that compiles to a discriminant extraction,
    /// which is typically a single memory load on the tag field.
    #[inline]
    pub const fn discriminant_index(&self) -> u8 {
        match self {
            // Data-carrying variants (indices 0-6)
            Self::Int(_) => 0,
            Self::Float(_) => 1,
            Self::String(_) => 2,
            Self::Char(_) => 3,
            Self::Duration(_, _) => 4,
            Self::Size(_, _) => 5,
            Self::Ident(_) => 6,

            // Keywords (indices 7-42)
            Self::Async => 7,
            Self::Break => 8,
            Self::Continue => 9,
            Self::Return => 10,
            Self::Def => 11,
            Self::Do => 12,
            Self::Else => 13,
            Self::False => 14,
            Self::For => 15,
            Self::If => 16,
            Self::Impl => 17,
            Self::In => 18,
            Self::Let => 19,
            Self::Loop => 20,
            Self::Match => 21,
            Self::Mut => 22,
            Self::Pub => 23,
            Self::SelfLower => 24,
            Self::SelfUpper => 25,
            Self::Then => 26,
            Self::Trait => 27,
            Self::True => 28,
            Self::Type => 29,
            Self::Use => 30,
            Self::Uses => 31,
            Self::Void => 32,
            Self::Where => 33,
            Self::With => 34,
            Self::Yield => 35,

            // Additional keywords (indices 36-41)
            Self::Tests => 36,
            Self::As => 37,
            Self::Dyn => 38,
            Self::Extend => 39,
            Self::Extension => 40,
            Self::Skip => 41,

            // Type keywords (indices 42-48)
            Self::IntType => 42,
            Self::FloatType => 43,
            Self::BoolType => 44,
            Self::StrType => 45,
            Self::CharType => 46,
            Self::ByteType => 47,
            Self::NeverType => 48,

            // Result/Option constructors (indices 49-52)
            Self::Ok => 49,
            Self::Err => 50,
            Self::Some => 51,
            Self::None => 52,

            // Pattern keywords (indices 53-65)
            Self::Cache => 53,
            Self::Catch => 54,
            Self::Parallel => 55,
            Self::Spawn => 56,
            Self::Recurse => 57,
            Self::Run => 58,
            Self::Timeout => 59,
            Self::Try => 60,
            Self::By => 61,
            Self::Print => 62,
            Self::Panic => 63,
            Self::Todo => 64,
            Self::Unreachable => 65,

            // Punctuation (indices 66-88)
            Self::HashBracket => 66,
            Self::At => 67,
            Self::Dollar => 68,
            Self::Hash => 69,
            Self::LParen => 70,
            Self::RParen => 71,
            Self::LBrace => 72,
            Self::RBrace => 73,
            Self::LBracket => 74,
            Self::RBracket => 75,
            Self::Colon => 76,
            Self::DoubleColon => 77,
            Self::Comma => 78,
            Self::Dot => 79,
            Self::DotDot => 80,
            Self::DotDotEq => 81,
            Self::DotDotDot => 82,
            Self::Arrow => 83,
            Self::FatArrow => 84,
            Self::Pipe => 85,
            Self::Question => 86,
            Self::DoubleQuestion => 87,
            Self::Underscore => 88,
            Self::Semicolon => 89,

            // Operators (indices 90-110)
            Self::Eq => 90,
            Self::EqEq => 91,
            Self::NotEq => 92,
            Self::Lt => 93,
            Self::LtEq => 94,
            Self::Shl => 95,
            Self::Gt => 96,
            Self::GtEq => 97,
            Self::Shr => 98,
            Self::Plus => 99,
            Self::Minus => 100,
            Self::Star => 101,
            Self::Slash => 102,
            Self::Percent => 103,
            Self::Bang => 104,
            Self::Tilde => 105,
            Self::Amp => 106,
            Self::AmpAmp => 107,
            Self::PipePipe => 108,
            Self::Caret => 109,
            Self::Div => 110,

            // Special tokens (indices 111-115)
            Self::Newline => 111,
            Self::Eof => 112,
            Self::Error => 113,
            Self::FloatDurationError => 114,
            Self::FloatSizeError => 115,
        }
    }

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
            TokenKind::DotDotDot => "...",
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

    /// Get a friendly name for a discriminant index, suitable for "expected X" messages.
    ///
    /// Returns `None` for tokens that shouldn't appear in expected lists
    /// (e.g., `Error`, `Newline`, `Eof`).
    ///
    /// Used by `TokenSet::format_expected()` for generating error messages like
    /// "expected `,`, `)`, or `}`".
    #[inline]
    pub fn friendly_name_from_index(index: u8) -> Option<&'static str> {
        // Map indices to friendly names, excluding internal/error tokens.
        // Some arms are merged when different tokens share the same display name
        // (e.g., Float literal and FloatType keyword both display as "float").
        match index {
            // Data-carrying variants (some merged with type keywords)
            0 => Some("integer"),    // Int
            1 | 43 => Some("float"), // Float (literal) and FloatType (keyword)
            2 => Some("string"),     // String
            3 | 46 => Some("char"),  // Char (literal) and CharType (keyword)
            4 => Some("duration"),   // Duration
            5 => Some("size"),       // Size
            6 => Some("identifier"), // Ident

            // Keywords (indices 7-41)
            7 => Some("async"),
            8 => Some("break"),
            9 => Some("continue"),
            10 => Some("return"),
            11 => Some("def"),
            12 => Some("do"),
            13 => Some("else"),
            14 => Some("false"),
            15 => Some("for"),
            16 => Some("if"),
            17 => Some("impl"),
            18 => Some("in"),
            19 => Some("let"),
            20 => Some("loop"),
            21 => Some("match"),
            22 => Some("mut"),
            23 => Some("pub"),
            24 => Some("self"),
            25 => Some("Self"),
            26 => Some("then"),
            27 => Some("trait"),
            28 => Some("true"),
            29 => Some("type"),
            30 => Some("use"),
            31 => Some("uses"),
            32 => Some("void"),
            33 => Some("where"),
            34 => Some("with"),
            35 => Some("yield"),
            36 => Some("tests"),
            37 => Some("as"),
            38 => Some("dyn"),
            39 => Some("extend"),
            40 => Some("extension"),
            41 => Some("skip"),

            // Type keywords (indices 42-48, some merged above)
            42 => Some("int"),
            // 43 merged with 1 (float)
            44 => Some("bool"),
            45 => Some("str"),
            // 46 merged with 3 (char)
            47 => Some("byte"),
            48 => Some("Never"),

            // Result/Option constructors (indices 49-52)
            49 => Some("Ok"),
            50 => Some("Err"),
            51 => Some("Some"),
            52 => Some("None"),

            // Pattern keywords (indices 53-65)
            53 => Some("cache"),
            54 => Some("catch"),
            55 => Some("parallel"),
            56 => Some("spawn"),
            57 => Some("recurse"),
            58 => Some("run"),
            59 => Some("timeout"),
            60 => Some("try"),
            61 => Some("by"),
            62 => Some("print"),
            63 => Some("panic"),
            64 => Some("todo"),
            65 => Some("unreachable"),

            // Punctuation (indices 66-89)
            66 => Some("#["),
            67 => Some("@"),
            68 => Some("$"),
            69 => Some("#"),
            70 => Some("("),
            71 => Some(")"),
            72 => Some("{"),
            73 => Some("}"),
            74 => Some("["),
            75 => Some("]"),
            76 => Some(":"),
            77 => Some("::"),
            78 => Some(","),
            79 => Some("."),
            80 => Some(".."),
            81 => Some("..="),
            82 => Some("..."),
            83 => Some("->"),
            84 => Some("=>"),
            85 => Some("|"),
            86 => Some("?"),
            87 => Some("??"),
            88 => Some("_"),
            89 => Some(";"),

            // Operators (indices 90-110)
            90 => Some("="),
            91 => Some("=="),
            92 => Some("!="),
            93 => Some("<"),
            94 => Some("<="),
            95 => Some("<<"),
            96 => Some(">"),
            97 => Some(">="),
            98 => Some(">>"),
            99 => Some("+"),
            100 => Some("-"),
            101 => Some("*"),
            102 => Some("/"),
            103 => Some("%"),
            104 => Some("!"),
            105 => Some("~"),
            106 => Some("&"),
            107 => Some("&&"),
            108 => Some("||"),
            109 => Some("^"),
            110 => Some("div"),

            // Internal tokens and unknown indices - exclude from expected lists
            // Indices 111-115 are Newline, Eof, Error, FloatDurationError, FloatSizeError
            _ => None,
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
    /// Convert value to bytes using SI units (powers of 1000).
    ///
    /// SI units: 1kb = 1000 bytes, 1mb = 1,000,000 bytes, etc.
    /// For exact powers of 1024, use explicit byte counts: `1024b`, `1048576b`.
    #[inline]
    pub fn to_bytes(self, value: u64) -> u64 {
        match self {
            SizeUnit::Bytes => value,
            SizeUnit::Kilobytes => value * 1000,
            SizeUnit::Megabytes => value * 1_000_000,
            SizeUnit::Gigabytes => value * 1_000_000_000,
            SizeUnit::Terabytes => value * 1_000_000_000_000,
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

/// Lazy token capture for AST nodes that may need token access.
///
/// Instead of storing tokens directly (which would be expensive), this stores
/// indices into the cached `TokenList`. Access is O(1) via `TokenList::get_range()`.
///
/// # Use Cases
/// - **Formatters**: Know exact token boundaries for lossless roundtrip
/// - **Future macros**: Store token ranges for macro expansion
/// - **Attribute processing**: Preserve attribute syntax for IDE features
///
/// # Memory Efficiency
/// - `None` variant: 0 bytes discriminant (most common)
/// - `Range` variant: 8 bytes (start + end as u32)
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug, Default
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TokenCapture {
    /// No tokens captured (default for most nodes).
    #[default]
    None,

    /// Range of token indices `[start, end)` in the `TokenList`.
    ///
    /// Invariant: `start <= end`. An empty range has `start == end`.
    Range {
        /// Starting token index (inclusive).
        start: u32,
        /// Ending token index (exclusive).
        end: u32,
    },
}

impl TokenCapture {
    /// Create a new capture range.
    ///
    /// Returns `None` if the range is empty (start == end).
    #[inline]
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end, "TokenCapture: start ({start}) > end ({end})");
        if start == end {
            Self::None
        } else {
            Self::Range { start, end }
        }
    }

    /// Check if this capture is empty (no tokens).
    #[inline]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Get the number of captured tokens.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Range { start, end } => (end - start) as usize,
        }
    }

    /// Get the byte span covered by this capture.
    ///
    /// Returns `None` if the capture is empty or the token list is unavailable.
    #[inline]
    pub fn span(&self, tokens: &TokenList) -> Option<Span> {
        match self {
            Self::None => None,
            Self::Range { start, end } => {
                let first = tokens.get(*start as usize)?;
                let last = tokens.get((*end as usize).saturating_sub(1))?;
                Some(first.span.merge(last.span))
            }
        }
    }
}

/// A list of tokens with Salsa-compatible traits.
///
/// Wraps `Vec<Token>` with Clone, Eq, Hash support.
/// Uses the tokens' own Hash impl for content hashing.
///
/// Includes a parallel `tags` array of `u8` discriminant indices for fast
/// dispatch. The tags are derived from `token.kind.discriminant_index()` at
/// insertion time, enabling O(1) tag comparison without touching the full
/// 16-byte `TokenKind`.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug, Default
#[derive(Clone, Default)]
pub struct TokenList {
    tokens: Vec<Token>,
    /// Parallel array of discriminant tags, one per token.
    /// `tags[i] == tokens[i].kind.discriminant_index()` for all `i`.
    tags: Vec<u8>,
}

// Manual Eq/PartialEq/Hash: `tags` is derived from `tokens`, so only compare/hash tokens.
impl PartialEq for TokenList {
    fn eq(&self, other: &Self) -> bool {
        self.tokens == other.tokens
    }
}
impl Eq for TokenList {}
impl Hash for TokenList {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
    }
}

impl TokenList {
    /// Create a new empty token list.
    #[inline]
    pub fn new() -> Self {
        TokenList {
            tokens: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Create a new token list with pre-allocated capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        TokenList {
            tokens: Vec::with_capacity(capacity),
            tags: Vec::with_capacity(capacity),
        }
    }

    /// Create from a Vec of tokens.
    #[inline]
    pub fn from_vec(tokens: Vec<Token>) -> Self {
        let tags = tokens.iter().map(|t| t.kind.discriminant_index()).collect();
        TokenList { tokens, tags }
    }

    /// Push a token.
    #[inline]
    pub fn push(&mut self, token: Token) {
        self.tags.push(token.kind.discriminant_index());
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

    /// Get tokens in a capture range.
    ///
    /// Returns an empty slice for `TokenCapture::None`.
    ///
    /// # Panics
    ///
    /// Panics if the capture range is out of bounds.
    #[inline]
    pub fn get_range(&self, capture: TokenCapture) -> &[Token] {
        match capture {
            TokenCapture::None => &[],
            TokenCapture::Range { start, end } => &self.tokens[start as usize..end as usize],
        }
    }

    /// Get tokens in a capture range, returning None if out of bounds.
    #[inline]
    pub fn try_get_range(&self, capture: TokenCapture) -> Option<&[Token]> {
        match capture {
            TokenCapture::None => Some(&[]),
            TokenCapture::Range { start, end } => self.tokens.get(start as usize..end as usize),
        }
    }

    /// Get the tag (discriminant index) at the given position.
    ///
    /// This is a fast O(1) read from the dense tag array, avoiding
    /// the need to access the full 16-byte `TokenKind`.
    #[inline]
    pub fn tag(&self, index: usize) -> u8 {
        self.tags[index]
    }

    /// Get the full tags slice.
    #[inline]
    pub fn tags(&self) -> &[u8] {
        &self.tags
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

    #[inline]
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
    use super::{DurationUnit, SizeUnit, Token, TokenCapture, TokenKind};
    // Token is frequently allocated in TokenList, keep it compact.
    // Contains: TokenKind (16 bytes) + Span (8 bytes) = 24 bytes
    crate::static_assert_size!(Token, 24);
    // TokenKind largest variant: Duration(u64, DurationUnit) or Int(u64)
    // 8 bytes payload + 8 bytes discriminant/padding = 16 bytes
    crate::static_assert_size!(TokenKind, 16);
    // Compact unit types
    crate::static_assert_size!(DurationUnit, 1);
    crate::static_assert_size!(SizeUnit, 1);
    // TokenCapture: discriminant (4 bytes) + start (4 bytes) + end (4 bytes) = 12 bytes
    // Optimized to 12 bytes thanks to niche optimization (None has no payload)
    crate::static_assert_size!(TokenCapture, 12);
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_discriminant_index_uniqueness() {
        // Verify all discriminant indices are unique and within range
        let mut seen = [false; TOKEN_KIND_COUNT];

        // Test representative tokens from each category
        let tokens = [
            TokenKind::Int(0),
            TokenKind::Float(0),
            TokenKind::String(crate::Name::EMPTY),
            TokenKind::Char('a'),
            TokenKind::Duration(0, DurationUnit::Seconds),
            TokenKind::Size(0, SizeUnit::Bytes),
            TokenKind::Ident(crate::Name::EMPTY),
            TokenKind::Async,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::Return,
            TokenKind::Def,
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
            TokenKind::Tests,
            TokenKind::As,
            TokenKind::Dyn,
            TokenKind::Extend,
            TokenKind::Extension,
            TokenKind::Skip,
            TokenKind::IntType,
            TokenKind::FloatType,
            TokenKind::BoolType,
            TokenKind::StrType,
            TokenKind::CharType,
            TokenKind::ByteType,
            TokenKind::NeverType,
            TokenKind::Ok,
            TokenKind::Err,
            TokenKind::Some,
            TokenKind::None,
            TokenKind::Cache,
            TokenKind::Catch,
            TokenKind::Parallel,
            TokenKind::Spawn,
            TokenKind::Recurse,
            TokenKind::Run,
            TokenKind::Timeout,
            TokenKind::Try,
            TokenKind::By,
            TokenKind::Print,
            TokenKind::Panic,
            TokenKind::Todo,
            TokenKind::Unreachable,
            TokenKind::HashBracket,
            TokenKind::At,
            TokenKind::Dollar,
            TokenKind::Hash,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Colon,
            TokenKind::DoubleColon,
            TokenKind::Comma,
            TokenKind::Dot,
            TokenKind::DotDot,
            TokenKind::DotDotEq,
            TokenKind::DotDotDot,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::Pipe,
            TokenKind::Question,
            TokenKind::DoubleQuestion,
            TokenKind::Underscore,
            TokenKind::Semicolon,
            TokenKind::Eq,
            TokenKind::EqEq,
            TokenKind::NotEq,
            TokenKind::Lt,
            TokenKind::LtEq,
            TokenKind::Shl,
            TokenKind::Gt,
            TokenKind::GtEq,
            TokenKind::Shr,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Bang,
            TokenKind::Tilde,
            TokenKind::Amp,
            TokenKind::AmpAmp,
            TokenKind::PipePipe,
            TokenKind::Caret,
            TokenKind::Div,
            TokenKind::Newline,
            TokenKind::Eof,
            TokenKind::Error,
            TokenKind::FloatDurationError,
            TokenKind::FloatSizeError,
        ];

        assert_eq!(
            tokens.len(),
            TOKEN_KIND_COUNT,
            "Test should cover all {TOKEN_KIND_COUNT} token kinds",
        );

        for token in &tokens {
            let idx = token.discriminant_index() as usize;
            assert!(
                idx < TOKEN_KIND_COUNT,
                "Discriminant index {idx} out of range for {token:?}",
            );
            assert!(
                !seen[idx],
                "Duplicate discriminant index {idx} for {token:?}",
            );
            seen[idx] = true;
        }

        // Verify all indices are used
        for (i, &s) in seen.iter().enumerate() {
            assert!(s, "Discriminant index {i} is not assigned to any token");
        }
    }

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
        // SI units: decimal notation (1kb = 1000 bytes)
        assert_eq!(SizeUnit::Kilobytes.to_bytes(4), 4_000);
        assert_eq!(SizeUnit::Megabytes.to_bytes(1), 1_000_000);
        assert_eq!(SizeUnit::Gigabytes.to_bytes(1), 1_000_000_000);
        assert_eq!(SizeUnit::Terabytes.to_bytes(1), 1_000_000_000_000);
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

    #[test]
    fn test_friendly_name_from_index() {
        // Test data-carrying variants
        assert_eq!(TokenKind::friendly_name_from_index(0), Some("integer"));
        assert_eq!(TokenKind::friendly_name_from_index(6), Some("identifier"));

        // Test keywords
        assert_eq!(TokenKind::friendly_name_from_index(16), Some("if"));
        assert_eq!(TokenKind::friendly_name_from_index(19), Some("let"));

        // Test punctuation
        assert_eq!(TokenKind::friendly_name_from_index(70), Some("("));
        assert_eq!(TokenKind::friendly_name_from_index(71), Some(")"));
        assert_eq!(TokenKind::friendly_name_from_index(78), Some(","));

        // Test operators
        assert_eq!(TokenKind::friendly_name_from_index(99), Some("+"));
        assert_eq!(TokenKind::friendly_name_from_index(100), Some("-"));

        // Test internal tokens (should return None)
        assert_eq!(TokenKind::friendly_name_from_index(111), None); // Newline
        assert_eq!(TokenKind::friendly_name_from_index(112), None); // Eof
        assert_eq!(TokenKind::friendly_name_from_index(113), None); // Error

        // Test out of range
        assert_eq!(TokenKind::friendly_name_from_index(200), None);
    }

    #[test]
    fn test_friendly_name_matches_discriminant() {
        // Verify that friendly_name_from_index returns correct names
        // for the corresponding discriminant indices
        let test_cases = [
            (TokenKind::Int(42), "integer"),
            (TokenKind::Ident(crate::Name::EMPTY), "identifier"),
            (TokenKind::If, "if"),
            (TokenKind::Let, "let"),
            (TokenKind::Plus, "+"),
            (TokenKind::LParen, "("),
            (TokenKind::Comma, ","),
        ];

        for (token, expected_name) in test_cases {
            let index = token.discriminant_index();
            let friendly = TokenKind::friendly_name_from_index(index);
            assert_eq!(
                friendly,
                Some(expected_name),
                "Mismatch for {token:?} at index {index}"
            );
        }
    }

    // TokenCapture tests
    #[test]
    fn test_token_capture_none() {
        let capture = TokenCapture::None;
        assert!(capture.is_empty());
        assert_eq!(capture.len(), 0);

        let list = TokenList::new();
        assert_eq!(list.get_range(capture), &[]);
    }

    #[test]
    fn test_token_capture_range() {
        let capture = TokenCapture::Range { start: 1, end: 3 };
        assert!(!capture.is_empty());
        assert_eq!(capture.len(), 2);
    }

    #[test]
    fn test_token_capture_new() {
        // Empty range becomes None
        assert_eq!(TokenCapture::new(5, 5), TokenCapture::None);

        // Non-empty range becomes Range
        assert_eq!(
            TokenCapture::new(1, 4),
            TokenCapture::Range { start: 1, end: 4 }
        );
    }

    #[test]
    fn test_token_capture_default() {
        let capture = TokenCapture::default();
        assert!(matches!(capture, TokenCapture::None));
    }

    #[test]
    fn test_token_list_get_range() {
        let mut list = TokenList::new();
        list.push(Token::new(TokenKind::Let, Span::new(0, 3)));
        list.push(Token::new(
            TokenKind::Ident(crate::Name::EMPTY),
            Span::new(4, 5),
        ));
        list.push(Token::new(TokenKind::Eq, Span::new(6, 7)));
        list.push(Token::new(TokenKind::Int(42), Span::new(8, 10)));

        // Get range [1, 3) = tokens at indices 1 and 2
        let capture = TokenCapture::Range { start: 1, end: 3 };
        let range = list.get_range(capture);
        assert_eq!(range.len(), 2);
        assert!(matches!(range[0].kind, TokenKind::Ident(_)));
        assert!(matches!(range[1].kind, TokenKind::Eq));
    }

    #[test]
    fn test_token_list_try_get_range() {
        let mut list = TokenList::new();
        list.push(Token::new(TokenKind::Let, Span::new(0, 3)));

        // Valid range
        let capture = TokenCapture::Range { start: 0, end: 1 };
        assert!(list.try_get_range(capture).is_some());

        // Invalid range (out of bounds)
        let capture = TokenCapture::Range { start: 0, end: 5 };
        assert!(list.try_get_range(capture).is_none());
    }

    #[test]
    fn test_token_capture_span() {
        let mut list = TokenList::new();
        list.push(Token::new(TokenKind::Let, Span::new(0, 3)));
        list.push(Token::new(
            TokenKind::Ident(crate::Name::EMPTY),
            Span::new(4, 5),
        ));
        list.push(Token::new(TokenKind::Eq, Span::new(6, 7)));

        // Span of range [0, 3) should merge first and last token spans
        let capture = TokenCapture::Range { start: 0, end: 3 };
        let span = capture.span(&list).unwrap();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 7);

        // None capture has no span
        assert!(TokenCapture::None.span(&list).is_none());
    }

    #[test]
    fn test_token_capture_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(TokenCapture::None);
        set.insert(TokenCapture::None); // duplicate
        set.insert(TokenCapture::Range { start: 0, end: 3 });
        set.insert(TokenCapture::Range { start: 0, end: 3 }); // duplicate
        set.insert(TokenCapture::Range { start: 1, end: 4 });

        assert_eq!(set.len(), 3);
    }
}
