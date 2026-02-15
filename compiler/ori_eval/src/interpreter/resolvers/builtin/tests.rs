use super::*;
use ori_ir::SharedInterner;

#[test]
fn test_priority() {
    let interner = SharedInterner::default();
    let resolver = BuiltinMethodResolver::new(&interner);
    assert_eq!(resolver.priority(), 2);
}

#[test]
fn known_method_returns_builtin() {
    let interner = SharedInterner::default();
    let resolver = BuiltinMethodResolver::new(&interner);

    let int_type = interner.intern("int");
    let add_method = interner.intern("add");
    let str_type = interner.intern("str");
    let len_method = interner.intern("len");

    let result = resolver.resolve(&Value::int(42), int_type, add_method);
    assert!(matches!(result, MethodResolution::Builtin));

    let result = resolver.resolve(&Value::string("hello"), str_type, len_method);
    assert!(matches!(result, MethodResolution::Builtin));
}

#[test]
fn unknown_method_returns_not_found() {
    let interner = SharedInterner::default();
    let resolver = BuiltinMethodResolver::new(&interner);

    let int_type = interner.intern("int");
    let nonexistent = interner.intern("nonexistent_method");

    let result = resolver.resolve(&Value::int(42), int_type, nonexistent);
    assert!(matches!(result, MethodResolution::NotFound));
}

#[test]
fn wrong_type_returns_not_found() {
    let interner = SharedInterner::default();
    let resolver = BuiltinMethodResolver::new(&interner);

    // "len" exists on "str" but not on "int"
    let int_type = interner.intern("int");
    let len_method = interner.intern("len");

    let result = resolver.resolve(&Value::int(42), int_type, len_method);
    assert!(matches!(result, MethodResolution::NotFound));
}

#[test]
fn newtype_unwrap_resolves_builtin() {
    let interner = SharedInterner::default();
    let resolver = BuiltinMethodResolver::new(&interner);

    let user_type = interner.intern("UserId");
    let unwrap = interner.intern("unwrap");

    let newtype_val = Value::newtype(user_type, Value::int(42));
    let result = resolver.resolve(&newtype_val, user_type, unwrap);
    assert!(matches!(result, MethodResolution::Builtin));
}

#[test]
fn newtype_unknown_method_returns_not_found() {
    let interner = SharedInterner::default();
    let resolver = BuiltinMethodResolver::new(&interner);

    let user_type = interner.intern("UserId");
    let nonexistent = interner.intern("nonexistent");

    let newtype_val = Value::newtype(user_type, Value::int(42));
    let result = resolver.resolve(&newtype_val, user_type, nonexistent);
    assert!(matches!(result, MethodResolution::NotFound));
}
