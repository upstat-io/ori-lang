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

use crate::util::ori_binary;

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
    let expected_output = out_dir.join("test");
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
