//! Compact discriminant tag for `TokenKind`.

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
/// | 128-139 | Compound assignment|
///
/// This enum serves as the single source of truth for discriminant values.
/// `TAG_*` constants and `discriminant_index()` both derive from these values.
///
/// # Invariant
///
/// All discriminants must be < 256. The parser's `OPER_TABLE[128]` and
/// `POSTFIX_BITSET` only cover indices 0-127 (with early-return guards for
/// higher values). Compound assignment tokens (128+) are handled outside
/// those tables via `compound_assign_op()`.
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

    // === Compound Assignment (128-139) ===
    PlusEq = 128,     // +=
    MinusEq = 129,    // -=
    StarEq = 130,     // *=
    SlashEq = 131,    // /=
    PercentEq = 132,  // %=
    AtEq = 133,       // @=
    AmpEq = 134,      // &=
    PipeEq = 135,     // |=
    CaretEq = 136,    // ^=
    ShlEq = 137,      // <<=
    AmpAmpEq = 138,   // &&=
    PipePipeEq = 139, // ||=
}

// TokenTag is repr(u8), so all discriminants fit in 0..255 by construction.
// TokenSet uses [u128; 2] (256 bits) to cover the full range.
// OPER_TABLE[128] and POSTFIX_BITSET only cover 0-127 with early-return
// guards for higher values — compound assignment tokens (128+) are handled
// outside those tables via compound_assign_op().

impl TokenTag {
    /// Maximum discriminant value across all variants.
    ///
    /// Must be < 256 for `TokenSet` ([u128; 2] bitset = 256 bits).
    /// Update this when adding variants with higher discriminants.
    pub const MAX_DISCRIMINANT: u8 = Self::PipePipeEq as u8;

    /// Get a human-readable name for this tag.
    #[expect(clippy::too_many_lines, reason = "exhaustive TokenTag → name dispatch")]
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
            // Compound assignment
            Self::PlusEq => "+=",
            Self::MinusEq => "-=",
            Self::StarEq => "*=",
            Self::SlashEq => "/=",
            Self::PercentEq => "%=",
            Self::AtEq => "@=",
            Self::AmpEq => "&=",
            Self::PipeEq => "|=",
            Self::CaretEq => "^=",
            Self::ShlEq => "<<=",
            Self::AmpAmpEq => "&&=",
            Self::PipePipeEq => "||=",
        }
    }
}
