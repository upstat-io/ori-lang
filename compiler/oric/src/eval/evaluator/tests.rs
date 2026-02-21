//! Tests for the Evaluator.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#![allow(
    clippy::semicolon_if_nothing_returned,
    clippy::items_after_statements,
    clippy::unnecessary_wraps,
    clippy::manual_assert,
    reason = "test ergonomics — relaxed style for clarity and brevity"
)]

use super::Evaluator;
use crate::db::CompilerDb;
use crate::eval::{EvalError, Value};
use crate::ir::{ExprArena, SharedInterner};

#[test]
fn test_eval_error() {
    let err = EvalError::new("test error");
    assert_eq!(err.message, "test error");
}

#[test]
fn test_control_action_propagate() {
    use ori_patterns::ControlAction;
    let action = ControlAction::Propagate(Value::None);
    assert!(!action.is_error());
    if let ControlAction::Propagate(v) = action {
        assert!(matches!(v, Value::None));
    } else {
        panic!("expected ControlAction::Propagate");
    }
}

#[test]
fn test_builtin_method_fallback() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();

    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    // Call built-in list.len() method (no user method registered)
    let list = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
    let method_name = interner.intern("len");
    let result = evaluator.eval_method_call(list, method_name, vec![]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::int(3));
}

// RAII Scope Guard Tests

use ori_eval::Mutability;

#[test]
fn test_scoped_evaluator_drops_on_normal_exit() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    // Start with 1 scope
    assert_eq!(evaluator.env().depth(), 1);

    {
        let scoped = evaluator.scoped();
        assert_eq!(scoped.env().depth(), 2);
    }

    // Back to 1 scope after guard dropped
    assert_eq!(evaluator.env().depth(), 1);
}

#[test]
fn test_scoped_evaluator_drops_on_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    assert_eq!(evaluator.env().depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        let scoped = evaluator.scoped();
        assert_eq!(scoped.env().depth(), 2);
        panic!("test panic");
    }));

    assert!(result.is_err());
    // Scope should still be popped due to Drop
    assert_eq!(evaluator.env().depth(), 1);
}

#[test]
fn test_scoped_evaluator_drops_on_nested_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    assert_eq!(evaluator.env().depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        evaluator.with_env_scope(|scoped1| {
            assert_eq!(scoped1.env().depth(), 2);
            scoped1.with_env_scope(|scoped2| {
                assert_eq!(scoped2.env().depth(), 3);
                scoped2.with_env_scope(|scoped3| {
                    assert_eq!(scoped3.env().depth(), 4);
                    panic!("deep panic");
                });
            });
        });
    }));

    assert!(result.is_err());
    // All 3 scopes should be popped due to Drop during unwinding
    assert_eq!(evaluator.env().depth(), 1);
}

#[test]
fn test_with_env_scope_closure() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    let name = interner.intern("x");
    let result = evaluator.with_env_scope(|scoped| {
        scoped
            .env_mut()
            .define(name, Value::int(42), Mutability::Immutable);
        scoped.env().lookup(name)
    });

    assert_eq!(result, Some(Value::int(42)));
    // Variable should be gone after scope exit
    assert_eq!(evaluator.env().lookup(name), None);
}

#[test]
fn test_with_env_scope_closure_panic() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    let name = interner.intern("x");
    assert_eq!(evaluator.env().depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        evaluator.with_env_scope(|scoped| {
            scoped
                .env_mut()
                .define(name, Value::int(42), Mutability::Immutable);
            assert_eq!(scoped.env().depth(), 2);
            panic!("closure panic");
        })
    }));

    assert!(result.is_err());
    // Scope should be popped even though closure panicked
    assert_eq!(evaluator.env().depth(), 1);
    // Variable should be gone
    assert_eq!(evaluator.env().lookup(name), None);
}

#[test]
fn test_nested_scopes() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    assert_eq!(evaluator.env().depth(), 1);

    evaluator.with_env_scope(|scoped1| {
        assert_eq!(scoped1.env().depth(), 2);

        scoped1.with_env_scope(|scoped2| {
            assert_eq!(scoped2.env().depth(), 3);
        });

        assert_eq!(scoped1.env().depth(), 2);
    });

    assert_eq!(evaluator.env().depth(), 1);
}

#[test]
fn test_scoped_deref_allows_method_calls() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    let name = interner.intern("test_var");

    // Create a scoped evaluator
    {
        let mut scoped = evaluator.scoped();

        // Can access env_mut through DerefMut
        scoped
            .env_mut()
            .define(name, Value::int(42), Mutability::Immutable);

        // Can lookup through the scoped evaluator
        assert_eq!(scoped.env().lookup(name), Some(Value::int(42)));

        // Can access interner through Deref
        assert_eq!(scoped.interner().lookup(name), "test_var");

        // Can access db through Deref
        let _ = scoped.db();
    }

    // Scope popped, variable gone
    assert_eq!(evaluator.env().lookup(name), None);
}

#[test]
fn test_early_return_still_cleans_up() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    fn helper(evaluator: &mut Evaluator) -> Option<i64> {
        let mut scoped = evaluator.scoped();
        let name = scoped.interner().intern("early");
        scoped
            .env_mut()
            .define(name, Value::int(999), Mutability::Immutable);

        // Early return - scope should still be cleaned up
        Some(42)
    }

    assert_eq!(evaluator.env().depth(), 1);
    let result = helper(&mut evaluator);
    assert_eq!(result, Some(42));
    assert_eq!(evaluator.env().depth(), 1); // Scope cleaned up
}

#[test]
fn test_scope_cleanup_with_result_error() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    let name = interner.intern("x");
    assert_eq!(evaluator.env().depth(), 1);

    let result = evaluator.with_env_scope_result(|scoped| {
        scoped
            .env_mut()
            .define(name, Value::int(42), Mutability::Immutable);
        assert_eq!(scoped.env().depth(), 2);

        // Return an error - scope should still be cleaned up
        Err(EvalError::new("intentional error").into())
    });

    assert!(result.is_err());
    assert_eq!(evaluator.env().depth(), 1); // Scope cleaned up
    assert_eq!(evaluator.env().lookup(name), None); // Variable gone
}

#[test]
fn test_multiple_sequential_scopes() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    assert_eq!(evaluator.env().depth(), 1);

    for i in 0i64..10 {
        let name = interner.intern(&format!("var_{i}"));
        evaluator.with_env_scope(|scoped| {
            scoped
                .env_mut()
                .define(name, Value::int(i), Mutability::Immutable);
            assert_eq!(scoped.env().depth(), 2);
            assert_eq!(scoped.env().lookup(name), Some(Value::int(i)));
        });

        // Each scope should be cleaned up before starting the next
        assert_eq!(evaluator.env().depth(), 1);
        assert_eq!(evaluator.env().lookup(name), None);
    }

    assert_eq!(evaluator.env().depth(), 1);
}

#[test]
fn test_deeply_nested_scopes() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    assert_eq!(evaluator.env().depth(), 1);

    fn nest(evaluator: &mut Evaluator, depth: usize, max_depth: usize) {
        if depth >= max_depth {
            return;
        }

        evaluator.with_env_scope(|scoped| {
            assert_eq!(scoped.env().depth(), depth + 2); // +1 for global, +1 for this scope
            nest(scoped, depth + 1, max_depth);
            assert_eq!(scoped.env().depth(), depth + 2); // Still same depth after returning
        });
    }

    nest(&mut evaluator, 0, 50);
    assert_eq!(evaluator.env().depth(), 1);
}

#[test]
fn test_panic_in_deeply_nested_scope() {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let db = CompilerDb::new();
    let mut evaluator = Evaluator::new(&interner, &arena, &db);

    fn nest_and_panic(evaluator: &mut Evaluator, depth: usize, panic_at: usize) {
        if depth >= panic_at {
            panic!("panic at depth {depth}");
        }

        evaluator.with_env_scope(|scoped| {
            nest_and_panic(scoped, depth + 1, panic_at);
        });
    }

    assert_eq!(evaluator.env().depth(), 1);

    let result = catch_unwind(AssertUnwindSafe(|| {
        nest_and_panic(&mut evaluator, 0, 25);
    }));

    assert!(result.is_err());
    // All 25 scopes should be unwound
    assert_eq!(evaluator.env().depth(), 1);
}
