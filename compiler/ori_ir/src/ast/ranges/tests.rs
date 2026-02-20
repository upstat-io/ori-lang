use super::*;
use std::collections::HashSet;

// Test that all range types work correctly using ParamRange as representative
#[test]
fn test_range_empty_constant() {
    assert_eq!(ParamRange::EMPTY.start, 0);
    assert_eq!(ParamRange::EMPTY.len, 0);
    assert!(ParamRange::EMPTY.is_empty());
    assert_eq!(ParamRange::EMPTY.len(), 0);

    // Also test other range types have EMPTY
    assert!(GenericParamRange::EMPTY.is_empty());
    assert!(ArmRange::EMPTY.is_empty());
    assert!(MapEntryRange::EMPTY.is_empty());
    assert!(MapElementRange::EMPTY.is_empty());
    assert!(FieldInitRange::EMPTY.is_empty());
    assert!(StructLitFieldRange::EMPTY.is_empty());
    assert!(NamedExprRange::EMPTY.is_empty());
    assert!(CallArgRange::EMPTY.is_empty());
}

#[test]
fn test_range_new() {
    let range = ParamRange::new(10, 5);
    assert_eq!(range.start, 10);
    assert_eq!(range.len, 5);
    assert_eq!(range.len(), 5);
    assert!(!range.is_empty());
}

#[test]
fn test_range_len_conversion() {
    // Test that len() correctly converts u16 to usize
    let range = ParamRange::new(0, u16::MAX);
    assert_eq!(range.len(), u16::MAX as usize);
}

#[test]
fn test_range_debug_format() {
    let range = ParamRange::new(5, 3);
    let debug = format!("{range:?}");
    assert_eq!(debug, "ParamRange(5..8)");

    let arm_range = ArmRange::new(10, 2);
    let debug = format!("{arm_range:?}");
    assert_eq!(debug, "ArmRange(10..12)");
}

#[test]
fn test_range_debug_format_empty() {
    let empty = ParamRange::EMPTY;
    let debug = format!("{empty:?}");
    assert_eq!(debug, "ParamRange(0..0)");
}

#[test]
fn test_range_hash_in_hashset() {
    let mut set = HashSet::new();

    let r1 = ParamRange::new(0, 5);
    let r2 = ParamRange::new(0, 5); // same as r1
    let r3 = ParamRange::new(0, 6); // different len
    let r4 = ParamRange::new(1, 5); // different start

    set.insert(r1);
    set.insert(r2); // duplicate, should not increase size
    set.insert(r3);
    set.insert(r4);

    assert_eq!(set.len(), 3);
    assert!(set.contains(&ParamRange::new(0, 5)));
    assert!(set.contains(&ParamRange::new(0, 6)));
    assert!(set.contains(&ParamRange::new(1, 5)));
}

#[test]
fn test_range_eq() {
    let r1 = ParamRange::new(10, 20);
    let r2 = ParamRange::new(10, 20);
    let r3 = ParamRange::new(10, 21);

    assert_eq!(r1, r2);
    assert_ne!(r1, r3);
}

#[test]
#[allow(
    clippy::clone_on_copy,
    reason = "Intentionally testing Clone trait impl"
)]
fn test_range_copy_clone() {
    let original = ParamRange::new(5, 10);
    let copied = original; // Copy
    let cloned = original.clone(); // Clone

    assert_eq!(original, copied);
    assert_eq!(original, cloned);
}

#[test]
fn test_range_default() {
    let default: ParamRange = ParamRange::default();
    assert_eq!(default, ParamRange::EMPTY);
}
