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
fn test_environment_capture_local_only() {
    // capture() returns LOCAL bindings only. Globals are reached at call time via
    // child() scope-sharing (prepare_call_env), so they are deliberately excluded
    // from the capture map — re-binding every global into each call env was the
    // dominant per-call allocation cost on the function-call hot path.
    let interner = SharedInterner::default();
    let local_a = interner.intern("local_a");
    let local_b = interner.intern("local_b");
    let glob = interner.intern("glob");

    let mut env = Environment::new();
    env.define_global(glob, Value::int(1));
    env.push_scope();
    env.define(local_a, Value::int(2), Mutability::Immutable);
    env.push_scope();
    env.define(local_b, Value::int(3), Mutability::Immutable);

    let captures = env.capture();
    assert_eq!(
        captures.get(&local_a),
        Some(&Value::int(2)),
        "outer-local binding is captured"
    );
    assert_eq!(
        captures.get(&local_b),
        Some(&Value::int(3)),
        "inner-local binding is captured"
    );
    assert_eq!(
        captures.get(&glob),
        None,
        "global binding is NOT in the capture map (reached via child scope-sharing)"
    );
}

#[test]
fn test_capture_excludes_global_but_child_resolves_it() {
    // Real invariant the local-only capture must preserve: a closure can still
    // resolve a global when called, because its call env is built via child()
    // (which shares the global scope) — independent of the capture map.
    let interner = SharedInterner::default();
    let glob = interner.intern("glob");

    let mut env = Environment::new();
    env.define_global(glob, Value::int(42));
    env.push_scope();

    // Not in the capture snapshot...
    assert_eq!(env.capture().get(&glob), None);
    // ...but a child env (the call-env shape) still resolves it.
    assert_eq!(env.child().lookup(glob), Some(Value::int(42)));
}

#[test]
fn test_environment_capture_shadowing_innermost_wins() {
    // A name bound in both an outer and inner local scope captures the innermost.
    let interner = SharedInterner::default();
    let n = interner.intern("n");

    let mut env = Environment::new();
    env.push_scope();
    env.define(n, Value::int(1), Mutability::Immutable);
    env.push_scope();
    env.define(n, Value::int(2), Mutability::Immutable);

    assert_eq!(
        env.capture().get(&n),
        Some(&Value::int(2)),
        "innermost local binding wins on shadowing"
    );
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
