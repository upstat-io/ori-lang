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

#[test]
fn test_e2032_e2033_registered() {
    assert!(ErrorDocs::has_docs(ErrorCode::E2032));
    assert!(ErrorDocs::has_docs(ErrorCode::E2033));
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

/// Documentation completeness guard: every registered code must have an
/// embedded explanation. The failure lists every missing code in one run.
#[test]
fn test_every_error_code_has_documentation() {
    let undocumented: Vec<_> = ErrorCode::ALL
        .iter()
        .filter(|code| !ErrorDocs::has_docs(**code))
        .collect();

    assert!(
        undocumented.is_empty(),
        "every error code needs an embedded explanation; missing: {:?}",
        undocumented.iter().map(|c| c.as_str()).collect::<Vec<_>>()
    );
}

/// Independent count clamp: the documentation registry and the error-code
/// registry must cover the same number of entries.
#[test]
fn test_undocumented_codes_inventory() {
    assert_eq!(
        ErrorDocs::all_codes().count(),
        ErrorCode::ALL.len(),
        "documentation registry must contain one entry for every error code"
    );
}

fn heading_matches_code(code: ErrorCode, documentation: &str) -> bool {
    documentation.starts_with(&format!("# {}:", code.as_str()))
}

#[test]
fn test_documentation_headings_match_registered_codes() {
    for (code, documentation) in DOCS {
        assert!(
            heading_matches_code(*code, documentation),
            "{} documentation must start with `# {}:`",
            code.as_str(),
            code.as_str()
        );
    }
}

/// Negative pin: a documentation file headed by another stable code must not
/// satisfy the registry's structural contract.
#[test]
fn test_documentation_heading_rejects_different_code() {
    assert!(!heading_matches_code(
        ErrorCode::E0001,
        "# E0002: Invalid Character"
    ));
}
