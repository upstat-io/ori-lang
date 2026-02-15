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
