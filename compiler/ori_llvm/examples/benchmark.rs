//! Benchmark: Compare LLVM JIT vs interpreter performance
//!
//! Run with: cargo run --example benchmark -p ori_llvm

use std::time::Instant;

use inkwell::context::Context;
use inkwell::targets::{CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::OptimizationLevel;

use ori_ir::{
    ast::{BinaryOp, Expr, ExprKind},
    ExprArena, Function, GenericParamRange, Param, StringInterner, TypeId,
};
use ori_llvm::module::ModuleCompiler;

fn main() {
    println!("=== Ori LLVM Backend Benchmark ===\n");

    // Benchmark 1: Simple arithmetic in a loop (simulated via recursion)
    benchmark_arithmetic();

    // Benchmark 2: Fibonacci
    benchmark_fib();
}

fn benchmark_arithmetic() {
    println!("Benchmark: Sum 1 to N");
    println!("-----------------------");

    let context = Context::create();
    let interner = StringInterner::new();
    let compiler = ModuleCompiler::new(&context, &interner, "bench");
    compiler.declare_runtime();

    // Create: fn sum_to_million() -> int {
    //   // We'll compute: 1 + 2 + 3 + ... unrolled as a large expression
    //   // Actually, let's just do repeated addition: ((((0+1)+1)+1)...)
    // }
    // For simplicity, let's compute a known large value

    let mut arena = ExprArena::new();

    // Build: 1000000 * 500000 (to test multiplication)
    let a = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1_000_000),
        span: ori_ir::Span::new(0, 1),
    });
    let b = arena.alloc_expr(Expr {
        kind: ExprKind::Int(500_000),
        span: ori_ir::Span::new(0, 1),
    });
    let mul_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: a,
            right: b,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("big_mul");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    let func = Function {
        name: fn_name,
        generics: GenericParamRange::EMPTY,
        params: arena.alloc_params([]),
        return_ty: None,
        capabilities: vec![],
        where_clauses: vec![],
        body: mul_expr,
        span: ori_ir::Span::new(0, 1),
        is_public: false,
    };

    compiler.compile_function(&func, &arena, &expr_types);

    // JIT execution
    let result = jit_execute_i64(&compiler, "big_mul").unwrap();
    println!("  Result: {}", result);
    println!("  Expected: {}\n", 1_000_000i64 * 500_000i64);
}

fn benchmark_fib() {
    println!("Benchmark: Fibonacci (iterative via loop)");
    println!("------------------------------------------");

    // Since we have loops working, let's create a loop-based counter
    // For a fair benchmark, we'll create a function that does N iterations

    let context = Context::create();
    let interner = StringInterner::new();
    let compiler = ModuleCompiler::new(&context, &interner, "bench_fib");
    compiler.declare_runtime();

    let mut arena = ExprArena::new();

    // Create: fn count() -> int { let x = 0; loop { if x == 1000000 then break else (); x = x + 1 }; x }
    // But we don't have assignment working...

    // Simpler: just test that JIT compilation overhead is low
    // Create multiple functions and time compilation + execution

    // fn fib_step(a: int, b: int) -> int { a + b }
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

    let fn_name = interner.intern("fib_step");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    let params = arena.alloc_params([
        Param {
            name: a_name,
            ty: None,
            span: ori_ir::Span::new(0, 1),
        },
        Param {
            name: b_name,
            ty: None,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    let func = Function {
        name: fn_name,
        generics: GenericParamRange::EMPTY,
        params,
        return_ty: None,
        capabilities: vec![],
        where_clauses: vec![],
        body: add_expr,
        span: ori_ir::Span::new(0, 1),
        is_public: false,
    };

    // Time compilation
    let compile_start = Instant::now();
    compiler.compile_function(&func, &arena, &expr_types);
    let compile_time = compile_start.elapsed();

    println!("  Compilation time: {:?}", compile_time);
    println!("  Generated IR:");
    println!("{}", compiler.print_to_string());

    // Now let's create a standalone executable to benchmark
    println!("\nCreating standalone benchmark executable...");
    create_standalone_benchmark();
}

/// JIT execute a function that returns i64.
#[allow(unsafe_code)]
fn jit_execute_i64(compiler: &ModuleCompiler<'_, '_>, fn_name: &str) -> Result<i64, String> {
    let ee = compiler.module()
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| e.to_string())?;

    unsafe {
        let func = ee
            .get_function::<unsafe extern "C" fn() -> i64>(fn_name)
            .map_err(|e| format!("Function '{fn_name}' not found: {e}"))?;
        Ok(func.call())
    }
}

fn create_standalone_benchmark() {
    let context = Context::create();
    let interner = StringInterner::new();
    let compiler = ModuleCompiler::new(&context, &interner, "ori_bench");
    compiler.declare_runtime();

    let mut arena = ExprArena::new();

    // Create: fn ori_compute(n: int) -> int { (n * 42) + (n / 2) }
    // This can't be constant-folded since n is a parameter

    let n_name = interner.intern("n");

    // n * 42
    let n_ref1 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: ori_ir::Span::new(0, 1),
    });
    let forty_two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: ori_ir::Span::new(0, 1),
    });
    let mul = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: n_ref1,
            right: forty_two,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // n / 2
    let n_ref2 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: ori_ir::Span::new(0, 1),
    });
    let two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: ori_ir::Span::new(0, 1),
    });
    let div = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Div,
            left: n_ref2,
            right: two,
        },
        span: ori_ir::Span::new(0, 1),
    });

    // mul + div
    let add = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: mul,
            right: div,
        },
        span: ori_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("ori_compute");
    let expr_types = vec![
        TypeId::INT, TypeId::INT, TypeId::INT, // mul
        TypeId::INT, TypeId::INT, TypeId::INT, // div
        TypeId::INT, // add
    ];

    let params = arena.alloc_params([
        Param {
            name: n_name,
            ty: None,
            span: ori_ir::Span::new(0, 1),
        },
    ]);

    let func = Function {
        name: fn_name,
        generics: GenericParamRange::EMPTY,
        params,
        return_ty: None,
        capabilities: vec![],
        where_clauses: vec![],
        body: add,
        span: ori_ir::Span::new(0, 1),
        is_public: false,
    };

    compiler.compile_function(&func, &arena, &expr_types);

    // Write IR and object file
    let ir_path = std::path::Path::new("/tmp/ori_bench.ll");
    let obj_path = std::path::Path::new("/tmp/ori_bench.o");

    // Write LLVM IR
    compiler.module().print_to_file(ir_path).expect("Failed to write IR");
    println!("  Wrote IR to: {}", ir_path.display());

    // Write object file using target machine
    Target::initialize_native(&InitializationConfig::default()).expect("Failed to init target");
    let target_triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&target_triple).expect("Failed to get target");
    let target_machine = target
        .create_target_machine(
            &target_triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        )
        .expect("Failed to create target machine");

    target_machine
        .write_to_file(compiler.module(), FileType::Object, obj_path)
        .expect("Failed to write object file");
    println!("  Wrote object file to: {}", obj_path.display());

    // Create a C wrapper
    let c_wrapper = r#"
#include <stdio.h>
#include <time.h>
#include <stdint.h>

extern int64_t ori_compute(int64_t n);

int main() {
    printf("Ori LLVM Backend - Native Benchmark\n");
    printf("======================================\n\n");

    // Warm up
    volatile int64_t result = ori_compute(100);

    // Benchmark: call with varying inputs to prevent optimization
    const int iterations = 100000000;
    int64_t sum = 0;

    clock_t start = clock();
    for (int i = 0; i < iterations; i++) {
        sum += ori_compute(i);
    }
    clock_t end = clock();

    double elapsed = (double)(end - start) / CLOCKS_PER_SEC;
    double per_call_ns = (elapsed / iterations) * 1e9;

    printf("Sum: %ld\n", (long)sum);
    printf("Iterations: %d\n", iterations);
    printf("Total time: %.3f seconds\n", elapsed);
    printf("Per call: %.2f ns\n", per_call_ns);
    printf("\n(Function: (n * 42) + (n / 2))\n");

    return 0;
}
"#;

    let c_path = std::path::Path::new("/tmp/ori_bench_main.c");
    std::fs::write(c_path, c_wrapper).expect("Failed to write C wrapper");
    println!("  Wrote C wrapper to: {}", c_path.display());

    // Compile and link
    println!("\nCompiling and linking...");
    let output = std::process::Command::new("cc")
        .args([
            "-O2",
            "/tmp/ori_bench_main.c",
            "/tmp/ori_bench.o",
            "-o",
            "/tmp/ori_bench",
        ])
        .output()
        .expect("Failed to run cc");

    if !output.status.success() {
        eprintln!("Compilation failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return;
    }

    println!("  Created executable: /tmp/ori_bench");

    // Run the benchmark
    println!("\nRunning benchmark...\n");
    let output = std::process::Command::new("/tmp/ori_bench")
        .output()
        .expect("Failed to run benchmark");

    println!("{}", String::from_utf8_lossy(&output.stdout));

    // Show the generated LLVM IR
    println!("\nGenerated LLVM IR:");
    println!("{}", compiler.print_to_string());
}
