use inkwell::context::Context;
use ori_ir::ast::{BinaryOp, Expr, ExprKind};
use ori_ir::{ExprArena, StringInterner, TypeId};

use crate::LLVMCodegen;

#[test]
fn test_function_with_params() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

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

    println!("Generated LLVM IR:\n{}", codegen.print_to_string());

    // We can't easily JIT a function with params without a wrapper,
    // but we can verify the IR is valid
    assert!(codegen.print_to_string().contains("define i64 @add(i64"));
}

#[test]
fn test_function_call_simple() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

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
    let arg1 = arena2.alloc_expr(Expr {
        kind: ExprKind::Int(10),
        span: ori_ir::Span::new(0, 1),
    });
    let arg2 = arena2.alloc_expr(Expr {
        kind: ExprKind::Int(20),
        span: ori_ir::Span::new(0, 1),
    });
    let args = arena2.alloc_expr_list([arg1, arg2]);

    // Function reference: add
    let func = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(add_fn_name),
        span: ori_ir::Span::new(0, 1),
    });

    // Call: add(10, 20)
    let call_expr = arena2.alloc_expr(Expr {
        kind: ExprKind::Call { func, args },
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

    println!("Function Call IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 30 (10 + 20)
    let result = codegen.jit_execute_i64("main").expect("JIT failed");
    assert_eq!(result, 30);
}

#[test]
fn test_function_call_nested() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

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
    let inner_args = arena2.alloc_expr_list([five]);
    let double_ref_inner = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(double_fn_name),
        span: ori_ir::Span::new(0, 1),
    });
    let inner_call = arena2.alloc_expr(Expr {
        kind: ExprKind::Call { func: double_ref_inner, args: inner_args },
        span: ori_ir::Span::new(0, 1),
    });

    // Outer call: double(double(5))
    let outer_args = arena2.alloc_expr_list([inner_call]);
    let double_ref_outer = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(double_fn_name),
        span: ori_ir::Span::new(0, 1),
    });
    let outer_call = arena2.alloc_expr(Expr {
        kind: ExprKind::Call { func: double_ref_outer, args: outer_args },
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

    println!("Nested Call IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 20 (double(double(5)) = double(10) = 20)
    let result = codegen.jit_execute_i64("main").expect("JIT failed");
    assert_eq!(result, 20);
}

#[test]
fn test_recursive_function() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create factorial:
    //   fn factorial(n: int) -> int {
    //       if n <= 1 then 1 else n * factorial(n - 1)
    //   }
    //   fn main() -> int { factorial(5) }
    //   Should return 120

    let mut arena = ExprArena::new();
    let n_name = interner.intern("n");
    let factorial_fn_name = interner.intern("factorial");

    // n <= 1
    let n_ref1 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: ori_ir::Span::new(0, 1),
    });
    let one_cond = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let cond = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::LtEq,
            left: n_ref1,
            right: one_cond,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // then branch: 1
    let then_branch = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });

    // n - 1
    let n_ref2 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: ori_ir::Span::new(0, 1),
    });
    let one_sub = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1),
        span: ori_ir::Span::new(0, 1),
    });
    let n_minus_1 = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Sub,
            left: n_ref2,
            right: one_sub,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // factorial(n - 1)
    let rec_args = arena.alloc_expr_list([n_minus_1]);
    let factorial_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(factorial_fn_name),
        span: ori_ir::Span::new(0, 1),
    });
    let rec_call = arena.alloc_expr(Expr {
        kind: ExprKind::Call { func: factorial_ref, args: rec_args },
        span: ori_ir::Span::new(0, 1),
    });

    // n * factorial(n - 1)
    let n_ref3 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: ori_ir::Span::new(0, 1),
    });
    let else_branch = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: n_ref3,
            right: rec_call,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // if n <= 1 then 1 else n * factorial(n - 1)
    let factorial_body = arena.alloc_expr(Expr {
        kind: ExprKind::If {
            cond,
            then_branch,
            else_branch: Some(else_branch),
        },
        span: ori_ir::Span::new(0, 1),
    });

    let factorial_expr_types = vec![TypeId::INT; 20];

    codegen.compile_function(
        factorial_fn_name,
        &[n_name],
        &[TypeId::INT],
        TypeId::INT,
        factorial_body,
        &arena,
        &factorial_expr_types,
    );

    // main function: factorial(5)
    let mut arena2 = ExprArena::new();

    let five = arena2.alloc_expr(Expr {
        kind: ExprKind::Int(5),
        span: ori_ir::Span::new(0, 1),
    });
    let args = arena2.alloc_expr_list([five]);
    let factorial_ref_main = arena2.alloc_expr(Expr {
        kind: ExprKind::Ident(factorial_fn_name),
        span: ori_ir::Span::new(0, 1),
    });
    let call = arena2.alloc_expr(Expr {
        kind: ExprKind::Call { func: factorial_ref_main, args },
        span: ori_ir::Span::new(0, 1),
    });

    let main_fn_name = interner.intern("main");
    let main_expr_types = vec![TypeId::INT; 5];

    codegen.compile_function(
        main_fn_name,
        &[],
        &[],
        TypeId::INT,
        call,
        &arena2,
        &main_expr_types,
    );

    println!("Factorial IR:\n{}", codegen.print_to_string());

    // JIT execute - should return 120 (5! = 120)
    let result = codegen.jit_execute_i64("main").expect("JIT failed");
    assert_eq!(result, 120);
}

#[test]
fn test_function_ref() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create a helper function first
    let mut arena = ExprArena::new();

    let x = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });

    let helper_name = interner.intern("helper");
    let expr_types = vec![TypeId::INT];

    codegen.compile_function(
        helper_name,
        &[],
        &[],
        TypeId::INT,
        x,
        &arena,
        &expr_types,
    );

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
        TypeId::INT, // Returns pointer, but we test IR generation
        func_ref_expr,
        &arena2,
        &expr_types2,
    );

    println!("Function Ref IR:\n{}", codegen.print_to_string());

    // Verify IR contains reference to helper function
    let ir = codegen.print_to_string();
    assert!(ir.contains("@helper")); // Reference to helper function
}

#[test]
fn test_lambda_simple() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "test");

    // Create: fn test() -> fn { x -> x + 1 }
    let mut arena = ExprArena::new();

    let x_name = interner.intern("x");

    // Parameter list with one param
    let params = arena.alloc_params([
        ori_ir::ast::Param {
            name: x_name,
            ty: None,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

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

    println!("Lambda IR:\n{}", codegen.print_to_string());

    // Verify IR contains lambda function
    let ir = codegen.print_to_string();
    assert!(ir.contains("__lambda_")); // Lambda function name
}
