//! Tests for `FunctionExp` patterns (recurse, print, panic).

use inkwell::context::Context;
use ori_ir::ast::patterns::{FunctionExp, FunctionExpKind, NamedExpr};
use ori_ir::ast::{Expr, ExprKind};
use ori_ir::{ExprArena, NamedExprRange, Span, StringInterner, TypeId};

use super::helper::setup_builder_test;
use crate::builder::{Builder, Locals};

#[test]
fn test_function_exp_recurse_with_condition() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create condition expression (true)
    let cond_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: Span::new(0, 1),
    });

    // Create base expression (42)
    let base_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    // Create step expression (0)
    let step_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0),
        span: Span::new(0, 1),
    });

    // Create named expressions
    let condition_name = interner.intern("condition");
    let base_name = interner.intern("base");
    let step_name = interner.intern("step");

    let named_exprs = vec![
        NamedExpr {
            name: condition_name,
            value: cond_expr,
            span: Span::new(0, 1),
        },
        NamedExpr {
            name: base_name,
            value: base_expr,
            span: Span::new(0, 1),
        },
        NamedExpr {
            name: step_name,
            value: step_expr,
            span: Span::new(0, 1),
        },
    ];
    let named_range = arena.alloc_named_exprs(named_exprs);

    let func_exp = FunctionExp {
        kind: FunctionExpKind::Recurse,
        props: named_range,
        span: Span::new(0, 1),
    };

    // Create entry block and builder
    let entry_bb = cx.llcx().append_basic_block(function, "recurse_entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![TypeId::BOOL, TypeId::INT, TypeId::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Recurse should return a value");

    // Verify IR was generated
    let ir = cx.llmod().print_to_string().to_string();
    assert!(
        ir.contains("recurse_base"),
        "Should have recurse_base block"
    );
    assert!(
        ir.contains("recurse_step"),
        "Should have recurse_step block"
    );
    assert!(
        ir.contains("recurse_merge"),
        "Should have recurse_merge block"
    );
}

#[test]
fn test_function_exp_recurse_incomplete_props() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Only condition, no base or step
    let cond_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: Span::new(0, 1),
    });

    let condition_name = interner.intern("condition");
    let named_exprs = vec![NamedExpr {
        name: condition_name,
        value: cond_expr,
        span: Span::new(0, 1),
    }];
    let named_range = arena.alloc_named_exprs(named_exprs);

    let func_exp = FunctionExp {
        kind: FunctionExpKind::Recurse,
        props: named_range,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry2");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![TypeId::BOOL];
    let mut locals = Locals::new();

    // Should return None because base and step are missing
    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Incomplete recurse should return None");
}

#[test]
fn test_function_exp_print() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create msg expression (string)
    let hello_name = interner.intern("hello");
    let msg_expr = arena.alloc_expr(Expr {
        kind: ExprKind::String(hello_name),
        span: Span::new(0, 1),
    });

    let msg_name = interner.intern("msg");
    let named_exprs = vec![NamedExpr {
        name: msg_name,
        value: msg_expr,
        span: Span::new(0, 1),
    }];
    let named_range = arena.alloc_named_exprs(named_exprs);

    let func_exp = FunctionExp {
        kind: FunctionExpKind::Print,
        props: named_range,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "print_entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![TypeId::STR];
    let mut locals = Locals::new();

    // Print returns void (None)
    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::VOID,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Print should return None (void)");
}

#[test]
fn test_function_exp_panic() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let arena = ExprArena::new();

    let func_exp = FunctionExp {
        kind: FunctionExpKind::Panic,
        props: NamedExprRange::EMPTY,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "panic_entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![];
    let mut locals = Locals::new();

    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::VOID,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Panic should return None");

    // Check that unreachable was generated
    let ir = cx.llmod().print_to_string().to_string();
    assert!(
        ir.contains("unreachable"),
        "Panic should generate unreachable"
    );
}

#[test]
fn test_function_exp_default_fallback_void() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let arena = ExprArena::new();

    // Use Parallel which falls through to default case
    let func_exp = FunctionExp {
        kind: FunctionExpKind::Parallel,
        props: NamedExprRange::EMPTY,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "parallel_entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![];
    let mut locals = Locals::new();

    // For void return type, should return None
    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::VOID,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Void result type should return None");
}

#[test]
fn test_function_exp_default_fallback_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let arena = ExprArena::new();

    // Use Spawn which falls through to default case
    let func_exp = FunctionExp {
        kind: FunctionExpKind::Spawn,
        props: NamedExprRange::EMPTY,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "spawn_entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![];
    let mut locals = Locals::new();

    // For int return type, should return a default value
    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "Int result type should return Some(default)"
    );
}

#[test]
fn test_function_exp_timeout() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let arena = ExprArena::new();

    let func_exp = FunctionExp {
        kind: FunctionExpKind::Timeout,
        props: NamedExprRange::EMPTY,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "timeout_entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![];
    let mut locals = Locals::new();

    // Timeout falls through to default
    let result = builder.compile_function_exp(
        &func_exp,
        TypeId::BOOL,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "Bool result type should return Some(default)"
    );
}
