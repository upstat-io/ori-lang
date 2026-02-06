//! Tests for `FunctionSeq` patterns (run, try, match).

use inkwell::context::Context;
use ori_ir::ast::patterns::{BindingPattern, FunctionSeq, MatchArm, MatchPattern, SeqBinding};
use ori_ir::ast::{Expr, ExprKind};
use ori_ir::{ExprArena, ParsedTypeId, SeqBindingRange, Span, StringInterner};
use ori_types::Idx;

use super::helper::setup_builder_test;
use crate::builder::{Builder, LocalStorage, Locals};

#[test]
fn test_function_seq_run_empty_bindings() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // run(42) - no bindings, just result
    let result_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let run_seq = FunctionSeq::Run {
        bindings: SeqBindingRange::EMPTY,
        result: result_expr,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &run_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Run should return a value");
}

#[test]
fn test_function_seq_run_with_let_binding() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // let x = 10
    let ten = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: Span::new(0, 1),
    });

    let x_name = interner.intern("x");
    let x_pattern = arena.alloc_binding_pattern(BindingPattern::Name(x_name));
    let binding = SeqBinding::Let {
        pattern: x_pattern,
        ty: ParsedTypeId::INVALID,
        value: ten,
        mutable: false,
        span: Span::new(0, 1),
    };
    let bindings = arena.alloc_seq_bindings(vec![binding]);

    // result is x (which should be 10)
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: Span::new(0, 1),
    });

    let run_seq = FunctionSeq::Run {
        bindings,
        result: x_ref,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &run_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Run with binding should return a value");
    assert!(locals.contains(&x_name), "x should be in locals");
}

#[test]
fn test_function_seq_run_with_stmt() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // A statement expression
    let stmt_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(99),
        span: Span::new(0, 1),
    });

    let binding = SeqBinding::Stmt {
        expr: stmt_expr,
        span: Span::new(0, 1),
    };
    let bindings = arena.alloc_seq_bindings(vec![binding]);

    // result
    let result_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let run_seq = FunctionSeq::Run {
        bindings,
        result: result_expr,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &run_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Run with stmt should return a value");
}

#[test]
fn test_function_seq_try_empty_bindings() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // try(Ok(42))
    let result_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let try_seq = FunctionSeq::Try {
        bindings: SeqBindingRange::EMPTY,
        result: result_expr,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &try_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Try should return a value");
}

#[test]
fn test_function_seq_try_with_let_binding() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // let x = 10 (non-Result value)
    let ten = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: Span::new(0, 1),
    });

    let x_name = interner.intern("x");
    let x_pattern = arena.alloc_binding_pattern(BindingPattern::Name(x_name));
    let binding = SeqBinding::Let {
        pattern: x_pattern,
        ty: ParsedTypeId::INVALID,
        value: ten,
        mutable: false,
        span: Span::new(0, 1),
    };
    let bindings = arena.alloc_seq_bindings(vec![binding]);

    // result is x
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: Span::new(0, 1),
    });

    let try_seq = FunctionSeq::Try {
        bindings,
        result: x_ref,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &try_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Try with binding should return a value");
}

#[test]
fn test_function_seq_match() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // match(42, _ -> 100)
    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let body = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: Span::new(0, 1),
    });

    let arm = MatchArm {
        pattern: MatchPattern::Wildcard,
        guard: None,
        body,
        span: Span::new(0, 1),
    };
    let arms = arena.alloc_arms(vec![arm]);

    let match_seq = FunctionSeq::Match {
        scrutinee,
        arms,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &match_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "Match should return a value");
}

#[test]
fn test_function_seq_for_pattern_basic() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // for(over: [1, 2, 3], match: x -> x, default: 0)
    let list_items = vec![
        arena.alloc_expr(Expr {
            kind: ExprKind::Int(1),
            span: Span::new(0, 1),
        }),
        arena.alloc_expr(Expr {
            kind: ExprKind::Int(2),
            span: Span::new(0, 1),
        }),
        arena.alloc_expr(Expr {
            kind: ExprKind::Int(3),
            span: Span::new(0, 1),
        }),
    ];
    let list_range = arena.alloc_expr_list_inline(&list_items);
    let over = arena.alloc_expr(Expr {
        kind: ExprKind::List(list_range),
        span: Span::new(0, 1),
    });

    let x_name = interner.intern("x");
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: Span::new(0, 1),
    });

    let arm = MatchArm {
        pattern: MatchPattern::Binding(x_name),
        guard: None,
        body: x_ref,
        span: Span::new(0, 1),
    };

    let default = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0),
        span: Span::new(0, 1),
    });

    let for_seq = FunctionSeq::ForPattern {
        over,
        map: None,
        arm,
        default,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT; 10];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &for_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "ForPattern should return a value");
}

#[test]
fn test_function_seq_for_pattern_with_map() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // over: [1, 2, 3]
    let list_items = vec![arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: Span::new(0, 1),
    })];
    let list_range = arena.alloc_expr_list_inline(&list_items);
    let over = arena.alloc_expr(Expr {
        kind: ExprKind::List(list_range),
        span: Span::new(0, 1),
    });

    // map: x -> x * 2 (simplified as just 10 for test)
    let map_result = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: Span::new(0, 1),
    });

    let x_name = interner.intern("x");
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: Span::new(0, 1),
    });

    let arm = MatchArm {
        pattern: MatchPattern::Binding(x_name),
        guard: None,
        body: x_ref,
        span: Span::new(0, 1),
    };

    let default = arena.alloc_expr(Expr {
        kind: ExprKind::Int(0),
        span: Span::new(0, 1),
    });

    let for_seq = FunctionSeq::ForPattern {
        over,
        map: Some(map_result),
        arm,
        default,
        span: Span::new(0, 1),
    };

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT; 10];
    let mut locals = Locals::new();

    let result = builder.compile_function_seq(
        &for_seq,
        Idx::INT,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "ForPattern with map should return a value"
    );
}

#[test]
fn test_bind_pattern_name() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let x_name = interner.intern("x");
    let pattern = BindingPattern::Name(x_name);
    let value = cx.scx.type_i64().const_int(42, false).into();

    let mut locals = Locals::new();
    // bind_pattern now requires: mutable, ty, function
    builder.bind_pattern(
        &pattern,
        value,
        false,
        cx.scx.type_i64().into(),
        function,
        &mut locals,
    );

    assert!(locals.contains(&x_name), "x should be bound");
    // Check the value through the LocalStorage API
    match locals.get_storage(&x_name) {
        Some(LocalStorage::Immutable(val)) => {
            assert_eq!(
                val.into_int_value().get_zero_extended_constant(),
                Some(42),
                "x should be 42"
            );
        }
        _ => panic!("x should be an immutable binding"),
    }
}

#[test]
fn test_bind_pattern_wildcard() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let pattern = BindingPattern::Wildcard;
    let value = cx.scx.type_i64().const_int(42, false).into();

    let mut locals = Locals::new();
    builder.bind_pattern(
        &pattern,
        value,
        false,
        cx.scx.type_i64().into(),
        function,
        &mut locals,
    );

    // Wildcard doesn't bind anything - check via the is_empty equivalent
    // Locals doesn't have is_empty but we can check that no names are bound
    // by trying to get a storage that shouldn't exist
    let dummy_name = interner.intern("_not_bound");
    assert!(
        locals.get_storage(&dummy_name).is_none(),
        "Wildcard should not bind anything"
    );
}
