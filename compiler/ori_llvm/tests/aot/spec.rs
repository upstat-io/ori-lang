//! AOT Spec Conformance Tests
//!
//! End-to-end tests that compile Ori programs through the full AOT pipeline
//! (compile → link → execute) and verify correct behavior.
//!
//! These tests mirror patterns from `tests/spec/` but run through AOT instead
//! of the interpreter or JIT backends.
//!
//! **NOTE**: These tests must run serially (`--test-threads=1`) due to a race
//! condition where the AOT compiler uses `/tmp/test.o` as an intermediate file.
//! TODO: Fix the AOT build path to use unique temp file names.

// Allow raw string hashes for readability in test program literals
#![allow(clippy::needless_raw_string_hashes)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

// ============================================================================
// Test Utilities
// ============================================================================

/// Get the path to the `ori` binary.
fn ori_binary() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("compiler").exists())
        .map_or_else(|| PathBuf::from("/workspace"), Path::to_path_buf);

    let release_path = workspace_root.join("target/release/ori");
    if release_path.exists() {
        return release_path;
    }

    let debug_path = workspace_root.join("target/debug/ori");
    if debug_path.exists() {
        return debug_path;
    }

    PathBuf::from("ori")
}

/// Compile and run an Ori program, returning the exit code.
/// Returns 0 on success, non-zero on failure.
fn compile_and_run(source: &str) -> i32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_{id}.ori"));
    let binary_path = temp_dir.path().join(format!("test_{id}"));

    fs::write(&source_path, source).expect("Failed to write source");

    // Compile
    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        eprintln!(
            "Compilation failed:\n{}",
            String::from_utf8_lossy(&compile_result.stderr)
        );
        return -1;
    }

    // Run
    let run_result = Command::new(&binary_path)
        .output()
        .expect("Failed to execute binary");

    run_result.status.code().unwrap_or(-1)
}

/// Assert that a program compiles and runs with exit code 0.
fn assert_aot_success(source: &str, test_name: &str) {
    let exit_code = compile_and_run(source);
    assert_eq!(
        exit_code, 0,
        "{test_name} failed with exit code {exit_code}"
    );
}

// ============================================================================
// Bindings (tests/spec/expressions/bindings.ori)
// ============================================================================

#[test]
fn test_aot_let_binding_basic() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x = 42,
    if x == 42 then 0 else 1
)
"#,
        "let_binding_basic",
    );
}

#[test]
fn test_aot_let_binding_annotated() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x: int = 42,
    let y: bool = true,
    if x == 42 && y then 0 else 1
)
"#,
        "let_binding_annotated",
    );
}

#[test]
fn test_aot_let_shadowing() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x = 1,
    let x = x + 1,
    let x = x * 2,
    if x == 4 then 0 else 1
)
"#,
        "let_shadowing",
    );
}

// ============================================================================
// Conditionals (tests/spec/expressions/conditionals.ori)
// ============================================================================

#[test]
fn test_aot_if_then_else() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let a = if true then 1 else 0,
    let b = if false then 0 else 2,
    if a == 1 && b == 2 then 0 else 1
)
"#,
        "if_then_else",
    );
}

#[test]
fn test_aot_nested_conditionals() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x = if true then if true then 1 else 2 else 3,
    let y = if false then 1 else if true then 2 else 3,
    if x == 1 && y == 2 then 0 else 1
)
"#,
        "nested_conditionals",
    );
}

#[test]
fn test_aot_comparison_conditions() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let x = 10,
    let a = if x > 5 then 1 else 0,
    let b = if x < 20 then 1 else 0,
    let c = if x == 10 then 1 else 0,
    let d = if x != 5 then 1 else 0,
    if a == 1 && b == 1 && c == 1 && d == 1 then 0 else 1
)
"#,
        "comparison_conditions",
    );
}

// ============================================================================
// Arithmetic (tests/spec/types/primitives.ori)
// ============================================================================

#[test]
fn test_aot_arithmetic_add_sub() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let add = 3 + 4,
    let sub = 10 - 3,
    if add == 7 && sub == 7 then 0 else 1
)
"#,
        "arithmetic_add_sub",
    );
}

#[test]
fn test_aot_arithmetic_mul_div() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let mul = 6 * 7,
    let div_result = 42 / 6,
    if mul == 42 && div_result == 7 then 0 else 1
)
"#,
        "arithmetic_mul_div",
    );
}

#[test]
fn test_aot_arithmetic_modulo() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let m1 = 17 % 5,
    let m2 = 10 % 3,
    if m1 == 2 && m2 == 1 then 0 else 1
)
"#,
        "arithmetic_modulo",
    );
}

#[test]
fn test_aot_arithmetic_negation() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let neg = -5,
    let double_neg = -(-10),
    if neg == -5 && double_neg == 10 then 0 else 1
)
"#,
        "arithmetic_negation",
    );
}

#[test]
fn test_aot_arithmetic_precedence() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let a = 2 + 3 * 4,
    let b = (2 + 3) * 4,
    if a == 14 && b == 20 then 0 else 1
)
"#,
        "arithmetic_precedence",
    );
}

// ============================================================================
// Boolean Operators
// ============================================================================

#[test]
fn test_aot_boolean_and() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let tt = true && true,
    let tf = true && false,
    let ft = false && true,
    let ff = false && false,
    if tt && !tf && !ft && !ff then 0 else 1
)
"#,
        "boolean_and",
    );
}

#[test]
fn test_aot_boolean_or() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let tt = true || true,
    let tf = true || false,
    let ft = false || true,
    let ff = false || false,
    if tt && tf && ft && !ff then 0 else 1
)
"#,
        "boolean_or",
    );
}

#[test]
fn test_aot_boolean_not() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let not_true = !true,
    let not_false = !false,
    if !not_true && not_false then 0 else 1
)
"#,
        "boolean_not",
    );
}

// ============================================================================
// Functions (tests/spec/declarations/functions.ori)
// ============================================================================

#[test]
fn test_aot_function_call() {
    assert_aot_success(
        r#"
@double (n: int) -> int = n * 2

@main () -> int = run(
    let result = double(n: 21),
    if result == 42 then 0 else 1
)
"#,
        "function_call",
    );
}

#[test]
fn test_aot_function_multiple_params() {
    assert_aot_success(
        r#"
@add (a: int, b: int) -> int = a + b

@main () -> int = run(
    let result = add(a: 35, b: 7),
    if result == 42 then 0 else 1
)
"#,
        "function_multiple_params",
    );
}

#[test]
fn test_aot_function_recursion() {
    assert_aot_success(
        r#"
@factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n: n - 1)

@main () -> int = run(
    let f5 = factorial(n: 5),
    if f5 == 120 then 0 else 1
)
"#,
        "function_recursion",
    );
}

#[test]
fn test_aot_function_nested_calls() {
    assert_aot_success(
        r#"
@double (n: int) -> int = n * 2
@add_one (n: int) -> int = n + 1

@main () -> int = run(
    let result = double(n: add_one(n: 20)),
    if result == 42 then 0 else 1
)
"#,
        "function_nested_calls",
    );
}

// ============================================================================
// Comparisons
// ============================================================================

#[test]
fn test_aot_comparison_equality() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let eq = 42 == 42,
    let neq = 42 != 43,
    if eq && neq then 0 else 1
)
"#,
        "comparison_equality",
    );
}

#[test]
fn test_aot_comparison_ordering() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let lt = 3 < 5,
    let le1 = 5 <= 5,
    let le2 = 4 <= 5,
    let gt = 7 > 3,
    let ge1 = 7 >= 7,
    let ge2 = 8 >= 7,
    if lt && le1 && le2 && gt && ge1 && ge2 then 0 else 1
)
"#,
        "comparison_ordering",
    );
}

// ============================================================================
// Print Function
// ============================================================================

#[test]
fn test_aot_print_string() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join("print_test.ori");
    let binary_path = temp_dir.path().join("print_test");

    let source = r#"@main () -> void = print(msg: "Hello AOT!")"#;

    fs::write(&source_path, source).expect("Failed to write source");

    // Compile
    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        compile_result.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&compile_result.stderr)
    );

    // Verify binary exists
    assert!(
        binary_path.exists(),
        "Binary was not created at {binary_path:?}"
    );

    // Run and capture output
    let run_result = Command::new(&binary_path)
        .output()
        .expect("Failed to execute binary");

    assert!(
        run_result.status.success(),
        "Binary execution failed with code {:?}",
        run_result.status.code()
    );

    let stdout = String::from_utf8_lossy(&run_result.stdout);
    assert!(
        stdout.contains("Hello AOT!"),
        "Expected output to contain 'Hello AOT!', got stdout: '{}', stderr: '{}'",
        stdout,
        String::from_utf8_lossy(&run_result.stderr)
    );
}

// ============================================================================
// Complex Expressions
// ============================================================================

#[test]
fn test_aot_complex_expression() {
    assert_aot_success(
        r#"
@max (a: int, b: int) -> int = if a > b then a else b
@min (a: int, b: int) -> int = if a < b then a else b
@clamp (value: int, lo: int, hi: int) -> int = max(a: lo, b: min(a: value, b: hi))

@main () -> int = run(
    let c1 = clamp(value: 5, lo: 0, hi: 10),
    let c2 = clamp(value: -5, lo: 0, hi: 10),
    let c3 = clamp(value: 15, lo: 0, hi: 10),
    if c1 == 5 && c2 == 0 && c3 == 10 then 0 else 1
)
"#,
        "complex_expression",
    );
}

#[test]
fn test_aot_fibonacci() {
    assert_aot_success(
        r#"
@fib (n: int) -> int = if n <= 1 then n else fib(n: n - 1) + fib(n: n - 2)

@main () -> int = run(
    let f0 = fib(n: 0),
    let f1 = fib(n: 1),
    let f5 = fib(n: 5),
    let f10 = fib(n: 10),
    if f0 == 0 && f1 == 1 && f5 == 5 && f10 == 55 then 0 else 1
)
"#,
        "fibonacci",
    );
}
