use super::*;

#[test]
fn test_name_layout() {
    let name = Name::new(5, 1000);
    assert_eq!(name.shard(), 5);
    assert_eq!(name.local(), 1000);
}

#[test]
fn test_name_empty() {
    assert_eq!(Name::EMPTY.shard(), 0);
    assert_eq!(Name::EMPTY.local(), 0);
}

#[test]
fn test_name_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Name::new(0, 1));
    set.insert(Name::new(0, 1)); // duplicate
    set.insert(Name::new(0, 2));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_name_ord() {
    let a = Name::new(0, 1);
    let b = Name::new(0, 2);
    assert!(a < b);
}
