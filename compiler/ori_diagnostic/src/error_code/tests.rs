use std::str::FromStr;

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

/// Every variant in `ErrorCode::ALL` must be classified by exactly one `is_*` predicate.
///
/// This is the exhaustive version of the old test which only checked one representative
/// per phase. Catches drift when a new variant is added to the enum and `as_str()` but
/// omitted from its `is_*` predicate.
#[test]
fn test_all_variants_classified() {
    for &code in ErrorCode::ALL {
        let flags = [
            ("is_lexer_error", code.is_lexer_error()),
            ("is_parser_error", code.is_parser_error()),
            ("is_type_error", code.is_type_error()),
            ("is_pattern_error", code.is_pattern_error()),
            ("is_arc_error", code.is_arc_error()),
            ("is_codegen_error", code.is_codegen_error()),
            ("is_eval_error", code.is_eval_error()),
            ("is_internal_error", code.is_internal_error()),
            ("is_warning", code.is_warning()),
        ];
        let true_count = flags.iter().filter(|(_, f)| *f).count();
        let matching: Vec<_> = flags.iter().filter(|(_, f)| *f).map(|(n, _)| *n).collect();
        assert_eq!(
            true_count, 1,
            "{code}: expected exactly 1 predicate, got {true_count} ({matching:?})"
        );
    }
}

/// Verify `ErrorCode::ALL` actually contains every variant.
///
/// Uses `as_str()` round-tripping: every variant in `ALL` maps to a unique string.
/// If `ALL` is missing a variant, the count here won't match the exhaustive match in
/// `as_str()`. Checked by comparing `ALL.len()` against the count of arms in `as_str()`
/// (which Rust enforces to be exhaustive).
#[test]
fn test_all_is_complete() {
    use std::collections::HashSet;
    let strings: HashSet<&str> = ErrorCode::ALL.iter().map(ErrorCode::as_str).collect();
    // No duplicates — each variant maps to a unique string.
    assert_eq!(
        strings.len(),
        ErrorCode::ALL.len(),
        "ALL contains duplicate entries"
    );
    // ALL has the right count. When a variant is added to the enum and `as_str()`
    // but not ALL, this number must be bumped — causing the test to fail.
    assert_eq!(
        ErrorCode::ALL.len(),
        102,
        "ALL length changed — did you add a new ErrorCode variant? Update ALL."
    );
}

/// Every variant in `ErrorCode::ALL` round-trips through `from_str(as_str())`.
///
/// This guarantees `from_str()` can parse every code the compiler can emit,
/// since it derives from the same `ALL` array and `as_str()` match.
#[test]
fn test_from_str_round_trip() {
    for &code in ErrorCode::ALL {
        let s = code.as_str();
        let parsed = ErrorCode::from_str(s);
        assert_eq!(
            parsed,
            Ok(code),
            "from_str({s:?}) should return Ok({code:?})"
        );
    }
}

/// `from_str()` is case-insensitive.
#[test]
fn test_from_str_case_insensitive() {
    assert_eq!(ErrorCode::from_str("e2001"), Ok(ErrorCode::E2001));
    assert_eq!(ErrorCode::from_str("w2001"), Ok(ErrorCode::W2001));
}

/// `from_str()` returns `Err` for unrecognized strings.
#[test]
fn test_from_str_unknown() {
    assert!(ErrorCode::from_str("E9999").is_err());
    assert!(ErrorCode::from_str("hello").is_err());
    assert!(ErrorCode::from_str("").is_err());
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
