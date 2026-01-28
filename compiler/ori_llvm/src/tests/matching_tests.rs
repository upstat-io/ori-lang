use inkwell::context::Context;
use ori_ir::ast::patterns::{MatchArm, MatchPattern};
use ori_ir::ast::{BinaryOp, Expr, ExprKind};
use ori_ir::{ExprArena, StringInterner, TypeId};

use super::helper::TestCodegen;

#[test]
fn test_match_literal() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int {
    //     match 1 {
    //         1 -> 100,
    //         _ -> 200,
    //     }
    // }
    let mut arena = ExprArena::new();

    // Scrutinee: 1
    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });

    // Arm 1: 1 -> 100
    let lit_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let body_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });

    // Arm 2: _ -> 200
    let body_2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(200),
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: MatchPattern::Literal(lit_1),
            guard: None,
            body: body_1,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body_2,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    // Match expression
    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_match_lit");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Literal IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 100 (matches first arm)
    let result = codegen.jit_execute_i64("test_match_lit").expect("JIT failed");
    assert_eq!(result, 100);
}

#[test]
fn test_match_wildcard() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int {
    //     match 5 {
    //         1 -> 100,
    //         _ -> 200,
    //     }
    // }
    let mut arena = ExprArena::new();

    // Scrutinee: 5
    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });

    // Arm 1: 1 -> 100
    let lit_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let body_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });

    // Arm 2: _ -> 200
    let body_2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(200),
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: MatchPattern::Literal(lit_1),
            guard: None,
            body: body_1,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body_2,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    // Match expression
    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_match_wild");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Wildcard IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 200 (matches wildcard)
    let result = codegen.jit_execute_i64("test_match_wild").expect("JIT failed");
    assert_eq!(result, 200);
}

#[test]
fn test_match_binding() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int {
    //     match 42 {
    //         x -> x + 1,
    //     }
    // }
    let mut arena = ExprArena::new();

    let x_name = interner.intern("x");

    // Scrutinee: 42
    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });

    // x + 1
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });
    let one = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let body = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: x_ref,
            right: one,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: MatchPattern::Binding(x_name),
            guard: None,
            body,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    // Match expression
    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_match_bind");
    let expr_types = vec![TypeId::INT; 10];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Binding IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 43 (42 + 1)
    let result = codegen.jit_execute_i64("test_match_bind").expect("JIT failed");
    assert_eq!(result, 43);
}

#[test]
fn test_match_with_guard_pass() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int {
    //     match 10 {
    //         x.match(x > 5) -> 100,
    //         _ -> 200,
    //     }
    // }
    let mut arena = ExprArena::new();

    let x_name = interner.intern("x");

    // Scrutinee: 10
    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });

    // Guard: x > 5
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });
    let five = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let guard = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Gt,
            left: x_ref,
            right: five,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // Body: 100
    let body_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });

    // Body: 200
    let body_2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(200),
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: MatchPattern::Binding(x_name),
            guard: Some(guard),
            body: body_1,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body_2,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_guard_pass");
    let expr_types = vec![TypeId::INT; 15];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Guard Pass IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 100 (guard passes: 10 > 5)
    let result = codegen.jit_execute_i64("test_guard_pass").expect("JIT failed");
    assert_eq!(result, 100);
}

#[test]
fn test_match_with_guard_fail() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int {
    //     match 3 {
    //         x.match(x > 5) -> 100,
    //         _ -> 200,
    //     }
    // }
    let mut arena = ExprArena::new();

    let x_name = interner.intern("x");

    // Scrutinee: 3
    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });

    // Guard: x > 5
    let x_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });
    let five = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let guard = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Gt,
            left: x_ref,
            right: five,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // Body: 100
    let body_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });

    // Body: 200
    let body_2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(200),
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: MatchPattern::Binding(x_name),
            guard: Some(guard),
            body: body_1,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body_2,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_guard_fail");
    let expr_types = vec![TypeId::INT; 15];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Guard Fail IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 200 (guard fails: 3 > 5 is false)
    let result = codegen.jit_execute_i64("test_guard_fail").expect("JIT failed");
    assert_eq!(result, 200);
}

#[test]
fn test_match_empty() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int { match 5 { } }
    let mut arena = ExprArena::new();

    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([]);

    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_match_empty");
    let expr_types = vec![TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Empty IR:\n{}", codegen.print_to_string());

    // JIT execute - should return default (0)
    let result = codegen.jit_execute_i64("test_match_empty").expect("JIT failed");
    assert_eq!(result, 0);
}

#[test]
fn test_match_multiple_literals() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> int {
    //     match 3 {
    //         1 -> 100,
    //         2 -> 200,
    //         3 -> 300,
    //         _ -> 400,
    //     }
    // }
    let mut arena = ExprArena::new();

    let scrutinee = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });

    let lit_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let lit_2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let lit_3 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(3),
        span: ori_ir::Span::new(0, 1),
    });

    let body_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(100),
        span: ori_ir::Span::new(0, 1),
    });
    let body_2 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(200),
        span: ori_ir::Span::new(0, 1),
    });
    let body_3 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(300),
        span: ori_ir::Span::new(0, 1),
    });
    let body_4 = arena.alloc_expr(Expr {
        kind: ExprKind::Int(400),
        span: ori_ir::Span::new(0, 1),
    });

    let arms = arena.alloc_arms([
        MatchArm {
            pattern: MatchPattern::Literal(lit_1),
            guard: None,
            body: body_1,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Literal(lit_2),
            guard: None,
            body: body_2,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Literal(lit_3),
            guard: None,
            body: body_3,
            span: ori_ir::Span::new(0, 1),
        },
        MatchArm {
            pattern: MatchPattern::Wildcard,
            guard: None,
            body: body_4,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    let match_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Match { scrutinee, arms },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_multi_lit");
    let expr_types = vec![TypeId::INT; 15];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        match_expr,
        &arena,
        &expr_types,
    );

    println!("Match Multiple Literals IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 300 (matches third arm)
    let result = codegen.jit_execute_i64("test_multi_lit").expect("JIT failed");
    assert_eq!(result, 300);
}
