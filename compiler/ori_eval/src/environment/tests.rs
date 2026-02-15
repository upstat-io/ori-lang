use super::*;
use ori_ir::SharedInterner;

#[test]
fn test_scope_define_lookup() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let mut scope = Scope::new();
    scope.define(x, Value::int(42), Mutability::Immutable);
    assert_eq!(scope.lookup(x), Some(Value::int(42)));
}

#[test]
fn test_scope_shadowing() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let parent = LocalScope::new(Scope::new());
    parent
        .borrow_mut()
        .define(x, Value::int(1), Mutability::Immutable);

    let mut child = Scope::with_parent(parent);
    child.define(x, Value::int(2), Mutability::Immutable);

    // Child's binding shadows parent's
    assert_eq!(child.lookup(x), Some(Value::int(2)));
}

#[test]
fn test_environment_push_pop() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let mut env = Environment::new();
    env.define(x, Value::int(1), Mutability::Immutable);

    env.push_scope();
    env.define(x, Value::int(2), Mutability::Immutable);
    assert_eq!(env.lookup(x), Some(Value::int(2)));

    env.pop_scope();
    assert_eq!(env.lookup(x), Some(Value::int(1)));
}

#[test]
fn test_environment_mutable() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let mut env = Environment::new();
    env.define(x, Value::int(1), Mutability::Mutable);
    assert!(env.assign(x, Value::int(2)).is_ok());
    assert_eq!(env.lookup(x), Some(Value::int(2)));
}

#[test]
fn test_environment_immutable() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let mut env = Environment::new();
    env.define(x, Value::int(1), Mutability::Immutable);
    assert!(env.assign(x, Value::int(2)).is_err());
}

#[test]
fn test_environment_capture() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");
    let y = interner.intern("y");

    let mut env = Environment::new();
    env.define(x, Value::int(1), Mutability::Immutable);
    env.push_scope();
    env.define(y, Value::int(2), Mutability::Immutable);

    let captures = env.capture();
    assert_eq!(captures.get(&x), Some(&Value::int(1)));
    assert_eq!(captures.get(&y), Some(&Value::int(2)));
}

#[test]
fn test_local_scope_new() {
    let scope = LocalScope::new(42);
    assert_eq!(*scope.borrow(), 42);
}

#[test]
fn test_local_scope_borrow_mut() {
    let scope = LocalScope::new(vec![1, 2, 3]);
    scope.borrow_mut().push(4);
    assert_eq!(*scope.borrow(), vec![1, 2, 3, 4]);
}

#[test]
fn test_local_scope_clone() {
    let scope1 = LocalScope::new(42);
    let scope2 = scope1.clone();

    // Both point to the same allocation
    scope1.borrow_mut().clone_from(&100);
    assert_eq!(*scope2.borrow(), 100);
}

#[test]
fn test_local_scope_default() {
    let scope: LocalScope<i32> = LocalScope::default();
    assert_eq!(*scope.borrow(), 0);
}

#[test]
fn test_local_scope_deref() {
    let scope = LocalScope::new(42);
    // Deref returns &RefCell<T>
    let borrowed = scope.deref().borrow();
    assert_eq!(*borrowed, 42);
}

#[test]
fn test_environment_child_preserves_global_bindings() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");
    let y = interner.intern("y");

    let mut env = Environment::new();
    env.define_global(x, Value::int(42));
    env.define_global(y, Value::string("hello".to_string()));

    // child() shares the global scope — all global bindings are visible
    let child = env.child();

    assert_eq!(child.lookup(x), Some(Value::int(42)));
    assert_eq!(child.lookup(y), Some(Value::string("hello".to_string())));
}

#[test]
fn test_environment_child_preserves_global() {
    let interner = SharedInterner::default();
    let x = interner.intern("x");

    let mut env = Environment::new();
    env.define_global(x, Value::int(99));

    // Child should have access to global bindings
    let child = env.child();
    assert_eq!(child.lookup(x), Some(Value::int(99)));
}
