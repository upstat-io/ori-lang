use super::*;
use ori_ir::{ExprArena, SharedInterner};

#[test]
fn test_scoped_interpreter_drops_on_normal_exit() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    // Start with 1 scope
    assert_eq!(interp.env.depth(), 1);

    {
        let scoped = interp.scoped();
        assert_eq!(scoped.env.depth(), 2);
    }

    // Back to 1 scope after guard dropped
    assert_eq!(interp.env.depth(), 1);
}

#[test]
fn test_scoped_interpreter_drops_on_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    assert_eq!(interp.env.depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let scoped = interp.scoped();
        assert_eq!(scoped.env.depth(), 2);
        panic!("test panic");
    }));

    assert!(result.is_err());
    // Scope should still be popped due to Drop
    assert_eq!(interp.env.depth(), 1);
}

#[test]
fn test_scoped_interpreter_drops_on_nested_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    assert_eq!(interp.env.depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        interp.with_env_scope(|scoped1| {
            assert_eq!(scoped1.env.depth(), 2);
            scoped1.with_env_scope(|scoped2| {
                assert_eq!(scoped2.env.depth(), 3);
                scoped2.with_env_scope(|scoped3| {
                    assert_eq!(scoped3.env.depth(), 4);
                    panic!("deep panic");
                });
            });
        });
    }));

    assert!(result.is_err());
    // All 3 scopes should be popped due to Drop during unwinding
    assert_eq!(interp.env.depth(), 1);
}

#[test]
fn test_with_env_scope_closure() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    let name = interner.intern("x");
    let result = interp.with_env_scope(|scoped| {
        scoped
            .env
            .define(name, Value::int(42), Mutability::Immutable);
        scoped.env.lookup(name)
    });

    assert_eq!(result, Some(Value::int(42)));
    // Variable should be gone after scope exit
    assert_eq!(interp.env.lookup(name), None);
}

#[test]
fn test_with_env_scope_closure_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    let name = interner.intern("x");
    assert_eq!(interp.env.depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        interp.with_env_scope(|scoped| {
            scoped
                .env
                .define(name, Value::int(42), Mutability::Immutable);
            assert_eq!(scoped.env.depth(), 2);
            panic!("closure panic");
        })
    }));

    assert!(result.is_err());
    // Scope should be popped even though closure panicked
    assert_eq!(interp.env.depth(), 1);
    // Variable should be gone
    assert_eq!(interp.env.lookup(name), None);
}

#[test]
fn test_nested_scopes() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    assert_eq!(interp.env.depth(), 1);

    interp.with_env_scope(|scoped1| {
        assert_eq!(scoped1.env.depth(), 2);

        scoped1.with_env_scope(|scoped2| {
            assert_eq!(scoped2.env.depth(), 3);
        });

        assert_eq!(scoped1.env.depth(), 2);
    });

    assert_eq!(interp.env.depth(), 1);
}

#[test]
fn test_with_binding() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    let name = interner.intern("x");

    let result = interp.with_binding(name, Value::int(100), Mutability::Immutable, |scoped| {
        scoped.env.lookup(name)
    });

    assert_eq!(result, Some(Value::int(100)));
    assert_eq!(interp.env.lookup(name), None);
}

#[test]
fn test_with_bindings_multiple() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    let a = interner.intern("a");
    let b = interner.intern("b");
    let c = interner.intern("c");

    let bindings = vec![
        (a, Value::int(1), Mutability::Immutable),
        (b, Value::int(2), Mutability::Immutable),
        (c, Value::int(3), Mutability::Immutable),
    ];

    let result = interp.with_bindings(bindings, |scoped| {
        (
            scoped.env.lookup(a),
            scoped.env.lookup(b),
            scoped.env.lookup(c),
        )
    });

    assert_eq!(result.0, Some(Value::int(1)));
    assert_eq!(result.1, Some(Value::int(2)));
    assert_eq!(result.2, Some(Value::int(3)));

    // All should be gone after scope exit
    assert_eq!(interp.env.lookup(a), None);
    assert_eq!(interp.env.lookup(b), None);
    assert_eq!(interp.env.lookup(c), None);
}

#[test]
fn test_scoped_deref_allows_method_calls() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    let name = interner.intern("test_var");

    // Create a scoped interpreter
    {
        let mut scoped = interp.scoped();

        // Can access env through Deref
        scoped
            .env
            .define(name, Value::int(42), Mutability::Immutable);

        // Can lookup through the scoped interpreter
        assert_eq!(scoped.env.lookup(name), Some(Value::int(42)));

        // Can access interner through Deref
        assert_eq!(scoped.interner.lookup(name), "test_var");
    }

    // Scope popped, variable gone
    assert_eq!(interp.env.lookup(name), None);
}

#[test]
fn test_early_return_still_cleans_up() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);

    fn helper(interp: &mut Interpreter) -> Option<i64> {
        let mut scoped = interp.scoped();
        let name = scoped.interner.intern("early");
        scoped
            .env
            .define(name, Value::int(999), Mutability::Immutable);

        // Early return - scope should still be cleaned up
        Some(42)
    }

    assert_eq!(interp.env.depth(), 1);
    let result = helper(&mut interp);
    assert_eq!(result, Some(42));
    assert_eq!(interp.env.depth(), 1); // Scope cleaned up
}
