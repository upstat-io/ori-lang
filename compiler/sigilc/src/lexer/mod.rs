use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r]+")]  // Skip whitespace (but not newlines)
pub enum Token {
    // Comments
    #[regex(r"//[^\n]*", logos::skip)]
    Comment,

    // Newlines (significant for statement separation)
    #[token("\n")]
    Newline,

    // Line continuation (_ at end of line continues to next line)
    #[regex(r"_[ \t]*\n", logos::skip)]
    LineContinuation,

    // Keywords
    #[token("type")]
    Type,
    #[token("pub")]
    Pub,
    #[token("use")]
    Use,
    #[token("match")]
    Match,
    #[token("if")]
    If,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("nil")]
    Nil,
    #[token("Ok")]
    Ok_,
    #[token("Err")]
    Err_,
    #[token("Some")]
    Some_,
    #[token("None")]
    None_,

    // Testing
    #[token("tests")]
    Tests,
    #[token("assert")]
    Assert,
    #[token("assert_err")]
    AssertErr,

    // Types
    #[token("int")]
    IntType,
    #[token("float")]
    FloatType,
    #[token("str")]
    StrType,
    #[token("bool")]
    BoolType,
    #[token("void")]
    VoidType,
    #[token("Result")]
    ResultType,

    // Symbols
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
    #[token(":")]
    Colon,
    #[token("::")]
    DoubleColon,
    #[token(":=")]
    ColonEq,
    #[token(":then")]
    ColonThen,
    #[token(":else")]
    ColonElse,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("|")]
    Pipe,
    #[token("|>")]
    PipeArrow,
    #[token("?")]
    Question,
    #[token("??")]
    DoubleQuestion,

    // Operators
    #[token("=")]
    Eq,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token("<=")]
    LtEq,
    #[token(">")]
    Gt,
    #[token(">=")]
    GtEq,
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
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("div")]
    Div,

    // Literals
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    String(String),

    // Duration literals (e.g., 24h, 100ms)
    #[regex(r"[0-9]+[hms]", |lex| Some(lex.slice().to_string()))]
    Duration(String),

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| Some(lex.slice().to_string()))]
    Ident(String),
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: std::ops::Range<usize>,
}

pub type SpannedToken = Spanned<Token>;

pub fn tokenize(source: &str, _filename: &str) -> Result<Vec<SpannedToken>, String> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(source);

    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => {
                tokens.push(Spanned {
                    value: token,
                    span: lexer.span(),
                });
            }
            Err(_) => {
                let span = lexer.span();
                return Err(format!(
                    "Unexpected character at position {}: '{}'",
                    span.start,
                    &source[span.clone()]
                ));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let source = r#"
            $config = 5
            @hello (name: str) -> str
        "#;

        let tokens = tokenize(source, "test.al").unwrap();
        let token_types: Vec<_> = tokens.iter().map(|t| &t.value).collect();

        assert!(token_types.contains(&&Token::Dollar));
        assert!(token_types.contains(&&Token::At));
        assert!(token_types.contains(&&Token::Arrow));
    }

    #[test]
    fn test_string_literal() {
        let source = r#""hello world""#;
        let tokens = tokenize(source, "test.al").unwrap();

        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0].value, Token::String(s) if s == "hello world"));
    }
}
