use super::*;

#[test]
fn test_value_truthy() {
    assert!(Value::Bool(true).is_truthy());
    assert!(!Value::Bool(false).is_truthy());
    assert!(Value::int(1).is_truthy());
    assert!(!Value::int(0).is_truthy());
    assert!(!Value::None.is_truthy());
}

#[test]
fn test_value_display() {
    assert_eq!(format!("{}", Value::int(42)), "42");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::string("hello")), "\"hello\"");
}

#[test]
fn test_factory_methods() {
    // Test that factory methods work
    let s = Value::string("hello");
    assert_eq!(s.as_str(), Some("hello"));

    let list = Value::list(vec![Value::int(1), Value::int(2)]);
    assert_eq!(list.as_list().map(<[Value]>::len), Some(2));

    let opt = Value::some(Value::int(42));
    match opt {
        Value::Some(v) => assert_eq!(*v, Value::int(42)),
        _ => panic!("expected Some"),
    }

    let ok = Value::ok(Value::int(42));
    match ok {
        Value::Ok(v) => assert_eq!(*v, Value::int(42)),
        _ => panic!("expected Ok"),
    }

    let err = Value::err(Value::string("error"));
    match err {
        Value::Err(v) => assert_eq!(v.as_str(), Some("error")),
        _ => panic!("expected Err"),
    }
}

#[test]
fn test_value_equality() {
    assert!(Value::int(42).equals(&Value::int(42)));
    assert!(!Value::int(42).equals(&Value::int(43)));
    assert!(Value::None.equals(&Value::None));

    let s1 = Value::string("hello");
    let s2 = Value::string("hello");
    assert!(s1.equals(&s2));
}

#[test]
fn test_range_iter() {
    let range = RangeValue::exclusive(0, 5);
    let values: Vec<_> = range.iter().collect();
    assert_eq!(values, vec![0, 1, 2, 3, 4]);

    let range = RangeValue::inclusive(0, 5);
    let values: Vec<_> = range.iter().collect();
    assert_eq!(values, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn test_range_contains() {
    let range = RangeValue::exclusive(0, 5);
    assert!(range.contains(0));
    assert!(range.contains(4));
    assert!(!range.contains(5));

    let range = RangeValue::inclusive(0, 5);
    assert!(range.contains(5));
}

#[test]
fn test_function_value() {
    use ori_ir::{ExprArena, SharedArena};
    use rustc_hash::FxHashMap;
    let arena = SharedArena::new(ExprArena::new());
    let func = FunctionValue::new(vec![], FxHashMap::default(), arena);
    assert!(func.params.is_empty());
    assert!(!func.has_captures());
}

#[test]
fn test_value_hash_consistency() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_value(v: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    // Equal values must have equal hashes
    assert_eq!(hash_value(&Value::int(42)), hash_value(&Value::int(42)));
    assert_eq!(
        hash_value(&Value::Bool(true)),
        hash_value(&Value::Bool(true))
    );
    assert_eq!(hash_value(&Value::Void), hash_value(&Value::Void));
    assert_eq!(hash_value(&Value::None), hash_value(&Value::None));

    // Equal strings
    let s1 = Value::string("hello");
    let s2 = Value::string("hello");
    assert_eq!(hash_value(&s1), hash_value(&s2));

    // Equal lists
    let l1 = Value::list(vec![Value::int(1), Value::int(2)]);
    let l2 = Value::list(vec![Value::int(1), Value::int(2)]);
    assert_eq!(hash_value(&l1), hash_value(&l2));

    // Equal Option values
    let o1 = Value::some(Value::int(42));
    let o2 = Value::some(Value::int(42));
    assert_eq!(hash_value(&o1), hash_value(&o2));
}

#[test]
#[expect(
    clippy::mutable_key_type,
    reason = "Value hash is based on immutable content"
)]
fn test_value_in_hashset() {
    use std::collections::HashSet;

    let mut set: HashSet<Value> = HashSet::new();
    set.insert(Value::int(1));
    set.insert(Value::int(2));
    set.insert(Value::int(1)); // Duplicate

    assert_eq!(set.len(), 2);
    assert!(set.contains(&Value::int(1)));
    assert!(set.contains(&Value::int(2)));
    assert!(!set.contains(&Value::int(3)));
}

#[test]
#[expect(
    clippy::mutable_key_type,
    reason = "Value hash is based on immutable content"
)]
fn test_value_as_hashmap_key() {
    use rustc_hash::FxHashMap;

    let mut map: FxHashMap<Value, &str> = FxHashMap::default();
    map.insert(Value::string("key1"), "value1");
    map.insert(Value::int(42), "value2");

    assert_eq!(map.get(&Value::string("key1")), Some(&"value1"));
    assert_eq!(map.get(&Value::int(42)), Some(&"value2"));
    assert_eq!(map.get(&Value::string("unknown")), None);
}

#[test]
fn test_value_different_types_different_hash() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_value(v: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    // Different value types should (likely) have different hashes
    // This isn't guaranteed but is generally true for well-designed hash functions
    let int_hash = hash_value(&Value::int(1));
    let bool_hash = hash_value(&Value::Bool(true));
    let str_hash = hash_value(&Value::string("1"));

    // At least some should differ (collision is possible but unlikely)
    let all_same = int_hash == bool_hash && bool_hash == str_hash;
    assert!(
        !all_same,
        "Different types should generally have different hashes"
    );
}
