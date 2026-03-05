//! Display, keyword, and formatting methods for `TokenKind`.
//!
//! Provides human-readable names and keyword-to-string mappings used by
//! the parser (error messages), formatter (spacing), and debug output.

use std::fmt;

use super::kind::TokenKind;

impl TokenKind {
    /// If this token is a keyword, return its string representation.
    ///
    /// Returns `None` for non-keyword tokens (identifiers, literals,
    /// operators, delimiters). Used to allow keywords as member names
    /// after `.` (e.g., `ordering.then(other: Less)`).
    pub fn keyword_str(&self) -> Option<&'static str> {
        match self {
            // Reserved keywords
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
            // Compound assignment
            TokenKind::PlusEq => "+=",
            TokenKind::MinusEq => "-=",
            TokenKind::StarEq => "*=",
            TokenKind::SlashEq => "/=",
            TokenKind::PercentEq => "%=",
            TokenKind::AtEq => "@=",
            TokenKind::AmpEq => "&=",
            TokenKind::PipeEq => "|=",
            TokenKind::CaretEq => "^=",
            TokenKind::ShlEq => "<<=",
            TokenKind::AmpAmpEq => "&&=",
            TokenKind::PipePipeEq => "||=",
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

            // Keywords — reserved (12-39)
            // 11: removed (was async)
            12 => Some("break"),
            13 => Some("continue"),
            14 => Some("return"),
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
            // 26 was "mut" — removed
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

            // Special (121-127): Newline, Error, Eof — internal, excluded

            // Compound assignment (128-139)
            128 => Some("+="),
            129 => Some("-="),
            130 => Some("*="),
            131 => Some("/="),
            132 => Some("%="),
            133 => Some("@="),
            134 => Some("&="),
            135 => Some("|="),
            136 => Some("^="),
            137 => Some("<<="),
            138 => Some("&&="),
            139 => Some("||="),

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
