use super::*;

#[test]
fn default_is_boxed() {
    assert_eq!(ValueCategory::default(), ValueCategory::Boxed);
}

#[test]
fn predicates_work() {
    assert!(ValueCategory::Boxed.is_boxed());
    assert!(!ValueCategory::Boxed.is_inline());
    assert!(!ValueCategory::Boxed.is_view());

    assert!(!ValueCategory::Inline.is_boxed());
    assert!(ValueCategory::Inline.is_inline());
    assert!(!ValueCategory::Inline.is_view());

    assert!(!ValueCategory::View.is_boxed());
    assert!(!ValueCategory::View.is_inline());
    assert!(ValueCategory::View.is_view());
}

#[test]
fn display_names() {
    assert_eq!(ValueCategory::Boxed.to_string(), "boxed");
    assert_eq!(ValueCategory::Inline.to_string(), "inline");
    assert_eq!(ValueCategory::View.to_string(), "view");
}

#[test]
fn size_is_1_byte() {
    assert_eq!(std::mem::size_of::<ValueCategory>(), 1);
}

#[test]
fn equality_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ValueCategory::Boxed);
    set.insert(ValueCategory::Inline);
    set.insert(ValueCategory::View);
    set.insert(ValueCategory::Boxed); // duplicate
    assert_eq!(set.len(), 3);
}
