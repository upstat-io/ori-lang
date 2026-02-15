use super::*;
use rustc_hash::FxHashMap;

#[test]
fn test_method_key_equality() {
    let interner = SharedInterner::default();
    let point = interner.intern("Point");
    let distance = interner.intern("distance");
    let other = interner.intern("other");

    let k1 = MethodKey::new(point, distance);
    let k2 = MethodKey::new(point, distance);
    let k3 = MethodKey::new(point, other);

    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn test_method_key_as_hashmap_key() {
    let interner = SharedInterner::default();
    let point = interner.intern("Point");
    let distance = interner.intern("distance");
    let scale = interner.intern("scale");
    let missing = interner.intern("missing");

    let mut map: FxHashMap<MethodKey, u32> = FxHashMap::default();
    map.insert(MethodKey::new(point, distance), 1);
    map.insert(MethodKey::new(point, scale), 2);

    assert_eq!(map.get(&MethodKey::new(point, distance)), Some(&1));
    assert_eq!(map.get(&MethodKey::new(point, scale)), Some(&2));
    assert_eq!(map.get(&MethodKey::new(point, missing)), None);
}

#[test]
fn test_method_key_display() {
    let interner = SharedInterner::default();
    let point = interner.intern("Point");
    let distance = interner.intern("distance");

    let key = MethodKey::new(point, distance);
    assert_eq!(format!("{}", key.display(&interner)), "Point::distance");
}

#[test]
fn test_method_key_is_copy() {
    let interner = SharedInterner::default();
    let point = interner.intern("Point");
    let distance = interner.intern("distance");

    let key = MethodKey::new(point, distance);
    let key_copy = key; // This should work since MethodKey is Copy
    assert_eq!(key, key_copy);
}
