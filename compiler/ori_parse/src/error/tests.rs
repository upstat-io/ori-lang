use ori_diagnostic::queue::DiagnosticSeverity;
use ori_diagnostic::ErrorCode;
use ori_ir::{Span, TokenKind};

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

// Educational Note Tests

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

// Enhanced Hint Tests

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

// Integration: from_kind with educational notes

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

// ErrorContext Tests

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

// Severity Tests

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
fn test_as_soft_changes_severity() {
    let error = ParseError::new(ErrorCode::E1001, "test", Span::new(0, 1)).as_soft();
    assert_eq!(error.severity, DiagnosticSeverity::Soft);
}
