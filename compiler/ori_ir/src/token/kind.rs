//! Token kinds for Ori.
//!
//! The enum definition and classification predicates live here.
//! Discriminant index mapping is in `kind_index.rs`.
//! Display names, keyword strings, and Debug impl are in `kind_display.rs`.

use std::hash::Hash;

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

    // Compound assignment operators
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PercentEq,  // %=
    AtEq,       // @=
    AmpEq,      // &=
    PipeEq,     // |=
    CaretEq,    // ^=
    ShlEq,      // <<=
    AmpAmpEq,   // &&=
    PipePipeEq, // ||=

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
}
