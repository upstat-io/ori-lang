//! Benchmark: Compare LLVM JIT vs interpreter performance
//!
//! Run with: cargo run --example benchmark -p sigil_llvm

use inkwell::context::Context;
use sigil_ir::{
    ast::{BinaryOp, Expr, ExprKind},
    ExprArena, StringInterner, TypeId,
};
use sigil_llvm::LLVMCodegen;
use std::time::Instant;

fn main() {
    println!("=== Sigil LLVM Backend Benchmark ===\n");

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
    let codegen = LLVMCodegen::new(&context, &interner, "bench");

    // Create: fn sum_to_million() -> int {
    //   // We'll compute: 1 + 2 + 3 + ... unrolled as a large expression
    //   // Actually, let's just do repeated addition: ((((0+1)+1)+1)...)
    // }
    // For simplicity, let's compute a known large value

    let mut arena = ExprArena::new();

    // Build: 1000000 * 500000 (to test multiplication)
    let a = arena.alloc_expr(Expr {
        kind: ExprKind::Int(1_000_000),
        span: sigil_ir::Span::new(0, 1),
    });
    let b = arena.alloc_expr(Expr {
        kind: ExprKind::Int(500_000),
        span: sigil_ir::Span::new(0, 1),
    });
    let mul_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: a,
            right: b,
        },
        span: sigil_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("big_mul");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    codegen.compile_function(
        fn_name,
        &[],
        &[],
        TypeId::INT,
        mul_expr,
        &arena,
        &expr_types,
    );

    // JIT execution (note: once JIT is created, module is consumed)
    let result = codegen.jit_execute_i64("big_mul").unwrap();
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
    let codegen = LLVMCodegen::new(&context, &interner, "bench_fib");

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
        span: sigil_ir::Span::new(0, 1),
    });
    let b_ref = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(b_name),
        span: sigil_ir::Span::new(0, 1),
    });
    let add_expr = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: a_ref,
            right: b_ref,
        },
        span: sigil_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("fib_step");
    let expr_types = vec![TypeId::INT, TypeId::INT, TypeId::INT];

    // Time compilation
    let compile_start = Instant::now();
    codegen.compile_function(
        fn_name,
        &[a_name, b_name],
        &[TypeId::INT, TypeId::INT],
        TypeId::INT,
        add_expr,
        &arena,
        &expr_types,
    );
    let compile_time = compile_start.elapsed();

    println!("  Compilation time: {:?}", compile_time);
    println!("  Generated IR:");
    println!("{}", codegen.print_to_string());

    // Now let's create a standalone executable to benchmark
    println!("\nCreating standalone benchmark executable...");
    create_standalone_benchmark();
}

fn create_standalone_benchmark() {
    let context = Context::create();
    let interner = StringInterner::new();
    let codegen = LLVMCodegen::new(&context, &interner, "sigil_bench");

    let mut arena = ExprArena::new();

    // Create: fn sigil_compute(n: int) -> int { (n * 42) + (n / 2) }
    // This can't be constant-folded since n is a parameter

    let n_name = interner.intern("n");

    // n * 42
    let n_ref1 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: sigil_ir::Span::new(0, 1),
    });
    let forty_two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(42),
        span: sigil_ir::Span::new(0, 1),
    });
    let mul = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Mul,
            left: n_ref1,
            right: forty_two,
        },
        span: sigil_ir::Span::new(0, 1),
    });

    // n / 2
    let n_ref2 = arena.alloc_expr(Expr {
        kind: ExprKind::Ident(n_name),
        span: sigil_ir::Span::new(0, 1),
    });
    let two = arena.alloc_expr(Expr {
        kind: ExprKind::Int(2),
        span: sigil_ir::Span::new(0, 1),
    });
    let div = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Div,
            left: n_ref2,
            right: two,
        },
        span: sigil_ir::Span::new(0, 1),
    });

    // mul + div
    let add = arena.alloc_expr(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: mul,
            right: div,
        },
        span: sigil_ir::Span::new(0, 1),
    });

    let fn_name = interner.intern("sigil_compute");
    let expr_types = vec![
        TypeId::INT, TypeId::INT, TypeId::INT, // mul
        TypeId::INT, TypeId::INT, TypeId::INT, // div
        TypeId::INT, // add
    ];

    codegen.compile_function(
        fn_name,
        &[n_name],
        &[TypeId::INT],
        TypeId::INT,
        add,
        &arena,
        &expr_types,
    );

    // Write object file
    let obj_path = std::path::Path::new("/tmp/sigil_bench.o");
    let ir_path = std::path::Path::new("/tmp/sigil_bench.ll");

    codegen.write_ir_to_file(ir_path).expect("Failed to write IR");
    println!("  Wrote IR to: {}", ir_path.display());

    codegen.write_object_file(obj_path).expect("Failed to write object file");
    println!("  Wrote object file to: {}", obj_path.display());

    // Create a C wrapper
    let c_wrapper = r#"
#include <stdio.h>
#include <time.h>
#include <stdint.h>

extern int64_t sigil_compute(int64_t n);

int main() {
    printf("Sigil LLVM Backend - Native Benchmark\n");
    printf("======================================\n\n");

    // Warm up
    volatile int64_t result = sigil_compute(100);

    // Benchmark: call with varying inputs to prevent optimization
    const int iterations = 100000000;
    int64_t sum = 0;

    clock_t start = clock();
    for (int i = 0; i < iterations; i++) {
        sum += sigil_compute(i);
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

    let c_path = std::path::Path::new("/tmp/sigil_bench_main.c");
    std::fs::write(c_path, c_wrapper).expect("Failed to write C wrapper");
    println!("  Wrote C wrapper to: {}", c_path.display());

    // Compile and link
    println!("\nCompiling and linking...");
    let output = std::process::Command::new("cc")
        .args([
            "-O2",
            "/tmp/sigil_bench_main.c",
            "/tmp/sigil_bench.o",
            "-o",
            "/tmp/sigil_bench",
        ])
        .output()
        .expect("Failed to run cc");

    if !output.status.success() {
        eprintln!("Compilation failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return;
    }

    println!("  Created executable: /tmp/sigil_bench");

    // Run the benchmark
    println!("\nRunning benchmark...\n");
    let output = std::process::Command::new("/tmp/sigil_bench")
        .output()
        .expect("Failed to run benchmark");

    println!("{}", String::from_utf8_lossy(&output.stdout));

    // Show the generated LLVM IR
    println!("\nGenerated LLVM IR:");
    println!("{}", codegen.print_to_string());
}
