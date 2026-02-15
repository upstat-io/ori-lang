use super::*;

#[test]
fn table_is_sorted() {
    for window in FOREIGN_KEYWORDS.windows(2) {
        assert!(
            window[0].0 < window[1].0,
            "table not sorted: {:?} >= {:?}",
            window[0].0,
            window[1].0
        );
    }
}

#[test]
fn lookup_return() {
    let msg = lookup_foreign_keyword("return").unwrap();
    assert!(msg.contains("expression-based"));
}

#[test]
fn lookup_null() {
    let msg = lookup_foreign_keyword("null").unwrap();
    assert!(msg.contains("void"));
}

#[test]
fn lookup_class() {
    let msg = lookup_foreign_keyword("class").unwrap();
    assert!(msg.contains("type"));
}

#[test]
fn lookup_unknown() {
    assert!(lookup_foreign_keyword("foo").is_none());
    assert!(lookup_foreign_keyword("let").is_none()); // Ori keyword, not foreign
}
