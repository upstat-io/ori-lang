//! Fibonacci Benchmark - The classic language performance test
//!
//! Since we don't have Sigil-to-Sigil function calls yet, we'll:
//! 1. Generate LLVM IR for an iterative fibonacci function
//! 2. Compile to native code
//! 3. Benchmark against other languages
//!
//! Run with: cargo run --example fib_benchmark -p sigil_llvm --release

use inkwell::context::Context;
use inkwell::IntPredicate;

fn main() {
    println!("=== Fibonacci Benchmark ===\n");

    // We'll manually construct LLVM IR for iterative fib
    // since our AST codegen doesn't have all features yet
    create_fib_benchmark();
}

fn create_fib_benchmark() {
    let context = Context::create();

    // Create module directly with inkwell for full control
    let module = context.create_module("fib_bench");
    let builder = context.create_builder();

    let i64_type = context.i64_type();

    // Create: fn fib(n: i64) -> i64
    let fn_type = i64_type.fn_type(&[i64_type.into()], false);
    let function = module.add_function("fib", fn_type, None);

    let n = function.get_nth_param(0).unwrap().into_int_value();
    n.set_name("n");

    // Entry block
    let entry = context.append_basic_block(function, "entry");
    let loop_header = context.append_basic_block(function, "loop_header");
    let loop_body = context.append_basic_block(function, "loop_body");
    let exit = context.append_basic_block(function, "exit");

    // Entry: check if n <= 1
    builder.position_at_end(entry);
    let one = i64_type.const_int(1, false);
    let zero = i64_type.const_int(0, false);
    let cond = builder.build_int_compare(IntPredicate::SLE, n, one, "n_le_1").unwrap();
    builder.build_conditional_branch(cond, exit, loop_header).unwrap();

    // Loop header: phi nodes for a, b, i
    builder.position_at_end(loop_header);

    let a_phi = builder.build_phi(i64_type, "a").unwrap();
    let b_phi = builder.build_phi(i64_type, "b").unwrap();
    let i_phi = builder.build_phi(i64_type, "i").unwrap();

    // Loop body: a, b = b, a + b; i++
    builder.position_at_end(loop_body);

    let a_val = a_phi.as_basic_value().into_int_value();
    let b_val = b_phi.as_basic_value().into_int_value();
    let i_val = i_phi.as_basic_value().into_int_value();

    let new_a = b_val;
    let new_b = builder.build_int_add(a_val, b_val, "a_plus_b").unwrap();
    let new_i = builder.build_int_add(i_val, one, "i_plus_1").unwrap();

    builder.build_unconditional_branch(loop_header).unwrap();

    // Back to loop header: add incoming edges to phi nodes
    builder.position_at_end(loop_header);

    // Initial values come from entry
    a_phi.add_incoming(&[(&zero, entry), (&new_a, loop_body)]);
    b_phi.add_incoming(&[(&one, entry), (&new_b, loop_body)]);
    let two = i64_type.const_int(2, false);
    i_phi.add_incoming(&[(&two, entry), (&new_i, loop_body)]);

    // Check loop condition: i <= n
    let loop_cond = builder.build_int_compare(IntPredicate::SLE, i_phi.as_basic_value().into_int_value(), n, "i_le_n").unwrap();
    builder.build_conditional_branch(loop_cond, loop_body, exit).unwrap();

    // Exit: return b (or n if n <= 1)
    builder.position_at_end(exit);
    let result_phi = builder.build_phi(i64_type, "result").unwrap();
    result_phi.add_incoming(&[(&n, entry), (&b_phi.as_basic_value(), loop_header)]);
    builder.build_return(Some(&result_phi.as_basic_value())).unwrap();

    // Print the IR
    println!("Generated LLVM IR:");
    println!("{}", module.print_to_string().to_string());

    // Write files
    let ir_path = std::path::Path::new("/tmp/fib_bench.ll");
    let obj_path = std::path::Path::new("/tmp/fib_bench.o");

    module.print_to_file(ir_path).expect("Failed to write IR");
    println!("\nWrote IR to: {}", ir_path.display());

    // Write object file
    use inkwell::targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
    };
    use inkwell::OptimizationLevel;

    Target::initialize_native(&InitializationConfig::default()).unwrap();

    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).unwrap();
    let cpu = TargetMachine::get_host_cpu_name();
    let features = TargetMachine::get_host_cpu_features();

    let target_machine = target
        .create_target_machine(
            &triple,
            cpu.to_str().unwrap(),
            features.to_str().unwrap(),
            OptimizationLevel::Aggressive,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap();

    target_machine
        .write_to_file(&module, FileType::Object, obj_path)
        .unwrap();
    println!("Wrote object file to: {}", obj_path.display());

    // Create C benchmark harness
    let c_code = r#"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

extern int64_t fib(int64_t n);

// Reference implementation - marked noinline to prevent optimization
__attribute__((noinline))
int64_t fib_c(int64_t n) {
    if (n <= 1) return n;
    int64_t a = 0, b = 1;
    for (int64_t i = 2; i <= n; i++) {
        int64_t tmp = a + b;
        a = b;
        b = tmp;
    }
    return b;
}

// Prevent compiler from optimizing away calls
__attribute__((noinline))
void use_value(int64_t x) {
    volatile int64_t sink = x;
    (void)sink;
}

int main(int argc, char **argv) {
    int64_t n = 40;  // Default
    if (argc > 1) n = atoll(argv[1]);

    printf("Fibonacci Benchmark\n");
    printf("===================\n\n");

    // Verify correctness
    printf("Verification (n=%ld):\n", n);
    printf("  Sigil/LLVM: fib(%ld) = %ld\n", n, fib(n));
    printf("  C reference: fib(%ld) = %ld\n", n, fib_c(n));
    printf("\n");

    // Benchmark
    const int iterations = 10000000;
    int64_t result;
    int64_t sum = 0;

    // Warm up
    for (int i = 0; i < 1000; i++) {
        use_value(fib(n));
    }

    // Benchmark Sigil/LLVM version
    // Use varying inputs to prevent loop hoisting
    volatile int64_t vn = n;  // volatile to prevent optimization
    clock_t start = clock();
    for (int i = 0; i < iterations; i++) {
        result = fib(vn);
        sum += result;
    }
    clock_t end = clock();

    double sigil_time = (double)(end - start) / CLOCKS_PER_SEC;
    double sigil_per_call = (sigil_time / iterations) * 1e9;
    use_value(sum);

    // Benchmark C version
    sum = 0;
    start = clock();
    for (int i = 0; i < iterations; i++) {
        result = fib_c(vn);
        sum += result;
    }
    end = clock();

    double c_time = (double)(end - start) / CLOCKS_PER_SEC;
    double c_per_call = (c_time / iterations) * 1e9;
    use_value(sum);

    printf("Benchmark (n=%ld, %d iterations):\n", n, iterations);
    printf("  Sigil/LLVM: %.3f sec (%.2f ns/call)\n", sigil_time, sigil_per_call);
    printf("  C (-O2):    %.3f sec (%.2f ns/call)\n", c_time, c_per_call);
    printf("  Ratio:      %.2fx (>1 = Sigil faster)\n", c_time / sigil_time);

    return 0;
}
"#;

    let c_path = std::path::Path::new("/tmp/fib_bench_main.c");
    std::fs::write(c_path, c_code).unwrap();
    println!("Wrote C harness to: {}", c_path.display());

    // Compile
    println!("\nCompiling...");
    let output = std::process::Command::new("cc")
        .args(["-O2", "-o", "/tmp/fib_bench", "/tmp/fib_bench_main.c", "/tmp/fib_bench.o"])
        .output()
        .unwrap();

    if !output.status.success() {
        eprintln!("Compilation failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return;
    }

    println!("Created: /tmp/fib_bench\n");

    // Run benchmark
    println!("Running benchmark...\n");
    let output = std::process::Command::new("/tmp/fib_bench")
        .arg("40")
        .output()
        .unwrap();

    println!("{}", String::from_utf8_lossy(&output.stdout));

    // Show disassembly
    println!("\n=== Generated Assembly ===");
    let output = std::process::Command::new("objdump")
        .args(["-d", "/tmp/fib_bench"])
        .output()
        .unwrap();

    let disasm = String::from_utf8_lossy(&output.stdout);
    // Find and print just the fib function
    if let Some(start) = disasm.find("<fib>:") {
        let section = &disasm[start..];
        if let Some(end) = section[1..].find("\n\n") {
            println!("{}", &section[..end + 1]);
        } else {
            // Print first 30 lines
            for line in section.lines().take(30) {
                println!("{}", line);
            }
        }
    }
}
