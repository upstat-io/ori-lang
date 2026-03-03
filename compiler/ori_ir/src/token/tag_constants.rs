//! Discriminant tag constants for `TokenKind`.
//!
//! O(1) tag-based dispatch constants derived from `TokenTag`.
//! These are the values returned by `discriminant_index()` and stored in
//! `TokenList::tags`. Use these instead of magic numbers in match arms.

use super::kind::TokenKind;
use super::tag::TokenTag;

impl TokenKind {
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

    // Compound assignment (128-139)
    pub const TAG_PLUS_EQ: u8 = TokenTag::PlusEq as u8;
    pub const TAG_MINUS_EQ: u8 = TokenTag::MinusEq as u8;
    pub const TAG_STAR_EQ: u8 = TokenTag::StarEq as u8;
    pub const TAG_SLASH_EQ: u8 = TokenTag::SlashEq as u8;
    pub const TAG_PERCENT_EQ: u8 = TokenTag::PercentEq as u8;
    pub const TAG_AT_EQ: u8 = TokenTag::AtEq as u8;
    pub const TAG_AMP_EQ: u8 = TokenTag::AmpEq as u8;
    pub const TAG_PIPE_EQ: u8 = TokenTag::PipeEq as u8;
    pub const TAG_CARET_EQ: u8 = TokenTag::CaretEq as u8;
    pub const TAG_SHL_EQ: u8 = TokenTag::ShlEq as u8;
    pub const TAG_AMPAMP_EQ: u8 = TokenTag::AmpAmpEq as u8;
    pub const TAG_PIPEPIPE_EQ: u8 = TokenTag::PipePipeEq as u8;
}
