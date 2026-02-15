use super::*;
use ori_ir::SharedInterner;

#[test]
fn test_priority() {
    let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
    let resolver = UserRegistryResolver::new(registry);
    assert_eq!(resolver.priority(), 0);
}

#[test]
fn test_not_found_for_missing_method() {
    let interner = SharedInterner::default();
    let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
    let resolver = UserRegistryResolver::new(registry);

    let int_type = interner.intern("int");
    let unknown_method = interner.intern("unknown_method");
    let result = resolver.resolve(&Value::int(42), int_type, unknown_method);
    assert!(matches!(result, MethodResolution::NotFound));
}
