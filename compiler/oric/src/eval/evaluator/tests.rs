//! Tests for the Evaluator.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#![allow(
    clippy::semicolon_if_nothing_returned,
    clippy::items_after_statements,
    clippy::unnecessary_wraps,
    clippy::manual_assert
)]

use super::{Evaluator, EvaluatorBuilder};
use crate::db::CompilerDb;
use crate::eval::{EvalError, StructValue, Value};
use crate::ir::ast::{Expr, ExprKind};
use crate::ir::{BinaryOp, ExprArena, SharedArena, SharedInterner, Span};
use ori_eval::SharedMutableRegistry;
use ori_eval::{UserMethod, UserMethodRegistry};
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_eval_error() {
    let err = EvalError::new("test error");
    assert_eq!(err.message, "test error");
    assert!(err.propagated_value.is_none());
}

#[test]
fn test_eval_error_propagate() {
    let err = EvalError::propagate(Value::None, "propagated");
    assert_eq!(err.message, "propagated");
    assert!(err.propagated_value.is_some());
}

#[test]
fn test_user_method_dispatch() {
    let interner = SharedInterner::default();
    let mut arena = ExprArena::new();

    // Create method body: self.x * 2
    let self_name = interner.intern("self");
    let x_name = interner.intern("x");

    // Build: self
    let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
    // Build: self.x
    let self_x = arena.alloc_expr(Expr::new(
        ExprKind::Field {
            receiver: self_ref,
            field: x_name,
        },
        Span::new(0, 6),
    ));
    // Build: 2
    let two = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(9, 10)));
    // Build: self.x * 2
    let body = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            left: self_x,
            op: BinaryOp::Mul,
            right: two,
        },
        Span::new(0, 10),
    ));

    // Build registry before creating evaluator (immutable after construction)
    let shared_arena = SharedArena::new(arena.clone());
    let user_method = UserMethod::new(
        vec![self_name],
        body,
        Arc::new(HashMap::new()),
        shared_arena,
    );
    let mut registry = UserMethodRegistry::new();
    let point_name = interner.intern("Point");
    let double_x_name = interner.intern("double_x");
    registry.register(point_name, double_x_name, user_method);

    let db = CompilerDb::new();
    let mut evaluator = EvaluatorBuilder::new(&interner, &arena, &db)
        .user_method_registry(SharedMutableRegistry::new(registry))
        .build();

    // Create a struct value with x = 5
    let mut fields = HashMap::new();
    fields.insert(x_name, Value::int(5));
    let point = Value::Struct(StructValue::new(point_name, fields));

    // Call point.double_x() -> should return 10
    let method_name = interner.intern("double_x");
    let result = evaluator.eval_method_call(point, method_name, vec![]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::int(10));
}

#[test]
fn test_user_method_with_self_access() {
    let interner = SharedInterner::default();
    let mut arena = ExprArena::new();

    // Create method body that accesses self.x: ExprKind::Field { receiver: self, field: x }
    let self_name = interner.intern("self");
    let x_name = interner.intern("x");

    // Build: self
    let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
    // Build: self.x
    let body = arena.alloc_expr(Expr::new(
        ExprKind::Field {
            receiver: self_ref,
            field: x_name,
        },
        Span::new(0, 6),
    ));

    // Build registry before creating evaluator (immutable after construction)
    let shared_arena = SharedArena::new(arena.clone());
    let user_method = UserMethod::new(
        vec![self_name],
        body,
        Arc::new(HashMap::new()),
        shared_arena,
    );
    let mut registry = UserMethodRegistry::new();
    let point_name = interner.intern("Point");
    let get_x_name = interner.intern("get_x");
    registry.register(point_name, get_x_name, user_method);

    let db = CompilerDb::new();
    let mut evaluator = EvaluatorBuilder::new(&interner, &arena, &db)
        .user_method_registry(SharedMutableRegistry::new(registry))
        .build();

    // Create a struct value with x = 7
    let mut fields = HashMap::new();
    fields.insert(x_name, Value::int(7));
    let point = Value::Struct(StructValue::new(point_name, fields));

    // Call point.get_x()
    let method_name = interner.intern("get_x");
    let result = evaluator.eval_method_call(point, method_name, vec![]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::int(7));
}

#[test]
fn test_user_method_with_args() {
    let interner = SharedInterner::default();
    let mut arena = ExprArena::new();

    // Create method body: self.x + n (where n is an argument)
    let self_name = interner.intern("self");
    let x_name = interner.intern("x");
    let n_name = interner.intern("n");

    // Build: self
    let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
    // Build: self.x
    let self_x = arena.alloc_expr(Expr::new(
        ExprKind::Field {
            receiver: self_ref,
            field: x_name,
        },
        Span::new(0, 6),
    ));
    // Build: n
    let n_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(n_name), Span::new(7, 8)));
    // Build: self.x + n
    let body = arena.alloc_expr(Expr::new(
        ExprKind::Binary {
            left: self_x,
            op: BinaryOp::Add,
            right: n_ref,
        },
        Span::new(0, 10),
    ));

    // Build registry before creating evaluator (immutable after construction)
    let shared_arena = SharedArena::new(arena.clone());
    let user_method = UserMethod::new(
        vec![self_name, n_name],
        body,
        Arc::new(HashMap::new()),
        shared_arena,
    );
    let mut registry = UserMethodRegistry::new();
    let point_name = interner.intern("Point");
    let add_to_x_name = interner.intern("add_to_x");
    registry.register(point_name, add_to_x_name, user_method);

    let db = CompilerDb::new();
    let mut evaluator = EvaluatorBuilder::new(&interner, &arena, &db)
        .user_method_registry(SharedMutableRegistry::new(registry))
        .build();

    // Create a struct value with x = 10
    let mut fields = HashMap::new();
    fields.insert(x_name, Value::int(10));
    let point = Value::Struct(StructValue::new(point_name, fields));

    // Call point.add_to_x(n: 5) -> should return 15
    let method_name = interner.intern("add_to_x");
    let result = evaluator.eval_method_call(point, method_name, vec![Value::int(5)]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::int(15));
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
        return Some(42);
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
        Err(EvalError::new("intentional error"))
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
