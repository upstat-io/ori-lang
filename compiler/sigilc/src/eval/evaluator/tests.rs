//! Tests for the Evaluator.

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use super::*;
use crate::ir::{Span, SharedArena, SharedInterner};
use crate::ir::ast::{Expr, ExprKind};
use crate::context::SharedMutableRegistry;
use std::collections::HashMap;
use sigil_eval::{UserMethod, UserMethodRegistry};

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
    let mut arena = crate::ir::ExprArena::new();

    // Create method body: self.x * 2
    let self_name = interner.intern("self");
    let x_name = interner.intern("x");

    // Build: self
    let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
    // Build: self.x
    let self_x = arena.alloc_expr(Expr::new(
        ExprKind::Field { receiver: self_ref, field: x_name },
        Span::new(0, 6),
    ));
    // Build: 2
    let two = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(9, 10)));
    // Build: self.x * 2
    let body = arena.alloc_expr(Expr::new(
        ExprKind::Binary { left: self_x, op: crate::ir::BinaryOp::Mul, right: two },
        Span::new(0, 10),
    ));

    // Build registry before creating evaluator (immutable after construction)
    let shared_arena = SharedArena::new(arena.clone());
    let user_method = UserMethod::new(vec![self_name], body, HashMap::new(), shared_arena);
    let mut registry = UserMethodRegistry::new();
    let point_name = interner.intern("Point");
    let double_x_name = interner.intern("double_x");
    registry.register(point_name, double_x_name, user_method);

    let mut evaluator = EvaluatorBuilder::new(&interner, &arena)
        .user_method_registry(SharedMutableRegistry::new(registry))
        .build();

    // Create a struct value with x = 5
    let mut fields = HashMap::new();
    fields.insert(x_name, Value::Int(5));
    let point = Value::Struct(StructValue::new(point_name, fields));

    // Call point.double_x() -> should return 10
    let method_name = interner.intern("double_x");
    let result = evaluator.eval_method_call(point, method_name, vec![]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Int(10));
}

#[test]
fn test_user_method_with_self_access() {
    let interner = SharedInterner::default();
    let mut arena = crate::ir::ExprArena::new();

    // Create method body that accesses self.x: ExprKind::Field { receiver: self, field: x }
    let self_name = interner.intern("self");
    let x_name = interner.intern("x");

    // Build: self
    let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
    // Build: self.x
    let body = arena.alloc_expr(Expr::new(
        ExprKind::Field { receiver: self_ref, field: x_name },
        Span::new(0, 6),
    ));

    // Build registry before creating evaluator (immutable after construction)
    let shared_arena = SharedArena::new(arena.clone());
    let user_method = UserMethod::new(vec![self_name], body, HashMap::new(), shared_arena);
    let mut registry = UserMethodRegistry::new();
    let point_name = interner.intern("Point");
    let get_x_name = interner.intern("get_x");
    registry.register(point_name, get_x_name, user_method);

    let mut evaluator = EvaluatorBuilder::new(&interner, &arena)
        .user_method_registry(SharedMutableRegistry::new(registry))
        .build();

    // Create a struct value with x = 7
    let mut fields = HashMap::new();
    fields.insert(x_name, Value::Int(7));
    let point = Value::Struct(StructValue::new(point_name, fields));

    // Call point.get_x()
    let method_name = interner.intern("get_x");
    let result = evaluator.eval_method_call(point, method_name, vec![]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Int(7));
}

#[test]
fn test_user_method_with_args() {
    let interner = SharedInterner::default();
    let mut arena = crate::ir::ExprArena::new();

    // Create method body: self.x + n (where n is an argument)
    let self_name = interner.intern("self");
    let x_name = interner.intern("x");
    let n_name = interner.intern("n");

    // Build: self
    let self_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(self_name), Span::new(0, 4)));
    // Build: self.x
    let self_x = arena.alloc_expr(Expr::new(
        ExprKind::Field { receiver: self_ref, field: x_name },
        Span::new(0, 6),
    ));
    // Build: n
    let n_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(n_name), Span::new(7, 8)));
    // Build: self.x + n
    let body = arena.alloc_expr(Expr::new(
        ExprKind::Binary { left: self_x, op: crate::ir::BinaryOp::Add, right: n_ref },
        Span::new(0, 10),
    ));

    // Build registry before creating evaluator (immutable after construction)
    let shared_arena = SharedArena::new(arena.clone());
    let user_method = UserMethod::new(vec![self_name, n_name], body, HashMap::new(), shared_arena);
    let mut registry = UserMethodRegistry::new();
    let point_name = interner.intern("Point");
    let add_to_x_name = interner.intern("add_to_x");
    registry.register(point_name, add_to_x_name, user_method);

    let mut evaluator = EvaluatorBuilder::new(&interner, &arena)
        .user_method_registry(SharedMutableRegistry::new(registry))
        .build();

    // Create a struct value with x = 10
    let mut fields = HashMap::new();
    fields.insert(x_name, Value::Int(10));
    let point = Value::Struct(StructValue::new(point_name, fields));

    // Call point.add_to_x(n: 5) -> should return 15
    let method_name = interner.intern("add_to_x");
    let result = evaluator.eval_method_call(point, method_name, vec![Value::Int(5)]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Int(15));
}

#[test]
fn test_builtin_method_fallback() {
    let interner = SharedInterner::default();
    let arena = crate::ir::ExprArena::new();

    let mut evaluator = Evaluator::new(&interner, &arena);

    // Call built-in list.len() method (no user method registered)
    let list = Value::list(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    let method_name = interner.intern("len");
    let result = evaluator.eval_method_call(list, method_name, vec![]);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Int(3));
}
