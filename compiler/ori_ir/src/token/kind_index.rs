//! Discriminant index mapping for `TokenKind`.
//!
//! Provides O(1) bitset membership testing in `TokenSet` by mapping each
//! `TokenKind` variant to a stable `u8` index via `TokenTag`.

use super::kind::TokenKind;
use super::tag::TokenTag;

impl TokenKind {
    /// Get a unique index for this token's discriminant (0-139).
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

            // Keywords — reserved (12-39)
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

            // Compound assignment (128-139)
            Self::PlusEq => TokenTag::PlusEq as u8,
            Self::MinusEq => TokenTag::MinusEq as u8,
            Self::StarEq => TokenTag::StarEq as u8,
            Self::SlashEq => TokenTag::SlashEq as u8,
            Self::PercentEq => TokenTag::PercentEq as u8,
            Self::AtEq => TokenTag::AtEq as u8,
            Self::AmpEq => TokenTag::AmpEq as u8,
            Self::PipeEq => TokenTag::PipeEq as u8,
            Self::CaretEq => TokenTag::CaretEq as u8,
            Self::ShlEq => TokenTag::ShlEq as u8,
            Self::AmpAmpEq => TokenTag::AmpAmpEq as u8,
            Self::PipePipeEq => TokenTag::PipePipeEq as u8,
        }
    }
}
