//! Tests for complex control flow scenarios: nested conditionals, loops, blocks.

use inkwell::context::Context;
use ori_ir::ast::patterns::BindingPattern;
use ori_ir::ast::{BinaryOp, Expr, ExprKind, Stmt, StmtKind};
use ori_ir::{ExprArena, Span, StmtRange, StringInterner};
use ori_types::Idx;

use super::helper::setup_builder_test;
use crate::builder::{Builder, Locals};

#[test]
fn test_if_no_else_void_result() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // if true then 42
    let cond = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: Span::new(0, 1),
    });
    let then_val = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::BOOL, Idx::INT];
    let mut locals = Locals::new();

    // No else branch with void result
    let result = builder.compile_if(
        cond,
        then_val,
        None,
        Idx::UNIT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Void if without else should return None");
}

#[test]
fn test_loop_terminates_without_body_terminator() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // loop { break }
    let break_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Break(None),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::UNIT];
    let mut locals = Locals::new();

    let result = builder.compile_loop(
        break_expr,
        Idx::UNIT,
        &arena,
        &expr_types,
        &mut locals,
        function,
    );

    assert!(result.is_none(), "Void loop should return None");
}

#[test]
fn test_loop_with_non_void_result() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // loop { break } with int result type
    let break_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Break(None),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::UNIT];
    let mut locals = Locals::new();

    let result = builder.compile_loop(
        break_expr,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
    );

    assert!(result.is_some(), "Int loop should return default value");
}

#[test]
fn test_break_without_loop_context() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let arena = ExprArena::new();

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![];
    let mut locals = Locals::new();

    // Break without loop context should return None
    let result = builder.compile_break(
        None,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None, // No loop context
    );

    assert!(
        result.is_none(),
        "Break without loop context should return None"
    );
}

#[test]
fn test_continue_without_loop_context() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let arena = ExprArena::new();
    let expr_types: Vec<ori_types::Idx> = vec![];
    let mut locals = Locals::new();

    // Continue without loop context should return None
    let result = builder.compile_continue(None, &arena, &expr_types, &mut locals, function, None);

    assert!(
        result.is_none(),
        "Continue without loop context should return None"
    );
}

#[test]
fn test_block_with_multiple_statements() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Block { let x = 10; let y = 20; x + y }
    let ten = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: Span::new(0, 1),
    });
    let x_name = interner.intern("x");

    let first_stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern: BindingPattern::Name(x_name),
            ty: None,
            init: ten,
            mutable: false,
        },
        span: Span::new(0, 1),
    });

    let twenty = arena.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: Span::new(0, 1),
    });
    let y_name = interner.intern("y");

    arena.alloc_stmt(Stmt {
        kind: StmtKind::Let {
            pattern: BindingPattern::Name(y_name),
            ty: None,
            init: twenty,
            mutable: false,
        },
        span: Span::new(0, 1),
    });

    let stmt_range = arena.alloc_stmt_range(first_stmt.index() as u32, 2);

    // x + y
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: Span::new(0, 1),
    });
    let y_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(y_name),
        span: Span::new(0, 1),
    });
    let add = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: x_ref,
            right: y_ref,
        },
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT; 10];
    let mut locals = Locals::new();

    let result = builder.compile_block(
        stmt_range,
        Some(add),
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Block should produce a value");
}

#[test]
fn test_block_with_empty_statements() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Empty block { 42 }
    let empty_stmts = StmtRange::EMPTY;

    let result_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_block(
        empty_stmts,
        Some(result_expr),
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "Block with no statements should return result"
    );
}

#[test]
fn test_block_with_statement_expr() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // { expr; 42 }
    let side_effect = arena.alloc_expr(Expr {
        kind: ExprKind::Int(99),
        span: Span::new(0, 1),
    });

    let first_stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Expr(side_effect),
        span: Span::new(0, 1),
    });

    let stmt_range = arena.alloc_stmt_range(first_stmt.index() as u32, 1);

    let result_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT; 5];
    let mut locals = Locals::new();

    let result = builder.compile_block(
        stmt_range,
        Some(result_expr),
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "Block with expr statement should return result"
    );
}

#[test]
fn test_block_no_result() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Block with no result expression { stmt; }
    let side_effect = arena.alloc_expr(Expr {
        kind: ExprKind::Int(99),
        span: Span::new(0, 1),
    });

    let first_stmt = arena.alloc_stmt(Stmt {
        kind: StmtKind::Expr(side_effect),
        span: Span::new(0, 1),
    });

    let stmt_range = arena.alloc_stmt_range(first_stmt.index() as u32, 1);

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_block(
        stmt_range,
        None, // No result expression
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Block without result should return None");
}

#[test]
fn test_assign_to_variable() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // First declare: let mut x = 0
    // Then assign: x = 42
    let x_name = interner.intern("x");

    // Declare x as mutable with initial value 0
    let initial_value = cx.scx.type_i64().const_int(0, false);
    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    // Create stack allocation for mutable variable
    let ptr = builder.create_entry_alloca(function, "x", cx.scx.type_i64().into());
    builder.store(initial_value.into(), ptr);

    let mut locals = Locals::new();
    locals.bind_mutable(x_name, ptr, cx.scx.type_i64().into());

    // Now test assignment: x = 42
    let target = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: Span::new(0, 1),
    });

    let value = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let expr_types = vec![Idx::INT, Idx::INT];

    let result = builder.compile_assign(
        target,
        value,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Assignment should produce the value");
    assert!(
        locals.get_storage(&x_name).is_some(),
        "x should be in locals after assignment"
    );
}

#[test]
fn test_assign_non_ident_target() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // 42 = 10 (invalid target)
    let target = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let value = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_assign(
        target,
        value,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_none(),
        "Assignment to non-ident should return None"
    );
}

#[test]
fn test_nested_if_else() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // if true then (if false then 1 else 2) else 3
    let inner_cond = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: Span::new(0, 1),
    });
    let one = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: Span::new(0, 1),
    });
    let two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: Span::new(0, 1),
    });
    let inner_if = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond: inner_cond,
            then_branch: one,
            else_branch: Some(two),
        },
        span: Span::new(0, 1),
    });

    let outer_cond = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: Span::new(0, 1),
    });
    let three = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::BOOL, Idx::INT, Idx::INT, Idx::INT, Idx::BOOL, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_if(
        outer_cond,
        inner_if,
        Some(three),
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Nested if should produce a value");
}
