//! Raw Token Definition
//!
//! The `RawToken` enum is the logos-derived tokenizer output before
//! string interning and final token conversion.

use logos::Logos;
use ori_ir::{DurationUnit, SizeUnit};

use crate::parse_helpers::{
    parse_float_skip_underscores, parse_int_skip_underscores, parse_with_suffix,
};

/// Raw token from logos (before interning).
#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\r]+")] // Skip horizontal whitespace
pub(crate) enum RawToken {
    #[regex(r"//[^\n]*")]
    LineComment,

    #[token("\n")]
    Newline,

    #[regex(r"\\[ \t]*\n")]
    LineContinuation,

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
    #[token("as")]
    As,
    #[token("dyn")]
    Dyn,
    #[token("extend")]
    Extend,
    #[token("extension")]
    Extension,
    #[token("skip")]
    Skip,

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

    #[token("Ok")]
    Ok,
    #[token("Err")]
    Err,
    #[token("Some")]
    Some,
    #[token("None")]
    None,

    #[token("cache")]
    Cache,
    #[token("catch")]
    Catch,
    #[token("parallel")]
    Parallel,
    #[token("spawn")]
    Spawn,
    #[token("recurse")]
    Recurse,
    #[token("run")]
    Run,
    #[token("timeout")]
    Timeout,
    #[token("try")]
    Try,

    #[token("print")]
    Print,
    #[token("panic")]
    Panic,

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
    // Note: `>>` and `>=` are NOT lexed as single tokens.
    // The lexer always produces individual `>` tokens.
    // The parser combines adjacent `>` tokens into `>>` or `>=` operators
    // in expression context. This enables parsing nested generics like
    // `Result<Result<T, E>, E>` where the `>>` at the end should be two
    // separate `>` tokens closing the generic parameter lists.
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

    // Hex integer (zero-allocation parsing)
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", |lex| {
        parse_int_skip_underscores(&lex.slice()[2..], 16)
    })]
    HexInt(u64),

    // Binary integer (zero-allocation parsing)
    #[regex(r"0b[01][01_]*", |lex| {
        parse_int_skip_underscores(&lex.slice()[2..], 2)
    })]
    BinInt(u64),

    // Integer (zero-allocation parsing)
    #[regex(r"[0-9][0-9_]*", |lex| {
        parse_int_skip_underscores(lex.slice(), 10)
    })]
    Int(u64),

    // Float (only allocates if underscores present)
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?", |lex| {
        parse_float_skip_underscores(lex.slice())
    })]
    Float(f64),

    // Duration literals (using shared helper)
    #[regex(r"[0-9]+ms", |lex| parse_with_suffix(lex.slice(), 2, DurationUnit::Milliseconds))]
    DurationMs((u64, DurationUnit)),

    #[regex(r"[0-9]+s", |lex| parse_with_suffix(lex.slice(), 1, DurationUnit::Seconds))]
    DurationS((u64, DurationUnit)),

    #[regex(r"[0-9]+m", |lex| parse_with_suffix(lex.slice(), 1, DurationUnit::Minutes))]
    DurationM((u64, DurationUnit)),

    #[regex(r"[0-9]+h", |lex| parse_with_suffix(lex.slice(), 1, DurationUnit::Hours))]
    DurationH((u64, DurationUnit)),

    // Size literals (using shared helper)
    #[regex(r"[0-9]+b", |lex| parse_with_suffix(lex.slice(), 1, SizeUnit::Bytes))]
    SizeB((u64, SizeUnit)),

    #[regex(r"[0-9]+kb", |lex| parse_with_suffix(lex.slice(), 2, SizeUnit::Kilobytes))]
    SizeKb((u64, SizeUnit)),

    #[regex(r"[0-9]+mb", |lex| parse_with_suffix(lex.slice(), 2, SizeUnit::Megabytes))]
    SizeMb((u64, SizeUnit)),

    #[regex(r"[0-9]+gb", |lex| parse_with_suffix(lex.slice(), 2, SizeUnit::Gigabytes))]
    SizeGb((u64, SizeUnit)),

    // String literal (no unescaped newlines allowed)
    #[regex(r#""([^"\\\n\r]|\\.)*""#)]
    String,

    // Char literal (no unescaped newlines allowed)
    #[regex(r"'([^'\\\n\r]|\\.)'")]
    Char,

    // Identifier
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
}
