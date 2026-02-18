use ori_patterns::IteratorValue;

use super::*;

#[test]
fn test_priority() {
    let interner = StringInterner::new();
    let resolver = CollectionMethodResolver::new(&interner);
    assert_eq!(resolver.priority(), 1);
}

#[test]
fn test_list_map_resolves() {
    let interner = StringInterner::new();
    let resolver = CollectionMethodResolver::new(&interner);
    let list = Value::list(vec![Value::int(1), Value::int(2)]);

    let list_type = interner.intern("[int]");
    let map_method = interner.intern("map");
    let result = resolver.resolve(&list, list_type, map_method);
    assert!(matches!(
        result,
        MethodResolution::Collection(CollectionMethod::Map)
    ));
}

#[test]
fn test_list_filter_resolves() {
    let interner = StringInterner::new();
    let resolver = CollectionMethodResolver::new(&interner);
    let list = Value::list(vec![Value::int(1), Value::int(2)]);

    let list_type = interner.intern("[int]");
    let filter_method = interner.intern("filter");
    let result = resolver.resolve(&list, list_type, filter_method);
    assert!(matches!(
        result,
        MethodResolution::Collection(CollectionMethod::Filter)
    ));
}

#[test]
fn test_list_unknown_not_found() {
    let interner = StringInterner::new();
    let resolver = CollectionMethodResolver::new(&interner);
    let list = Value::list(vec![Value::int(1)]);

    let list_type = interner.intern("[int]");
    let unknown = interner.intern("unknown");
    let result = resolver.resolve(&list, list_type, unknown);
    assert!(matches!(result, MethodResolution::NotFound));
}

#[test]
fn test_int_not_collection() {
    let interner = StringInterner::new();
    let resolver = CollectionMethodResolver::new(&interner);

    let int_type = interner.intern("int");
    let map_method = interner.intern("map");
    let result = resolver.resolve(&Value::int(42), int_type, map_method);
    assert!(matches!(result, MethodResolution::NotFound));
}

// Iterator method resolution

/// Helper: create an iterator Value for testing resolution.
fn make_iter_value() -> Value {
    Value::iterator(IteratorValue::from_range(0, Some(10), 1, false))
}

/// Helper: resolve a method on an iterator value.
fn resolve_iter(interner: &StringInterner, method: &str) -> MethodResolution {
    let resolver = CollectionMethodResolver::new(interner);
    let iter_val = make_iter_value();
    let type_name = interner.intern("Iterator");
    let method_name = interner.intern(method);
    resolver.resolve(&iter_val, type_name, method_name)
}

/// All iterator methods from `all_iterator_variants()` resolve correctly.
///
/// Uses the canonical variant list as source of truth — if a new variant
/// is added there but not wired into `resolve_iterator_method()`, this
/// test will catch the drift.
#[test]
fn iterator_methods_resolve() {
    let interner = StringInterner::new();

    for &(name, expected) in CollectionMethod::all_iterator_variants() {
        let result = resolve_iter(&interner, name);
        assert!(
            matches!(result, MethodResolution::Collection(m) if m == expected),
            "iterator method '{name}' should resolve to {expected:?}, got {result:?}"
        );
    }
}

#[test]
fn test_list_join_resolves() {
    let interner = StringInterner::new();
    let resolver = CollectionMethodResolver::new(&interner);
    let list = Value::list(vec![Value::int(1), Value::int(2)]);

    let list_type = interner.intern("[int]");
    let join_method = interner.intern("join");
    let result = resolver.resolve(&list, list_type, join_method);
    assert!(matches!(
        result,
        MethodResolution::Collection(CollectionMethod::Join)
    ));
}

/// Unknown methods on iterators return `NotFound`.
#[test]
fn iterator_unknown_method_not_found() {
    let interner = StringInterner::new();
    let result = resolve_iter(&interner, "nonexistent");
    assert!(matches!(result, MethodResolution::NotFound));
}
