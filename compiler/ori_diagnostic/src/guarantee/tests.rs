use super::*;

#[test]
fn from_error_count_returns_some_for_nonzero() {
    assert!(ErrorGuaranteed::from_error_count(1).is_some());
    assert!(ErrorGuaranteed::from_error_count(100).is_some());
}

#[test]
fn from_error_count_returns_none_for_zero() {
    assert!(ErrorGuaranteed::from_error_count(0).is_none());
}

#[test]
fn display_shows_error_message() {
    let g = ErrorGuaranteed::from_error_count(1).unwrap();
    assert_eq!(g.to_string(), "error(s) emitted");
}

#[test]
fn error_guaranteed_is_copy() {
    let g1 = ErrorGuaranteed::from_error_count(1).unwrap();
    let g2 = g1; // Copy
    let _ = g1; // Still usable after copy
    let _ = g2;
}

#[test]
fn error_guaranteed_is_eq() {
    let g1 = ErrorGuaranteed::from_error_count(1).unwrap();
    let g2 = ErrorGuaranteed::from_error_count(1).unwrap();
    assert_eq!(g1, g2);
}
