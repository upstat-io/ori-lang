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
    assert!(codes.len() >= 35); // We have at least 35 documented errors
}
