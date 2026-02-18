//! Token kinds for Ori.

use std::fmt;
use std::hash::Hash;

use super::tag::TokenTag;
use super::units::{DurationUnit, SizeUnit};
use crate::Name;

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

impl TokenKind {
    // Discriminant tag constants for O(1) tag-based dispatch.
    //
    // These are the values returned by `discriminant_index()` and stored in
    // `TokenList::tags`. Use these instead of magic numbers in match arms.
    //
    // All values derive from `TokenTag` — the single source of truth.

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
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive TokenKind → discriminant index mapping"
    )]
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

    /// If this token is a keyword, return its string representation.
    ///
    /// Returns `None` for non-keyword tokens (identifiers, literals,
    /// operators, delimiters). Used to allow keywords as member names
    /// after `.` (e.g., `ordering.then(other: Less)`).
    pub fn keyword_str(&self) -> Option<&'static str> {
        match self {
            // Reserved keywords
            TokenKind::Async => Some("async"),
            TokenKind::Break => Some("break"),
            TokenKind::Continue => Some("continue"),
            TokenKind::Return => Some("return"),
            TokenKind::Def => Some("def"),
            TokenKind::Do => Some("do"),
            TokenKind::Else => Some("else"),
            TokenKind::False => Some("false"),
            TokenKind::For => Some("for"),
            TokenKind::If => Some("if"),
            TokenKind::Impl => Some("impl"),
            TokenKind::In => Some("in"),
            TokenKind::Let => Some("let"),
            TokenKind::Loop => Some("loop"),
            TokenKind::Match => Some("match"),
            TokenKind::Mut => Some("mut"),
            TokenKind::Pub => Some("pub"),
            TokenKind::SelfLower => Some("self"),
            TokenKind::SelfUpper => Some("Self"),
            TokenKind::Then => Some("then"),
            TokenKind::Trait => Some("trait"),
            TokenKind::True => Some("true"),
            TokenKind::Type => Some("type"),
            TokenKind::Use => Some("use"),
            TokenKind::Uses => Some("uses"),
            TokenKind::Void => Some("void"),
            TokenKind::Where => Some("where"),
            TokenKind::With => Some("with"),
            TokenKind::Yield => Some("yield"),
            TokenKind::Suspend => Some("suspend"),
            TokenKind::Unsafe => Some("unsafe"),
            TokenKind::Tests => Some("tests"),
            TokenKind::As => Some("as"),
            TokenKind::Dyn => Some("dyn"),
            TokenKind::Extend => Some("extend"),
            TokenKind::Extension => Some("extension"),
            TokenKind::Skip => Some("skip"),
            TokenKind::Extern => Some("extern"),
            // Type keywords
            TokenKind::IntType => Some("int"),
            TokenKind::FloatType => Some("float"),
            TokenKind::BoolType => Some("bool"),
            TokenKind::StrType => Some("str"),
            TokenKind::CharType => Some("char"),
            TokenKind::ByteType => Some("byte"),
            TokenKind::NeverType => Some("Never"),
            // Built-in variant names
            TokenKind::Ok => Some("Ok"),
            TokenKind::Err => Some("Err"),
            TokenKind::Some => Some("Some"),
            TokenKind::None => Some("None"),
            // Context-sensitive keywords
            TokenKind::Cache => Some("cache"),
            TokenKind::Catch => Some("catch"),
            TokenKind::Parallel => Some("parallel"),
            TokenKind::Spawn => Some("spawn"),
            TokenKind::Recurse => Some("recurse"),
            TokenKind::Run => Some("run"),
            TokenKind::Timeout => Some("timeout"),
            TokenKind::Try => Some("try"),
            TokenKind::By => Some("by"),
            // Built-in functions
            TokenKind::Print => Some("print"),
            TokenKind::Panic => Some("panic"),
            TokenKind::Todo => Some("todo"),
            TokenKind::Unreachable => Some("unreachable"),
            // Operators: `div` is also a keyword
            TokenKind::Div => Some("div"),
            // Not keywords
            _ => Option::None,
        }
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
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive TokenKind → display name dispatch"
    )]
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
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive discriminant index → friendly name lookup"
    )]
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
