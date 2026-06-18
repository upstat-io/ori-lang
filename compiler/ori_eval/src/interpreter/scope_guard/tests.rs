use super::*;
use ori_ir::{ExprArena, SharedArena, SharedInterner};

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
fn test_call_frame_guard_restores_state_on_normal_drop() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);
    let callee = interner.intern("callee_fn");
    let local = interner.intern("local");
    let base_env_depth = interp.env.depth();
    let base_stack_depth = interp.call_stack.depth();

    let mut call_env = interp.env.child();
    call_env.push_scope();
    call_env.define(local, Value::int(7), Mutability::Immutable);
    // Distinct callee arena (NOT a clone of the caller's) so the imported_arena restore is
    // observable — pins the arena slot, not just env/call_stack. `Drop` restores all three
    // saved slots unconditionally (no branches), so pinning env + imported_arena also covers
    // the `canon` slot, which travels the identical restore path.
    let caller_arena = interp.imported_arena.clone();
    let callee_arena = SharedArena::new(ExprArena::new());

    {
        let guard = CallFrameGuard::install(&mut interp, call_env, callee_arena, None, callee);
        assert_eq!(
            guard.env.lookup(local),
            Some(Value::int(7)),
            "callee env installed during the call"
        );
        assert!(
            !std::ptr::eq(&raw const *guard.imported_arena, &raw const *caller_arena),
            "callee arena installed during the call (distinct from caller's)"
        );
        assert!(
            guard.call_stack.depth() > base_stack_depth,
            "call frame pushed during the call"
        );
    }

    // Drop restored the caller's state.
    assert_eq!(interp.env.depth(), base_env_depth, "env restored on drop");
    assert_eq!(
        interp.env.lookup(local),
        None,
        "callee binding gone after restore"
    );
    assert!(
        std::ptr::eq(&raw const *interp.imported_arena, &raw const *caller_arena),
        "caller imported_arena restored on drop"
    );
    assert_eq!(
        interp.call_stack.depth(),
        base_stack_depth,
        "call frame popped on drop"
    );
}

#[test]
fn test_call_frame_guard_restores_state_on_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let mut interp = Interpreter::new(&interner, &arena);
    let callee = interner.intern("callee_fn");
    let local = interner.intern("local");
    let base_env_depth = interp.env.depth();
    let base_stack_depth = interp.call_stack.depth();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut call_env = interp.env.child();
        call_env.push_scope();
        call_env.define(local, Value::int(7), Mutability::Immutable);
        let imported = interp.imported_arena.clone();
        let guard = CallFrameGuard::install(&mut interp, call_env, imported, None, callee);
        assert!(guard.call_stack.depth() > base_stack_depth);
        panic!("callee body panicked mid-eval");
    }));

    assert!(result.is_err());
    // Panic-safety regression pin: a panic unwinding through the call MUST NOT leak the callee
    // env / call frame — CallFrameGuard::drop restores during unwinding, as the removed
    // per-call child interpreter Drop did.
    assert_eq!(
        interp.env.depth(),
        base_env_depth,
        "env restored after panic"
    );
    assert_eq!(
        interp.env.lookup(local),
        None,
        "callee binding gone after panic"
    );
    assert_eq!(
        interp.call_stack.depth(),
        base_stack_depth,
        "call frame popped after panic"
    );
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
