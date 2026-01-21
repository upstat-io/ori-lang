// Comprehensive tests for the Sigil lexer
//
// Test coverage:
// - All token types (50+)
// - Edge cases (empty input, single chars, invalid input)
// - Property-based tests for identifiers and numbers
// - Whitespace and newline handling
// - Line continuation
// - Comments

#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use proptest::prelude::*;
use test_case::test_case;

// ============================================================================
// Helper Functions
// ============================================================================

fn tokens(source: &str) -> Vec<Token> {
    tokenize(source, "test.si")
        .unwrap()
        .into_iter()
        .map(|t| t.value)
        .collect()
}

fn single_token(source: &str) -> Token {
    let toks = tokens(source);
    assert_eq!(toks.len(), 1, "Expected 1 token, got {:?}", toks);
    toks.into_iter().next().unwrap()
}

fn token_error(source: &str) -> String {
    tokenize(source, "test.si").unwrap_err()
}

// ============================================================================
// Keyword Tests
// ============================================================================

#[test_case("type" => Token::Type; "type keyword")]
#[test_case("pub" => Token::Pub; "pub keyword")]
#[test_case("use" => Token::Use; "use keyword")]
#[test_case("match" => Token::Match; "match keyword")]
#[test_case("if" => Token::If; "if keyword")]
#[test_case("for" => Token::For; "for keyword")]
#[test_case("in" => Token::In; "in keyword")]
#[test_case("true" => Token::True; "true keyword")]
#[test_case("false" => Token::False; "false keyword")]
#[test_case("nil" => Token::Nil; "nil keyword")]
#[test_case("Ok" => Token::Ok_; "Ok keyword")]
#[test_case("Err" => Token::Err_; "Err keyword")]
#[test_case("Some" => Token::Some_; "Some keyword")]
#[test_case("None" => Token::None_; "None keyword")]
#[test_case("tests" => Token::Tests; "tests keyword")]
#[test_case("assert" => Token::Assert; "assert keyword")]
#[test_case("assert_err" => Token::AssertErr; "assert_err keyword")]
fn test_keyword(input: &str) -> Token {
    single_token(input)
}

// ============================================================================
// Type Keyword Tests
// ============================================================================

#[test_case("int" => Token::IntType; "int type")]
#[test_case("float" => Token::FloatType; "float type")]
#[test_case("str" => Token::StrType; "str type")]
#[test_case("bool" => Token::BoolType; "bool type")]
#[test_case("void" => Token::VoidType; "void type")]
#[test_case("Result" => Token::ResultType; "Result type")]
fn test_type_keyword(input: &str) -> Token {
    single_token(input)
}

// ============================================================================
// Symbol Tests
// ============================================================================

#[test_case("@" => Token::At; "at symbol")]
#[test_case("$" => Token::Dollar; "dollar symbol")]
#[test_case("#" => Token::Hash; "hash symbol")]
#[test_case("(" => Token::LParen; "left paren")]
#[test_case(")" => Token::RParen; "right paren")]
#[test_case("{" => Token::LBrace; "left brace")]
#[test_case("}" => Token::RBrace; "right brace")]
#[test_case("[" => Token::LBracket; "left bracket")]
#[test_case("]" => Token::RBracket; "right bracket")]
#[test_case(":" => Token::Colon; "colon")]
#[test_case("::" => Token::DoubleColon; "double colon")]
#[test_case(":=" => Token::ColonEq; "colon eq")]
#[test_case(":then" => Token::ColonThen; "colon then")]
#[test_case(":else" => Token::ColonElse; "colon else")]
#[test_case("," => Token::Comma; "comma")]
#[test_case("." => Token::Dot; "dot")]
#[test_case(".." => Token::DotDot; "dot dot")]
#[test_case("->" => Token::Arrow; "arrow")]
#[test_case("=>" => Token::FatArrow; "fat arrow")]
#[test_case("|" => Token::Pipe; "pipe")]
#[test_case("|>" => Token::PipeArrow; "pipe arrow")]
#[test_case("?" => Token::Question; "question")]
#[test_case("??" => Token::DoubleQuestion; "double question")]
fn test_symbol(input: &str) -> Token {
    single_token(input)
}

// ============================================================================
// Operator Tests
// ============================================================================

#[test_case("=" => Token::Eq; "equals")]
#[test_case("==" => Token::EqEq; "double equals")]
#[test_case("!=" => Token::NotEq; "not equals")]
#[test_case("<" => Token::Lt; "less than")]
#[test_case("<=" => Token::LtEq; "less than equals")]
#[test_case(">" => Token::Gt; "greater than")]
#[test_case(">=" => Token::GtEq; "greater than equals")]
#[test_case("+" => Token::Plus; "plus")]
#[test_case("-" => Token::Minus; "minus")]
#[test_case("*" => Token::Star; "star")]
#[test_case("/" => Token::Slash; "slash")]
#[test_case("%" => Token::Percent; "percent")]
#[test_case("!" => Token::Bang; "bang")]
#[test_case("&&" => Token::And; "and")]
#[test_case("||" => Token::Or; "or")]
#[test_case("div" => Token::Div; "integer division")]
fn test_operator(input: &str) -> Token {
    single_token(input)
}

// ============================================================================
// Integer Literal Tests
// ============================================================================

#[test]
fn test_integer_zero() {
    assert_eq!(single_token("0"), Token::Int(0));
}

#[test]
fn test_integer_positive() {
    assert_eq!(single_token("42"), Token::Int(42));
}

#[test]
fn test_integer_large() {
    assert_eq!(single_token("9223372036854775807"), Token::Int(i64::MAX));
}

#[test]
fn test_integer_with_leading_zeros() {
    // Leading zeros are parsed as a single number
    assert_eq!(single_token("007"), Token::Int(7));
}

#[test_case("1" => Token::Int(1); "single digit")]
#[test_case("10" => Token::Int(10); "two digits")]
#[test_case("123" => Token::Int(123); "three digits")]
#[test_case("999999" => Token::Int(999999); "six digits")]
fn test_integer_values(input: &str) -> Token {
    single_token(input)
}

// ============================================================================
// Float Literal Tests
// ============================================================================

#[test]
#[allow(clippy::approx_constant)] // Testing that source literal "3.14" parses correctly
fn test_float_simple() {
    assert_eq!(single_token("3.14"), Token::Float(3.14));
}

#[test]
fn test_float_zero_decimal() {
    assert_eq!(single_token("0.0"), Token::Float(0.0));
}

#[test]
fn test_float_small() {
    assert_eq!(single_token("0.001"), Token::Float(0.001));
}

#[test_case("1.0" => Token::Float(1.0); "one point zero")]
#[test_case("99.99" => Token::Float(99.99); "ninety nine point ninety nine")]
#[test_case("123.456" => Token::Float(123.456); "multiple decimal places")]
fn test_float_values(input: &str) -> Token {
    single_token(input)
}

// ============================================================================
// String Literal Tests
// ============================================================================

#[test]
fn test_string_empty() {
    assert_eq!(single_token(r#""""#), Token::String("".to_string()));
}

#[test]
fn test_string_simple() {
    assert_eq!(
        single_token(r#""hello""#),
        Token::String("hello".to_string())
    );
}

#[test]
fn test_string_with_spaces() {
    assert_eq!(
        single_token(r#""hello world""#),
        Token::String("hello world".to_string())
    );
}

#[test]
fn test_string_with_numbers() {
    assert_eq!(
        single_token(r#""abc123""#),
        Token::String("abc123".to_string())
    );
}

#[test]
fn test_string_with_escaped_quote() {
    assert_eq!(
        single_token(r#""say \"hi\"""#),
        Token::String(r#"say \"hi\""#.to_string())
    );
}

#[test]
fn test_string_with_newline_escape() {
    assert_eq!(
        single_token(r#""line1\nline2""#),
        Token::String(r#"line1\nline2"#.to_string())
    );
}

// ============================================================================
// Duration Literal Tests
// ============================================================================

#[test_case("24h" => Token::Duration("24h".to_string()); "hours")]
#[test_case("60s" => Token::Duration("60s".to_string()); "seconds")]
#[test_case("30m" => Token::Duration("30m".to_string()); "minutes")]
fn test_duration(input: &str) -> Token {
    single_token(input)
}

#[test]
fn test_duration_ms_parses_as_two_tokens() {
    // Note: "ms" is not a supported duration unit, so "100ms" parses as "100m" + "s"
    let toks = tokens("100ms");
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0], Token::Duration("100m".to_string()));
    assert_eq!(toks[1], Token::Ident("s".to_string()));
}

// ============================================================================
// Identifier Tests
// ============================================================================

#[test]
fn test_identifier_simple() {
    assert_eq!(single_token("foo"), Token::Ident("foo".to_string()));
}

#[test]
fn test_identifier_with_underscore() {
    assert_eq!(single_token("my_var"), Token::Ident("my_var".to_string()));
}

#[test]
fn test_identifier_starts_with_underscore() {
    assert_eq!(
        single_token("_private"),
        Token::Ident("_private".to_string())
    );
}

#[test]
fn test_identifier_with_numbers() {
    assert_eq!(single_token("var1"), Token::Ident("var1".to_string()));
}

#[test]
fn test_identifier_uppercase() {
    assert_eq!(single_token("MyType"), Token::Ident("MyType".to_string()));
}

#[test]
fn test_identifier_all_caps() {
    assert_eq!(
        single_token("CONSTANT"),
        Token::Ident("CONSTANT".to_string())
    );
}

// ============================================================================
// Newline and Whitespace Tests
// ============================================================================

#[test]
fn test_newline_token() {
    let toks = tokens("a\nb");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[1], Token::Newline);
}

#[test]
fn test_multiple_newlines() {
    let toks = tokens("a\n\n\nb");
    // Each newline is a separate token
    assert!(toks.contains(&Token::Newline));
}

#[test]
fn test_whitespace_stripped() {
    let toks = tokens("  a  b  ");
    // Whitespace is skipped, only identifiers remain
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0], Token::Ident("a".to_string()));
    assert_eq!(toks[1], Token::Ident("b".to_string()));
}

#[test]
fn test_tabs_stripped() {
    let toks = tokens("\ta\t\tb");
    assert_eq!(toks.len(), 2);
}

// ============================================================================
// Line Continuation Tests
// ============================================================================

#[test]
fn test_line_continuation_simple() {
    let toks = tokens("a _\nb");
    // Line continuation should be skipped
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0], Token::Ident("a".to_string()));
    assert_eq!(toks[1], Token::Ident("b".to_string()));
}

#[test]
fn test_line_continuation_with_spaces() {
    let toks = tokens("a _  \nb");
    assert_eq!(toks.len(), 2);
}

// ============================================================================
// Comment Tests
// ============================================================================

#[test]
fn test_comment_skipped() {
    let toks = tokens("a // comment\nb");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0], Token::Ident("a".to_string()));
    assert_eq!(toks[1], Token::Newline);
    assert_eq!(toks[2], Token::Ident("b".to_string()));
}

#[test]
fn test_comment_at_end() {
    let toks = tokens("a // comment");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0], Token::Ident("a".to_string()));
}

#[test]
fn test_comment_only() {
    let toks = tokens("// just a comment");
    assert_eq!(toks.len(), 0);
}

// ============================================================================
// Complex Expression Tests
// ============================================================================

#[test]
fn test_function_definition() {
    let toks = tokens("@hello (name: str) -> str");
    assert!(toks.contains(&Token::At));
    assert!(toks.contains(&Token::Ident("hello".to_string())));
    assert!(toks.contains(&Token::LParen));
    assert!(toks.contains(&Token::Ident("name".to_string())));
    assert!(toks.contains(&Token::Colon));
    assert!(toks.contains(&Token::StrType));
    assert!(toks.contains(&Token::RParen));
    assert!(toks.contains(&Token::Arrow));
}

#[test]
fn test_config_definition() {
    let toks = tokens("$timeout = 5000");
    assert!(toks.contains(&Token::Dollar));
    assert!(toks.contains(&Token::Ident("timeout".to_string())));
    assert!(toks.contains(&Token::Eq));
    assert!(toks.contains(&Token::Int(5000)));
}

#[test]
fn test_conditional_expression() {
    let toks = tokens("if x > 0 :then true :else false");
    assert!(toks.contains(&Token::If));
    assert!(toks.contains(&Token::Gt));
    assert!(toks.contains(&Token::ColonThen));
    assert!(toks.contains(&Token::True));
    assert!(toks.contains(&Token::ColonElse));
    assert!(toks.contains(&Token::False));
}

#[test]
fn test_list_literal() {
    let toks = tokens("[1, 2, 3]");
    assert!(toks.contains(&Token::LBracket));
    assert!(toks.contains(&Token::Int(1)));
    assert!(toks.contains(&Token::Comma));
    assert!(toks.contains(&Token::Int(2)));
    assert!(toks.contains(&Token::Int(3)));
    assert!(toks.contains(&Token::RBracket));
}

#[test]
fn test_method_call() {
    let toks = tokens("arr.push(x)");
    assert!(toks.contains(&Token::Ident("arr".to_string())));
    assert!(toks.contains(&Token::Dot));
    assert!(toks.contains(&Token::Ident("push".to_string())));
    assert!(toks.contains(&Token::LParen));
    assert!(toks.contains(&Token::Ident("x".to_string())));
    assert!(toks.contains(&Token::RParen));
}

#[test]
fn test_range_expression() {
    let toks = tokens("1..10");
    assert!(toks.contains(&Token::Int(1)));
    assert!(toks.contains(&Token::DotDot));
    assert!(toks.contains(&Token::Int(10)));
}

#[test]
fn test_result_type() {
    let toks = tokens("Result<int, str>");
    assert!(toks.contains(&Token::ResultType));
    assert!(toks.contains(&Token::Lt));
    assert!(toks.contains(&Token::IntType));
    assert!(toks.contains(&Token::Comma));
    assert!(toks.contains(&Token::StrType));
    assert!(toks.contains(&Token::Gt));
}

#[test]
fn test_optional_type() {
    let toks = tokens("?int");
    assert!(toks.contains(&Token::Question));
    assert!(toks.contains(&Token::IntType));
}

// ============================================================================
// Error Tests
// ============================================================================

#[test]
fn test_error_unterminated_string() {
    let err = token_error(r#""unterminated"#);
    assert!(err.contains("Unexpected character"));
}

#[test]
fn test_error_invalid_character() {
    let err = token_error("abc`def");
    assert!(err.contains("Unexpected character"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_input() {
    let toks = tokens("");
    assert!(toks.is_empty());
}

#[test]
fn test_only_whitespace() {
    let toks = tokens("   \t\t   ");
    assert!(toks.is_empty());
}

#[test]
fn test_adjacent_operators() {
    let toks = tokens("a==b");
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[1], Token::EqEq);
}

#[test]
fn test_negative_number_as_tokens() {
    // -1 is actually Minus followed by Int(1)
    let toks = tokens("-1");
    assert_eq!(toks.len(), 2);
    assert_eq!(toks[0], Token::Minus);
    assert_eq!(toks[1], Token::Int(1));
}

// ============================================================================
// Span Tests
// ============================================================================

#[test]
fn test_spans_are_correct() {
    let result = tokenize("abc def", "test.si").unwrap();
    assert_eq!(result[0].span, 0..3);
    assert_eq!(result[1].span, 4..7);
}

#[test]
fn test_string_span_includes_quotes() {
    let result = tokenize(r#""hello""#, "test.si").unwrap();
    assert_eq!(result[0].span, 0..7); // Includes quotes
}

// ============================================================================
// Property-Based Tests
// ============================================================================

proptest! {
    #[test]
    fn test_valid_identifiers(s in "[a-zA-Z_][a-zA-Z0-9_]*") {
        let result = tokenize(&s, "test.si");
        // Should either be a valid identifier or a keyword
        prop_assert!(result.is_ok());
        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn test_positive_integers(n in 0i64..1_000_000i64) {
        let s = n.to_string();
        let result = tokenize(&s, "test.si");
        prop_assert!(result.is_ok());
        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
        prop_assert_eq!(tokens[0].value.clone(), Token::Int(n));
    }

    #[test]
    fn test_floats(n in 0.0f64..1000.0f64) {
        // Format with one decimal place to ensure valid float literal
        let s = format!("{:.1}", n);
        let result = tokenize(&s, "test.si");
        prop_assert!(result.is_ok());
        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
        match &tokens[0].value {
            Token::Float(_) => (),
            other => prop_assert!(false, "Expected Float, got {:?}", other),
        }
    }
}

// ============================================================================
// Regression Tests
// ============================================================================

#[test]
fn test_double_colon_not_two_colons() {
    let toks = tokens("::");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0], Token::DoubleColon);
}

#[test]
fn test_fat_arrow_not_eq_gt() {
    let toks = tokens("=>");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0], Token::FatArrow);
}

#[test]
fn test_pipe_arrow_not_pipe_gt() {
    let toks = tokens("|>");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0], Token::PipeArrow);
}

#[test]
fn test_double_question_not_two_questions() {
    let toks = tokens("??");
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0], Token::DoubleQuestion);
}
