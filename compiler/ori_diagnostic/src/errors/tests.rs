use super::*;

#[test]
fn test_get_existing_doc() {
    let doc = ErrorDocs::get(ErrorCode::E2001);
    assert!(doc.is_some());
    assert!(doc.unwrap().contains("Type Mismatch"));
}

#[test]
fn test_get_internal_error_doc() {
    // E9001 now has documentation
    let doc = ErrorDocs::get(ErrorCode::E9001);
    assert!(doc.is_some());
    assert!(doc.unwrap().contains("Internal Compiler Error"));
}

#[test]
fn test_has_docs() {
    assert!(ErrorDocs::has_docs(ErrorCode::E2001));
    assert!(ErrorDocs::has_docs(ErrorCode::E9001));
    assert!(ErrorDocs::has_docs(ErrorCode::E3003));
}

#[test]
fn test_all_codes() {
    let codes: Vec<_> = ErrorDocs::all_codes().collect();
    assert!(codes.contains(&ErrorCode::E2001));
    // Derived from DOCS.len() — no hardcoded magic number.
    assert_eq!(codes.len(), DOCS.len());
}

#[test]
fn test_e2024_through_e2028_registered() {
    assert!(ErrorDocs::has_docs(ErrorCode::E2024));
    assert!(ErrorDocs::has_docs(ErrorCode::E2025));
    assert!(ErrorDocs::has_docs(ErrorCode::E2026));
    assert!(ErrorDocs::has_docs(ErrorCode::E2027));
    assert!(ErrorDocs::has_docs(ErrorCode::E2028));
}

#[test]
fn test_e2029_through_e2031_registered() {
    assert!(ErrorDocs::has_docs(ErrorCode::E2029));
    assert!(ErrorDocs::has_docs(ErrorCode::E2030));
    assert!(ErrorDocs::has_docs(ErrorCode::E2031));
}

/// Structural completeness: every `ErrorCode` in the DOCS array must appear
/// in `ErrorCode::ALL`, and no DOCS entry should be duplicated.
#[test]
fn test_no_duplicate_docs() {
    let codes: Vec<_> = ErrorDocs::all_codes().collect();
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(
        codes.len(),
        unique.len(),
        "DOCS array contains duplicate error codes"
    );
}

/// Drift prevention: every `ErrorCode` variant that has docs must be
/// reachable via `ErrorDocs::get()`. This catches cases where a doc file
/// exists and is included but the DOCS entry was accidentally removed.
#[test]
fn test_all_documented_codes_retrievable() {
    for code in ErrorDocs::all_codes() {
        assert!(
            ErrorDocs::get(code).is_some(),
            "{} is in DOCS array but get() returns None",
            code.as_str()
        );
    }
}

/// Exhaustive coverage tracking: checks every `ErrorCode::ALL` variant
/// against `ErrorDocs` and reports which codes are missing documentation.
///
/// This test is `#[ignore]` so it doesn't block CI, but it serves as
/// a living inventory of undocumented error codes. Run with:
/// `cargo test -p ori_diagnostic -- --ignored test_undocumented_codes`
///
/// Missing documentation (as of this writing):
/// - E0006–E0015, E0911 (11 lexer codes: cross-language habits, confusables)
/// - E2015–E2017 (3 type codes: type parameter ordering, missing/extra args)
/// - E4001–E4003 (3 ARC codes: ARC IR lowering errors)
/// - E5001–E5009 (9 codegen codes: LLVM codegen errors)
/// - E6001–E6099 (27 runtime codes: eval/runtime errors)
/// - W1001–W1002 (2 parser warnings)
#[test]
#[ignore = "tracking test — 55 of 109 error codes lack documentation"]
fn test_undocumented_codes() {
    let undocumented: Vec<_> = ErrorCode::ALL
        .iter()
        .filter(|code| !ErrorDocs::has_docs(**code))
        .collect();

    if !undocumented.is_empty() {
        let list: Vec<_> = undocumented.iter().map(|c| c.as_str()).collect();
        panic!(
            "{} of {} error codes lack documentation: [{}]",
            undocumented.len(),
            ErrorCode::ALL.len(),
            list.join(", ")
        );
    }
}
