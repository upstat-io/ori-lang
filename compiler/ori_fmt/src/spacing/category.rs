//! Token categories for spacing rule matching.
//!
//! Abstracts `TokenKind` into categories that ignore literal values,
//! enabling declarative spacing rules.

use ori_ir::TokenKind;

/// Abstract token category for spacing rule matching.
///
/// This enum groups related tokens and ignores data (like literal values)
/// that isn't relevant for spacing decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenCategory {
    // Literals (ignore values)
    /// Integer literal: `42`, `1_000`
    Int,
    /// Float literal: 3.14, 2.5e-8
    Float,
    /// String literal: "hello"
    String,
    /// Char literal: 'a'
    Char,
    /// Duration literal: 100ms, 5s
    Duration,
    /// Size literal: 4kb, 10mb
    Size,

    // Identifiers
    /// Identifier: foo, bar
    Ident,

    // Keywords
    /// break
    Break,
    /// continue
    Continue,
    /// def
    Def,
    /// do
    Do,
    /// else
    Else,
    /// false
    False,
    /// for
    For,
    /// if
    If,
    /// impl
    Impl,
    /// in
    In,
    /// let
    Let,
    /// loop
    Loop,
    /// match
    Match,
    /// pub
    Pub,
    /// self
    SelfLower,
    /// Self
    SelfUpper,
    /// then
    Then,
    /// trait
    Trait,
    /// true
    True,
    /// type
    Type,
    /// use
    Use,
    /// uses
    Uses,
    /// void
    Void,
    /// where
    Where,
    /// with
    With,
    /// yield
    Yield,
    /// tests
    Tests,
    /// as
    As,
    /// extend
    Extend,
    /// extension
    Extension,

    // Type keywords
    /// int type
    IntType,
    /// float type
    FloatType,
    /// bool type
    BoolType,
    /// str type
    StrType,
    /// char type
    CharType,
    /// byte type
    ByteType,
    /// Never type
    NeverType,

    // Wrappers
    /// Ok
    Ok,
    /// Err
    Err,
    /// Some
    Some,
    /// None
    None,

    // Compiler constructs
    /// cache
    Cache,
    /// catch
    Catch,
    /// parallel
    Parallel,
    /// spawn
    Spawn,
    /// recurse
    Recurse,
    /// run
    Run,
    /// timeout
    Timeout,
    /// try
    Try,
    /// by (range step)
    By,
    /// print
    Print,
    /// panic
    Panic,
    /// todo
    Todo,
    /// unreachable
    Unreachable,

    // Delimiters
    /// (
    LParen,
    /// )
    RParen,
    /// {
    LBrace,
    /// }
    RBrace,
    /// [
    LBracket,
    /// ]
    RBracket,

    // Punctuation
    /// @
    At,
    /// $
    Dollar,
    /// #
    Hash,
    /// #[
    HashBracket,
    /// #!
    HashBang,
    /// :
    Colon,
    /// ::
    DoubleColon,
    /// ,
    Comma,
    /// .
    Dot,
    /// ..
    DotDot,
    /// ..=
    DotDotEq,
    /// ...
    DotDotDot,
    /// ->
    Arrow,
    /// =>
    FatArrow,
    /// |
    Pipe,
    /// ?
    Question,
    /// ??
    DoubleQuestion,
    /// _
    Underscore,
    /// ;
    Semicolon,

    // Operators
    /// =
    Eq,
    /// ==
    EqEq,
    /// !=
    NotEq,
    /// <
    Lt,
    /// <=
    LtEq,
    /// <<
    Shl,
    /// >
    Gt,
    /// >=
    GtEq,
    /// >>
    Shr,
    /// +
    Plus,
    /// -
    Minus,
    /// *
    Star,
    /// /
    Slash,
    /// %
    Percent,
    /// !
    Bang,
    /// ~
    Tilde,
    /// &
    Amp,
    /// &&
    AmpAmp,
    /// ||
    PipePipe,
    /// ^
    Caret,
    /// div
    Div,

    // Special
    /// Newline
    Newline,
    /// End of file
    Eof,
    /// Error token
    Error,
}

impl TokenCategory {
    /// Check if this is a binary operator.
    #[inline]
    pub fn is_binary_op(self) -> bool {
        matches!(
            self,
            TokenCategory::Plus
                | TokenCategory::Minus
                | TokenCategory::Star
                | TokenCategory::Slash
                | TokenCategory::Percent
                | TokenCategory::Div
                | TokenCategory::EqEq
                | TokenCategory::NotEq
                | TokenCategory::Lt
                | TokenCategory::LtEq
                | TokenCategory::Gt
                | TokenCategory::GtEq
                | TokenCategory::Amp
                | TokenCategory::Pipe
                | TokenCategory::Caret
                | TokenCategory::Shl
                | TokenCategory::Shr
                | TokenCategory::AmpAmp
                | TokenCategory::PipePipe
                | TokenCategory::DoubleQuestion
        )
    }

    /// Check if this is a unary operator.
    #[inline]
    pub fn is_unary_op(self) -> bool {
        matches!(
            self,
            TokenCategory::Minus | TokenCategory::Bang | TokenCategory::Tilde
        )
    }

    /// Check if this is an opening delimiter.
    #[inline]
    pub fn is_open_delim(self) -> bool {
        matches!(
            self,
            TokenCategory::LParen | TokenCategory::LBrace | TokenCategory::LBracket
        )
    }

    /// Check if this is a closing delimiter.
    #[inline]
    pub fn is_close_delim(self) -> bool {
        matches!(
            self,
            TokenCategory::RParen | TokenCategory::RBrace | TokenCategory::RBracket
        )
    }

    /// Check if this is a literal.
    #[inline]
    pub fn is_literal(self) -> bool {
        matches!(
            self,
            TokenCategory::Int
                | TokenCategory::Float
                | TokenCategory::String
                | TokenCategory::Char
                | TokenCategory::Duration
                | TokenCategory::Size
                | TokenCategory::True
                | TokenCategory::False
        )
    }

    /// Check if this is a keyword.
    #[inline]
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            TokenCategory::Break
                | TokenCategory::Continue
                | TokenCategory::Def
                | TokenCategory::Do
                | TokenCategory::Else
                | TokenCategory::For
                | TokenCategory::If
                | TokenCategory::Impl
                | TokenCategory::In
                | TokenCategory::Let
                | TokenCategory::Loop
                | TokenCategory::Match
                | TokenCategory::Pub
                | TokenCategory::SelfLower
                | TokenCategory::SelfUpper
                | TokenCategory::Then
                | TokenCategory::Trait
                | TokenCategory::Type
                | TokenCategory::Use
                | TokenCategory::Uses
                | TokenCategory::Void
                | TokenCategory::Where
                | TokenCategory::With
                | TokenCategory::Yield
                | TokenCategory::Tests
                | TokenCategory::As
                | TokenCategory::Extend
                | TokenCategory::Extension
        )
    }
}

impl From<&TokenKind> for TokenCategory {
    fn from(kind: &TokenKind) -> Self {
        match kind {
            TokenKind::Int(_) => TokenCategory::Int,
            TokenKind::Float(_) => TokenCategory::Float,
            TokenKind::String(_)
            | TokenKind::TemplateHead(_)
            | TokenKind::TemplateMiddle(_)
            | TokenKind::TemplateTail(_)
            | TokenKind::TemplateFull(_)
            | TokenKind::FormatSpec(_) => TokenCategory::String,
            TokenKind::Char(_) => TokenCategory::Char,
            TokenKind::Duration(_, _) => TokenCategory::Duration,
            TokenKind::Size(_, _) => TokenCategory::Size,
            // Keywords treated as identifiers for spacing purposes
            TokenKind::Ident(_)
            | TokenKind::Async
            | TokenKind::Mut
            | TokenKind::Dyn
            | TokenKind::Skip
            | TokenKind::Suspend
            | TokenKind::Unsafe
            | TokenKind::Extern => TokenCategory::Ident,
            TokenKind::Break => TokenCategory::Break,
            TokenKind::Continue => TokenCategory::Continue,
            TokenKind::Def => TokenCategory::Def,
            TokenKind::Do => TokenCategory::Do,
            TokenKind::Else => TokenCategory::Else,
            TokenKind::False => TokenCategory::False,
            TokenKind::For => TokenCategory::For,
            TokenKind::If => TokenCategory::If,
            TokenKind::Impl => TokenCategory::Impl,
            TokenKind::In => TokenCategory::In,
            TokenKind::Let => TokenCategory::Let,
            TokenKind::Loop => TokenCategory::Loop,
            TokenKind::Match => TokenCategory::Match,
            TokenKind::Pub => TokenCategory::Pub,
            TokenKind::SelfLower => TokenCategory::SelfLower,
            TokenKind::SelfUpper => TokenCategory::SelfUpper,
            TokenKind::Then => TokenCategory::Then,
            TokenKind::Trait => TokenCategory::Trait,
            TokenKind::True => TokenCategory::True,
            TokenKind::Type => TokenCategory::Type,
            TokenKind::Use => TokenCategory::Use,
            TokenKind::Uses => TokenCategory::Uses,
            TokenKind::Void => TokenCategory::Void,
            TokenKind::Where => TokenCategory::Where,
            TokenKind::With => TokenCategory::With,
            TokenKind::Yield => TokenCategory::Yield,
            TokenKind::Tests => TokenCategory::Tests,
            TokenKind::As => TokenCategory::As,
            TokenKind::Extend => TokenCategory::Extend,
            TokenKind::Extension => TokenCategory::Extension,
            TokenKind::IntType => TokenCategory::IntType,
            TokenKind::FloatType => TokenCategory::FloatType,
            TokenKind::BoolType => TokenCategory::BoolType,
            TokenKind::StrType => TokenCategory::StrType,
            TokenKind::CharType => TokenCategory::CharType,
            TokenKind::ByteType => TokenCategory::ByteType,
            TokenKind::NeverType => TokenCategory::NeverType,
            TokenKind::Ok => TokenCategory::Ok,
            TokenKind::Err => TokenCategory::Err,
            TokenKind::Some => TokenCategory::Some,
            TokenKind::None => TokenCategory::None,
            TokenKind::Cache => TokenCategory::Cache,
            TokenKind::Catch => TokenCategory::Catch,
            TokenKind::Parallel => TokenCategory::Parallel,
            TokenKind::Spawn => TokenCategory::Spawn,
            TokenKind::Recurse => TokenCategory::Recurse,
            TokenKind::Run => TokenCategory::Run,
            TokenKind::Timeout => TokenCategory::Timeout,
            TokenKind::Try => TokenCategory::Try,
            TokenKind::By => TokenCategory::By,
            TokenKind::Print => TokenCategory::Print,
            TokenKind::Panic => TokenCategory::Panic,
            TokenKind::Todo => TokenCategory::Todo,
            TokenKind::Unreachable => TokenCategory::Unreachable,
            TokenKind::HashBracket => TokenCategory::HashBracket,
            TokenKind::HashBang => TokenCategory::HashBang,
            TokenKind::At => TokenCategory::At,
            TokenKind::Dollar => TokenCategory::Dollar,
            TokenKind::Hash => TokenCategory::Hash,
            TokenKind::LParen => TokenCategory::LParen,
            TokenKind::RParen => TokenCategory::RParen,
            TokenKind::LBrace => TokenCategory::LBrace,
            TokenKind::RBrace => TokenCategory::RBrace,
            TokenKind::LBracket => TokenCategory::LBracket,
            TokenKind::RBracket => TokenCategory::RBracket,
            TokenKind::Colon => TokenCategory::Colon,
            TokenKind::DoubleColon => TokenCategory::DoubleColon,
            TokenKind::Comma => TokenCategory::Comma,
            TokenKind::Dot => TokenCategory::Dot,
            TokenKind::DotDot => TokenCategory::DotDot,
            TokenKind::DotDotEq => TokenCategory::DotDotEq,
            TokenKind::DotDotDot => TokenCategory::DotDotDot,
            TokenKind::Arrow => TokenCategory::Arrow,
            TokenKind::FatArrow => TokenCategory::FatArrow,
            TokenKind::Pipe => TokenCategory::Pipe,
            TokenKind::Question => TokenCategory::Question,
            TokenKind::DoubleQuestion => TokenCategory::DoubleQuestion,
            TokenKind::Underscore => TokenCategory::Underscore,
            TokenKind::Semicolon => TokenCategory::Semicolon,
            TokenKind::Eq => TokenCategory::Eq,
            TokenKind::EqEq => TokenCategory::EqEq,
            TokenKind::NotEq => TokenCategory::NotEq,
            TokenKind::Lt => TokenCategory::Lt,
            TokenKind::LtEq => TokenCategory::LtEq,
            TokenKind::Shl => TokenCategory::Shl,
            TokenKind::Gt => TokenCategory::Gt,
            TokenKind::GtEq => TokenCategory::GtEq,
            TokenKind::Shr => TokenCategory::Shr,
            TokenKind::Plus => TokenCategory::Plus,
            TokenKind::Minus => TokenCategory::Minus,
            TokenKind::Star => TokenCategory::Star,
            TokenKind::Slash => TokenCategory::Slash,
            TokenKind::Percent => TokenCategory::Percent,
            TokenKind::Bang => TokenCategory::Bang,
            TokenKind::Tilde => TokenCategory::Tilde,
            TokenKind::Amp => TokenCategory::Amp,
            TokenKind::AmpAmp => TokenCategory::AmpAmp,
            TokenKind::PipePipe => TokenCategory::PipePipe,
            TokenKind::Caret => TokenCategory::Caret,
            TokenKind::Div => TokenCategory::Div,
            TokenKind::Newline => TokenCategory::Newline,
            TokenKind::Eof => TokenCategory::Eof,
            // Return is recognized but invalid - treat as error for spacing
            TokenKind::Return | TokenKind::Error => TokenCategory::Error,
        }
    }
}

impl From<TokenKind> for TokenCategory {
    fn from(kind: TokenKind) -> Self {
        TokenCategory::from(&kind)
    }
}
