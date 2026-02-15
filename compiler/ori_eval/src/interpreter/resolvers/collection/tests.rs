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
