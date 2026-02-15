use super::*;

#[test]
fn static_lifetime_is_zero() {
    assert_eq!(LifetimeId::STATIC.raw(), 0);
}

#[test]
fn scoped_lifetime_is_one() {
    assert_eq!(LifetimeId::SCOPED.raw(), 1);
}

#[test]
fn is_static_works() {
    assert!(LifetimeId::STATIC.is_static());
    assert!(!LifetimeId::SCOPED.is_static());
    assert!(!LifetimeId::from_raw(42).is_static());
}

#[test]
fn roundtrip_raw() {
    let lt = LifetimeId::from_raw(42);
    assert_eq!(lt.raw(), 42);
}

#[test]
fn display_named_lifetimes() {
    assert_eq!(LifetimeId::STATIC.to_string(), "'static");
    assert_eq!(LifetimeId::SCOPED.to_string(), "'scoped");
    assert_eq!(LifetimeId::from_raw(5).to_string(), "'5");
}

#[test]
fn equality_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(LifetimeId::STATIC);
    set.insert(LifetimeId::SCOPED);
    set.insert(LifetimeId::STATIC); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn size_is_4_bytes() {
    assert_eq!(std::mem::size_of::<LifetimeId>(), 4);
}
