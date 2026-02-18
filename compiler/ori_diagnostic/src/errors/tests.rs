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
    assert!(codes.len() >= 51); // We have at least 51 documented codes
}

#[test]
fn test_e2024_through_e2028_registered() {
    assert!(ErrorDocs::has_docs(ErrorCode::E2024));
    assert!(ErrorDocs::has_docs(ErrorCode::E2025));
    assert!(ErrorDocs::has_docs(ErrorCode::E2026));
    assert!(ErrorDocs::has_docs(ErrorCode::E2027));
    assert!(ErrorDocs::has_docs(ErrorCode::E2028));
}
