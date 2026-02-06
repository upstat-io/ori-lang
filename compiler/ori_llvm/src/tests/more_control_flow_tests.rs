//! Additional tests for control flow.

use inkwell::context::Context;
use ori_ir::ast::{BinaryOp, BindingPattern, Expr, ExprKind, Stmt, StmtKind};
use ori_ir::{ExprArena, ExprId, StringInterner};
use ori_types::Idx;

use super::helper::TestCodegen;

#[test]
fn test_nested_if() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: if true then (if false then 1 else 2) else 3 = 2
    let inner_cond = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let one = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let three = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });

    let inner_if = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond: inner_cond,
            then_branch: one,
            else_branch: two,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let outer_cond = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });

    let outer_if = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond: outer_cond,
            then_branch: inner_if,
            else_branch: three,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_nested_if");
    let expr_types = vec![
        Idx::BOOL,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::BOOL,
        Idx::INT,
        Idx::INT,
    ];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, outer_if, &arena, &expr_types);

    let result = codegen
        .jit_execute_i64("test_nested_if")
        .expect("JIT failed");
    assert_eq!(result, 2);
}

#[test]
fn test_if_with_comparison() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: if 5 > 3 then 100 else 200 = 100
    let five = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let three = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });
    let cond = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Gt,
            left: five,
            right: three,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let then_val = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });
    let else_val = arena.alloc_expr(Expr {
        kind: ExprKind::Int(200),
        span: ori_ir::Span::new(0, 1),
    });

    let if_expr = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond,
            then_branch: then_val,
            else_branch: else_val,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_if_cmp");
    let expr_types = vec![Idx::INT, Idx::INT, Idx::BOOL, Idx::INT, Idx::INT, Idx::INT];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, if_expr, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_if_cmp").expect("JIT failed");
    assert_eq!(result, 100);
}

#[test]
fn test_if_without_else() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: if true then 42
    let cond = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(true),
        span: ori_ir::Span::new(0, 1),
    });
    let then_val = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });

    let if_expr = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond,
            then_branch: then_val,
            else_branch: ExprId::INVALID,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_if_no_else");
    let expr_types = vec![Idx::BOOL, Idx::INT, Idx::INT];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, if_expr, &arena, &expr_types);

    let result = codegen
        .jit_execute_i64("test_if_no_else")
        .expect("JIT failed");
    assert_eq!(result, 42);
}

#[test]
fn test_if_chain() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: if false then 1 else if false then 2 else 3 = 3
    let false1 = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let false2 = arena.alloc_expr(Expr {
        kind: ExprKind::Bool(false),
        span: ori_ir::Span::new(0, 1),
    });
    let one = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let three = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });

    let inner_if = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond: false2,
            then_branch: two,
            else_branch: three,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let outer_if = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond: false1,
            then_branch: one,
            else_branch: inner_if,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_if_chain");
    let expr_types = vec![
        Idx::BOOL,
        Idx::BOOL,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
    ];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, outer_if, &arena, &expr_types);

    let result = codegen
        .jit_execute_i64("test_if_chain")
        .expect("JIT failed");
    assert_eq!(result, 3);
}

#[test]
fn test_multiple_let_bindings() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: let x = 10; let y = 20; x + y
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let ten = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let twenty = arena.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });
    let y_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(y_name),
        span: ori_ir::Span::new(0, 1),
    });

    let add_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: x_ref,
            right: y_ref,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // Create block with two let bindings
    let x_pattern = arena.alloc_binding_pattern(BindingPattern::Name(x_name));
    let let_x = Stmt {
        kind: StmtKind::Let {
            pattern: x_pattern,
            ty: ori_ir::ParsedTypeId::INVALID,
            mutable: false,
            init: ten,
        },
        span: ori_ir::Span::new(0, 1),
    };
    let y_pattern = arena.alloc_binding_pattern(BindingPattern::Name(y_name));
    let let_y = Stmt {
        kind: StmtKind::Let {
            pattern: y_pattern,
            ty: ori_ir::ParsedTypeId::INVALID,
            mutable: false,
            init: twenty,
        },
        span: ori_ir::Span::new(0, 1),
    };

    // Allocate statements to arena and create range
    let first_stmt_id = arena.alloc_stmt(let_x);
    arena.alloc_stmt(let_y);
    let stmt_range = arena.alloc_stmt_range(first_stmt_id.index() as u32, 2);

    let block = arena.alloc_expr(Expr {
        kind: ExprKind::Block {
            stmts: stmt_range,
            result: add_expr,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_multi_let");
    let expr_types = vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT, Idx::INT, Idx::INT];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, block, &arena, &expr_types);

    let result = codegen
        .jit_execute_i64("test_multi_let")
        .expect("JIT failed");
    assert_eq!(result, 30);
}

#[test]
fn test_shadowing() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    let mut arena = ExprArena::new();

    // Build: let x = 5; let x = 10; x (should be 10)
    let x_name = interner.intern("x");

    let five = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let ten = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });

    let x_pattern_first = arena.alloc_binding_pattern(BindingPattern::Name(x_name));
    let let_x_first = Stmt {
        kind: StmtKind::Let {
            pattern: x_pattern_first,
            ty: ori_ir::ParsedTypeId::INVALID,
            mutable: false,
            init: five,
        },
        span: ori_ir::Span::new(0, 1),
    };
    let x_pattern_shadow = arena.alloc_binding_pattern(BindingPattern::Name(x_name));
    let let_x_shadow = Stmt {
        kind: StmtKind::Let {
            pattern: x_pattern_shadow,
            ty: ori_ir::ParsedTypeId::INVALID,
            mutable: false,
            init: ten,
        },
        span: ori_ir::Span::new(0, 1),
    };

    // Allocate statements to arena and create range
    let first_stmt_id = arena.alloc_stmt(let_x_first);
    arena.alloc_stmt(let_x_shadow);
    let stmt_range = arena.alloc_stmt_range(first_stmt_id.index() as u32, 2);

    let block = arena.alloc_expr(Expr {
        kind: ExprKind::Block {
            stmts: stmt_range,
            result: x_ref,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_shadow");
    let expr_types = vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT];

    codegen.compile_function(fn_name, &[], &[], Idx::INT, block, &arena, &expr_types);

    let result = codegen.jit_execute_i64("test_shadow").expect("JIT failed");
    assert_eq!(result, 10);
}
