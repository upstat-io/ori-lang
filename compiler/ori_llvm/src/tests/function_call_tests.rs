//! Tests for function call compilation including closures and indirect calls.

use inkwell::context::Context;
use ori_ir::ast::{Expr, ExprKind};
use ori_ir::{ExprArena, Span, StringInterner};
use ori_types::Idx;

use super::helper::setup_builder_test;
use crate::builder::{Builder, Locals};

#[test]
fn test_call_builtin_str() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: str(42)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });
    let args = arena.alloc_expr_list_inline(&[arg]);

    let str_name = interner.intern("str");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(str_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT, Idx::STR];
    let mut locals = Locals::new();

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "str(42) should produce a value");
}

#[test]
fn test_call_builtin_int() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: int(3.14)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Float(3.5f64.to_bits()),
        span: Span::new(0, 1),
    });
    let args = arena.alloc_expr_list_inline(&[arg]);

    let int_name = interner.intern("int");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(int_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::FLOAT, Idx::FLOAT, Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "int(3.14) should produce a value");
}

#[test]
fn test_call_builtin_float() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: float(42)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });
    let args = arena.alloc_expr_list_inline(&[arg]);

    let float_name = interner.intern("float");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(float_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT, Idx::FLOAT];
    let mut locals = Locals::new();

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "float(42) should produce a value");
}

#[test]
fn test_call_builtin_byte() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: byte(300)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(300),
        span: Span::new(0, 1),
    });
    let args = arena.alloc_expr_list_inline(&[arg]);

    let byte_name = interner.intern("byte");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(byte_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT, Idx::INT, Idx::BYTE];
    let mut locals = Locals::new();

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_some(), "byte(300) should produce a value");
}

#[test]
fn test_call_non_ident_returns_none() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: 42() - calling an int (not valid)
    let args = arena.alloc_expr_list_inline(&[]);
    let func_int = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    let result = builder.compile_call(
        func_int,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(result.is_none(), "Calling an int should return None");
}

#[test]
fn test_call_unknown_function_returns_none() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: unknown_func()
    let args = arena.alloc_expr_list_inline(&[]);
    let unknown_name = interner.intern("unknown_func");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(unknown_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::UNIT];
    let mut locals = Locals::new();

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_none(),
        "Calling unknown function should return None"
    );
}

#[test]
fn test_call_closure_from_locals() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    // Create: f(10) where f is in locals as a function pointer (i64)
    let arg = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: Span::new(0, 1),
    });
    let args = arena.alloc_expr_list_inline(&[arg]);

    let f_name = interner.intern("f");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(f_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT; 5];
    let mut locals = Locals::new();

    // Add a closure to locals as an i64 (function pointer)
    let fn_ptr_val = cx.scx.type_i64().const_int(0, false);
    locals.bind_immutable(f_name, fn_ptr_val.into());

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    // Should produce a value (even though we're calling address 0, we're testing the codegen)
    assert!(result.is_some(), "Closure call should produce a value");
}

#[test]
fn test_call_closure_as_pointer() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    let args = arena.alloc_expr_list_inline(&[]);
    let f_name = interner.intern("ptr_fn");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(f_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    // Add a closure to locals as a pointer
    let ptr_val = cx.scx.type_ptr().const_null();
    locals.bind_immutable(f_name, ptr_val.into());

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "Pointer closure call should produce a value"
    );
}

#[test]
fn test_call_closure_as_struct_with_captures() {
    let context = Context::create();
    let interner = StringInterner::new();
    let (cx, function) = setup_builder_test(&context, &interner);

    let mut arena = ExprArena::new();

    let args = arena.alloc_expr_list_inline(&[]);
    let f_name = interner.intern("struct_fn");
    let func_ident = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(f_name),
        span: Span::new(0, 1),
    });

    let entry_bb = cx.llcx().append_basic_block(function, "entry");
    let builder = Builder::build(&cx, entry_bb);

    let expr_types = vec![Idx::INT];
    let mut locals = Locals::new();

    // Add a closure struct: { i8 tag, i64 fn_ptr, i64 capture0 }
    let struct_type = cx.llcx().struct_type(
        &[
            cx.scx.type_i8().into(),
            cx.scx.type_i64().into(),
            cx.scx.type_i64().into(),
        ],
        false,
    );
    let struct_val = struct_type.const_zero();
    locals.bind_immutable(f_name, struct_val.into());

    let result = builder.compile_call(
        func_ident,
        args,
        &arena,
        &expr_types,
        &mut locals,
        function,
        None,
    );

    assert!(
        result.is_some(),
        "Struct closure call should produce a value"
    );
}
