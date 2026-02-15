use super::*;

#[test]
fn test_primitive_types() {
    assert!(TypeId::INT.is_primitive());
    assert!(TypeId::FLOAT.is_primitive());
    assert!(TypeId::BOOL.is_primitive());
    assert!(TypeId::STR.is_primitive());
    assert!(TypeId::CHAR.is_primitive());
    assert!(TypeId::BYTE.is_primitive());
    assert!(TypeId::UNIT.is_primitive());
    assert!(TypeId::NEVER.is_primitive());
    assert!(TypeId::ERROR.is_primitive());
    assert!(TypeId::DURATION.is_primitive());
    assert!(TypeId::SIZE.is_primitive());
    assert!(TypeId::ORDERING.is_primitive());
}

#[test]
fn test_markers_not_primitive() {
    // INFER and SELF_TYPE are markers, not primitives
    assert!(!TypeId::INFER.is_primitive());
    assert!(!TypeId::SELF_TYPE.is_primitive());
}

#[test]
fn test_compound_types() {
    let compound = TypeId::from_raw(TypeId::FIRST_COMPOUND);
    assert!(!compound.is_primitive());
}

#[test]
fn test_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(TypeId::INT);
    set.insert(TypeId::INT); // duplicate
    set.insert(TypeId::FLOAT);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_infer() {
    assert!(TypeId::INFER.is_infer());
    assert!(!TypeId::INT.is_infer());
    // INFER no longer overlaps with DURATION
    assert!(!TypeId::DURATION.is_infer());
}

#[test]
fn test_self_type() {
    assert!(TypeId::SELF_TYPE.is_self_type());
    assert!(!TypeId::INT.is_self_type());
    // SELF_TYPE no longer overlaps with SIZE
    assert!(!TypeId::SIZE.is_self_type());
}

#[test]
fn test_void_is_unit_alias() {
    assert_eq!(TypeId::VOID, TypeId::UNIT);
    assert_eq!(TypeId::VOID.raw(), 6);
}

#[test]
fn test_raw_roundtrip() {
    let id = TypeId::from_raw(12345);
    let raw = id.raw();
    let recovered = TypeId::from_raw(raw);
    assert_eq!(id, recovered);
}

#[test]
fn test_indices_match_idx_layout() {
    // These indices must match ori_types::Idx for identity mapping
    assert_eq!(TypeId::INT.raw(), 0);
    assert_eq!(TypeId::FLOAT.raw(), 1);
    assert_eq!(TypeId::BOOL.raw(), 2);
    assert_eq!(TypeId::STR.raw(), 3);
    assert_eq!(TypeId::CHAR.raw(), 4);
    assert_eq!(TypeId::BYTE.raw(), 5);
    assert_eq!(TypeId::UNIT.raw(), 6);
    assert_eq!(TypeId::NEVER.raw(), 7);
    assert_eq!(TypeId::ERROR.raw(), 8);
    assert_eq!(TypeId::DURATION.raw(), 9);
    assert_eq!(TypeId::SIZE.raw(), 10);
    assert_eq!(TypeId::ORDERING.raw(), 11);
    // Markers have their own dedicated indices
    assert_eq!(TypeId::INFER.raw(), 12);
    assert_eq!(TypeId::SELF_TYPE.raw(), 13);
}

#[test]
fn test_no_overlapping_indices() {
    // Every constant must have a unique raw value
    let all = [
        TypeId::INT,
        TypeId::FLOAT,
        TypeId::BOOL,
        TypeId::STR,
        TypeId::CHAR,
        TypeId::BYTE,
        TypeId::UNIT,
        TypeId::NEVER,
        TypeId::ERROR,
        TypeId::DURATION,
        TypeId::SIZE,
        TypeId::ORDERING,
        TypeId::INFER,
        TypeId::SELF_TYPE,
    ];
    let mut set = std::collections::HashSet::new();
    for id in &all {
        assert!(set.insert(id.raw()), "duplicate raw value: {}", id.raw());
    }
}
