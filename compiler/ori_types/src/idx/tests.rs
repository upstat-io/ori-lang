use super::*;

#[test]
fn primitive_indices_are_correct() {
    assert_eq!(Idx::INT.raw(), 0);
    assert_eq!(Idx::FLOAT.raw(), 1);
    assert_eq!(Idx::BOOL.raw(), 2);
    assert_eq!(Idx::STR.raw(), 3);
    assert_eq!(Idx::CHAR.raw(), 4);
    assert_eq!(Idx::BYTE.raw(), 5);
    assert_eq!(Idx::UNIT.raw(), 6);
    assert_eq!(Idx::NEVER.raw(), 7);
    assert_eq!(Idx::ERROR.raw(), 8);
    assert_eq!(Idx::DURATION.raw(), 9);
    assert_eq!(Idx::SIZE.raw(), 10);
    assert_eq!(Idx::ORDERING.raw(), 11);
}

#[test]
fn primitive_check_works() {
    assert!(Idx::INT.is_primitive());
    assert!(Idx::ERROR.is_primitive());
    assert!(!Idx::from_raw(64).is_primitive());
    assert!(!Idx::from_raw(1000).is_primitive());
}

#[test]
fn none_sentinel_works() {
    assert!(Idx::NONE.is_none());
    assert!(!Idx::INT.is_none());
    assert!(!Idx::from_raw(1000).is_none());
}

#[test]
fn idx_is_copy() {
    let a = Idx::INT;
    let b = a; // Copy, not move
    assert_eq!(a, b);
}

#[test]
fn idx_equality() {
    assert_eq!(Idx::INT, Idx::INT);
    assert_ne!(Idx::INT, Idx::FLOAT);
    assert_eq!(Idx::from_raw(100), Idx::from_raw(100));
}
