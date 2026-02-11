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
    Suspend,
    Unsafe,

    // Additional keywords
    Tests,
    As,
    Dyn,
    Extend,
    Extension,
    Skip,
    Extern,

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
    HashBang,       // #!
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

    /// Template head: `` `text{ `` (opening backtick to first unescaped `{`).
    TemplateHead(Name),
    /// Template middle: `}text{` (between interpolations).
    TemplateMiddle(Name),
    /// Template tail: `` }text` `` (last `}` to closing backtick).
    TemplateTail(Name),
    /// Complete template: `` `text` `` (no interpolation).
    TemplateFull(Name),
    /// Format spec in template interpolation: `{value:>10.2f}` → `>10.2f`
    FormatSpec(Name),
}

/// Compact discriminant tag for `TokenKind`, with semantic range layout.
///
/// All values fit in a single `u8` (max 127), with categories arranged in
/// contiguous ranges separated by gaps for future expansion:
///
/// | Range   | Category           |
/// |---------|--------------------|
/// | 0-10    | Literals           |
/// | 11-49   | Keywords (reserved + additional) |
/// | 50-56   | Type keywords      |
/// | 57-60   | Constructors       |
/// | 61-73   | Pattern keywords   |
/// | 74-75   | Gap (future keywords) |
/// | 76-99   | Punctuation        |
/// | 100-120 | Operators          |
/// | 121-127 | Special            |
///
/// This enum serves as the single source of truth for discriminant values.
/// `TAG_*` constants and `discriminant_index()` both derive from these values.
///
/// # Invariant
///
/// All discriminants must be < 128 to fit within the parser's `OPER_TABLE[128]`
/// and `POSTFIX_BITSET` (2 × u64 = 128 bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TokenTag {
    // === Literals (0-10) ===
    Ident = 0,
    Int = 1,
    Float = 2,
    String = 3,
    Char = 4,
    Duration = 5,
    Size = 6,
    TemplateHead = 7,
    TemplateMiddle = 8,
    TemplateTail = 9,
    TemplateComplete = 10,

    // === Keywords — reserved (11-39) ===
    KwAsync = 11,
    KwBreak = 12,
    KwContinue = 13,
    KwReturn = 14,
    KwDef = 15,
    KwDo = 16,
    KwElse = 17,
    KwFalse = 18,
    KwFor = 19,
    KwIf = 20,
    KwImpl = 21,
    KwIn = 22,
    KwLet = 23,
    KwLoop = 24,
    KwMatch = 25,
    KwMut = 26,
    KwPub = 27,
    KwSelfLower = 28,
    KwSelfUpper = 29,
    KwSuspend = 30,
    KwThen = 31,
    KwTrait = 32,
    KwTrue = 33,
    KwType = 34,
    KwUnsafe = 35,
    KwUse = 36,
    KwUses = 37,
    KwVoid = 38,
    KwWhere = 39,

    // === Keywords — additional (40-49) ===
    KwWith = 40,
    KwYield = 41,
    KwTests = 42,
    KwAs = 43,
    KwDyn = 44,
    KwExtend = 45,
    KwExtension = 46,
    KwSkip = 47,
    KwExtern = 48,
    // 49: reserved for future keyword

    // === Type keywords (50-56) ===
    KwIntType = 50,
    KwFloatType = 51,
    KwBoolType = 52,
    KwStrType = 53,
    KwCharType = 54,
    KwByteType = 55,
    KwNeverType = 56,

    // === Constructors (57-60) ===
    KwOk = 57,
    KwErr = 58,
    KwSome = 59,
    KwNone = 60,

    // === Pattern keywords (61-73) ===
    KwCache = 61,
    KwCatch = 62,
    KwParallel = 63,
    KwSpawn = 64,
    KwRecurse = 65,
    KwRun = 66,
    KwTimeout = 67,
    KwTry = 68,
    KwBy = 69,
    KwPrint = 70,
    KwPanic = 71,
    KwTodo = 72,
    KwUnreachable = 73,

    // 74: Template format spec
    FormatSpec = 74,
    HashBang = 75, // #!

    // === Punctuation (76-99) ===
    HashBracket = 76,    // #[
    At = 77,             // @
    Dollar = 78,         // $
    Hash = 79,           // #
    LParen = 80,         // (
    RParen = 81,         // )
    LBrace = 82,         // {
    RBrace = 83,         // }
    LBracket = 84,       // [
    RBracket = 85,       // ]
    Colon = 86,          // :
    DoubleColon = 87,    // ::
    Comma = 88,          // ,
    Dot = 89,            // .
    DotDot = 90,         // ..
    DotDotEq = 91,       // ..=
    DotDotDot = 92,      // ...
    Arrow = 93,          // ->
    FatArrow = 94,       // =>
    Pipe = 95,           // |
    Question = 96,       // ?
    DoubleQuestion = 97, // ??
    Underscore = 98,     // _
    Semicolon = 99,      // ;

    // === Operators (100-120) ===
    Eq = 100,       // =
    EqEq = 101,     // ==
    NotEq = 102,    // !=
    Lt = 103,       // <
    LtEq = 104,     // <=
    Shl = 105,      // <<
    Gt = 106,       // >
    GtEq = 107,     // >=
    Shr = 108,      // >>
    Plus = 109,     // +
    Minus = 110,    // -
    Star = 111,     // *
    Slash = 112,    // /
    Percent = 113,  // %
    Bang = 114,     // !
    Tilde = 115,    // ~
    Amp = 116,      // &
    AmpAmp = 117,   // &&
    PipePipe = 118, // ||
    Caret = 119,    // ^
    Div = 120,      // div

    // === Special (121-127) ===
    Newline = 121,
    Error = 122,
    Eof = 127,
    // 123-126: reserved for future special tokens
}

// Compile-time assertion: all TokenTag values fit in 7 bits (< 128).
// This is required for TokenSet (u128 bitset), OPER_TABLE[128], and POSTFIX_BITSET.
const _: () = assert!(TokenTag::MAX_DISCRIMINANT <= 127);

impl TokenTag {
    /// Maximum discriminant value across all variants.
    ///
    /// Must be < 128 for `TokenSet` (u128 bitset), `OPER_TABLE[128]`,
    /// and `POSTFIX_BITSET`. Update this when adding new variants.
    pub const MAX_DISCRIMINANT: u8 = Self::Eof as u8;

    /// Get a human-readable name for this tag.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ident => "identifier",
            Self::Int => "integer",
            Self::Float | Self::KwFloatType => "float",
            Self::String => "string",
            Self::Char | Self::KwCharType => "char",
            Self::Duration => "duration",
            Self::Size => "size",
            Self::TemplateHead => "template head",
            Self::TemplateMiddle => "template middle",
            Self::TemplateTail => "template tail",
            Self::TemplateComplete => "template literal",
            Self::FormatSpec => "format spec",
            Self::KwAsync => "async",
            Self::KwBreak => "break",
            Self::KwContinue => "continue",
            Self::KwReturn => "return",
            Self::KwDef => "def",
            Self::KwDo => "do",
            Self::KwElse => "else",
            Self::KwFalse => "false",
            Self::KwFor => "for",
            Self::KwIf => "if",
            Self::KwImpl => "impl",
            Self::KwIn => "in",
            Self::KwLet => "let",
            Self::KwLoop => "loop",
            Self::KwMatch => "match",
            Self::KwMut => "mut",
            Self::KwPub => "pub",
            Self::KwSelfLower => "self",
            Self::KwSelfUpper => "Self",
            Self::KwSuspend => "suspend",
            Self::KwThen => "then",
            Self::KwTrait => "trait",
            Self::KwTrue => "true",
            Self::KwType => "type",
            Self::KwUnsafe => "unsafe",
            Self::KwUse => "use",
            Self::KwUses => "uses",
            Self::KwVoid => "void",
            Self::KwWhere => "where",
            Self::KwWith => "with",
            Self::KwYield => "yield",
            Self::KwTests => "tests",
            Self::KwAs => "as",
            Self::KwDyn => "dyn",
            Self::KwExtend => "extend",
            Self::KwExtension => "extension",
            Self::KwSkip => "skip",
            Self::KwExtern => "extern",
            Self::KwIntType => "int",
            Self::KwBoolType => "bool",
            Self::KwStrType => "str",
            Self::KwByteType => "byte",
            Self::KwNeverType => "Never",
            Self::KwOk => "Ok",
            Self::KwErr => "Err",
            Self::KwSome => "Some",
            Self::KwNone => "None",
            Self::KwCache => "cache",
            Self::KwCatch => "catch",
            Self::KwParallel => "parallel",
            Self::KwSpawn => "spawn",
            Self::KwRecurse => "recurse",
            Self::KwRun => "run",
            Self::KwTimeout => "timeout",
            Self::KwTry => "try",
            Self::KwBy => "by",
            Self::KwPrint => "print",
            Self::KwPanic => "panic",
            Self::KwTodo => "todo",
            Self::KwUnreachable => "unreachable",
            Self::HashBracket => "#[",
            Self::HashBang => "#!",
            Self::At => "@",
            Self::Dollar => "$",
            Self::Hash => "#",
            Self::LParen => "(",
            Self::RParen => ")",
            Self::LBrace => "{",
            Self::RBrace => "}",
            Self::LBracket => "[",
            Self::RBracket => "]",
            Self::Colon => ":",
            Self::DoubleColon => "::",
            Self::Comma => ",",
            Self::Dot => ".",
            Self::DotDot => "..",
            Self::DotDotEq => "..=",
            Self::DotDotDot => "...",
            Self::Arrow => "->",
            Self::FatArrow => "=>",
            Self::Pipe => "|",
            Self::Question => "?",
            Self::DoubleQuestion => "??",
            Self::Underscore => "_",
            Self::Semicolon => ";",
            Self::Eq => "=",
            Self::EqEq => "==",
            Self::NotEq => "!=",
            Self::Lt => "<",
            Self::LtEq => "<=",
            Self::Shl => "<<",
            Self::Gt => ">",
            Self::GtEq => ">=",
            Self::Shr => ">>",
            Self::Plus => "+",
            Self::Minus => "-",
            Self::Star => "*",
            Self::Slash => "/",
            Self::Percent => "%",
            Self::Bang => "!",
            Self::Tilde => "~",
            Self::Amp => "&",
            Self::AmpAmp => "&&",
            Self::PipePipe => "||",
            Self::Caret => "^",
            Self::Div => "div",
            Self::Newline => "newline",
            Self::Error => "error",
            Self::Eof => "end of file",
        }
    }
}

/// Number of [`TokenKind`] variants. Used for bitset sizing and test verification.
#[cfg(test)]
pub(crate) const TOKEN_KIND_COUNT: usize = 123;

impl TokenKind {
    // ─────────────────────────────────────────────────────────────────────────
    // Discriminant tag constants for O(1) tag-based dispatch.
    //
    // These are the values returned by `discriminant_index()` and stored in
    // `TokenList::tags`. Use these instead of magic numbers in match arms.
    //
    // All values derive from `TokenTag` — the single source of truth.
    // ─────────────────────────────────────────────────────────────────────────

    // Literals (0-10)
    pub const TAG_IDENT: u8 = TokenTag::Ident as u8;
    pub const TAG_INT: u8 = TokenTag::Int as u8;
    pub const TAG_FLOAT: u8 = TokenTag::Float as u8;
    pub const TAG_STRING: u8 = TokenTag::String as u8;
    pub const TAG_CHAR: u8 = TokenTag::Char as u8;
    pub const TAG_DURATION: u8 = TokenTag::Duration as u8;
    pub const TAG_SIZE: u8 = TokenTag::Size as u8;
    pub const TAG_TEMPLATE_HEAD: u8 = TokenTag::TemplateHead as u8;
    pub const TAG_TEMPLATE_MIDDLE: u8 = TokenTag::TemplateMiddle as u8;
    pub const TAG_TEMPLATE_TAIL: u8 = TokenTag::TemplateTail as u8;
    pub const TAG_TEMPLATE_FULL: u8 = TokenTag::TemplateComplete as u8;
    pub const TAG_FORMAT_SPEC: u8 = TokenTag::FormatSpec as u8;

    // Keywords — reserved (11-39)
    pub const TAG_ASYNC: u8 = TokenTag::KwAsync as u8;
    pub const TAG_BREAK: u8 = TokenTag::KwBreak as u8;
    pub const TAG_CONTINUE: u8 = TokenTag::KwContinue as u8;
    pub const TAG_RETURN: u8 = TokenTag::KwReturn as u8;
    pub const TAG_DEF: u8 = TokenTag::KwDef as u8;
    pub const TAG_DO: u8 = TokenTag::KwDo as u8;
    pub const TAG_ELSE: u8 = TokenTag::KwElse as u8;
    pub const TAG_FALSE: u8 = TokenTag::KwFalse as u8;
    pub const TAG_FOR: u8 = TokenTag::KwFor as u8;
    pub const TAG_IF: u8 = TokenTag::KwIf as u8;
    pub const TAG_IMPL: u8 = TokenTag::KwImpl as u8;
    pub const TAG_IN: u8 = TokenTag::KwIn as u8;
    pub const TAG_LET: u8 = TokenTag::KwLet as u8;
    pub const TAG_LOOP: u8 = TokenTag::KwLoop as u8;
    pub const TAG_MATCH: u8 = TokenTag::KwMatch as u8;
    pub const TAG_MUT: u8 = TokenTag::KwMut as u8;
    pub const TAG_PUB: u8 = TokenTag::KwPub as u8;
    pub const TAG_SELF_LOWER: u8 = TokenTag::KwSelfLower as u8;
    pub const TAG_SELF_UPPER: u8 = TokenTag::KwSelfUpper as u8;
    pub const TAG_SUSPEND: u8 = TokenTag::KwSuspend as u8;
    pub const TAG_THEN: u8 = TokenTag::KwThen as u8;
    pub const TAG_TRAIT: u8 = TokenTag::KwTrait as u8;
    pub const TAG_TRUE: u8 = TokenTag::KwTrue as u8;
    pub const TAG_TYPE: u8 = TokenTag::KwType as u8;
    pub const TAG_UNSAFE: u8 = TokenTag::KwUnsafe as u8;
    pub const TAG_USE: u8 = TokenTag::KwUse as u8;
    pub const TAG_USES: u8 = TokenTag::KwUses as u8;
    pub const TAG_VOID: u8 = TokenTag::KwVoid as u8;
    pub const TAG_WHERE: u8 = TokenTag::KwWhere as u8;

    // Keywords — additional (40-49)
    pub const TAG_WITH: u8 = TokenTag::KwWith as u8;
    pub const TAG_YIELD: u8 = TokenTag::KwYield as u8;
    pub const TAG_TESTS: u8 = TokenTag::KwTests as u8;
    pub const TAG_AS: u8 = TokenTag::KwAs as u8;
    pub const TAG_DYN: u8 = TokenTag::KwDyn as u8;
    pub const TAG_EXTEND: u8 = TokenTag::KwExtend as u8;
    pub const TAG_EXTENSION: u8 = TokenTag::KwExtension as u8;
    pub const TAG_SKIP: u8 = TokenTag::KwSkip as u8;
    pub const TAG_EXTERN: u8 = TokenTag::KwExtern as u8;

    // Type keywords (50-56)
    pub const TAG_INT_TYPE: u8 = TokenTag::KwIntType as u8;
    pub const TAG_FLOAT_TYPE: u8 = TokenTag::KwFloatType as u8;
    pub const TAG_BOOL_TYPE: u8 = TokenTag::KwBoolType as u8;
    pub const TAG_STR_TYPE: u8 = TokenTag::KwStrType as u8;
    pub const TAG_CHAR_TYPE: u8 = TokenTag::KwCharType as u8;
    pub const TAG_BYTE_TYPE: u8 = TokenTag::KwByteType as u8;
    pub const TAG_NEVER_TYPE: u8 = TokenTag::KwNeverType as u8;

    // Constructors (57-60)
    pub const TAG_OK: u8 = TokenTag::KwOk as u8;
    pub const TAG_ERR: u8 = TokenTag::KwErr as u8;
    pub const TAG_SOME: u8 = TokenTag::KwSome as u8;
    pub const TAG_NONE: u8 = TokenTag::KwNone as u8;

    // Pattern keywords (61-73)
    pub const TAG_CACHE: u8 = TokenTag::KwCache as u8;
    pub const TAG_CATCH: u8 = TokenTag::KwCatch as u8;
    pub const TAG_PARALLEL: u8 = TokenTag::KwParallel as u8;
    pub const TAG_SPAWN: u8 = TokenTag::KwSpawn as u8;
    pub const TAG_RECURSE: u8 = TokenTag::KwRecurse as u8;
    pub const TAG_RUN: u8 = TokenTag::KwRun as u8;
    pub const TAG_TIMEOUT: u8 = TokenTag::KwTimeout as u8;
    pub const TAG_TRY: u8 = TokenTag::KwTry as u8;
    pub const TAG_BY: u8 = TokenTag::KwBy as u8;
    pub const TAG_PRINT: u8 = TokenTag::KwPrint as u8;
    pub const TAG_PANIC: u8 = TokenTag::KwPanic as u8;
    pub const TAG_TODO: u8 = TokenTag::KwTodo as u8;
    pub const TAG_UNREACHABLE: u8 = TokenTag::KwUnreachable as u8;

    // Punctuation (75-99)
    pub const TAG_HASH_BANG: u8 = TokenTag::HashBang as u8;
    pub const TAG_HASH_BRACKET: u8 = TokenTag::HashBracket as u8;
    pub const TAG_AT: u8 = TokenTag::At as u8;
    pub const TAG_DOLLAR: u8 = TokenTag::Dollar as u8;
    pub const TAG_HASH: u8 = TokenTag::Hash as u8;
    pub const TAG_LPAREN: u8 = TokenTag::LParen as u8;
    pub const TAG_RPAREN: u8 = TokenTag::RParen as u8;
    pub const TAG_LBRACE: u8 = TokenTag::LBrace as u8;
    pub const TAG_RBRACE: u8 = TokenTag::RBrace as u8;
    pub const TAG_LBRACKET: u8 = TokenTag::LBracket as u8;
    pub const TAG_RBRACKET: u8 = TokenTag::RBracket as u8;
    pub const TAG_COLON: u8 = TokenTag::Colon as u8;
    pub const TAG_DOUBLE_COLON: u8 = TokenTag::DoubleColon as u8;
    pub const TAG_COMMA: u8 = TokenTag::Comma as u8;
    pub const TAG_DOT: u8 = TokenTag::Dot as u8;
    pub const TAG_DOTDOT: u8 = TokenTag::DotDot as u8;
    pub const TAG_DOTDOTEQ: u8 = TokenTag::DotDotEq as u8;
    pub const TAG_DOTDOTDOT: u8 = TokenTag::DotDotDot as u8;
    pub const TAG_ARROW: u8 = TokenTag::Arrow as u8;
    pub const TAG_FAT_ARROW: u8 = TokenTag::FatArrow as u8;
    pub const TAG_PIPE: u8 = TokenTag::Pipe as u8;
    pub const TAG_QUESTION: u8 = TokenTag::Question as u8;
    pub const TAG_DOUBLE_QUESTION: u8 = TokenTag::DoubleQuestion as u8;
    pub const TAG_UNDERSCORE: u8 = TokenTag::Underscore as u8;
    pub const TAG_SEMICOLON: u8 = TokenTag::Semicolon as u8;

    // Operators (100-120)
    pub const TAG_EQ: u8 = TokenTag::Eq as u8;
    pub const TAG_EQEQ: u8 = TokenTag::EqEq as u8;
    pub const TAG_NOTEQ: u8 = TokenTag::NotEq as u8;
    pub const TAG_LT: u8 = TokenTag::Lt as u8;
    pub const TAG_LTEQ: u8 = TokenTag::LtEq as u8;
    pub const TAG_SHL: u8 = TokenTag::Shl as u8;
    pub const TAG_GT: u8 = TokenTag::Gt as u8;
    pub const TAG_GTEQ: u8 = TokenTag::GtEq as u8;
    pub const TAG_SHR: u8 = TokenTag::Shr as u8;
    pub const TAG_PLUS: u8 = TokenTag::Plus as u8;
    pub const TAG_MINUS: u8 = TokenTag::Minus as u8;
    pub const TAG_STAR: u8 = TokenTag::Star as u8;
    pub const TAG_SLASH: u8 = TokenTag::Slash as u8;
    pub const TAG_PERCENT: u8 = TokenTag::Percent as u8;
    pub const TAG_BANG: u8 = TokenTag::Bang as u8;
    pub const TAG_TILDE: u8 = TokenTag::Tilde as u8;
    pub const TAG_AMP: u8 = TokenTag::Amp as u8;
    pub const TAG_AMPAMP: u8 = TokenTag::AmpAmp as u8;
    pub const TAG_PIPEPIPE: u8 = TokenTag::PipePipe as u8;
    pub const TAG_CARET: u8 = TokenTag::Caret as u8;
    pub const TAG_DIV: u8 = TokenTag::Div as u8;

    // Special (121-127)
    pub const TAG_NEWLINE: u8 = TokenTag::Newline as u8;
    pub const TAG_ERROR: u8 = TokenTag::Error as u8;
    pub const TAG_EOF: u8 = TokenTag::Eof as u8;

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
            // Literals (0-10)
            Self::Ident(_) => TokenTag::Ident as u8,
            Self::Int(_) => TokenTag::Int as u8,
            Self::Float(_) => TokenTag::Float as u8,
            Self::String(_) => TokenTag::String as u8,
            Self::Char(_) => TokenTag::Char as u8,
            Self::Duration(_, _) => TokenTag::Duration as u8,
            Self::Size(_, _) => TokenTag::Size as u8,
            Self::TemplateHead(_) => TokenTag::TemplateHead as u8,
            Self::TemplateMiddle(_) => TokenTag::TemplateMiddle as u8,
            Self::TemplateTail(_) => TokenTag::TemplateTail as u8,
            Self::TemplateFull(_) => TokenTag::TemplateComplete as u8,
            Self::FormatSpec(_) => TokenTag::FormatSpec as u8,

            // Keywords — reserved (11-39)
            Self::Async => TokenTag::KwAsync as u8,
            Self::Break => TokenTag::KwBreak as u8,
            Self::Continue => TokenTag::KwContinue as u8,
            Self::Return => TokenTag::KwReturn as u8,
            Self::Def => TokenTag::KwDef as u8,
            Self::Do => TokenTag::KwDo as u8,
            Self::Else => TokenTag::KwElse as u8,
            Self::False => TokenTag::KwFalse as u8,
            Self::For => TokenTag::KwFor as u8,
            Self::If => TokenTag::KwIf as u8,
            Self::Impl => TokenTag::KwImpl as u8,
            Self::In => TokenTag::KwIn as u8,
            Self::Let => TokenTag::KwLet as u8,
            Self::Loop => TokenTag::KwLoop as u8,
            Self::Match => TokenTag::KwMatch as u8,
            Self::Mut => TokenTag::KwMut as u8,
            Self::Pub => TokenTag::KwPub as u8,
            Self::SelfLower => TokenTag::KwSelfLower as u8,
            Self::SelfUpper => TokenTag::KwSelfUpper as u8,
            Self::Suspend => TokenTag::KwSuspend as u8,
            Self::Then => TokenTag::KwThen as u8,
            Self::Trait => TokenTag::KwTrait as u8,
            Self::True => TokenTag::KwTrue as u8,
            Self::Type => TokenTag::KwType as u8,
            Self::Unsafe => TokenTag::KwUnsafe as u8,
            Self::Use => TokenTag::KwUse as u8,
            Self::Uses => TokenTag::KwUses as u8,
            Self::Void => TokenTag::KwVoid as u8,
            Self::Where => TokenTag::KwWhere as u8,

            // Keywords — additional (40-49)
            Self::With => TokenTag::KwWith as u8,
            Self::Yield => TokenTag::KwYield as u8,
            Self::Tests => TokenTag::KwTests as u8,
            Self::As => TokenTag::KwAs as u8,
            Self::Dyn => TokenTag::KwDyn as u8,
            Self::Extend => TokenTag::KwExtend as u8,
            Self::Extension => TokenTag::KwExtension as u8,
            Self::Skip => TokenTag::KwSkip as u8,
            Self::Extern => TokenTag::KwExtern as u8,

            // Type keywords (50-56)
            Self::IntType => TokenTag::KwIntType as u8,
            Self::FloatType => TokenTag::KwFloatType as u8,
            Self::BoolType => TokenTag::KwBoolType as u8,
            Self::StrType => TokenTag::KwStrType as u8,
            Self::CharType => TokenTag::KwCharType as u8,
            Self::ByteType => TokenTag::KwByteType as u8,
            Self::NeverType => TokenTag::KwNeverType as u8,

            // Constructors (57-60)
            Self::Ok => TokenTag::KwOk as u8,
            Self::Err => TokenTag::KwErr as u8,
            Self::Some => TokenTag::KwSome as u8,
            Self::None => TokenTag::KwNone as u8,

            // Pattern keywords (61-73)
            Self::Cache => TokenTag::KwCache as u8,
            Self::Catch => TokenTag::KwCatch as u8,
            Self::Parallel => TokenTag::KwParallel as u8,
            Self::Spawn => TokenTag::KwSpawn as u8,
            Self::Recurse => TokenTag::KwRecurse as u8,
            Self::Run => TokenTag::KwRun as u8,
            Self::Timeout => TokenTag::KwTimeout as u8,
            Self::Try => TokenTag::KwTry as u8,
            Self::By => TokenTag::KwBy as u8,
            Self::Print => TokenTag::KwPrint as u8,
            Self::Panic => TokenTag::KwPanic as u8,
            Self::Todo => TokenTag::KwTodo as u8,
            Self::Unreachable => TokenTag::KwUnreachable as u8,

            // Punctuation (75-99)
            Self::HashBang => TokenTag::HashBang as u8,
            Self::HashBracket => TokenTag::HashBracket as u8,
            Self::At => TokenTag::At as u8,
            Self::Dollar => TokenTag::Dollar as u8,
            Self::Hash => TokenTag::Hash as u8,
            Self::LParen => TokenTag::LParen as u8,
            Self::RParen => TokenTag::RParen as u8,
            Self::LBrace => TokenTag::LBrace as u8,
            Self::RBrace => TokenTag::RBrace as u8,
            Self::LBracket => TokenTag::LBracket as u8,
            Self::RBracket => TokenTag::RBracket as u8,
            Self::Colon => TokenTag::Colon as u8,
            Self::DoubleColon => TokenTag::DoubleColon as u8,
            Self::Comma => TokenTag::Comma as u8,
            Self::Dot => TokenTag::Dot as u8,
            Self::DotDot => TokenTag::DotDot as u8,
            Self::DotDotEq => TokenTag::DotDotEq as u8,
            Self::DotDotDot => TokenTag::DotDotDot as u8,
            Self::Arrow => TokenTag::Arrow as u8,
            Self::FatArrow => TokenTag::FatArrow as u8,
            Self::Pipe => TokenTag::Pipe as u8,
            Self::Question => TokenTag::Question as u8,
            Self::DoubleQuestion => TokenTag::DoubleQuestion as u8,
            Self::Underscore => TokenTag::Underscore as u8,
            Self::Semicolon => TokenTag::Semicolon as u8,

            // Operators (100-120)
            Self::Eq => TokenTag::Eq as u8,
            Self::EqEq => TokenTag::EqEq as u8,
            Self::NotEq => TokenTag::NotEq as u8,
            Self::Lt => TokenTag::Lt as u8,
            Self::LtEq => TokenTag::LtEq as u8,
            Self::Shl => TokenTag::Shl as u8,
            Self::Gt => TokenTag::Gt as u8,
            Self::GtEq => TokenTag::GtEq as u8,
            Self::Shr => TokenTag::Shr as u8,
            Self::Plus => TokenTag::Plus as u8,
            Self::Minus => TokenTag::Minus as u8,
            Self::Star => TokenTag::Star as u8,
            Self::Slash => TokenTag::Slash as u8,
            Self::Percent => TokenTag::Percent as u8,
            Self::Bang => TokenTag::Bang as u8,
            Self::Tilde => TokenTag::Tilde as u8,
            Self::Amp => TokenTag::Amp as u8,
            Self::AmpAmp => TokenTag::AmpAmp as u8,
            Self::PipePipe => TokenTag::PipePipe as u8,
            Self::Caret => TokenTag::Caret as u8,
            Self::Div => TokenTag::Div as u8,

            // Special (121-127)
            Self::Newline => TokenTag::Newline as u8,
            Self::Error => TokenTag::Error as u8,
            Self::Eof => TokenTag::Eof as u8,
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
                | TokenKind::TemplateFull(_)
                | TokenKind::TemplateHead(_)
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
            TokenKind::Suspend => "suspend",
            TokenKind::Unsafe => "unsafe",
            TokenKind::Tests => "tests",
            TokenKind::As => "as",
            TokenKind::Dyn => "dyn",
            TokenKind::Extend => "extend",
            TokenKind::Extension => "extension",
            TokenKind::Skip => "skip",
            TokenKind::Extern => "extern",
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
            TokenKind::HashBang => "#!",
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
            TokenKind::TemplateHead(_) => "template head",
            TokenKind::TemplateMiddle(_) => "template middle",
            TokenKind::TemplateTail(_) => "template tail",
            TokenKind::TemplateFull(_) => "template literal",
            TokenKind::FormatSpec(_) => "format spec",
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
        // Uses TokenTag values as indices. Some arms are merged when different
        // tokens share the same display name (e.g., Float literal and FloatType).
        match index {
            // Literals (0-10)
            0 => Some("identifier"),        // Ident
            1 => Some("integer"),           // Int
            2 | 51 => Some("float"),        // Float (literal) and FloatType (keyword)
            3 => Some("string"),            // String
            4 | 54 => Some("char"),         // Char (literal) and CharType (keyword)
            5 => Some("duration"),          // Duration
            6 => Some("size"),              // Size
            7 => Some("template head"),     // TemplateHead
            8 => Some("template middle"),   // TemplateMiddle
            9 => Some("template tail"),     // TemplateTail
            10 => Some("template literal"), // TemplateComplete

            // Keywords — reserved (11-39)
            11 => Some("async"),
            12 => Some("break"),
            13 => Some("continue"),
            // 14 was "return" — removed (not a spec keyword)
            15 => Some("def"),
            16 => Some("do"),
            17 => Some("else"),
            18 => Some("false"),
            19 => Some("for"),
            20 => Some("if"),
            21 => Some("impl"),
            22 => Some("in"),
            23 => Some("let"),
            24 => Some("loop"),
            25 => Some("match"),
            26 => Some("mut"),
            27 => Some("pub"),
            28 => Some("self"),
            29 => Some("Self"),
            30 => Some("suspend"),
            31 => Some("then"),
            32 => Some("trait"),
            33 => Some("true"),
            34 => Some("type"),
            35 => Some("unsafe"),
            36 => Some("use"),
            37 => Some("uses"),
            38 => Some("void"),
            39 => Some("where"),

            // Keywords — additional (40-49)
            40 => Some("with"),
            41 => Some("yield"),
            42 => Some("tests"),
            43 => Some("as"),
            44 => Some("dyn"),
            45 => Some("extend"),
            46 => Some("extension"),
            47 => Some("skip"),
            48 => Some("extern"),

            // Type keywords (50-56, some merged above)
            50 => Some("int"),
            // 51 merged with 2 (float)
            52 => Some("bool"),
            53 => Some("str"),
            // 54 merged with 4 (char)
            55 => Some("byte"),
            56 => Some("Never"),

            // Constructors (57-60)
            57 => Some("Ok"),
            58 => Some("Err"),
            59 => Some("Some"),
            60 => Some("None"),

            // Pattern keywords (61-73)
            61 => Some("cache"),
            62 => Some("catch"),
            63 => Some("parallel"),
            64 => Some("spawn"),
            65 => Some("recurse"),
            66 => Some("run"),
            67 => Some("timeout"),
            68 => Some("try"),
            69 => Some("by"),
            70 => Some("print"),
            71 => Some("panic"),
            72 => Some("todo"),
            73 => Some("unreachable"),

            // 74: FormatSpec
            74 => Some("format spec"),
            75 => Some("#!"),

            // Punctuation (76-99)
            76 => Some("#["),
            77 => Some("@"),
            78 => Some("$"),
            79 => Some("#"),
            80 => Some("("),
            81 => Some(")"),
            82 => Some("{"),
            83 => Some("}"),
            84 => Some("["),
            85 => Some("]"),
            86 => Some(":"),
            87 => Some("::"),
            88 => Some(","),
            89 => Some("."),
            90 => Some(".."),
            91 => Some("..="),
            92 => Some("..."),
            93 => Some("->"),
            94 => Some("=>"),
            95 => Some("|"),
            96 => Some("?"),
            97 => Some("??"),
            98 => Some("_"),
            99 => Some(";"),

            // Operators (100-120)
            100 => Some("="),
            101 => Some("=="),
            102 => Some("!="),
            103 => Some("<"),
            104 => Some("<="),
            105 => Some("<<"),
            106 => Some(">"),
            107 => Some(">="),
            108 => Some(">>"),
            109 => Some("+"),
            110 => Some("-"),
            111 => Some("*"),
            112 => Some("/"),
            113 => Some("%"),
            114 => Some("!"),
            115 => Some("~"),
            116 => Some("&"),
            117 => Some("&&"),
            118 => Some("||"),
            119 => Some("^"),
            120 => Some("div"),

            // Special (121-127): Newline, Error, Eof
            // These are internal tokens — exclude from expected lists.
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
            TokenKind::TemplateHead(name) => write!(f, "TemplateHead({name:?})"),
            TokenKind::TemplateMiddle(name) => write!(f, "TemplateMiddle({name:?})"),
            TokenKind::TemplateTail(name) => write!(f, "TemplateTail({name:?})"),
            TokenKind::TemplateFull(name) => write!(f, "TemplateFull({name:?})"),
            TokenKind::FormatSpec(name) => write!(f, "FormatSpec({name:?})"),
            _ => write!(f, "{}", self.display_name()),
        }
    }
}

/// Duration unit for duration literals.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum DurationUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl DurationUnit {
    /// Nanosecond multiplier for this unit.
    ///
    /// Used by the lexer to convert decimal duration literals to nanoseconds
    /// via integer arithmetic (no floats involved).
    #[inline]
    pub fn nanos_multiplier(self) -> u64 {
        match self {
            DurationUnit::Nanoseconds => 1,
            DurationUnit::Microseconds => 1_000,
            DurationUnit::Milliseconds => 1_000_000,
            DurationUnit::Seconds => 1_000_000_000,
            DurationUnit::Minutes => 60_000_000_000,
            DurationUnit::Hours => 3_600_000_000_000,
        }
    }

    /// Convert value to nanoseconds.
    #[inline]
    pub fn to_nanos(self, value: u64) -> i64 {
        let ns = value * self.nanos_multiplier();
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
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl SizeUnit {
    /// Byte multiplier for this unit (SI, powers of 1000).
    ///
    /// Used by the lexer to convert decimal size literals to bytes
    /// via integer arithmetic (no floats involved).
    #[inline]
    pub fn bytes_multiplier(self) -> u64 {
        match self {
            SizeUnit::Bytes => 1,
            SizeUnit::Kilobytes => 1_000,
            SizeUnit::Megabytes => 1_000_000,
            SizeUnit::Gigabytes => 1_000_000_000,
            SizeUnit::Terabytes => 1_000_000_000_000,
        }
    }

    /// Convert value to bytes using SI units (powers of 1000).
    ///
    /// SI units: 1kb = 1000 bytes, 1mb = 1,000,000 bytes, etc.
    /// For exact powers of 1024, use explicit byte counts: `1024b`, `1048576b`.
    #[inline]
    pub fn to_bytes(self, value: u64) -> u64 {
        value * self.bytes_multiplier()
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

/// Typed index into a `TokenList`.
///
/// Provides type safety over raw `u32` indices when referring to tokens.
/// Uses `u32::MAX` as a sentinel for "no token".
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct TokenIdx(u32);

impl TokenIdx {
    /// Sentinel value indicating no token.
    pub const NONE: TokenIdx = TokenIdx(u32::MAX);

    /// Create a `TokenIdx` from a raw index.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        TokenIdx(raw)
    }

    /// Get the raw `u32` index.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a valid index (not the `NONE` sentinel).
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

// Compile-time assertion: TokenIdx is exactly 4 bytes.
const _: () = assert!(size_of::<TokenIdx>() == 4);

/// Per-token metadata flags packed into a single byte.
///
/// These flags capture the whitespace/trivia context preceding each token,
/// enabling downstream consumers (formatter, parser) to reconstruct layout
/// without storing trivia tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TokenFlags(u8);

impl TokenFlags {
    /// Whitespace preceded this token (spaces or tabs).
    pub const SPACE_BEFORE: u8 = 1 << 0;
    /// A newline preceded this token.
    pub const NEWLINE_BEFORE: u8 = 1 << 1;
    /// A comment preceded this token.
    pub const TRIVIA_BEFORE: u8 = 1 << 2;
    /// Token is the first non-trivia token on its line.
    pub const LINE_START: u8 = 1 << 3;
    /// Cooking detected an error in this token.
    pub const HAS_ERROR: u8 = 1 << 4;
    /// A doc comment preceded this token (markers: `#`, `*`, `!`, `>`).
    pub const IS_DOC: u8 = 1 << 5;
    /// No whitespace, newline, or trivia preceded this token (adjacent to previous).
    pub const ADJACENT: u8 = 1 << 6;
    /// Token was resolved as a context-sensitive keyword (soft keyword with `(` lookahead).
    pub const CONTEXTUAL_KW: u8 = 1 << 7;

    /// Empty flags (no bits set).
    pub const EMPTY: Self = TokenFlags(0);

    /// Create flags from raw bits.
    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        TokenFlags(bits)
    }

    /// Get the raw bits.
    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Check if a specific flag is set.
    #[inline]
    pub const fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    /// Set a flag.
    #[inline]
    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    /// Check if space preceded this token.
    #[inline]
    pub const fn has_space_before(self) -> bool {
        self.contains(Self::SPACE_BEFORE)
    }

    /// Check if a newline preceded this token.
    #[inline]
    pub const fn has_newline_before(self) -> bool {
        self.contains(Self::NEWLINE_BEFORE)
    }

    /// Check if a comment preceded this token.
    #[inline]
    pub const fn has_trivia_before(self) -> bool {
        self.contains(Self::TRIVIA_BEFORE)
    }

    /// Check if this token is first on its line.
    #[inline]
    pub const fn is_line_start(self) -> bool {
        self.contains(Self::LINE_START)
    }

    /// Check if cooking detected an error.
    #[inline]
    pub const fn has_error(self) -> bool {
        self.contains(Self::HAS_ERROR)
    }

    /// Check if a doc comment preceded this token.
    #[inline]
    pub const fn is_doc(self) -> bool {
        self.contains(Self::IS_DOC)
    }

    /// Check if this token is adjacent to the previous (no whitespace/trivia between).
    #[inline]
    pub const fn is_adjacent(self) -> bool {
        self.contains(Self::ADJACENT)
    }

    /// Check if this token was resolved as a context-sensitive keyword.
    #[inline]
    pub const fn is_contextual_kw(self) -> bool {
        self.contains(Self::CONTEXTUAL_KW)
    }
}

// Compile-time assertion: TokenFlags is exactly 1 byte.
const _: () = assert!(size_of::<TokenFlags>() == 1);

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
    /// Parallel array of per-token metadata flags, one per token.
    /// `flags[i]` captures whitespace/trivia context for `tokens[i]`.
    flags: Vec<TokenFlags>,
}

// Manual Eq/PartialEq/Hash: position-independent comparison.
//
// We skip `tags` (derived from `tokens.kind`) AND skip `Span` positions.
// Only `TokenKind` and `TokenFlags` are compared/hashed. This enables
// Salsa early cutoff: whitespace-only edits shift token positions but
// produce the same kinds+flags, so downstream queries (parsing, type
// checking) are not re-executed.
impl PartialEq for TokenList {
    fn eq(&self, other: &Self) -> bool {
        if self.tokens.len() != other.tokens.len() {
            return false;
        }
        self.tokens
            .iter()
            .zip(other.tokens.iter())
            .all(|(a, b)| a.kind == b.kind)
            && self.flags == other.flags
    }
}
impl Eq for TokenList {}
impl Hash for TokenList {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tokens.len().hash(state);
        for token in &self.tokens {
            token.kind.hash(state);
        }
        self.flags.hash(state);
    }
}

impl TokenList {
    /// Create a new empty token list.
    #[inline]
    pub fn new() -> Self {
        TokenList {
            tokens: Vec::new(),
            tags: Vec::new(),
            flags: Vec::new(),
        }
    }

    /// Create a new token list with pre-allocated capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        TokenList {
            tokens: Vec::with_capacity(capacity),
            tags: Vec::with_capacity(capacity),
            flags: Vec::with_capacity(capacity),
        }
    }

    /// Create from a Vec of tokens.
    ///
    /// All tokens get `TokenFlags::EMPTY` since no trivia context is available.
    #[inline]
    pub fn from_vec(tokens: Vec<Token>) -> Self {
        let tags = tokens.iter().map(|t| t.kind.discriminant_index()).collect();
        let flags = vec![TokenFlags::EMPTY; tokens.len()];
        TokenList {
            tokens,
            tags,
            flags,
        }
    }

    /// Push a token with default (empty) flags.
    #[inline]
    pub fn push(&mut self, token: Token) {
        self.tags.push(token.kind.discriminant_index());
        self.flags.push(TokenFlags::EMPTY);
        self.tokens.push(token);
    }

    /// Push a token with explicit flags.
    #[inline]
    pub fn push_with_flags(&mut self, token: Token, flags: TokenFlags) {
        self.tags.push(token.kind.discriminant_index());
        self.flags.push(flags);
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

    /// Get the flags for the token at the given position.
    #[inline]
    pub fn flag(&self, index: usize) -> TokenFlags {
        self.flags[index]
    }

    /// Get the full flags slice.
    #[inline]
    pub fn flags(&self) -> &[TokenFlags] {
        &self.flags
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
        let mut seen = [false; 128];

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
            TokenKind::Suspend,
            TokenKind::Unsafe,
            TokenKind::Tests,
            TokenKind::As,
            TokenKind::Dyn,
            TokenKind::Extend,
            TokenKind::Extension,
            TokenKind::Skip,
            TokenKind::Extern,
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
            TokenKind::HashBang,
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
            TokenKind::TemplateHead(crate::Name::EMPTY),
            TokenKind::TemplateMiddle(crate::Name::EMPTY),
            TokenKind::TemplateTail(crate::Name::EMPTY),
            TokenKind::TemplateFull(crate::Name::EMPTY),
            TokenKind::FormatSpec(crate::Name::EMPTY),
        ];

        assert_eq!(
            tokens.len(),
            TOKEN_KIND_COUNT,
            "Test should cover all {TOKEN_KIND_COUNT} token kinds",
        );

        for token in &tokens {
            let idx = token.discriminant_index() as usize;
            assert!(
                idx < 128,
                "Discriminant index {idx} out of range for {token:?}",
            );
            assert!(
                !seen[idx],
                "Duplicate discriminant index {idx} for {token:?}",
            );
            seen[idx] = true;
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
        // Test literals (use TokenTag values)
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Ident as u8),
            Some("identifier")
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Int as u8),
            Some("integer")
        );

        // Test keywords
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::KwIf as u8),
            Some("if")
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::KwLet as u8),
            Some("let")
        );

        // Test punctuation
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::LParen as u8),
            Some("(")
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::RParen as u8),
            Some(")")
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Comma as u8),
            Some(",")
        );

        // Test operators
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Plus as u8),
            Some("+")
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Minus as u8),
            Some("-")
        );

        // Test internal tokens (should return None)
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Newline as u8),
            None
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Eof as u8),
            None
        );
        assert_eq!(
            TokenKind::friendly_name_from_index(TokenTag::Error as u8),
            None
        );

        // Test non-gap indices in the 74-75 range
        assert_eq!(TokenKind::friendly_name_from_index(74), Some("format spec"));
        assert_eq!(TokenKind::friendly_name_from_index(75), Some("#!"));

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

    // ─── Phase 8: Section 04 validation tests ────────────────────────

    #[test]
    fn test_token_tag_stability() {
        // Pin key discriminant values that external code may depend on.
        // If these change, update all downstream consumers.
        assert_eq!(TokenTag::Ident as u8, 0);
        assert_eq!(TokenTag::Int as u8, 1);
        assert_eq!(TokenTag::Float as u8, 2);
        assert_eq!(TokenTag::String as u8, 3);
        assert_eq!(TokenTag::KwIf as u8, 20);
        assert_eq!(TokenTag::KwLet as u8, 23);
        assert_eq!(TokenTag::KwIntType as u8, 50);
        assert_eq!(TokenTag::KwNeverType as u8, 56);
        assert_eq!(TokenTag::LParen as u8, 80);
        assert_eq!(TokenTag::Pipe as u8, 95);
        assert_eq!(TokenTag::Eq as u8, 100);
        assert_eq!(TokenTag::Eof as u8, 127);
    }

    #[test]
    fn test_token_tag_name_non_empty() {
        // Every TokenTag variant must return a non-empty name
        let all_tags: &[TokenTag] = &[
            TokenTag::Ident,
            TokenTag::Int,
            TokenTag::Float,
            TokenTag::String,
            TokenTag::Char,
            TokenTag::Duration,
            TokenTag::Size,
            TokenTag::TemplateHead,
            TokenTag::TemplateMiddle,
            TokenTag::TemplateTail,
            TokenTag::TemplateComplete,
            TokenTag::FormatSpec,
            TokenTag::HashBang,
            TokenTag::KwAsync,
            TokenTag::KwBreak,
            TokenTag::KwContinue,
            TokenTag::KwReturn,
            TokenTag::KwDef,
            TokenTag::KwDo,
            TokenTag::KwElse,
            TokenTag::KwFalse,
            TokenTag::KwFor,
            TokenTag::KwIf,
            TokenTag::KwImpl,
            TokenTag::KwIn,
            TokenTag::KwLet,
            TokenTag::KwLoop,
            TokenTag::KwMatch,
            TokenTag::KwMut,
            TokenTag::KwPub,
            TokenTag::KwSelfLower,
            TokenTag::KwSelfUpper,
            TokenTag::KwSuspend,
            TokenTag::KwThen,
            TokenTag::KwTrait,
            TokenTag::KwTrue,
            TokenTag::KwType,
            TokenTag::KwUnsafe,
            TokenTag::KwUse,
            TokenTag::KwUses,
            TokenTag::KwVoid,
            TokenTag::KwWhere,
            TokenTag::KwWith,
            TokenTag::KwYield,
            TokenTag::KwTests,
            TokenTag::KwAs,
            TokenTag::KwDyn,
            TokenTag::KwExtend,
            TokenTag::KwExtension,
            TokenTag::KwSkip,
            TokenTag::KwExtern,
            TokenTag::KwIntType,
            TokenTag::KwFloatType,
            TokenTag::KwBoolType,
            TokenTag::KwStrType,
            TokenTag::KwCharType,
            TokenTag::KwByteType,
            TokenTag::KwNeverType,
            TokenTag::KwOk,
            TokenTag::KwErr,
            TokenTag::KwSome,
            TokenTag::KwNone,
            TokenTag::KwCache,
            TokenTag::KwCatch,
            TokenTag::KwParallel,
            TokenTag::KwSpawn,
            TokenTag::KwRecurse,
            TokenTag::KwRun,
            TokenTag::KwTimeout,
            TokenTag::KwTry,
            TokenTag::KwBy,
            TokenTag::Div,
            TokenTag::KwPrint,
            TokenTag::KwPanic,
            TokenTag::KwTodo,
            TokenTag::KwUnreachable,
            TokenTag::HashBracket,
            TokenTag::HashBang,
            TokenTag::At,
            TokenTag::Dollar,
            TokenTag::Hash,
            TokenTag::LParen,
            TokenTag::RParen,
            TokenTag::LBrace,
            TokenTag::RBrace,
            TokenTag::LBracket,
            TokenTag::RBracket,
            TokenTag::Colon,
            TokenTag::DoubleColon,
            TokenTag::Comma,
            TokenTag::Dot,
            TokenTag::DotDot,
            TokenTag::DotDotEq,
            TokenTag::DotDotDot,
            TokenTag::Arrow,
            TokenTag::FatArrow,
            TokenTag::Underscore,
            TokenTag::Semicolon,
            TokenTag::Pipe,
            TokenTag::Eq,
            TokenTag::EqEq,
            TokenTag::NotEq,
            TokenTag::Lt,
            TokenTag::LtEq,
            TokenTag::Shl,
            TokenTag::Gt,
            TokenTag::GtEq,
            TokenTag::Shr,
            TokenTag::Plus,
            TokenTag::Minus,
            TokenTag::Star,
            TokenTag::Slash,
            TokenTag::Percent,
            TokenTag::Bang,
            TokenTag::Tilde,
            TokenTag::Amp,
            TokenTag::AmpAmp,
            TokenTag::PipePipe,
            TokenTag::Question,
            TokenTag::DoubleQuestion,
            TokenTag::Caret,
            TokenTag::Newline,
            TokenTag::Error,
            TokenTag::Eof,
        ];

        for tag in all_tags {
            let name = tag.name();
            assert!(!name.is_empty(), "TokenTag::{tag:?} returned empty name",);
        }
    }

    #[test]
    fn test_token_list_push_invariant() {
        // After every push, tags[i] must equal tokens[i].kind.discriminant_index()
        let mut list = TokenList::new();

        let tokens_to_push = [
            Token::new(TokenKind::Int(42), Span::new(0, 2)),
            Token::new(TokenKind::Plus, Span::new(3, 4)),
            Token::new(TokenKind::Ident(crate::Name::EMPTY), Span::new(5, 6)),
            Token::new(TokenKind::If, Span::new(7, 9)),
            Token::new(TokenKind::LParen, Span::new(10, 11)),
        ];

        for token in tokens_to_push {
            list.push(token);
        }

        let tags = list.tags();
        for (i, token) in list.iter().enumerate() {
            assert_eq!(
                tags[i],
                token.kind.discriminant_index(),
                "Tag mismatch at index {i}: tags[{i}]={} but discriminant={}",
                tags[i],
                token.kind.discriminant_index(),
            );
        }
    }

    #[test]
    fn test_token_list_flags_parallel_invariant() {
        // flags.len() must always equal tokens.len()
        let mut list = TokenList::new();
        assert_eq!(list.flags().len(), list.len());

        list.push(Token::new(TokenKind::Int(1), Span::new(0, 1)));
        assert_eq!(list.flags().len(), list.len());

        list.push(Token::new(TokenKind::Plus, Span::new(2, 3)));
        assert_eq!(list.flags().len(), list.len());

        list.push(Token::new(TokenKind::Int(2), Span::new(4, 5)));
        assert_eq!(list.flags().len(), list.len());
    }

    #[test]
    fn test_token_flags_operations() {
        let empty = TokenFlags::EMPTY;
        assert!(!empty.contains(TokenFlags::SPACE_BEFORE));
        assert!(!empty.contains(TokenFlags::NEWLINE_BEFORE));
        assert!(!empty.has_space_before());
        assert!(!empty.has_newline_before());

        let with_space = TokenFlags::from_bits(TokenFlags::SPACE_BEFORE);
        assert!(with_space.contains(TokenFlags::SPACE_BEFORE));
        assert!(!with_space.contains(TokenFlags::NEWLINE_BEFORE));
        assert!(with_space.has_space_before());

        let with_both =
            TokenFlags::from_bits(TokenFlags::SPACE_BEFORE | TokenFlags::NEWLINE_BEFORE);
        assert!(with_both.has_space_before());
        assert!(with_both.has_newline_before());
    }

    // ─── Position-independent Hash/Eq tests ──────────────────────────

    #[test]
    fn position_independent_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_of(list: &TokenList) -> u64 {
            let mut h = DefaultHasher::new();
            list.hash(&mut h);
            h.finish()
        }

        // Same kinds, different spans → same hash (position-independent)
        let list_a = TokenList::from_vec(vec![
            Token::new(TokenKind::Int(1), Span::new(0, 1)),
            Token::new(TokenKind::Plus, Span::new(2, 3)),
            Token::new(TokenKind::Int(2), Span::new(4, 5)),
        ]);
        let list_b = TokenList::from_vec(vec![
            Token::new(TokenKind::Int(1), Span::new(10, 11)),
            Token::new(TokenKind::Plus, Span::new(20, 21)),
            Token::new(TokenKind::Int(2), Span::new(30, 31)),
        ]);

        assert_eq!(hash_of(&list_a), hash_of(&list_b));
        assert_eq!(list_a, list_b);
    }

    #[test]
    fn different_kinds_not_equal() {
        // Different token kinds → not equal even at same positions
        let list_a = TokenList::from_vec(vec![
            Token::new(TokenKind::Int(1), Span::new(0, 1)),
            Token::new(TokenKind::Plus, Span::new(2, 3)),
        ]);
        let list_b = TokenList::from_vec(vec![
            Token::new(TokenKind::Int(1), Span::new(0, 1)),
            Token::new(TokenKind::Minus, Span::new(2, 3)),
        ]);

        assert_ne!(list_a, list_b);
    }

    #[test]
    fn position_independent_hash_in_hashset() {
        use std::collections::HashSet;

        let list_pos1 = TokenList::from_vec(vec![
            Token::new(TokenKind::Let, Span::new(0, 3)),
            Token::new(TokenKind::Ident(crate::Name::EMPTY), Span::new(4, 5)),
        ]);
        let list_pos2 = TokenList::from_vec(vec![
            Token::new(TokenKind::Let, Span::new(100, 103)),
            Token::new(TokenKind::Ident(crate::Name::EMPTY), Span::new(200, 201)),
        ]);

        let mut set = HashSet::new();
        set.insert(list_pos1);
        set.insert(list_pos2); // same kinds/flags, different positions → deduped
        assert_eq!(set.len(), 1, "position-shifted lists should be equal");
    }
}
