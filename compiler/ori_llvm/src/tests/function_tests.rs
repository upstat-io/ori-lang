use inkwell::context::Context;
use ori_ir::ast::{BinaryOp, Expr, ExprKind};
use ori_ir::{ExprArena, ExprId, Name, StringInterner, TypeId};

use super::helper::TestCodegen;

// Helper functions to reduce test boilerplate

fn make_ident(arena: &mut ExprArena, name: Name) -> ExprId {
    arena.alloc_expr(Expr {
        kind: ExprKind::Ident(name),
        span: ori_ir::Span::new(0, 1),
    })
}

fn make_int(arena: &mut ExprArena, value: i64) -> ExprId {
    arena.alloc_expr(Expr {
        kind: ExprKind::Int(value),
        span: ori_ir::Span::new(0, 1),
    })
}

fn make_binary(arena: &mut ExprArena, op: BinaryOp, left: ExprId, right: ExprId) -> ExprId {
    arena.alloc_expr(Expr {
        kind: ExprKind::Binary { op, left, right },
        span: ori_ir::Span::new(0, 1),
    })
}

fn make_call(arena: &mut ExprArena, func_name: Name, arg: ExprId) -> ExprId {
    let func = make_ident(arena, func_name);
    let args = arena.alloc_expr_list_inline(&[arg]);
    arena.alloc_expr(Expr {
        kind: ExprKind::Call { func, args },
        span: ori_ir::Span::new(0, 1),
    })
}

fn make_if(arena: &mut ExprArena, cond: ExprId, then_br: ExprId, else_br: ExprId) -> ExprId {
    arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond,
            then_branch: then_br,
            else_branch: Some(else_br),
        },
        span: ori_ir::Span::new(0, 1),
    })
}

#[test]
fn test_function_with_params() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn add(a: int, b: int) -> int { a + b }
    let mut arena = ExprArena::new();

    let a_name = interner.intern("a");
    let b_name = interner.intern("b");

    let a_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(a_name),
        span: ori_ir::Span::new(0, 1),
    });
    let b_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(b_name),
        span: ori_ir::Span::new(0, 1),
    });
    let add_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: a_ref,
            right: b_ref,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("add");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[a_name, b_name],
        &[TypeId::INT, TypeId::INT],
        TypeId::INT,
        add_expr,
        &arena,
        &expr_types,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Generated LLVM IR:\n{}", codegen.print_to_string());
    }

    // We can't easily JIT a function with params without a wrapper,
    // but we can verify the IR is valid
    assert!(codegen.print_to_string().contains("define i64 @add(i64"));
}

#[test]
fn test_function_call_simple() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create:
    //   fn add(a: int, b: int) -> int { a + b }
    //   fn main() -> int { add(10, 20) }

    let mut arena = ExprArena::new();

    // First, create the add function
    let a_name = interner.intern("a");
    let b_name = interner.intern("b");

    let a_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(a_name),
        span: ori_ir::Span::new(0, 1),
    });
    let b_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(b_name),
        span: ori_ir::Span::new(0, 1),
    });
    let add_body = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: a_ref,
            right: b_ref,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let add_fn_name = interner.intern("add");
    let add_expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        add_fn_name,
        &[a_name, b_name],
        &[TypeId::INT, TypeId::INT],
        TypeId::INT,
        add_body,
        &arena,
        &add_expr_types,
    );

    // Now create the main function that calls add(10, 20)
    let mut arena2 = ExprArena::new();

    // Arguments: 10, 20
    let first_arg = arena2.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let second_arg = arena2.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });
    let arg_list = arena2.alloc_expr_list_inline(&[first_arg, second_arg]);

    // Function reference: add
    let func = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(add_fn_name),
        span: ori_ir::Span::new(0, 1),
    });

    // Call: add(10, 20)
    let call_expr = arena2.alloc_expr(Expr {
        kind: ExprKind::Call {
            func,
            args: arg_list,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let main_fn_name = interner.intern("main");
    let main_expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        main_fn_name,
        &[],
        &[],
        TypeId::INT,
        call_expr,
        &arena2,
        &main_expr_types,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Function Call IR:\n{}", codegen.print_to_string());
    }

    // JIT execute - should return 30 (10 + 20)
    let result = codegen.jit_execute_i64("main").expect("JIT failed");
    assert_eq!(result, 30);
}

#[test]
fn test_function_call_nested() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create:
    //   fn double(x: int) -> int { x + x }
    //   fn main() -> int { double(double(5)) }
    //   Should return 20

    let mut arena = ExprArena::new();
    let x_name = interner.intern("x");

    // double function: x + x
    let x_ref1 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });
    let x_ref2 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(x_name),
        span: ori_ir::Span::new(0, 1),
    });
    let double_body = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: x_ref1,
            right: x_ref2,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let double_fn_name = interner.intern("double");
    let double_expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        double_fn_name,
        &[x_name],
        &[TypeId::INT],
        TypeId::INT,
        double_body,
        &arena,
        &double_expr_types,
    );

    // main function: double(double(5))
    let mut arena2 = ExprArena::new();

    // Inner call: double(5)
    let five = arena2.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let inner_args = arena2.alloc_expr_list_inline(&[five]);
    let double_ref_inner = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(double_fn_name),
        span: ori_ir::Span::new(0, 1),
    });
    let inner_call = arena2.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: double_ref_inner,
            args: inner_args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // Outer call: double(double(5))
    let outer_args = arena2.alloc_expr_list_inline(&[inner_call]);
    let double_ref_outer = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(double_fn_name),
        span: ori_ir::Span::new(0, 1),
    });
    let outer_call = arena2.alloc_expr(Expr {
        kind: ExprKind::Call {
            func: double_ref_outer,
            args: outer_args,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let main_fn_name = interner.intern("main");
    let main_expr_types = vec![TypeId::INT; 10];

    codegen.compile_function(
        main_fn_name,
        &[],
        &[],
        TypeId::INT,
        outer_call,
        &arena2,
        &main_expr_types,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Nested Call IR:\n{}", codegen.print_to_string());
    }

    // JIT execute - should return 20 (double(double(5)) = double(10) = 20)
    let result = codegen.jit_execute_i64("main").expect("JIT failed");
    assert_eq!(result, 20);
}

/// Build the factorial function body: if n <= 1 then 1 else n * factorial(n - 1)
fn build_factorial_body(arena: &mut ExprArena, n_name: Name, fn_name: Name) -> ExprId {
    // Condition: n <= 1
    let n_ref = make_ident(arena, n_name);
    let one = make_int(arena, 1);
    let cond = make_binary(arena, BinaryOp::LtEq, n_ref, one);
    // Then branch: 1
    let then_branch = make_int(arena, 1);
    // n - 1
    let n_ref2 = make_ident(arena, n_name);
    let one2 = make_int(arena, 1);
    let n_minus_1 = make_binary(arena, BinaryOp::Sub, n_ref2, one2);
    // factorial(n - 1)
    let rec_call = make_call(arena, fn_name, n_minus_1);
    // n * factorial(n - 1)
    let n_ref3 = make_ident(arena, n_name);
    let else_branch = make_binary(arena, BinaryOp::Mul, n_ref3, rec_call);
    // if n <= 1 then 1 else n * factorial(n - 1)
    make_if(arena, cond, then_branch, else_branch)
}

#[test]
fn test_recursive_function() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create factorial: fn factorial(n: int) -> int = if n <= 1 then 1 else n * factorial(n - 1)
    // main: fn main() -> int = factorial(5) => 120
    let mut arena = ExprArena::new();
    let n_name = interner.intern("n");
    let factorial_fn_name = interner.intern("factorial");

    let factorial_body = build_factorial_body(&mut arena, n_name, factorial_fn_name);

    codegen.compile_function(
        factorial_fn_name,
        &[n_name],
        &[TypeId::INT],
        TypeId::INT,
        factorial_body,
        &arena,
        &[TypeId::INT; 20],
    );

    // main function: factorial(5)
    let mut arena2 = ExprArena::new();
    let five = make_int(&mut arena2, 5);
    let call = make_call(&mut arena2, factorial_fn_name, five);

    let main_fn_name = interner.intern("main");
    codegen.compile_function(
        main_fn_name,
        &[],
        &[],
        TypeId::INT,
        call,
        &arena2,
        &[TypeId::INT; 5],
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Factorial IR:\n{}", codegen.print_to_string());
    }

    // JIT execute - should return 120 (5! = 120)
    let result = codegen.jit_execute_i64("main").expect("JIT failed");
    assert_eq!(result, 120);
}

#[test]
fn test_function_ref() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create a helper function first
    let mut arena = ExprArena::new();

    let x = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });

    let helper_name = interner.intern("helper");
    let expr_types = vec![TypeId::INT];

    codegen.compile_function(helper_name, &[], &[], TypeId::INT, x, &arena, &expr_types);

    // Now test FunctionRef
    let mut arena2 = ExprArena::new();

    let func_ref_expr = arena2.alloc_expr(Expr {
        kind: ExprKind::FunctionRef(helper_name),
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_func_ref");
    let expr_types2 = vec![TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Returns function pointer
        func_ref_expr,
        &arena2,
        &expr_types2,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Function Ref IR:\n{}", codegen.print_to_string());
    }

    // Verify IR contains reference to helper function
    let ir = codegen.print_to_string();
    assert!(ir.contains("@helper")); // Reference to helper function
}

#[test]
fn test_lambda_simple() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = TestCodegen::new(&context, &interner, "test");

    // Create: fn test() -> fn { x -> x + 1 }
    let mut arena = ExprArena::new();

    let x_name = interner.intern("x");

    // Parameter list with one param
    let params = arena.alloc_params([ori_ir::ast::Param {
        name: x_name,
        ty: None,
        span: ori_ir::Span::new(0, 1),
    }]);

    // Body: x + 1
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

    let lambda_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Lambda {
            params,
            ret_ty: None,
            body,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("test_lambda");
    let expr_types = vec![TypeId::INT; 5];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT, // Returns function pointer
        lambda_expr,
        &arena,
        &expr_types,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        println!("Lambda IR:\n{}", codegen.print_to_string());
    }

    // Verify IR contains lambda function
    let ir = codegen.print_to_string();
    assert!(ir.contains("__lambda_")); // Lambda function name
}
