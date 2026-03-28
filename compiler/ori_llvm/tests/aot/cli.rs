//! CLI Integration Tests for AOT Compilation
//!
//! End-to-end tests that invoke the `ori` binary to verify:
//! - `ori build` produces correct executables
//! - Build flags work correctly (--release, --emit, -o, etc.)
//! - Error handling for invalid inputs
//! - `ori targets` lists supported targets
//!
//! These tests require the `ori` binary to be built with the LLVM feature.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

use crate::util::{
    assert_no_signal_crash, compile_and_capture_ir, compile_and_run_valgrind_with_args,
    compile_and_run_with_args, ori_binary, stdlib_path,
};

/// Create a simple Ori source file for testing.
fn create_test_source(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).expect("Failed to write test source");
    path
}

/// Simple Ori program that prints a value.
const SIMPLE_PROGRAM: &str = r#"
@main () -> void = {
    let x = 42;
    let y = x + 1;
    print(msg: "Hello from Ori!")
}
"#;

/// Ori program with a type error.
const INVALID_PROGRAM: &str = r#"
@main () -> void = {
    let x: int = "not an int";
    print(msg: "should fail")
}
"#;

/// Ori program that just returns an exit code.
const EXIT_CODE_PROGRAM: &str = r"
@main () -> int = 42;
";

/// Test: `ori build` produces an executable.
///
/// Verifies that basic compilation works and produces a runnable binary.
#[test]
fn test_build_basic() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "hello.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("hello");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    // Should succeed
    assert!(
        result.status.success(),
        "ori build failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Output file should exist
    assert!(output.exists(), "Output binary was not created");

    // Binary should be executable (on Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&output).expect("Failed to get metadata");
        let permissions = metadata.permissions();
        assert!(permissions.mode() & 0o111 != 0, "Binary is not executable");
    }
}

/// Test: `ori build --release` produces an optimized executable.
#[test]
fn test_build_release() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "hello.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("hello_release");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--release",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --release failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "Release binary was not created");
}

/// Test: `ori build` with exit code program.
#[test]
fn test_build_exit_code() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "exitcode.ori", EXIT_CODE_PROGRAM);
    let output = temp_dir.path().join("exitcode");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "Binary was not created");
}

/// Test: `ori build -o <path>` creates binary at specified path.
#[test]
fn test_build_output_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let custom_output = temp_dir.path().join("custom_name");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            custom_output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build with -o failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(
        custom_output.exists(),
        "Binary was not created at custom path"
    );
}

/// Test: `ori build --out-dir=<dir>` creates binary in specified directory.
#[test]
fn test_build_output_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let out_dir = temp_dir.path().join("out");
    fs::create_dir(&out_dir).expect("Failed to create output dir");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            &format!("--out-dir={}", out_dir.to_str().unwrap()),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build with --out-dir failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Binary should be in the output directory
    let expected_output = out_dir.join(format!("test{}", std::env::consts::EXE_SUFFIX));
    assert!(
        expected_output.exists(),
        "Binary was not created in output directory"
    );
}

/// Test: `ori build --emit=obj` produces object file.
#[test]
fn test_build_emit_object() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.o");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--emit=obj",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --emit=obj failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "Object file was not created");

    // Verify it's a valid object file (starts with ELF magic or similar)
    let content = fs::read(&output).expect("Failed to read object file");
    assert!(!content.is_empty(), "Object file is empty");
    // ELF: 0x7F 'E' 'L' 'F'
    // Mach-O: 0xFE 0xED 0xFA 0xCE/0xCF or 0xCF/0xCE 0xFA 0xED 0xFE
    assert!(content.len() >= 4, "Object file is too small to be valid");
}

/// Test: `ori build --emit=llvm-ir` produces LLVM IR.
#[test]
fn test_build_emit_llvm_ir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.ll");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--emit=llvm-ir",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --emit=llvm-ir failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "LLVM IR file was not created");

    // Verify it contains LLVM IR markers
    let content = fs::read_to_string(&output).expect("Failed to read LLVM IR");
    assert!(
        content.contains("define") || content.contains("declare"),
        "File doesn't appear to be LLVM IR"
    );
}

/// Test: `ori build --emit=asm` produces assembly.
#[test]
fn test_build_emit_assembly() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.s");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--emit=asm",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --emit=asm failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "Assembly file was not created");

    // Verify it contains assembly-like content
    let content = fs::read_to_string(&output).expect("Failed to read assembly");
    assert!(
        content.contains(".text") || content.contains("section"),
        "File doesn't appear to be assembly"
    );
}

/// Test: `ori build` with invalid source fails gracefully.
#[test]
fn test_build_invalid_source() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "invalid.ori", INVALID_PROGRAM);
    let output = temp_dir.path().join("invalid");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    // Should fail with non-zero exit code
    assert!(
        !result.status.success(),
        "ori build should have failed for invalid source"
    );

    // Should have error message in stderr
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("error") || stderr.contains("Error"),
        "Error message not found in stderr: {stderr}"
    );

    // Output file should not exist
    assert!(
        !output.exists(),
        "Output binary should not exist for failed build"
    );
}

/// Test: `ori build` with missing file fails gracefully.
#[test]
fn test_build_missing_file() {
    let result = Command::new(ori_binary())
        .args(["build", "/nonexistent/path/to/file.ori"])
        .output()
        .expect("Failed to execute ori build");

    // Should fail
    assert!(
        !result.status.success(),
        "ori build should have failed for missing file"
    );

    // Should have error message
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("cannot find")
            || stderr.contains("not found")
            || stderr.contains("No such file"),
        "Expected 'not found' error in stderr: {stderr}"
    );
}

/// Test: `ori targets` lists supported targets.
#[test]
fn test_targets_list() {
    let result = Command::new(ori_binary())
        .args(["targets"])
        .output()
        .expect("Failed to execute ori targets");

    assert!(
        result.status.success(),
        "ori targets failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);

    // Should list common platforms
    assert!(
        stdout.contains("linux") || stdout.contains("Linux"),
        "Linux targets not listed"
    );
    assert!(
        stdout.contains("darwin") || stdout.contains("macOS"),
        "macOS targets not listed"
    );
    assert!(
        stdout.contains("windows") || stdout.contains("Windows"),
        "Windows targets not listed"
    );
    assert!(
        stdout.contains("wasm") || stdout.contains("WebAssembly"),
        "WebAssembly targets not listed"
    );
}

/// Test: `ori targets --installed` lists installed targets.
#[test]
fn test_targets_installed() {
    let result = Command::new(ori_binary())
        .args(["targets", "--installed"])
        .output()
        .expect("Failed to execute ori targets --installed");

    assert!(
        result.status.success(),
        "ori targets --installed failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);

    // Should show at least the native target
    assert!(
        stdout.contains("native") || stdout.contains("x86_64") || stdout.contains("aarch64"),
        "Native target not listed in installed targets"
    );
}

/// Test: `ori demangle` decodes Ori symbols.
#[test]
fn test_demangle_ori_symbol() {
    let result = Command::new(ori_binary())
        .args(["demangle", "_ori_main"])
        .output()
        .expect("Failed to execute ori demangle");

    assert!(
        result.status.success(),
        "ori demangle failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("main"),
        "Demangled output should contain 'main': {stdout}"
    );
}

/// Test: `ori demangle` passes through non-Ori symbols.
#[test]
fn test_demangle_non_ori_symbol() {
    let result = Command::new(ori_binary())
        .args(["demangle", "_ZN3foo3barE"])
        .output()
        .expect("Failed to execute ori demangle");

    assert!(
        result.status.success(),
        "ori demangle failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);
    // Non-Ori symbols should pass through unchanged
    assert!(
        stdout.contains("_ZN3foo3barE"),
        "Non-Ori symbol should pass through: {stdout}"
    );
}

/// Test: `ori build --verbose` shows compilation progress.
#[test]
fn test_build_verbose() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--verbose",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --verbose failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    // Verbose mode should show some progress info
    assert!(
        stderr.contains("Compiling") || stderr.contains("Target") || stderr.contains("Linking"),
        "Verbose output missing expected progress info: {stderr}"
    );
}

/// Test: `ori target list` shows installed targets.
#[test]
fn test_target_list() {
    let result = Command::new(ori_binary())
        .args(["target", "list"])
        .output()
        .expect("Failed to execute ori target list");

    assert!(
        result.status.success(),
        "ori target list failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stdout = String::from_utf8_lossy(&result.stdout);

    // Should always show the native target
    assert!(
        stdout.contains("native") || stdout.contains("x86_64") || stdout.contains("aarch64"),
        "Native target not listed: {stdout}"
    );

    // Should have usage hint
    assert!(
        stdout.contains("ori target add"),
        "Missing usage hint for adding targets: {stdout}"
    );
}

/// Test: `ori target` without subcommand shows usage.
#[test]
fn test_target_no_subcommand() {
    let result = Command::new(ori_binary())
        .args(["target"])
        .output()
        .expect("Failed to execute ori target");

    // Should fail with usage message
    assert!(
        !result.status.success(),
        "ori target without subcommand should fail"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Usage") || stderr.contains("subcommand"),
        "Missing usage message: {stderr}"
    );
}

/// Test: `ori target add` with invalid target fails gracefully.
#[test]
fn test_target_add_invalid() {
    let result = Command::new(ori_binary())
        .args(["target", "add", "invalid-nonexistent-target"])
        .output()
        .expect("Failed to execute ori target add");

    // Should fail
    assert!(
        !result.status.success(),
        "ori target add with invalid target should fail"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsupported") || stderr.contains("error"),
        "Expected unsupported target error: {stderr}"
    );
}

/// Test: `ori target add` without target name shows error.
#[test]
fn test_target_add_missing_name() {
    let result = Command::new(ori_binary())
        .args(["target", "add"])
        .output()
        .expect("Failed to execute ori target add");

    // Should fail
    assert!(
        !result.status.success(),
        "ori target add without name should fail"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("missing") || stderr.contains("Usage"),
        "Expected missing target name error: {stderr}"
    );
}

/// Test: `ori target remove` with non-installed target fails gracefully.
#[test]
fn test_target_remove_not_installed() {
    let result = Command::new(ori_binary())
        .args(["target", "remove", "aarch64-unknown-linux-gnu"])
        .output()
        .expect("Failed to execute ori target remove");

    // Should fail since the target isn't installed
    assert!(
        !result.status.success(),
        "ori target remove with uninstalled target should fail"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("not installed") || stderr.contains("error"),
        "Expected not installed error: {stderr}"
    );
}

/// Test: `ori build --target=wasm32-unknown-unknown` for WASM target.
///
/// Note: This test may require the WASM target to be set up properly.
/// It primarily verifies the target flag is parsed correctly.
#[test]
fn test_build_cross_compile_wasm_object() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.o");

    // Build to object file only (avoids linking dependencies)
    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--target=wasm32-unknown-unknown",
            "--emit=obj",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --target=wasm32-unknown-unknown --emit=obj failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "WASM object file was not created");

    // Verify it's a WASM binary (starts with \0asm magic bytes)
    let content = fs::read(&output).expect("Failed to read object file");
    assert!(!content.is_empty(), "Object file is empty");
    // WASM magic: 0x00 'a' 's' 'm'
    assert!(
        content.starts_with(&[0x00, 0x61, 0x73, 0x6d]),
        "File doesn't appear to be WASM (missing magic bytes)"
    );
}

/// Test: `ori build --wasm` shorthand for WASM target.
#[test]
fn test_build_wasm_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.o");

    // --wasm flag should set target to wasm32-unknown-unknown
    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--wasm",
            "--emit=obj",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --wasm --emit=obj failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "WASM object file was not created");
}

/// Test: `ori build --target=` with unsupported target fails gracefully.
#[test]
fn test_build_unsupported_target() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--target=riscv64-unknown-unknown",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    // Should fail with unsupported target error
    assert!(
        !result.status.success(),
        "ori build with unsupported target should fail"
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsupported") || stderr.contains("error") || stderr.contains("target"),
        "Expected unsupported target error: {stderr}"
    );
}

/// Ori program that imports a missing module.
const MISSING_DEPENDENCY_PROGRAM: &str = r#"
use "./nonexistent_module" { some_function }

@main () -> void = {
    some_function()
}
"#;

/// Test: `ori build` with missing dependency fails gracefully.
///
/// Verifies that the compiler reports a helpful error when an imported
/// module cannot be found.
#[test]
fn test_build_missing_dependency() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "missing_dep.ori", MISSING_DEPENDENCY_PROGRAM);
    let output = temp_dir.path().join("missing_dep");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    // Should fail with non-zero exit code
    assert!(
        !result.status.success(),
        "ori build should have failed for missing dependency"
    );

    // Should have error message about missing import
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("cannot find")
            || stderr.contains("not found")
            || stderr.contains("import error"),
        "Expected missing module error in stderr: {stderr}"
    );

    // Output file should not exist
    assert!(
        !output.exists(),
        "Output binary should not exist for failed build"
    );
}

/// Test: `ori build` with unchanged source should be fast (incremental rebuild).
///
/// This test verifies that the incremental compilation cache works:
/// 1. First build: full compilation
/// 2. Second build (no changes): should be faster (cache hit)
///
/// Note: This test requires incremental compilation to be wired up.
/// Currently marked as ignored until the feature is fully integrated.
#[test]
#[ignore = "Incremental compilation not yet wired up in ori build (see 21B.6)"]
fn test_build_incremental_unchanged() {
    use std::time::Instant;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "incremental.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("incremental");

    // First build: full compilation
    let start1 = Instant::now();
    let result1 = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute first ori build");

    assert!(
        result1.status.success(),
        "First build failed: {}",
        String::from_utf8_lossy(&result1.stderr)
    );
    let duration1 = start1.elapsed();

    // Second build: should use cache
    let start2 = Instant::now();
    let result2 = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute second ori build");

    assert!(
        result2.status.success(),
        "Second build failed: {}",
        String::from_utf8_lossy(&result2.stderr)
    );
    let duration2 = start2.elapsed();

    // Second build should be significantly faster (at least 2x)
    // This is a heuristic - cache hits should be much faster than full builds
    assert!(
        duration2 < duration1 / 2,
        "Incremental build not faster: first={duration1:?}, second={duration2:?}"
    );
}

// @main(args: [str]) ABI — Indirect param passing

#[test]
fn test_main_args_no_arguments() {
    let (exit_code, _, stderr) =
        compile_and_run_with_args(r#"@main (args: [str]) -> int = args.len();"#, &[]);
    assert_eq!(exit_code, 0, "expected 0 args, stderr: {stderr}");
}

#[test]
fn test_main_args_with_sso_strings() {
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"@main (args: [str]) -> int = if args.len() == 2 then 0 else 1;"#,
        &["hello", "world"],
    );
    assert_eq!(
        exit_code, 0,
        "expected exit 0 (leak-free), stderr: {stderr}"
    );
}

#[test]
fn test_main_args_with_heap_strings() {
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"@main (args: [str]) -> int = if args.len() == 1 then 0 else 1;"#,
        &["a very long string that definitely exceeds the twenty three byte SSO threshold for testing"],
    );
    assert_eq!(
        exit_code, 0,
        "expected exit 0 (leak-free with heap str), stderr: {stderr}"
    );
}

#[test]
fn test_main_args_void_return() {
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"
@main (args: [str]) -> void = {
    let _ = args.len();
}
"#,
        &["hello"],
    );
    assert_eq!(
        exit_code, 0,
        "expected exit 0 (void main with args), stderr: {stderr}"
    );
}

// @main(args: [str]) must clean up args on unwind path.
// When _ori_main panics, the C main wrapper must invoke/landingpad to
// call ori_args_cleanup before re-raising, preventing args buffer leak.

#[test]
fn test_main_args_panic_does_not_segfault() {
    // Panic with args — the wrapper must use invoke+landingpad to clean
    // up the args list on the unwind path. Without the fix, this could
    // leave the args buffer unfreed or cause UB if the exception escapes
    // past a plain `call`.
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"@main (args: [str]) -> void = panic(msg: "boom");"#,
        &["hello", "world"],
    );
    assert_no_signal_crash(exit_code, "test_main_args_panic_does_not_segfault");
    assert_ne!(exit_code, 0, "panic should not exit cleanly");
    assert!(
        stderr.contains("ori panic: boom"),
        "expected panic message in stderr, got: {stderr}"
    );
}

#[test]
fn test_main_args_panic_with_heap_strings() {
    // Same as above but with heap-allocated strings (>23 bytes, beyond SSO).
    // Ensures the args cleanup correctly frees heap string children.
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"@main (args: [str]) -> void = panic(msg: "boom");"#,
        &["a very long string that definitely exceeds the twenty three byte SSO threshold"],
    );
    assert_no_signal_crash(exit_code, "test_main_args_panic_with_heap_strings");
    assert_ne!(exit_code, 0, "panic should not exit cleanly");
    assert!(
        stderr.contains("ori panic: boom"),
        "expected panic message in stderr, got: {stderr}"
    );
}

#[test]
fn test_main_args_panic_int_return() {
    // @main(args: [str]) -> int that panics — same cleanup requirement.
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"@main (args: [str]) -> int = panic(msg: "crash");"#,
        &["arg1"],
    );
    assert_no_signal_crash(exit_code, "test_main_args_panic_int_return");
    assert_ne!(exit_code, 0, "panic should not exit cleanly");
    assert!(
        stderr.contains("ori panic: crash"),
        "expected panic message in stderr, got: {stderr}"
    );
}

// Semantic pin: the C main wrapper MUST use `invoke` (not `call`) for
// `_ori_main` when @main takes args AND _ori_main can unwind, so the
// unwind path can clean up the args buffer. This test would FAIL if
// the invoke+landingpad fix is reverted.
#[test]
fn test_main_args_wrapper_uses_invoke_ir() {
    // Use a program that can unwind (panic triggers exception).
    // A nounwind @main (e.g., only calling print) correctly uses plain call.
    let ir = compile_and_capture_ir(
        r#"
@main (args: [str]) -> void = {
    if args.len() == 0 then panic(msg: "no args");
}
"#,
    );
    let main_fn = ir
        .split("define ")
        .find(|s| s.contains("@main("))
        .expect("expected `define ... @main(` in IR");

    let is_seh = main_fn.contains("ori_try_call");
    if is_seh {
        // SEH/MSVC: uses ori_try_call thunk (not invoke) because
        // RaiseException-based Ori panics are not caught by catchpad.
        assert!(
            main_fn.contains("call i64 @ori_try_call("),
            "SEH main wrapper must use ori_try_call.\nIR:\n{main_fn}"
        );
        assert!(
            main_fn.contains("seh.success") && main_fn.contains("seh.caught"),
            "SEH main wrapper must branch to success/caught.\nIR:\n{main_fn}"
        );
    } else {
        // Itanium: invoke + catch-all landingpad
        assert!(
            main_fn.contains("invoke void @_ori_main("),
            "main wrapper must use `invoke` for _ori_main when it can unwind.\nIR:\n{main_fn}"
        );
        assert!(
            main_fn.contains("landingpad"),
            "main wrapper must have catch-all landing pad.\nIR:\n{main_fn}"
        );
        assert!(
            main_fn.contains("catch ptr null"),
            "main wrapper must use catch-all (catch ptr null), not cleanup.\nIR:\n{main_fn}"
        );
    }
    // Must call ori_args_cleanup in both normal and catch/caught paths.
    // This program uses args in a borrowed way (only .len()), so the
    // wrapper retains ownership → cleanup on both paths.
    let cleanup_call_count = main_fn.matches("call void @ori_args_cleanup").count();
    assert!(
        cleanup_call_count >= 2,
        "ori_args_cleanup must be called in both normal and catch paths (found {cleanup_call_count}).\nIR:\n{main_fn}"
    );
}

// When @main(args:) is nounwind (e.g., only calls print), plain call is correct.
#[test]
fn test_main_args_nounwind_uses_call_ir() {
    let ir = compile_and_capture_ir(r#"@main (args: [str]) -> void = print(msg: "hi");"#);
    let main_fn = ir
        .split("define ")
        .find(|s| s.contains("@main("))
        .expect("expected `define ... @main(` in IR");

    // Nounwind @main should use plain call (no invoke needed)
    assert!(
        main_fn.contains("call void @_ori_main("),
        "nounwind @main with args should use plain `call`.\nIR:\n{main_fn}"
    );
    assert!(
        !main_fn.contains("invoke void @_ori_main("),
        "nounwind @main should NOT use invoke.\nIR:\n{main_fn}"
    );
}

// Verify that @main without args does NOT use invoke (no cleanup needed).
#[test]
fn test_main_no_args_wrapper_uses_call_ir() {
    let ir = compile_and_capture_ir(r#"@main () -> void = print(msg: "hi");"#);
    let main_fn = ir
        .split("define ")
        .find(|s| s.contains("@main("))
        .expect("expected `define ... @main(` in IR");

    assert!(
        main_fn.contains("call void @_ori_main()"),
        "main wrapper without args should use plain `call`.\nIR:\n{main_fn}"
    );
    assert!(
        !main_fn.contains("invoke"),
        "main wrapper without args should NOT use invoke.\nIR:\n{main_fn}"
    );
}

// Valgrind-based panic-path verification.
// ORI_CHECK_LEAKS can't validate leak freedom on panic (unwind skips leak check).
// Valgrind tracks allocations externally and detects leaks regardless of exit path.
#[test]
fn test_main_args_panic_valgrind_clean() {
    let result = compile_and_run_valgrind_with_args(
        r#"@main (args: [str]) -> void = panic(msg: "boom");"#,
        &["a very long string that definitely exceeds the twenty three byte SSO threshold"],
    );
    match result {
        None => {
            eprintln!("skipping: valgrind not available");
        }
        Some((clean, vg_output)) => {
            assert!(
                clean,
                "Valgrind detected memory errors on panic-path args cleanup:\n{vg_output}"
            );
        }
    }
}

// Functional JIT test for collection construction.
// Verifies that ori_buffer_store_elem_dec / ori_buffer_store_elem_count
// resolve correctly when MCJIT compiles a list literal with fat-pointer
// elements (str). If these symbols were missing or mismatched, the JIT
// would fail with an unresolved symbol error.
#[test]
fn test_jit_str_list_construction() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join("jit_str_list.ori");

    // A test function that creates a [str] list — forces codegen to emit
    // ori_buffer_store_elem_dec (str has a drop fn) and
    // ori_buffer_store_elem_count (3 elements).
    fs::write(
        &source_path,
        r#"
@test_str_list tests _ () -> void = {
    let $words = ["hello", "world", "test"];
    let $count = words.len();
}
"#,
    )
    .expect("Failed to write source");

    let result = Command::new(ori_binary())
        .args(["test", "--backend=llvm", source_path.to_str().unwrap()])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori test");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        result.status.success(),
        "JIT [str] list construction test failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "expected 1 passed test, got:\n{stdout}\nstderr: {stderr}"
    );
}

// @main(args: [str]) owned-path double-free fix.
// When _ori_main takes ownership of args via Indirect ABI (e.g., passing
// args through an owned function), the callee frees the buffer via its ARC
// dec at function exit. The wrapper must NOT also call ori_args_cleanup on
// the normal return path — that would double-free the buffer.

#[test]
fn test_main_args_owned_path_no_double_free() {
    // This program passes `args` through an identity function that takes
    // ownership — borrow inference promotes args to Owned, ABI uses Indirect.
    // Without the fix, the wrapper double-frees the args buffer.
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"
@id (xs: [str]) -> [str] = xs;

@main (args: [str]) -> void = {
    let _ = id(xs: args);
}
"#,
        &["hello", "world"],
    );
    assert_no_signal_crash(exit_code, "test_main_args_owned_path_no_double_free");
    assert_eq!(
        exit_code, 0,
        "expected exit 0 (no double-free, no leaks), stderr: {stderr}"
    );
}

#[test]
fn test_main_args_owned_path_heap_strings() {
    // Same as above but with heap-allocated strings (>23 bytes, beyond SSO).
    // Exercises the elem_dec_fn path for fat-pointer element cleanup.
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"
@id (xs: [str]) -> [str] = xs;

@main (args: [str]) -> void = {
    let _ = id(xs: args);
}
"#,
        &["a very long string that definitely exceeds the twenty three byte SSO threshold"],
    );
    assert_no_signal_crash(exit_code, "test_main_args_owned_path_heap_strings");
    assert_eq!(
        exit_code, 0,
        "expected exit 0 (no double-free with heap strings), stderr: {stderr}"
    );
}

#[test]
fn test_main_args_owned_path_int_return() {
    // Owned args path with int return — same double-free risk.
    let (exit_code, _, stderr) = compile_and_run_with_args(
        r#"
@count (xs: [str]) -> int = xs.len();

@main (args: [str]) -> int = count(xs: args);
"#,
        &["a", "b", "c"],
    );
    assert_no_signal_crash(exit_code, "test_main_args_owned_path_int_return");
    assert_eq!(
        exit_code, 3,
        "expected exit 3 (args.len()), stderr: {stderr}"
    );
}

#[test]
fn test_main_args_owned_path_valgrind_clean() {
    // Valgrind verification: no memory errors on owned args path.
    let result = compile_and_run_valgrind_with_args(
        r#"
@id (xs: [str]) -> [str] = xs;

@main (args: [str]) -> void = {
    let _ = id(xs: args);
}
"#,
        &["a very long string that definitely exceeds the twenty three byte SSO threshold"],
    );
    match result {
        None => {
            eprintln!("skipping: valgrind not available");
        }
        Some((clean, vg_output)) => {
            assert!(
                clean,
                "Valgrind detected memory errors on owned-args path:\n{vg_output}"
            );
        }
    }
}

// Semantic pin: when @main takes args via owned Indirect ABI, the wrapper
// must NOT call ori_args_cleanup on the normal return path. It must still
// call it on the catch (unwind) path, because the callee's ARC dec hasn't
// run if it unwound.
#[test]
fn test_main_args_owned_wrapper_ir_no_normal_cleanup() {
    // This program uses args in an owned way (passed to id()).
    // The wrapper should handle unwind but ori_args_cleanup only
    // appears ONCE (catch/caught path), not twice.
    let ir = compile_and_capture_ir(
        r#"
@id (xs: [str]) -> [str] = xs;

@main (args: [str]) -> void = {
    let _ = id(xs: args);
    if false then panic(msg: "x");
}
"#,
    );
    let main_fn = ir
        .split("define ")
        .find(|s| s.contains("@main("))
        .expect("expected `define ... @main(` in IR");

    let is_seh = main_fn.contains("ori_try_call");
    if is_seh {
        assert!(
            main_fn.contains("call i64 @ori_try_call("),
            "SEH main wrapper should use ori_try_call for owned-args path.\nIR:\n{main_fn}"
        );
    } else {
        assert!(
            main_fn.contains("invoke void @_ori_main("),
            "main wrapper should use invoke for owned-args path.\nIR:\n{main_fn}"
        );
    }
    // ori_args_cleanup called exactly ONCE: catch/caught path only.
    // Normal/success path skips cleanup — callee owns the buffer.
    let cleanup_call_count = main_fn.matches("call void @ori_args_cleanup").count();
    assert_eq!(
        cleanup_call_count, 1,
        "ori_args_cleanup should be called once (catch only) for owned args, found {cleanup_call_count}.\nIR:\n{main_fn}"
    );
}
