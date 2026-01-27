use inkwell::context::Context;
use ori_ir::ast::{Expr, ExprKind, FieldInit};
use ori_ir::{ExprArena, StringInterner, TypeId};

use crate::LLVMCodegen;

#[test]
fn test_tuple_creation() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> (int, int) { (10, 20) }
    let mut arena = ExprArena::new();

    // Tuple elements
    let elem1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let elem2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });

    // Allocate the tuple elements in the arena's expr_list
    let range = arena.alloc_expr_list([elem1, elem2]);

    // Create tuple expression
    let tuple_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Tuple(range),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_tuple");
    // The result type would be a tuple, but we're using opaque ptr for now
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Placeholder - we'd need a tuple TypeId
        tuple_expr,
        &arena,
        &expr_types,
    );

    println!("Tuple IR:\n{}", codegen.print_to_string());

    // Verify IR contains struct type
    // Note: LLVM may optimize insertvalue to a constant struct
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i64, i64 }")); // Struct type for tuple
    assert!(ir.contains("i64 10")); // First element
    assert!(ir.contains("i64 20")); // Second element
}

#[test]
fn test_struct_creation() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> { x: int, y: int } { Point { x: 10, y: 20 } }
    let mut arena = ExprArena::new();

    // Field values
    let val_x = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let val_y = arena.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });

    // Field initializers
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let field_range = arena.alloc_field_inits([
        FieldInit {
            name: x_name,
            value: Some(val_x),
            span: ori_ir::Span::new(0, 1),
        },
        FieldInit {
            name: y_name,
            value: Some(val_y),
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    // Struct literal: Point { x: 10, y: 20 }
    let point_name = interner.intern("Point");
    let struct_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Struct {
            name: point_name,
            fields: field_range,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_struct");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Placeholder
        struct_expr,
        &arena,
        &expr_types,
    );

    println!("Struct IR:\n{}", codegen.print_to_string());

    // Verify IR contains struct construction
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i64, i64 }")); // Struct type
}

#[test]
fn test_field_access() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int { Point { x: 10, y: 20 }.x }
    let mut arena = ExprArena::new();

    // Build struct
    let val_x = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let val_y = arena.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });

    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let field_range = arena.alloc_field_inits([
        FieldInit {
            name: x_name,
            value: Some(val_x),
            span: ori_ir::Span::new(0, 1),
        },
        FieldInit {
            name: y_name,
            value: Some(val_y),
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    let point_name = interner.intern("Point");
    let struct_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Struct {
            name: point_name,
            fields: field_range,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // Field access: .x
    let field_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Field {
            receiver: struct_expr,
            field: x_name,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_field");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        field_expr,
        &arena,
        &expr_types,
    );

    println!("Field Access IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 10 (the x field)
    let result = codegen.jit_execute_i64("test_field").expect("JIT failed");
    assert_eq!(result, 10);
}

#[test]
fn test_some() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> Option<int> { Some(42) }
    let mut arena = ExprArena::new();

    // Inner value
    let inner = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });

    // Some(42)
    let some_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Some(inner),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_some");
    let expr_types = vec![TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Placeholder - actual type would be Option<int>
        some_expr,
        &arena,
        &expr_types,
    );

    println!("Some IR:\n{}", codegen.print_to_string());

    // Verify IR contains tagged union structure
    // Note: LLVM constant-folds the struct, so we see the final value
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
    assert!(ir.contains("i8 1")); // Tag = 1 (Some)
    assert!(ir.contains("i64 42")); // Payload = 42
}

#[test]
fn test_none() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> Option<int> { None }
    let mut arena = ExprArena::new();

    // None
    let none_expr = arena.alloc_expr(Expr {
        kind: ExprKind::None,
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_none");
    let expr_types = vec![TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Placeholder
        none_expr,
        &arena,
        &expr_types,
    );

    println!("None IR:\n{}", codegen.print_to_string());

    // Verify IR contains tagged union with tag = 0
    // Note: LLVM optimizes { i8 0, i64 0 } to zeroinitializer
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
    // None produces zeroinitializer (tag=0, value=0)
    assert!(ir.contains("zeroinitializer") || ir.contains("i8 0"));
}

#[test]
fn test_ok() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> Result<int, int> { Ok(100) }
    let mut arena = ExprArena::new();

    // Inner value
    let inner = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });

    // Ok(100)
    let ok_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Ok(Some(inner)),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_ok");
    let expr_types = vec![TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Placeholder
        ok_expr,
        &arena,
        &expr_types,
    );

    println!("Ok IR:\n{}", codegen.print_to_string());

    // Verify IR contains tagged union structure
    // Note: LLVM constant-folds the struct, so we see the final value
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
    assert!(ir.contains("i8 0")); // Tag = 0 (Ok)
    assert!(ir.contains("i64 100")); // Payload = 100
}

#[test]
fn test_err() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> Result<int, int> { Err(999) }
    let mut arena = ExprArena::new();

    // Inner value
    let inner = arena.alloc_expr(Expr {
        kind: ExprKind::Int(999),
        span: ori_ir::Span::new(0, 1),
    });

    // Err(999)
    let err_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Err(Some(inner)),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_err");
    let expr_types = vec![TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Placeholder
        err_expr,
        &arena,
        &expr_types,
    );

    println!("Err IR:\n{}", codegen.print_to_string());

    // Verify IR contains tagged union structure
    // Note: LLVM constant-folds the struct, so we see the final value
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i8, i64 }")); // Tag + payload struct
    assert!(ir.contains("i8 1")); // Tag = 1 (Err)
    assert!(ir.contains("i64 999")); // Payload = 999
}

#[test]
fn test_range_literal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> Range { 1..10 }
    let mut arena = ExprArena::new();

    let start = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let end = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });

    let range_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Range {
            start: Some(start),
            end: Some(end),
            inclusive: false,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_range");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Returns struct, but we test IR generation
        range_expr,
        &arena,
        &expr_types,
    );

    println!("Range IR:\n{}", codegen.print_to_string());

    // Verify IR contains range struct type
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i64, i64, i1 }")); // Range struct type
}

#[test]
fn test_list_literal_empty() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> List { [] }
    let mut arena = ExprArena::new();

    let range = arena.alloc_expr_list([]);
    let list_expr = arena.alloc_expr(Expr {
        kind: ExprKind::List(range),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_empty_list");
    let expr_types = vec![TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        list_expr,
        &arena,
        &expr_types,
    );

    println!("Empty List IR:\n{}", codegen.print_to_string());

    // Verify IR contains list struct type
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i64, i64, ptr }")); // List struct type
}

#[test]
fn test_list_literal_with_elements() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> List { [1, 2, 3] }
    let mut arena = ExprArena::new();

    let elem1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let elem2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let elem3 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });

    let range = arena.alloc_expr_list([elem1, elem2, elem3]);
    let list_expr = arena.alloc_expr(Expr {
        kind: ExprKind::List(range),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_list_elems");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        list_expr,
        &arena,
        &expr_types,
    );

    println!("List with Elements IR:\n{}", codegen.print_to_string());

    // Verify IR contains list construction
    let ir = codegen.print_to_string();
    assert!(ir.contains("{ i64, i64, ptr }")); // List struct type
    assert!(ir.contains("i64 3")); // Length = 3
}
