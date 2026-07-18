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
    compile_and_run_with_args, ori_binary, stdlib_path, wasm_ld_available, wasm_opt_available,
};

/// Create a simple Ori source file for testing.
fn create_test_source(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, content).expect("Failed to write test source");
    path
}

/// Simple Ori program that prints a value.
const SIMPLE_PROGRAM: &str = include_str!("fixtures/cli/simple_program.ori");

/// Ori program with a type error.
const INVALID_PROGRAM: &str = include_str!("fixtures/cli/invalid_program.ori");

/// Ori program that just returns an exit code.
const EXIT_CODE_PROGRAM: &str = include_str!("fixtures/cli/exit_code_program.ori");

/// Test: `ori build` produces an executable.
///
/// Verifies that basic compilation works and produces a runnable binary.
#[test]
fn test_build_produces_runnable_executable() {
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

/// Production-path test: `ori build --wasm --wasm-opt` links a real WASM
/// binary then runs the wasm-opt post-processor on it in place.
///
/// Requires `wasm-ld` (to link) and `wasm-opt` (Binaryen); skips gracefully
/// when either is unavailable, matching the valgrind-gated tests above.
#[test]
fn test_build_wasm_opt_runs_post_processor_on_linked_binary() {
    if !wasm_ld_available() {
        eprintln!("skipping: wasm-ld not available");
        return;
    }
    if !wasm_opt_available() {
        eprintln!("skipping: wasm-opt not available");
        return;
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.wasm");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--wasm",
            "--wasm-opt",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --wasm --wasm-opt failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "wasm-opt-processed binary was not created");

    let content = fs::read(&output).expect("Failed to read wasm-opt output");
    assert!(
        content.starts_with(&[0x00, 0x61, 0x73, 0x6d]),
        "wasm-opt output doesn't appear to be a valid WASM module (missing magic bytes)"
    );
}

/// Negative pin: `ori build --wasm` without `--wasm-opt` still links a valid
/// WASM binary — the post-processor is opt-in, never required for a plain
/// wasm build.
#[test]
fn test_build_wasm_without_wasm_opt_flag_still_links() {
    if !wasm_ld_available() {
        eprintln!("skipping: wasm-ld not available");
        return;
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source = create_test_source(&temp_dir, "test.ori", SIMPLE_PROGRAM);
    let output = temp_dir.path().join("test.wasm");

    let result = Command::new(ori_binary())
        .args([
            "build",
            source.to_str().unwrap(),
            "--wasm",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ori build");

    assert!(
        result.status.success(),
        "ori build --wasm failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    assert!(output.exists(), "WASM binary was not created");
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
const MISSING_DEPENDENCY_PROGRAM: &str = include_str!("fixtures/cli/build_unsupported_target.ori");

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

fn isolated_incremental_build_command(cache_root: &std::path::Path) -> Command {
    let mut command = Command::new(ori_binary());
    for (name, _) in std::env::vars_os() {
        let rendered = name.to_string_lossy();
        if rendered.starts_with("ORI_")
            || rendered.starts_with("LLVM_")
            || rendered.starts_with("CLANG_")
            || rendered.starts_with("LD_")
            || rendered.starts_with("DYLD_")
            || rendered == "RUST_LOG"
            || rendered == "SOURCE_DATE_EPOCH"
        {
            command.env_remove(name);
        }
    }
    command.env("XDG_CACHE_HOME", cache_root);
    command
}

fn assert_incremental_cache_token(output: &std::process::Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.lines().any(|line| line.trim() == expected),
        "expected exact cache token '{expected}' in stderr:\n{stderr}"
    );
}

fn assert_program_exit(path: &std::path::Path, expected: i32) {
    let result = Command::new(path)
        .status()
        .expect("failed to execute incrementally built program");
    assert_eq!(
        result.code(),
        Some(expected),
        "incrementally built program returned the wrong value"
    );
}

struct IncrementalBuildFixture {
    temp_dir: TempDir,
    cache_root: PathBuf,
    prelude: PathBuf,
    source: PathBuf,
    output: PathBuf,
}

impl IncrementalBuildFixture {
    fn new(program: &str) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache_root = temp_dir.path().join("xdg-cache");
        let prelude_dir = temp_dir.path().join("library/std");
        fs::create_dir_all(&prelude_dir).expect("failed to create test prelude directory");
        let prelude = prelude_dir.join("prelude.ori");
        fs::write(&prelude, "// incremental-cache dependency version 1\n")
            .expect("failed to write test prelude");
        let source = create_test_source(&temp_dir, "incremental.ori", program);
        let output = temp_dir.path().join("incremental");

        Self {
            temp_dir,
            cache_root,
            prelude,
            source,
            output,
        }
    }

    fn write_source(&self, program: &str) {
        fs::write(&self.source, program).expect("failed to mutate incremental source");
    }

    fn write_prelude(&self, content: &str) {
        fs::write(&self.prelude, content).expect("failed to mutate test prelude");
    }

    fn remove_output(&self) {
        fs::remove_file(&self.output).expect("failed to remove incremental build output");
    }

    fn build_command(&self, cache_root: &std::path::Path, options: &[&str]) -> Command {
        let mut command = isolated_incremental_build_command(cache_root);
        command
            .arg("build")
            .arg(&self.source)
            .arg("--verbose")
            .args(options)
            .arg("-o")
            .arg(&self.output);
        command
    }

    fn build(&self, options: &[&str]) -> std::process::Output {
        self.build_command(&self.cache_root, options)
            .output()
            .expect("failed to execute incremental ori build")
    }
}

fn assert_incremental_build_succeeded(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn prime_incremental_cache(
    fixture: &IncrementalBuildFixture,
    options: &[&str],
    expected_exit: i32,
) {
    let first = fixture.build(options);
    assert_incremental_build_succeeded(&first, "First");
    assert_incremental_cache_token(&first, "incremental cache: miss");
    assert_program_exit(&fixture.output, expected_exit);
}

fn cache_object_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut objects = Vec::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "o") {
                objects.push(path);
            }
        }
    }
    objects.sort();
    objects
}

/// Exact incremental-build contract: an unchanged object is reused and the
/// requested executable is relinked even when the prior output was removed.
#[test]
fn test_build_incremental_unchanged() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 17;\n");
    prime_incremental_cache(&fixture, &[], 17);

    fixture.remove_output();
    let second = fixture.build(&[]);
    assert_incremental_build_succeeded(&second, "Second");
    assert_incremental_cache_token(&second, "incremental cache: hit");
    assert!(
        fixture.output.exists(),
        "cache hit must relink the deleted output"
    );
    assert_program_exit(&fixture.output, 17);
}

#[test]
fn incremental_build_invalidates_source_change() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 17;\n");
    prime_incremental_cache(&fixture, &[], 17);

    fixture.write_source("@main () -> int = 23;\n");
    fixture.remove_output();
    let source_changed = fixture.build(&[]);
    assert_incremental_build_succeeded(&source_changed, "Source-invalidated");
    assert_incremental_cache_token(&source_changed, "incremental cache: miss");
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_invalidates_codegen_option_change() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 23;\n");
    prime_incremental_cache(&fixture, &[], 23);

    fixture.remove_output();
    let options_changed = fixture.build(&["--opt=1"]);
    assert_incremental_build_succeeded(&options_changed, "Option-invalidated");
    assert_incremental_cache_token(&options_changed, "incremental cache: miss");
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_invalidates_implicit_prelude_change() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 23;\n");
    prime_incremental_cache(&fixture, &["--opt=1"], 23);

    fixture.write_prelude("// incremental-cache dependency version 2\n");
    fixture.remove_output();
    let dependency_changed = fixture.build(&["--opt=1"]);
    assert_incremental_build_succeeded(&dependency_changed, "Dependency-invalidated");
    assert_incremental_cache_token(&dependency_changed, "incremental cache: miss");
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_bypasses_unfingerprinted_environment() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 23;\n");
    prime_incremental_cache(&fixture, &["--opt=1"], 23);

    fixture.remove_output();
    let bypassed = fixture
        .build_command(&fixture.cache_root, &["--opt=1"])
        .env("ORI_WORKSPACE_DIR", "unfingerprinted-test-input")
        .output()
        .expect("Failed to execute cache-bypassed ori build");
    assert_incremental_build_succeeded(&bypassed, "Cache-bypassed");
    assert_incremental_cache_token(
        &bypassed,
        concat!(
            "incremental cache: bypass (ORI_WORKSPACE_DIR is set and is not part of the cache ",
            "fingerprint; unset it to enable reuse)"
        ),
    );
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_bypasses_rust_log() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 23;\n");
    prime_incremental_cache(&fixture, &["--opt=1"], 23);

    fixture.remove_output();
    let bypassed = fixture
        .build_command(&fixture.cache_root, &["--opt=1"])
        .env("RUST_LOG", "ori_llvm=trace")
        .output()
        .expect("Failed to execute RUST_LOG cache-bypassed ori build");
    assert_incremental_build_succeeded(&bypassed, "RUST_LOG cache-bypassed");
    assert_incremental_cache_token(
        &bypassed,
        concat!(
            "incremental cache: bypass (RUST_LOG is set and is not part of the cache ",
            "fingerprint; unset it to enable reuse)"
        ),
    );
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_bypasses_dynamic_loader_environment() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 23;\n");
    prime_incremental_cache(&fixture, &["--opt=1"], 23);

    fixture.remove_output();
    let bypassed = fixture
        .build_command(&fixture.cache_root, &["--opt=1"])
        .env("LD_BIND_NOW", "1")
        .output()
        .expect("Failed to execute loader-environment cache-bypassed ori build");
    assert_incremental_build_succeeded(&bypassed, "Loader-environment cache-bypassed");
    assert_incremental_cache_token(
        &bypassed,
        concat!(
            "incremental cache: bypass (LD_BIND_NOW is set and is not part of the cache ",
            "fingerprint; unset it to enable reuse)"
        ),
    );
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_warns_and_bypasses_cache_io_failure() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 23;\n");
    let blocked_cache_root = fixture.temp_dir.path().join("cache-root-is-a-file");
    fs::write(&blocked_cache_root, b"not a directory").expect("failed to block cache root");
    let io_bypassed = fixture
        .build_command(&blocked_cache_root, &["--opt=1"])
        .output()
        .expect("Failed to execute cache-I/O-bypassed ori build");
    assert_incremental_build_succeeded(&io_bypassed, "Cache-I/O-bypassed");
    let io_stderr = String::from_utf8_lossy(&io_bypassed.stderr);
    assert!(
        io_stderr.contains("warning: incremental cache: bypass (cache I/O error at")
            && io_stderr
                .contains("set XDG_CACHE_HOME to a writable directory or fix its permissions")
            && io_stderr.contains("rebuild proceeds without reuse"),
        "cache I/O failure must warn, explain the safe bypass, and name the fix:\n{io_stderr}"
    );
    assert_program_exit(&fixture.output, 23);
}

#[test]
fn incremental_build_rejects_wrong_object_at_cached_path() {
    let fixture = IncrementalBuildFixture::new("@main () -> int = 17;\n");

    let first = fixture.build(&[]);
    assert_incremental_build_succeeded(&first, "first integrity-test");
    let first_object = cache_object_files(&fixture.cache_root)
        .into_iter()
        .next()
        .expect("first build must publish one cache object");

    fixture.write_source("@main () -> int = 23;\n");
    fixture.remove_output();
    let second = fixture.build(&[]);
    assert_incremental_build_succeeded(&second, "second integrity-test");
    let second_object = cache_object_files(&fixture.cache_root)
        .into_iter()
        .find(|candidate| candidate != &first_object)
        .expect("changed source must publish a distinct cache object");

    fs::copy(&second_object, &first_object).expect("failed to corrupt the first cache entry");
    fixture.write_source("@main () -> int = 17;\n");
    fixture.remove_output();
    let repaired = fixture.build(&[]);
    assert_incremental_build_succeeded(&repaired, "cache-repair");
    assert_incremental_cache_token(&repaired, "incremental cache: miss");
    assert_program_exit(&fixture.output, 17);
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
        include_str!("fixtures/cli/main_args_void_return.ori"),
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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/cli/main_args_wrapper_uses_invoke_ir.ori"
    ));
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

// A nounwind @main without args has no exception boundary to provide.
#[test]
fn test_main_no_args_nounwind_uses_call_ir() {
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

// An unwinding @main without args still needs the process-level panic boundary:
// source-level catches must stay silent, while an uncaught panic is reported.
#[test]
fn test_main_no_args_unwinding_uses_panic_boundary_ir() {
    let ir = compile_and_capture_ir(r#"@main () -> void = panic(msg: "boom");"#);
    let main_fn = ir
        .split("define ")
        .find(|s| s.contains("@main("))
        .expect("expected `define ... @main(` in IR");

    if main_fn.contains("ori_try_call") {
        assert!(
            main_fn.contains("call i64 @ori_try_call("),
            "SEH main wrapper must use ori_try_call.\nIR:\n{main_fn}"
        );
        assert!(
            main_fn.contains("seh.success") && main_fn.contains("seh.caught"),
            "SEH main wrapper must branch to success/caught.\nIR:\n{main_fn}"
        );
    } else {
        assert!(
            main_fn.contains("invoke void @_ori_main("),
            "Itanium main wrapper must invoke an unwinding _ori_main.\nIR:\n{main_fn}"
        );
        assert!(
            main_fn.contains("landingpad") && main_fn.contains("catch ptr null"),
            "Itanium main wrapper must provide a catch-all boundary.\nIR:\n{main_fn}"
        );
    }
    assert!(
        main_fn.contains("call void @ori_report_uncaught_panic()"),
        "the true process boundary must report an uncaught panic.\nIR:\n{main_fn}"
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
        include_str!("fixtures/cli/jit_str_list_construction.ori"),
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

/// Regression: source-level catch owns the observable panic. User-defined
/// operator panics recovered by the LLVM executor must not reach stderr.
#[test]
fn llvm_test_caught_user_operator_panics_emit_no_stderr() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = create_test_source(
        &temp_dir,
        "caught_user_operator_panics.ori",
        include_str!("../../../../tests/spec/traits/user_operator_unwind_aot.ori"),
    );

    let result = Command::new(ori_binary())
        .args(["test", "--backend=llvm", source_path.to_str().unwrap()])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        result.status.success(),
        "caught user-operator tests failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("3 passed"),
        "expected all three operator catch cells to pass:\n{stdout}"
    );
    assert!(
        stderr.is_empty(),
        "caught user-operator panics must not emit uncaught diagnostics:\n{stderr}"
    );
}

/// Regression: a generic derive shell is not a concrete LLVM work item. The
/// valid instantiation compiles without an unresolved Named-type warning.
#[test]
fn llvm_test_generic_derived_comparable_emits_no_type_resolution_warning() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = create_test_source(
        &temp_dir,
        "generic_derived_comparable.ori",
        include_str!("../../../../tests/spec/traits/generic_derived_comparable_aot.ori"),
    );

    let result = Command::new(ori_binary())
        .args(["test", "--backend=llvm", source_path.to_str().unwrap()])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori test");
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        result.status.success(),
        "generic derived Comparable test failed.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "expected the generic derive cell to pass:\n{stdout}"
    );
    assert!(
        stderr.is_empty(),
        "a valid concrete derive must not emit type-resolution warnings:\n{stderr}"
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
        include_str!("fixtures/cli/main_args_owned_path_no_double_free.ori"),
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
        include_str!("fixtures/cli/main_args_owned_path_heap_strings.ori"),
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
        include_str!("fixtures/cli/main_args_owned_path_int_return.ori"),
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
        include_str!("fixtures/cli/main_args_owned_path_valgrind_clean.ori"),
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
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/cli/main_args_owned_wrapper_ir_no_normal_cleanup.ori"
    ));
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
