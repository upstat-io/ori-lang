//! Raw token tag and token type for the low-level tokenizer.
//!
//! `RawTag` is the standalone token kind produced by the raw scanner.
//! It has no `ori_*` dependencies and maps to `ori_ir::TokenKind` in the
//! integration layer (`ori_lexer`).
//!
//! # Discriminant Layout
//!
//! Variants are organized into semantic ranges with gaps for future expansion:
//!
//! | Range   | Category              |
//! |---------|-----------------------|
//! | 0-15    | Identifiers & Literals|
//! | 16-20   | Template Literals     |
//! | 32-61   | Operators             |
//! | 80-95   | Delimiters            |
//! | 112-114 | Trivia                |
//! | 240-245 | Errors                |
//! | 255     | EOF                   |

/// Raw token kind produced by the low-level tokenizer.
///
/// This is the standalone equivalent of `ori_ir::TokenKind`, with no compiler
/// dependencies. The integration layer (`ori_lexer`) maps `RawTag` to
/// `TokenKind` during the "cooking" phase.
///
/// # Stability
///
/// This enum is `#[non_exhaustive]` — new variants may be added in future
/// versions without breaking downstream code. Match arms should include a
/// wildcard (`_`) to handle unknown variants.
///
/// # Representation
///
/// `#[repr(u8)]` ensures each variant is a single byte, enabling compact
/// storage and efficient tag-based dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u8)]
pub enum RawTag {
    // === Identifiers & Literals (0-15) ===
    /// Identifier (not yet classified as keyword — resolution happens in cooking layer).
    Ident = 0,
    /// Integer literal (decimal).
    Int = 1,
    /// Float literal.
    Float = 2,
    /// Hexadecimal integer literal (`0x...`).
    HexInt = 3,
    /// String literal (double-quoted).
    String = 4,
    /// Character literal (single-quoted).
    Char = 5,
    /// Duration literal (e.g., `100ms`, `0.5s`).
    Duration = 6,
    /// Size literal (e.g., `1kb`, `1.5mb`).
    Size = 7,
    /// Binary integer literal (`0b...`).
    BinInt = 8,

    // === Template Literals (16-19) ===
    /// Template head: `` `text{ `` (opening backtick to first unescaped `{`).
    TemplateHead = 16,
    /// Template middle: `}text{` (between interpolations).
    TemplateMiddle = 17,
    /// Template tail: `` }text` `` (last `}` to closing backtick).
    TemplateTail = 18,
    /// Complete template: `` `text` `` (no interpolation).
    TemplateComplete = 19,
    /// Format spec inside template interpolation: `{value:>10.2f}` → `>10.2f`
    ///
    /// Emitted when `:` appears at interpolation top-level (no unclosed parens,
    /// brackets, or braces). Covers everything between `:` (exclusive) and `}`
    /// (exclusive). The `}` is NOT consumed — it triggers `template_middle_or_tail`.
    FormatSpec = 20,

    // === Operators (32-61) ===
    /// `+`
    Plus = 32,
    /// `-`
    Minus = 33,
    /// `*`
    Star = 34,
    /// `/`
    Slash = 35,
    /// `%`
    Percent = 36,
    /// `^`
    Caret = 37,
    /// `&`
    Ampersand = 38,
    /// `|`
    Pipe = 39,
    /// `~`
    Tilde = 40,
    /// `!`
    Bang = 41,
    /// `=`
    Equal = 42,
    /// `<`
    Less = 43,
    /// `>`
    Greater = 44,
    /// `.`
    Dot = 45,
    /// `?`
    Question = 46,
    // 47 reserved

    // Compound operators
    /// `==`
    EqualEqual = 48,
    /// `!=`
    BangEqual = 49,
    /// `<=`
    LessEqual = 50,
    // 51 reserved (no GreaterEqual — parser synthesizes from adjacent `>` `=`)
    /// `&&`
    AmpersandAmpersand = 52,
    /// `||`
    PipePipe = 53,
    /// `->`
    Arrow = 54,
    /// `=>`
    FatArrow = 55,
    /// `..`
    DotDot = 56,
    /// `..=`
    DotDotEqual = 57,
    /// `...`
    DotDotDot = 58,
    /// `::`
    ColonColon = 59,
    /// `<<`
    Shl = 60,
    /// `??`
    QuestionQuestion = 61,

    // === Delimiters (80-95) ===
    /// `(`
    LeftParen = 80,
    /// `)`
    RightParen = 81,
    /// `[`
    LeftBracket = 82,
    /// `]`
    RightBracket = 83,
    /// `{`
    LeftBrace = 84,
    /// `}`
    RightBrace = 85,
    /// `,`
    Comma = 86,
    /// `:`
    Colon = 87,
    /// `;` — error-detection token (semicolons are not valid Ori syntax).
    Semicolon = 88,
    /// `@`
    At = 89,
    /// `#`
    Hash = 90,
    /// `_` (standalone underscore, not part of an identifier).
    Underscore = 91,
    /// `\` — error-detection token (backslash only valid inside escape sequences).
    Backslash = 92,
    /// `$`
    Dollar = 93,
    /// `#[` (attribute prefix).
    HashBracket = 94,
    /// `#!` (file attribute prefix).
    HashBang = 95,

    // === Trivia (112-114) ===
    /// Horizontal whitespace (spaces, tabs).
    Whitespace = 112,
    /// Line feed (`\n`) or CRLF (`\r\n`).
    Newline = 113,
    /// Line comment (`//` to end of line).
    LineComment = 114,

    // === Errors (240-245) ===
    /// Invalid byte (non-ASCII, control character).
    InvalidByte = 240,
    /// Unterminated string literal (missing closing `"`).
    UnterminatedString = 241,
    /// Unterminated character literal (missing closing `'`).
    UnterminatedChar = 242,
    /// Invalid escape sequence.
    ///
    /// Currently unused by the raw scanner — escape validation is fully deferred
    /// to the cooking layer's `unescape_*_v2()` functions. Reserved for potential
    /// future scanner-level escape validation. The cooker has a defensive match
    /// arm for this variant.
    InvalidEscape = 243,
    /// Unterminated template literal (missing closing `` ` ``).
    UnterminatedTemplate = 244,
    /// Interior null byte (U+0000) in source content.
    ///
    /// Emitted by the scanner when it encounters a `0x00` byte that is NOT the
    /// sentinel (i.e., `pos < source_len`). The integration layer (`ori_lexer`)
    /// skips these tokens because `SourceBuffer` already detected interior nulls
    /// via `encoding_issues()` and reported them with more specific diagnostics.
    InteriorNull = 245,

    // === Control (255) ===
    /// End of file (sentinel reached).
    Eof = 255,
}

impl RawTag {
    /// Returns the fixed lexeme for this tag, if it has one.
    ///
    /// Operators and delimiters have fixed lexemes. Identifiers, literals,
    /// and error tokens return `None` (their text varies).
    #[must_use]
    pub fn lexeme(self) -> Option<&'static str> {
        match self {
            Self::Plus => Some("+"),
            Self::Minus => Some("-"),
            Self::Star => Some("*"),
            Self::Slash => Some("/"),
            Self::Percent => Some("%"),
            Self::Caret => Some("^"),
            Self::Ampersand => Some("&"),
            Self::Pipe => Some("|"),
            Self::Tilde => Some("~"),
            Self::Bang => Some("!"),
            Self::Equal => Some("="),
            Self::Less => Some("<"),
            Self::Greater => Some(">"),
            Self::Dot => Some("."),
            Self::Question => Some("?"),
            Self::EqualEqual => Some("=="),
            Self::BangEqual => Some("!="),
            Self::LessEqual => Some("<="),
            Self::AmpersandAmpersand => Some("&&"),
            Self::PipePipe => Some("||"),
            Self::Arrow => Some("->"),
            Self::FatArrow => Some("=>"),
            Self::DotDot => Some(".."),
            Self::DotDotEqual => Some("..="),
            Self::DotDotDot => Some("..."),
            Self::ColonColon => Some("::"),
            Self::Shl => Some("<<"),
            Self::QuestionQuestion => Some("??"),
            Self::LeftParen => Some("("),
            Self::RightParen => Some(")"),
            Self::LeftBracket => Some("["),
            Self::RightBracket => Some("]"),
            Self::LeftBrace => Some("{"),
            Self::RightBrace => Some("}"),
            Self::Comma => Some(","),
            Self::Colon => Some(":"),
            Self::Semicolon => Some(";"),
            Self::At => Some("@"),
            Self::Hash => Some("#"),
            Self::Underscore => Some("_"),
            Self::Backslash => Some("\\"),
            Self::Dollar => Some("$"),
            Self::HashBracket => Some("#["),
            Self::HashBang => Some("#!"),
            Self::Newline => Some("\n"),
            _ => None,
        }
    }

    /// Returns a human-readable name for this tag.
    ///
    /// Used in diagnostic messages and debugging output.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ident => "identifier",
            Self::Int => "integer literal",
            Self::Float => "float literal",
            Self::HexInt => "hex integer literal",
            Self::BinInt => "binary integer literal",
            Self::String => "string literal",
            Self::Char => "character literal",
            Self::Duration => "duration literal",
            Self::Size => "size literal",
            Self::TemplateHead => "template head",
            Self::TemplateMiddle => "template middle",
            Self::TemplateTail => "template tail",
            Self::TemplateComplete => "template literal",
            Self::FormatSpec => "format spec",
            Self::Plus => "`+`",
            Self::Minus => "`-`",
            Self::Star => "`*`",
            Self::Slash => "`/`",
            Self::Percent => "`%`",
            Self::Caret => "`^`",
            Self::Ampersand => "`&`",
            Self::Pipe => "`|`",
            Self::Tilde => "`~`",
            Self::Bang => "`!`",
            Self::Equal => "`=`",
            Self::Less => "`<`",
            Self::Greater => "`>`",
            Self::Dot => "`.`",
            Self::Question => "`?`",
            Self::EqualEqual => "`==`",
            Self::BangEqual => "`!=`",
            Self::LessEqual => "`<=`",
            Self::AmpersandAmpersand => "`&&`",
            Self::PipePipe => "`||`",
            Self::Arrow => "`->`",
            Self::FatArrow => "`=>`",
            Self::DotDot => "`..`",
            Self::DotDotEqual => "`..=`",
            Self::DotDotDot => "`...`",
            Self::ColonColon => "`::`",
            Self::Shl => "`<<`",
            Self::QuestionQuestion => "`??`",
            Self::LeftParen => "`(`",
            Self::RightParen => "`)`",
            Self::LeftBracket => "`[`",
            Self::RightBracket => "`]`",
            Self::LeftBrace => "`{`",
            Self::RightBrace => "`}`",
            Self::Comma => "`,`",
            Self::Colon => "`:`",
            Self::Semicolon => "`;`",
            Self::At => "`@`",
            Self::Hash => "`#`",
            Self::Underscore => "`_`",
            Self::Backslash => "`\\`",
            Self::Dollar => "`$`",
            Self::HashBracket => "`#[`",
            Self::HashBang => "`#!`",
            Self::Whitespace => "whitespace",
            Self::Newline => "newline",
            Self::LineComment => "line comment",
            Self::InvalidByte => "invalid byte",
            Self::UnterminatedString => "unterminated string",
            Self::UnterminatedChar => "unterminated character literal",
            Self::InvalidEscape => "invalid escape",
            Self::UnterminatedTemplate => "unterminated template",
            Self::InteriorNull => "interior null byte",
            Self::Eof => "end of file",
        }
    }

    /// Returns `true` if this tag represents trivia (whitespace, comments).
    ///
    /// Newlines are NOT trivia — they are significant as implicit statement
    /// separators in Ori.
    #[must_use]
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::LineComment)
    }
}

/// Raw token produced by the low-level tokenizer.
///
/// A lightweight pair of tag and byte length. The integration layer
/// (`ori_lexer`) uses the length to compute spans and extract source slices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawToken {
    /// What kind of token this is.
    pub tag: RawTag,
    /// Length of the token in bytes.
    pub len: u32,
}

/// Size assertions: `RawTag` is 1 byte, `RawToken` is 8 bytes.
const _: () = assert!(std::mem::size_of::<RawTag>() == 1);
const _: () = assert!(std::mem::size_of::<RawToken>() == 8);

#[cfg(test)]
mod tests {
    use super::*;

    // === RawTag discriminants ===

    #[test]
    fn repr_u8_semantic_ranges() {
        // Identifiers & Literals: 0-15
        assert_eq!(RawTag::Ident as u8, 0);
        assert_eq!(RawTag::Int as u8, 1);
        assert_eq!(RawTag::Float as u8, 2);
        assert_eq!(RawTag::HexInt as u8, 3);
        assert_eq!(RawTag::String as u8, 4);
        assert_eq!(RawTag::Char as u8, 5);
        assert_eq!(RawTag::Duration as u8, 6);
        assert_eq!(RawTag::Size as u8, 7);
        assert_eq!(RawTag::BinInt as u8, 8);

        // Template Literals: 16-20
        assert_eq!(RawTag::TemplateHead as u8, 16);
        assert_eq!(RawTag::TemplateMiddle as u8, 17);
        assert_eq!(RawTag::TemplateTail as u8, 18);
        assert_eq!(RawTag::TemplateComplete as u8, 19);
        assert_eq!(RawTag::FormatSpec as u8, 20);

        // Operators: 32-61
        assert_eq!(RawTag::Plus as u8, 32);
        assert_eq!(RawTag::QuestionQuestion as u8, 61);

        // Delimiters: 80-95
        assert_eq!(RawTag::LeftParen as u8, 80);
        assert_eq!(RawTag::HashBang as u8, 95);

        // Trivia: 112-114
        assert_eq!(RawTag::Whitespace as u8, 112);
        assert_eq!(RawTag::Newline as u8, 113);
        assert_eq!(RawTag::LineComment as u8, 114);

        // Errors: 240-245
        assert_eq!(RawTag::InvalidByte as u8, 240);
        assert_eq!(RawTag::UnterminatedTemplate as u8, 244);
        assert_eq!(RawTag::InteriorNull as u8, 245);

        // Control: 255
        assert_eq!(RawTag::Eof as u8, 255);
    }

    #[test]
    fn tag_is_one_byte() {
        assert_eq!(std::mem::size_of::<RawTag>(), 1);
    }

    // === Lexeme ===

    #[test]
    fn fixed_lexeme_single_char_operators() {
        assert_eq!(RawTag::Plus.lexeme(), Some("+"));
        assert_eq!(RawTag::Minus.lexeme(), Some("-"));
        assert_eq!(RawTag::Star.lexeme(), Some("*"));
        assert_eq!(RawTag::Slash.lexeme(), Some("/"));
        assert_eq!(RawTag::Percent.lexeme(), Some("%"));
        assert_eq!(RawTag::Caret.lexeme(), Some("^"));
        assert_eq!(RawTag::Ampersand.lexeme(), Some("&"));
        assert_eq!(RawTag::Pipe.lexeme(), Some("|"));
        assert_eq!(RawTag::Tilde.lexeme(), Some("~"));
        assert_eq!(RawTag::Bang.lexeme(), Some("!"));
        assert_eq!(RawTag::Equal.lexeme(), Some("="));
        assert_eq!(RawTag::Less.lexeme(), Some("<"));
        assert_eq!(RawTag::Greater.lexeme(), Some(">"));
        assert_eq!(RawTag::Dot.lexeme(), Some("."));
        assert_eq!(RawTag::Question.lexeme(), Some("?"));
    }

    #[test]
    fn fixed_lexeme_compound_operators() {
        assert_eq!(RawTag::Arrow.lexeme(), Some("->"));
        assert_eq!(RawTag::FatArrow.lexeme(), Some("=>"));
        assert_eq!(RawTag::DotDot.lexeme(), Some(".."));
        assert_eq!(RawTag::DotDotEqual.lexeme(), Some("..="));
        assert_eq!(RawTag::DotDotDot.lexeme(), Some("..."));
        assert_eq!(RawTag::EqualEqual.lexeme(), Some("=="));
        assert_eq!(RawTag::BangEqual.lexeme(), Some("!="));
        assert_eq!(RawTag::LessEqual.lexeme(), Some("<="));
        assert_eq!(RawTag::AmpersandAmpersand.lexeme(), Some("&&"));
        assert_eq!(RawTag::PipePipe.lexeme(), Some("||"));
        assert_eq!(RawTag::QuestionQuestion.lexeme(), Some("??"));
        assert_eq!(RawTag::ColonColon.lexeme(), Some("::"));
        assert_eq!(RawTag::Shl.lexeme(), Some("<<"));
    }

    #[test]
    fn fixed_lexeme_delimiters() {
        assert_eq!(RawTag::LeftParen.lexeme(), Some("("));
        assert_eq!(RawTag::RightParen.lexeme(), Some(")"));
        assert_eq!(RawTag::LeftBracket.lexeme(), Some("["));
        assert_eq!(RawTag::RightBracket.lexeme(), Some("]"));
        assert_eq!(RawTag::LeftBrace.lexeme(), Some("{"));
        assert_eq!(RawTag::RightBrace.lexeme(), Some("}"));
        assert_eq!(RawTag::Comma.lexeme(), Some(","));
        assert_eq!(RawTag::Colon.lexeme(), Some(":"));
        assert_eq!(RawTag::Semicolon.lexeme(), Some(";"));
        assert_eq!(RawTag::At.lexeme(), Some("@"));
        assert_eq!(RawTag::Hash.lexeme(), Some("#"));
        assert_eq!(RawTag::Underscore.lexeme(), Some("_"));
        assert_eq!(RawTag::Backslash.lexeme(), Some("\\"));
        assert_eq!(RawTag::Dollar.lexeme(), Some("$"));
        assert_eq!(RawTag::HashBracket.lexeme(), Some("#["));
        assert_eq!(RawTag::HashBang.lexeme(), Some("#!"));
    }

    #[test]
    fn variable_lexeme_returns_none() {
        assert_eq!(RawTag::Ident.lexeme(), None);
        assert_eq!(RawTag::Int.lexeme(), None);
        assert_eq!(RawTag::Float.lexeme(), None);
        assert_eq!(RawTag::HexInt.lexeme(), None);
        assert_eq!(RawTag::BinInt.lexeme(), None);
        assert_eq!(RawTag::String.lexeme(), None);
        assert_eq!(RawTag::Char.lexeme(), None);
        assert_eq!(RawTag::Duration.lexeme(), None);
        assert_eq!(RawTag::Size.lexeme(), None);
        assert_eq!(RawTag::TemplateHead.lexeme(), None);
        assert_eq!(RawTag::TemplateComplete.lexeme(), None);
        assert_eq!(RawTag::FormatSpec.lexeme(), None);
        assert_eq!(RawTag::InvalidByte.lexeme(), None);
        assert_eq!(RawTag::InteriorNull.lexeme(), None);
        assert_eq!(RawTag::Whitespace.lexeme(), None);
        assert_eq!(RawTag::Eof.lexeme(), None);
    }

    // === Name ===

    #[test]
    fn name_returns_readable_description() {
        assert_eq!(RawTag::Ident.name(), "identifier");
        assert_eq!(RawTag::Int.name(), "integer literal");
        assert_eq!(RawTag::Float.name(), "float literal");
        assert_eq!(RawTag::HexInt.name(), "hex integer literal");
        assert_eq!(RawTag::BinInt.name(), "binary integer literal");
        assert_eq!(RawTag::Duration.name(), "duration literal");
        assert_eq!(RawTag::Size.name(), "size literal");
        assert_eq!(RawTag::TemplateHead.name(), "template head");
        assert_eq!(RawTag::TemplateComplete.name(), "template literal");
        assert_eq!(RawTag::FormatSpec.name(), "format spec");
        assert_eq!(RawTag::Plus.name(), "`+`");
        assert_eq!(RawTag::Arrow.name(), "`->`");
        assert_eq!(RawTag::Shl.name(), "`<<`");
        assert_eq!(RawTag::HashBracket.name(), "`#[`");
        assert_eq!(RawTag::Eof.name(), "end of file");
        assert_eq!(RawTag::InvalidByte.name(), "invalid byte");
        assert_eq!(RawTag::InteriorNull.name(), "interior null byte");
        assert_eq!(RawTag::UnterminatedString.name(), "unterminated string");
    }

    // === Trivia ===

    #[test]
    fn trivia_classification() {
        assert!(RawTag::Whitespace.is_trivia());
        assert!(RawTag::LineComment.is_trivia());

        // Newlines are NOT trivia in Ori (they're significant)
        assert!(!RawTag::Newline.is_trivia());
        assert!(!RawTag::Ident.is_trivia());
        assert!(!RawTag::Eof.is_trivia());
    }

    // === RawToken ===

    #[test]
    fn raw_token_construction() {
        let tok = RawToken {
            tag: RawTag::Ident,
            len: 5,
        };
        assert_eq!(tok.tag, RawTag::Ident);
        assert_eq!(tok.len, 5);
    }

    #[test]
    fn raw_token_is_copy() {
        let tok = RawToken {
            tag: RawTag::Plus,
            len: 1,
        };
        let tok2 = tok; // Copy
        assert_eq!(tok, tok2);
    }
}
