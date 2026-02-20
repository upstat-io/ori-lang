use ori_diagnostic::queue::DiagnosticSeverity;
use ori_diagnostic::{Applicability, ErrorCode};
use ori_ir::{Span, TokenKind};

use super::details::{CodeSuggestion, ExtraLabel, ParseErrorDetails};
use super::*;

#[test]
fn test_unexpected_token_message() {
    let kind = ParseErrorKind::UnexpectedToken {
        found: TokenKind::Semicolon,
        expected: "expression",
        context: Some("function body"),
    };
    assert_eq!(
        kind.message(),
        "expected expression, found `;` in function body"
    );
    assert!(kind.hint().is_some());
}

#[test]
fn test_expected_expression_message() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::RParen,
        position: ExprPosition::CallArgument,
    };
    assert_eq!(
        kind.message(),
        "expected expression in function call, found `)`"
    );
}

#[test]
fn test_pattern_error_message() {
    let kind = ParseErrorKind::PatternArgumentError {
        pattern_name: "cache",
        reason: PatternArgError::Missing { name: "key" },
    };
    assert_eq!(kind.message(), "cache requires `key:` argument");
}

#[test]
fn test_unsupported_keyword_hint() {
    let kind = ParseErrorKind::UnsupportedKeyword {
        keyword: TokenKind::Return,
        reason: "Ori is expression-based",
    };
    assert!(kind.hint().is_some());
    assert!(kind.hint().unwrap().contains("last expression"));
}

#[test]
fn test_error_code_mapping() {
    assert_eq!(
        ParseErrorKind::UnexpectedToken {
            found: TokenKind::Plus,
            expected: "identifier",
            context: None
        }
        .error_code(),
        ErrorCode::E1001
    );
    assert_eq!(
        ParseErrorKind::ExpectedExpression {
            found: TokenKind::Eof,
            position: ExprPosition::Primary
        }
        .error_code(),
        ErrorCode::E1002
    );
    assert_eq!(
        ParseErrorKind::ExpectedIdentifier {
            found: TokenKind::Plus,
            context: IdentContext::FunctionName
        }
        .error_code(),
        ErrorCode::E1004
    );
}

#[test]
fn test_from_kind() {
    let kind = ParseErrorKind::UnexpectedToken {
        found: TokenKind::Semicolon,
        expected: "expression",
        context: None,
    };
    let error = ParseError::from_kind(&kind, Span::new(0, 1));

    assert_eq!(error.code, ErrorCode::E1001);
    assert!(error.message.contains("expected expression"));
    assert!(!error.help.is_empty()); // Has hint about semicolons
}

#[test]
fn test_title() {
    assert_eq!(
        ParseErrorKind::UnexpectedToken {
            found: TokenKind::Plus,
            expected: "identifier",
            context: None
        }
        .title(),
        "UNEXPECTED TOKEN"
    );
    assert_eq!(
        ParseErrorKind::UnclosedDelimiter {
            open: TokenKind::LParen,
            open_span: Span::DUMMY,
            expected_close: TokenKind::RParen
        }
        .title(),
        "UNCLOSED DELIMITER"
    );
}

#[test]
fn test_empathetic_unexpected_token() {
    let kind = ParseErrorKind::UnexpectedToken {
        found: TokenKind::Semicolon,
        expected: "an expression",
        context: Some("function body"),
    };
    let msg = kind.empathetic_message();

    // Check for empathetic phrasing
    assert!(msg.contains("I ran into"));
    assert!(msg.contains("while parsing function body"));
    assert!(msg.contains("I was expecting"));
    assert!(msg.contains("`;\u{60}")); // backtick-semicolon-backtick
}

#[test]
fn test_empathetic_expected_expression() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::Plus,
        position: ExprPosition::Operand,
    };
    let msg = kind.empathetic_message();

    assert!(msg.contains("I was expecting an expression"));
    assert!(msg.contains("after this operator"));
    assert!(msg.contains("Expressions include"));
}

#[test]
fn test_empathetic_trailing_operator() {
    let kind = ParseErrorKind::TrailingOperator {
        operator: TokenKind::Plus,
    };
    let msg = kind.empathetic_message();

    assert!(msg.contains("without a right-hand side"));
    assert!(msg.contains("a + b"));
}

#[test]
fn test_empathetic_unclosed_delimiter() {
    let kind = ParseErrorKind::UnclosedDelimiter {
        open: TokenKind::LParen,
        open_span: Span::new(10, 11),
        expected_close: TokenKind::RParen,
    };
    let msg = kind.empathetic_message();

    assert!(msg.contains("unclosed `(`"));
    assert!(msg.contains("matching `)`"));
}

#[test]
fn test_empathetic_expected_declaration() {
    let kind = ParseErrorKind::ExpectedDeclaration {
        found: TokenKind::Plus,
    };
    let msg = kind.empathetic_message();

    assert!(msg.contains("I was expecting a declaration"));
    assert!(msg.contains("Functions:"));
    assert!(msg.contains("Types:"));
    assert!(msg.contains("Imports:"));
}

#[test]
fn test_empathetic_unsupported_keyword() {
    let kind = ParseErrorKind::UnsupportedKeyword {
        keyword: TokenKind::Return,
        reason: "Ori is expression-based",
    };
    let msg = kind.empathetic_message();

    assert!(msg.contains("`return` isn't supported"));
    assert!(msg.contains("Ori is expression-based"));
}

// === Common Mistake Detection Tests ===

#[test]
fn test_detect_triple_equals() {
    let (desc, help) = detect_common_mistake("===").unwrap();
    assert_eq!(desc, "triple equals");
    assert!(help.contains("=="));
    assert!(help.contains("statically typed"));
}

#[test]
fn test_detect_increment_operator() {
    let (desc, help) = detect_common_mistake("++").unwrap();
    assert_eq!(desc, "increment operator");
    assert!(help.contains("x = x + 1"));
}

#[test]
fn test_detect_decrement_operator() {
    let (desc, help) = detect_common_mistake("--").unwrap();
    assert_eq!(desc, "decrement operator");
    assert!(help.contains("x = x - 1"));
}

#[test]
fn test_detect_compound_assignment() {
    for op in &["+=", "-=", "*=", "/=", "%="] {
        let result = detect_common_mistake(op);
        assert!(result.is_some(), "Should detect {op}");
        let (desc, help) = result.unwrap();
        assert_eq!(desc, "compound assignment");
        assert!(help.contains("x = x"));
    }
}

#[test]
fn test_detect_class_keyword() {
    let (desc, help) = check_common_keyword_mistake("class").unwrap();
    assert_eq!(desc, "class keyword");
    assert!(help.contains("type"));
    assert!(help.contains("trait"));
}

#[test]
fn test_detect_switch_keyword() {
    let (desc, help) = check_common_keyword_mistake("switch").unwrap();
    assert_eq!(desc, "switch keyword");
    assert!(help.contains("match"));
}

#[test]
fn test_detect_function_keyword() {
    for keyword in &["function", "func", "fn"] {
        let result = check_common_keyword_mistake(keyword);
        assert!(result.is_some(), "Should detect {keyword}");
        let (desc, help) = result.unwrap();
        assert_eq!(desc, "function keyword");
        assert!(help.contains('@'));
    }
}

#[test]
fn test_detect_null_variants() {
    for keyword in &["null", "nil", "NULL"] {
        let result = check_common_keyword_mistake(keyword);
        assert!(result.is_some(), "Should detect {keyword}");
        let (_, help) = result.unwrap();
        assert!(help.contains("None"));
    }
}

#[test]
fn test_detect_string_type() {
    let (desc, help) = check_common_keyword_mistake("String").unwrap();
    assert_eq!(desc, "string type");
    assert!(help.contains("str"));
}

#[test]
fn test_detect_boolean_case() {
    let (desc, help) = check_common_keyword_mistake("True").unwrap();
    assert_eq!(desc, "boolean literal");
    assert!(help.contains("true"));
    assert!(help.contains("false"));
}

#[test]
fn test_valid_tokens_not_detected() {
    // These should NOT be detected as mistakes (they're valid in Ori)
    assert!(detect_common_mistake("??").is_none());
    assert!(detect_common_mistake("=>").is_none());
    assert!(check_common_keyword_mistake("int").is_none());
    assert!(check_common_keyword_mistake("float").is_none());
    assert!(check_common_keyword_mistake("str").is_none());
}

// === Educational Note Tests ===

#[test]
fn test_educational_note_conditional() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::RBrace,
        position: ExprPosition::Conditional,
    };
    let note = kind.educational_note();
    assert!(note.is_some());
    assert!(note.unwrap().contains("expression"));
    assert!(note.unwrap().contains("same type"));
}

#[test]
fn test_educational_note_match_arm() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::Comma,
        position: ExprPosition::MatchArm,
    };
    let note = kind.educational_note();
    assert!(note.is_some());
    assert!(note.unwrap().contains("match"));
}

#[test]
fn test_educational_note_let_pattern() {
    let kind = ParseErrorKind::InvalidPattern {
        found: TokenKind::Plus,
        context: PatternContext::Let,
    };
    let note = kind.educational_note();
    assert!(note.is_some());
    assert!(note.unwrap().contains("destructuring"));
}

#[test]
fn test_educational_note_unclosed_brace() {
    let kind = ParseErrorKind::UnclosedDelimiter {
        open: TokenKind::LBrace,
        open_span: Span::DUMMY,
        expected_close: TokenKind::RBrace,
    };
    let note = kind.educational_note();
    assert!(note.is_some());
    assert!(note.unwrap().contains("blocks"));
}

#[test]
fn test_educational_note_unclosed_bracket() {
    let kind = ParseErrorKind::UnclosedDelimiter {
        open: TokenKind::LBracket,
        open_span: Span::DUMMY,
        expected_close: TokenKind::RBracket,
    };
    let note = kind.educational_note();
    assert!(note.is_some());
    assert!(note.unwrap().contains("list"));
}

// === From Error Token Tests ===

#[test]
fn test_from_error_token_with_known_mistake() {
    let error = ParseError::from_error_token(Span::new(0, 3), "===");
    assert!(error.message.contains("triple equals"));
    assert!(!error.help.is_empty());
    assert!(error.help[0].contains("=="));
}

#[test]
fn test_from_error_token_with_unknown() {
    let error = ParseError::from_error_token(Span::new(0, 3), "xyz");
    assert!(error.message.contains("unrecognized token"));
    assert!(error.help.is_empty());
}

// === Enhanced Hint Tests ===

#[test]
fn test_enhanced_hint_semicolon() {
    let kind = ParseErrorKind::UnexpectedToken {
        found: TokenKind::Semicolon,
        expected: "expression",
        context: None,
    };
    let hint = kind.hint().unwrap();
    assert!(hint.contains("Semicolons"));
    assert!(hint.contains("block expressions"));
}

#[test]
fn test_enhanced_hint_trailing_star() {
    let kind = ParseErrorKind::TrailingOperator {
        operator: TokenKind::Star,
    };
    let hint = kind.hint().unwrap();
    assert!(hint.contains('*'));
    assert!(hint.contains("both sides"));
}

#[test]
fn test_enhanced_hint_empty_block() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::RBrace,
        position: ExprPosition::Primary,
    };
    let hint = kind.hint().unwrap();
    assert!(hint.contains("void"));
}

// === Integration: from_kind with educational notes ===

#[test]
fn test_from_kind_includes_educational_note() {
    let kind = ParseErrorKind::InvalidPattern {
        found: TokenKind::Plus,
        context: PatternContext::Match,
    };
    let error = ParseError::from_kind(&kind, Span::new(0, 1));

    // Should have both hint (if any) and educational note
    // For InvalidPattern in Match context, we have an educational note
    assert!(
        !error.help.is_empty(),
        "Should have at least educational note"
    );
    let combined_help = error.help.join(" ");
    assert!(
        combined_help.contains("pattern"),
        "Help should mention patterns"
    );
}

#[test]
fn test_from_kind_includes_hint_and_educational() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::RBrace,
        position: ExprPosition::Conditional,
    };
    let error = ParseError::from_kind(&kind, Span::new(0, 1));

    // Should have both hint (for empty block) and educational note (for conditional)
    assert!(!error.help.is_empty(), "Should have help messages");
}

// === ErrorContext Tests ===

#[test]
fn test_error_context_description() {
    assert_eq!(ErrorContext::IfExpression.description(), "an if expression");
    assert_eq!(
        ErrorContext::MatchExpression.description(),
        "a match expression"
    );
    assert_eq!(
        ErrorContext::FunctionDef.description(),
        "a function definition"
    );
    assert_eq!(ErrorContext::Pattern.description(), "a pattern");
}

#[test]
fn test_error_context_label() {
    assert_eq!(ErrorContext::IfExpression.label(), "if expression");
    assert_eq!(ErrorContext::MatchExpression.label(), "match expression");
    assert_eq!(ErrorContext::FunctionDef.label(), "function definition");
    assert_eq!(ErrorContext::Pattern.label(), "pattern");
}

#[test]
fn test_error_context_all_variants_have_description() {
    // Ensure all variants have non-empty descriptions
    let contexts = [
        ErrorContext::Module,
        ErrorContext::FunctionDef,
        ErrorContext::TypeDef,
        ErrorContext::TraitDef,
        ErrorContext::ImplBlock,
        ErrorContext::UseStatement,
        ErrorContext::ExternBlock,
        ErrorContext::Expression,
        ErrorContext::IfExpression,
        ErrorContext::MatchExpression,
        ErrorContext::ForLoop,
        ErrorContext::WhileLoop,
        ErrorContext::Block,
        ErrorContext::Closure,
        ErrorContext::FunctionCall,
        ErrorContext::MethodCall,
        ErrorContext::ListLiteral,
        ErrorContext::MapLiteral,
        ErrorContext::StructLiteral,
        ErrorContext::IndexExpression,
        ErrorContext::BinaryOp,
        ErrorContext::FieldAccess,
        ErrorContext::Pattern,
        ErrorContext::MatchArm,
        ErrorContext::LetPattern,
        ErrorContext::FunctionParams,
        ErrorContext::TypeAnnotation,
        ErrorContext::GenericParams,
        ErrorContext::FunctionSignature,
        ErrorContext::Attribute,
        ErrorContext::TestDef,
        ErrorContext::Contract,
    ];

    for ctx in &contexts {
        let desc = ctx.description();
        assert!(
            !desc.is_empty(),
            "Description for {ctx:?} should not be empty"
        );
        // Descriptions should read naturally after "while parsing"
        // e.g., "while parsing an if expression" or "while parsing function parameters"
        assert!(
            desc.starts_with("a ")
                || desc.starts_with("an ")
                || !desc.contains(' ')
                || desc.ends_with('s'),
            "Description for {ctx:?} should be grammatically correct: {desc}"
        );

        let label = ctx.label();
        assert!(!label.is_empty(), "Label for {ctx:?} should not be empty");
    }
}

// === ParseErrorDetails Tests ===

#[test]
fn test_details_unexpected_token() {
    let kind = ParseErrorKind::UnexpectedToken {
        found: TokenKind::Semicolon,
        expected: "an expression",
        context: Some("function body"),
    };
    let details = kind.details(Span::new(10, 1));

    assert_eq!(details.title, "UNEXPECTED TOKEN");
    assert!(details.text.contains("I ran into"));
    assert!(details.text.contains("function body"));
    assert!(details.label_text.contains("expected"));
    assert!(details.hint.is_some()); // Has semicolon hint
    assert_eq!(details.error_code, ErrorCode::E1001);
}

#[test]
fn test_details_unclosed_delimiter() {
    let kind = ParseErrorKind::UnclosedDelimiter {
        open: TokenKind::LParen,
        open_span: Span::new(5, 1),
        expected_close: TokenKind::RParen,
    };
    let details = kind.details(Span::new(20, 0));

    assert_eq!(details.title, "UNCLOSED DELIMITER");
    assert!(details.text.contains("unclosed"));
    assert!(!details.extra_labels.is_empty());
    assert!(details.extra_labels[0].text.contains("opened here"));
    assert!(details.suggestion.is_some());
    assert_eq!(
        details.suggestion.as_ref().unwrap().applicability,
        Applicability::MachineApplicable
    );
}

#[test]
fn test_details_expected_expression() {
    let kind = ParseErrorKind::ExpectedExpression {
        found: TokenKind::RBrace,
        position: ExprPosition::Conditional,
    };
    let details = kind.details(Span::new(15, 1));

    assert_eq!(details.title, "EXPECTED EXPRESSION");
    assert!(details.text.contains("condition"));
    assert!(details.hint.is_some()); // Has educational note or hint
}

#[test]
fn test_details_trailing_operator() {
    let kind = ParseErrorKind::TrailingOperator {
        operator: TokenKind::Plus,
    };
    let details = kind.details(Span::new(8, 1));

    assert_eq!(details.title, "INCOMPLETE EXPRESSION");
    assert!(details.text.contains("right-hand side"));
    assert!(details.label_text.contains("needs a right operand"));
}

#[test]
fn test_details_pattern_error_missing() {
    let kind = ParseErrorKind::PatternArgumentError {
        pattern_name: "cache",
        reason: PatternArgError::Missing { name: "key" },
    };
    let details = kind.details(Span::new(0, 5));

    assert_eq!(details.title, "PATTERN ERROR");
    assert!(details.text.contains("cache"));
    assert!(details.text.contains("key"));
    assert!(details.label_text.contains("missing"));
}

#[test]
fn test_details_unexpected_eof_with_unclosed() {
    let kind = ParseErrorKind::UnexpectedEof {
        expected: "expression",
        unclosed: Some((TokenKind::LBrace, Span::new(2, 1))),
    };
    let details = kind.details(Span::new(50, 0));

    assert_eq!(details.title, "UNEXPECTED END OF FILE");
    assert!(details.text.contains("closing"));
    assert!(!details.extra_labels.is_empty());
    assert!(details.extra_labels[0].text.contains("opened"));
}

// === CodeSuggestion Tests ===

#[test]
fn test_code_suggestion_machine_applicable() {
    let suggestion =
        CodeSuggestion::machine_applicable(Span::new(10, 3), "==", "Replace `===` with `==`");
    assert_eq!(suggestion.replacement, "==");
    assert_eq!(suggestion.applicability, Applicability::MachineApplicable);
}

#[test]
fn test_code_suggestion_with_placeholders() {
    let suggestion =
        CodeSuggestion::with_placeholders(Span::new(10, 0), ": ???", "Add type annotation");
    assert_eq!(suggestion.applicability, Applicability::HasPlaceholders);
}

// === ExtraLabel Tests ===

#[test]
fn test_extra_label_same_file() {
    let label = ExtraLabel::same_file(Span::new(5, 1), "opened here");
    assert!(label.src_info.is_none());
    assert_eq!(label.text, "opened here");
}

#[test]
fn test_extra_label_cross_file() {
    let label = ExtraLabel::cross_file(
        Span::new(10, 5),
        "src/lib.ori",
        "fn foo() { }",
        "defined here",
    );
    assert!(label.src_info.is_some());
    let info = label.src_info.unwrap();
    assert_eq!(info.path, "src/lib.ori");
    assert!(info.content.contains("foo"));
}

// === ParseErrorDetails Builder Tests ===

#[test]
fn test_parse_error_details_builder() {
    let details = ParseErrorDetails::new(
        "TEST ERROR",
        "Test explanation",
        "test label",
        ErrorCode::E1001,
    )
    .with_hint("Try this fix")
    .with_extra_label(ExtraLabel::same_file(Span::new(0, 1), "related"))
    .with_suggestion(CodeSuggestion::machine_applicable(
        Span::new(5, 2),
        "fix",
        "Apply fix",
    ));

    assert_eq!(details.title, "TEST ERROR");
    assert!(details.has_extra_context());
    assert!(details.hint.is_some());
    assert!(!details.extra_labels.is_empty());
    assert!(details.suggestion.is_some());
}

#[test]
fn test_parse_error_details_has_extra_context() {
    let basic = ParseErrorDetails::new("TEST", "text", "label", ErrorCode::E1001);
    assert!(!basic.has_extra_context());

    let with_hint = basic.clone().with_hint("hint");
    assert!(with_hint.has_extra_context());
}

// === Integration: details() generates complete information ===

#[test]
fn test_details_all_variants_produce_output() {
    // Ensure all error variants produce valid details
    let variants: Vec<ParseErrorKind> = vec![
        ParseErrorKind::UnexpectedToken {
            found: TokenKind::Plus,
            expected: "identifier",
            context: None,
        },
        ParseErrorKind::UnexpectedEof {
            expected: "expression",
            unclosed: None,
        },
        ParseErrorKind::ExpectedExpression {
            found: TokenKind::Plus,
            position: ExprPosition::Primary,
        },
        ParseErrorKind::TrailingOperator {
            operator: TokenKind::Star,
        },
        ParseErrorKind::ExpectedDeclaration {
            found: TokenKind::Plus,
        },
        ParseErrorKind::ExpectedIdentifier {
            found: TokenKind::Plus,
            context: IdentContext::FunctionName,
        },
        ParseErrorKind::InvalidFunctionClause {
            reason: "test reason",
        },
        ParseErrorKind::InvalidPattern {
            found: TokenKind::Plus,
            context: PatternContext::Match,
        },
        ParseErrorKind::PatternArgumentError {
            pattern_name: "test",
            reason: PatternArgError::Missing { name: "arg" },
        },
        ParseErrorKind::ExpectedType {
            found: TokenKind::Plus,
        },
        ParseErrorKind::UnclosedDelimiter {
            open: TokenKind::LBrace,
            open_span: Span::new(0, 1),
            expected_close: TokenKind::RBrace,
        },
        ParseErrorKind::InvalidAttribute {
            reason: "test reason",
        },
        ParseErrorKind::UnsupportedKeyword {
            keyword: TokenKind::Return,
            reason: "test reason",
        },
    ];

    for kind in &variants {
        let details = kind.details(Span::new(0, 1));
        assert!(
            !details.title.is_empty(),
            "Title should not be empty for {kind:?}"
        );
        assert!(
            !details.text.is_empty(),
            "Text should not be empty for {kind:?}"
        );
        assert!(
            !details.label_text.is_empty(),
            "Label text should not be empty for {kind:?}"
        );
    }
}

// === Diagnostic Conversion Tests ===

#[test]
fn test_parse_error_details_to_diagnostic() {
    let details = ParseErrorDetails::new(
        "UNEXPECTED TOKEN",
        "I ran into something unexpected",
        "expected expression",
        ErrorCode::E1001,
    )
    .with_hint("Try removing this");

    let diag = details.to_diagnostic(Span::new(10, 20));

    assert_eq!(diag.code, ErrorCode::E1001);
    assert!(diag.message.contains("I ran into"));
    assert_eq!(diag.labels.len(), 1);
    assert!(diag.labels[0].is_primary);
    assert_eq!(diag.labels[0].span, Span::new(10, 20));
    assert!(diag.labels[0].message.contains("expected expression"));
    assert!(!diag.suggestions.is_empty());
}

#[test]
fn test_parse_error_details_to_diagnostic_with_extra_labels() {
    let details = ParseErrorDetails::new(
        "UNCLOSED DELIMITER",
        "I found an unclosed `{`",
        "expected `}` here",
        ErrorCode::E1003,
    )
    .with_extra_label(ExtraLabel::same_file(
        Span::new(0, 1),
        "the `{` was opened here",
    ));

    let diag = details.to_diagnostic(Span::new(50, 50));

    assert_eq!(diag.labels.len(), 2);
    assert!(diag.labels[0].is_primary);
    assert!(!diag.labels[1].is_primary);
    assert_eq!(diag.labels[1].span, Span::new(0, 1));
    assert!(diag.labels[1].message.contains("opened here"));
}

#[test]
fn test_parse_error_details_to_diagnostic_cross_file() {
    let details = ParseErrorDetails::new(
        "TYPE MISMATCH",
        "Expected `int`, found `str`",
        "this expression is `str`",
        ErrorCode::E2001,
    )
    .with_extra_label(ExtraLabel::cross_file(
        Span::new(0, 19),
        "src/lib.ori",
        "@get_name () -> str",
        "return type defined here",
    ));

    let diag = details.to_diagnostic(Span::new(100, 110));

    assert_eq!(diag.labels.len(), 2);
    // Primary label should not be cross-file
    assert!(!diag.labels[0].is_cross_file());
    // Secondary label should be cross-file
    assert!(diag.labels[1].is_cross_file());
    assert_eq!(
        diag.labels[1].source_info.as_ref().unwrap().path,
        "src/lib.ori"
    );
}

#[test]
fn test_parse_error_details_to_diagnostic_with_suggestion() {
    let details = ParseErrorDetails::new(
        "SYNTAX ERROR",
        "Use `==` for equality",
        "found `===`",
        ErrorCode::E1001,
    )
    .with_suggestion(CodeSuggestion::machine_applicable(
        Span::new(5, 8),
        "==",
        "Replace `===` with `==`",
    ));

    let diag = details.to_diagnostic(Span::new(5, 8));

    assert!(!diag.structured_suggestions.is_empty());
    let suggestion = &diag.structured_suggestions[0];
    assert_eq!(suggestion.substitutions[0].snippet, "==");
    assert!(suggestion.applicability.is_machine_applicable());
}

// === Severity Tests ===

#[test]
fn test_new_produces_hard_severity() {
    let error = ParseError::new(ErrorCode::E1001, "test error", Span::new(0, 1));
    assert_eq!(error.severity, DiagnosticSeverity::Hard);
}

#[test]
fn test_from_expected_tokens_produces_soft_severity() {
    let ts = crate::TokenSet::new().with(TokenKind::Ident(ori_ir::Name::EMPTY));
    let error = ParseError::from_expected_tokens(&ts, 0);
    assert_eq!(error.severity, DiagnosticSeverity::Soft);
}

#[test]
fn test_from_expected_tokens_with_context_produces_hard_severity() {
    let ts = crate::TokenSet::new().with(TokenKind::Ident(ori_ir::Name::EMPTY));
    let error = ParseError::from_expected_tokens_with_context(&ts, 0, "if expression");
    assert_eq!(error.severity, DiagnosticSeverity::Hard);
}

#[test]
fn test_from_kind_produces_hard_severity() {
    let kind = ParseErrorKind::UnexpectedToken {
        found: TokenKind::Semicolon,
        expected: "expression",
        context: None,
    };
    let error = ParseError::from_kind(&kind, Span::new(0, 1));
    assert_eq!(error.severity, DiagnosticSeverity::Hard);
}

#[test]
fn test_from_error_token_produces_hard_severity() {
    let error = ParseError::from_error_token(Span::new(0, 3), "===");
    assert_eq!(error.severity, DiagnosticSeverity::Hard);
}

#[test]
fn test_as_soft_changes_severity() {
    let error = ParseError::new(ErrorCode::E1001, "test", Span::new(0, 1)).as_soft();
    assert_eq!(error.severity, DiagnosticSeverity::Soft);
}
