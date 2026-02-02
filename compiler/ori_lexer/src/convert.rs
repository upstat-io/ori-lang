//! Token Conversion
//!
//! Converts raw logos tokens to final `TokenKind` with string interning.

use ori_ir::{StringInterner, TokenKind};

use crate::escape::{unescape_char, unescape_string};
use crate::raw_token::RawToken;

/// Convert a raw token to a `TokenKind`, interning strings.
pub(crate) fn convert_token(raw: RawToken, slice: &str, interner: &StringInterner) -> TokenKind {
    match raw {
        // Literals
        RawToken::Int(n) | RawToken::HexInt(n) | RawToken::BinInt(n) => TokenKind::Int(n),
        RawToken::Float(f) => TokenKind::Float(f.to_bits()),
        RawToken::String => {
            let content = &slice[1..slice.len() - 1];
            let unescaped = unescape_string(content);
            // Use intern_owned to avoid double allocation
            TokenKind::String(interner.intern_owned(unescaped))
        }
        RawToken::Char => {
            let content = &slice[1..slice.len() - 1];
            let c = unescape_char(content);
            TokenKind::Char(c)
        }
        RawToken::Ident => TokenKind::Ident(interner.intern(slice)),

        // Duration
        RawToken::DurationNs((v, u))
        | RawToken::DurationUs((v, u))
        | RawToken::DurationMs((v, u))
        | RawToken::DurationS((v, u))
        | RawToken::DurationM((v, u))
        | RawToken::DurationH((v, u)) => TokenKind::Duration(v, u),

        // Size
        RawToken::SizeB((v, u))
        | RawToken::SizeKb((v, u))
        | RawToken::SizeMb((v, u))
        | RawToken::SizeGb((v, u))
        | RawToken::SizeTb((v, u)) => TokenKind::Size(v, u),

        // Float with duration suffix errors (e.g., 1.5s, 2.5ms)
        RawToken::FloatDurationNs
        | RawToken::FloatDurationUs
        | RawToken::FloatDurationMs
        | RawToken::FloatDurationS
        | RawToken::FloatDurationM
        | RawToken::FloatDurationH => TokenKind::FloatDurationError,

        // Float with size suffix errors (e.g., 1.5kb, 2.5mb)
        RawToken::FloatSizeB
        | RawToken::FloatSizeKb
        | RawToken::FloatSizeMb
        | RawToken::FloatSizeGb
        | RawToken::FloatSizeTb => TokenKind::FloatSizeError,

        // Keywords
        RawToken::Async => TokenKind::Async,
        RawToken::Break => TokenKind::Break,
        RawToken::Continue => TokenKind::Continue,
        RawToken::Return => TokenKind::Return,
        RawToken::Def => TokenKind::Def,
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
        RawToken::By => TokenKind::By,

        // Built-in I/O primitives
        RawToken::Print => TokenKind::Print,
        RawToken::Panic => TokenKind::Panic,
        RawToken::Todo => TokenKind::Todo,
        RawToken::Unreachable => TokenKind::Unreachable,

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
        // Note: GtEq (>=) and Shr (>>) are synthesized by the parser from
        // adjacent Gt tokens. The lexer only produces individual Gt tokens.
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
