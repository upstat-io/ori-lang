use super::*;

#[test]
fn test_error_code_display() {
    assert_eq!(ErrorCode::E1001.to_string(), "E1001");
    assert_eq!(ErrorCode::E2001.as_str(), "E2001");
}

#[test]
fn test_arc_error_codes() {
    assert_eq!(ErrorCode::E4001.as_str(), "E4001");
    assert_eq!(ErrorCode::E4002.as_str(), "E4002");
    assert_eq!(ErrorCode::E4003.as_str(), "E4003");

    assert!(ErrorCode::E4001.is_arc_error());
    assert!(ErrorCode::E4002.is_arc_error());
    assert!(ErrorCode::E4003.is_arc_error());

    assert!(!ErrorCode::E4001.is_codegen_error());
    assert!(!ErrorCode::E4001.is_parser_error());
    assert!(!ErrorCode::E4001.is_warning());
}

#[test]
fn test_codegen_error_codes() {
    assert_eq!(ErrorCode::E5001.as_str(), "E5001");
    assert_eq!(ErrorCode::E5005.as_str(), "E5005");
    assert_eq!(ErrorCode::E5009.as_str(), "E5009");

    assert!(ErrorCode::E5001.is_codegen_error());
    assert!(ErrorCode::E5006.is_codegen_error());
    assert!(ErrorCode::E5009.is_codegen_error());

    assert!(!ErrorCode::E5001.is_arc_error());
    assert!(!ErrorCode::E5001.is_parser_error());
    assert!(!ErrorCode::E5001.is_warning());
}

#[test]
fn test_eval_error_codes() {
    assert_eq!(ErrorCode::E6001.as_str(), "E6001");
    assert_eq!(ErrorCode::E6020.as_str(), "E6020");
    assert_eq!(ErrorCode::E6099.as_str(), "E6099");

    assert!(ErrorCode::E6001.is_eval_error());
    assert!(ErrorCode::E6031.is_eval_error());
    assert!(ErrorCode::E6099.is_eval_error());

    assert!(!ErrorCode::E6001.is_arc_error());
    assert!(!ErrorCode::E6001.is_codegen_error());
    assert!(!ErrorCode::E6001.is_parser_error());
    assert!(!ErrorCode::E6001.is_warning());
}

#[test]
fn test_predicate_exclusivity() {
    // Ensure predicates don't overlap
    let all_codes = [
        ErrorCode::E0001,
        ErrorCode::E1001,
        ErrorCode::E2001,
        ErrorCode::E3001,
        ErrorCode::E4001,
        ErrorCode::E5001,
        ErrorCode::E6001,
        ErrorCode::E9001,
        ErrorCode::W1001,
        ErrorCode::W1002,
    ];

    for code in &all_codes {
        let flags = [
            code.is_lexer_error(),
            code.is_parser_error(),
            code.is_type_error(),
            code.is_pattern_error(),
            code.is_arc_error(),
            code.is_codegen_error(),
            code.is_eval_error(),
            code.is_internal_error(),
            code.is_warning(),
        ];
        // Exactly one predicate should be true for every code
        let true_count = flags.iter().filter(|&&f| f).count();
        assert_eq!(
            true_count, 1,
            "expected exactly 1 predicate true for {code}, got {true_count}"
        );
    }
}

#[test]
fn test_new_predicates() {
    assert!(ErrorCode::E0001.is_lexer_error());
    assert!(ErrorCode::E0911.is_lexer_error());
    assert!(!ErrorCode::E0001.is_parser_error());

    assert!(ErrorCode::E2001.is_type_error());
    assert!(ErrorCode::E2018.is_type_error());
    assert!(!ErrorCode::E2001.is_parser_error());

    assert!(ErrorCode::E3001.is_pattern_error());
    assert!(!ErrorCode::E3001.is_type_error());

    assert!(ErrorCode::E9001.is_internal_error());
    assert!(ErrorCode::E9002.is_internal_error());
    assert!(!ErrorCode::E9001.is_eval_error());
}
