//! Production-entry diagnostics for executable builds.

use std::process::{Command, Output};

fn require_success<T, E: std::fmt::Display>(result: Result<T, E>, operation: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{operation} failed: {error}"),
    }
}

fn run_build(source: &str, extra_args: &[&str]) -> Output {
    let dir = require_success(tempfile::tempdir(), "create temporary build directory");
    let source_path = dir.path().join("module.ori");
    require_success(std::fs::write(&source_path, source), "write Ori source");

    require_success(
        Command::new(env!("CARGO_BIN_EXE_ori"))
            .arg("build")
            .arg(&source_path)
            .args(extra_args)
            .env("NO_COLOR", "1")
            .current_dir(dir.path())
            .output(),
        "run ori build",
    )
}

#[test]
fn executable_without_main_reports_cause_and_fix_before_linking() {
    let output = run_build("@helper () -> int = 42;\n", &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "build unexpectedly succeeded");
    assert!(
        stderr.contains("no @main function was declared"),
        "diagnostic must name the missing Ori entry point:\n{stderr}"
    );
    assert!(
        stderr.contains("add an @main function") && stderr.contains("--lib"),
        "diagnostic must give executable and library fixes:\n{stderr}"
    );
    assert!(
        !stderr.contains("undefined reference to `main`")
            && !stderr.contains("undefined reference to main"),
        "missing entry point must be rejected before the system linker:\n{stderr}"
    );
}

#[test]
fn ir_emission_without_main_remains_supported() {
    let output = run_build("@helper () -> int = 42;\n", &["--emit=llvm-ir"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "non-executable IR emission must not require @main:\n{stderr}"
    );
}
