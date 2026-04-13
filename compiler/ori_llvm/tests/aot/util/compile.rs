//! Compile-and-run helpers for AOT integration tests.
//!
//! Provides `compile_and_run()`, `compile_and_run_capture()`, and assertion
//! helpers for compiling Ori source through the AOT pipeline and running
//! the resulting binary.

use std::fs;
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};

use tempfile::TempDir;

use super::binary::{ori_binary, stdlib_path};

/// Extract exit code from a process status, distinguishing signal kills from normal exits.
///
/// On Unix, processes killed by a signal (e.g., SIGSEGV=11, SIGABRT=6) have
/// `status.code() == None`. This function maps signal-killed processes to
/// `-(128 + signal)` (e.g., SIGSEGV → -139, SIGABRT → -134), matching the
/// bash convention of 128+N for signal exits.
fn exit_code_from_status(status: ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return -(128 + signal);
        }
    }
    status.code().unwrap_or(-1)
}

/// Compile and run an Ori program, returning the exit code.
///
/// Returns 0 on success, non-zero on failure, -1 if compilation fails.
pub fn compile_and_run(source: &str) -> i32 {
    let (exit_code, _, stderr) = compile_and_run_capture(source);
    if exit_code < 0 && !stderr.is_empty() {
        eprintln!("Compilation failed:\n{stderr}");
    }
    exit_code
}

/// Compile and run a multi-file Ori program (entry + libraries), capturing output.
///
/// `files` is a list of `(filename, content)` pairs. The first file is the
/// entry point passed to `ori build`. All files are written to the same temp
/// directory so relative imports (`use "./lib" { ... }`) resolve correctly.
///
/// Returns `(exit_code, stdout, stderr)`. Enables `ORI_CHECK_LEAKS=1`.
pub fn compile_multifile_and_run_capture(files: &[(&str, &str)]) -> (i32, String, String) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Write all source files
    for (name, content) in files {
        let path = temp_dir.path().join(name);
        fs::write(&path, content).expect("Failed to write source");
    }

    let entry_path = temp_dir.path().join(files[0].0);
    let binary_path = temp_dir
        .path()
        .join(format!("test_multi_{id}{}", std::env::consts::EXE_SUFFIX));

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            entry_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return (-1, String::new(), stderr);
    }

    let run_result = Command::new(&binary_path)
        .env("ORI_CHECK_LEAKS", "1")
        .output()
        .expect("Failed to execute binary");

    let exit_code = exit_code_from_status(run_result.status);
    let stdout = String::from_utf8_lossy(&run_result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
    (exit_code, stdout, stderr)
}

/// Assert that a multi-file Ori program compiles and runs with exit code 0.
///
/// See [`compile_multifile_and_run_capture`] for the file format.
pub fn assert_multifile_aot_success(files: &[(&str, &str)], test_name: &str) {
    let (exit_code, _, stderr) = compile_multifile_and_run_capture(files);
    match exit_code {
        0 => {} // success
        2 => panic!("{test_name} leaked memory:\n{stderr}"),
        -1 => panic!("{test_name} compilation failed:\n{stderr}"),
        code => panic!("{test_name} failed with exit code {code}:\n{stderr}"),
    }
}

/// Assert that a program compiles and runs with exit code 0.
///
/// Automatically enables ARC leak detection (`ORI_CHECK_LEAKS=1`) in the
/// child process. Exit codes: 0=success, 1=panic, 2=leak detected.
pub fn assert_aot_success(source: &str, test_name: &str) {
    let (exit_code, _, stderr) = compile_and_run_capture(source);
    match exit_code {
        0 => {} // success
        2 => panic!("{test_name} leaked memory:\n{stderr}"),
        -1 => panic!("{test_name} compilation failed:\n{stderr}"),
        code => panic!("{test_name} failed with exit code {code}:\n{stderr}"),
    }
}

/// Compile and run an Ori program, capturing stdout output.
///
/// Returns `(exit_code, stdout, stderr)`.
pub fn compile_and_run_capture(source: &str) -> (i32, String, String) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_{id}{}", std::env::consts::EXE_SUFFIX));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return (-1, String::new(), stderr);
    }

    let run_result = Command::new(&binary_path)
        .env("ORI_CHECK_LEAKS", "1")
        .output()
        .expect("Failed to execute binary");

    let exit_code = exit_code_from_status(run_result.status);
    let stdout = String::from_utf8_lossy(&run_result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
    (exit_code, stdout, stderr)
}

/// Compile and run an Ori program with arguments, capturing output.
///
/// Returns `(exit_code, stdout, stderr)`. Enables `ORI_CHECK_LEAKS=1`.
pub fn compile_and_run_with_args(source: &str, args: &[&str]) -> (i32, String, String) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_{id}{}", std::env::consts::EXE_SUFFIX));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return (-1, String::new(), stderr);
    }

    let run_result = Command::new(&binary_path)
        .args(args)
        .env("ORI_CHECK_LEAKS", "1")
        .output()
        .expect("Failed to execute binary");

    let exit_code = exit_code_from_status(run_result.status);
    let stdout = String::from_utf8_lossy(&run_result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
    (exit_code, stdout, stderr)
}

/// Assert that an exit code does not indicate signal termination (crash).
///
/// On Unix, signal-killed processes are mapped to `-(128 + signal)` by
/// `exit_code_from_status`. This function panics if the exit code indicates
/// SIGSEGV (11 → -139), SIGABRT (6 → -134), SIGBUS (7 → -135), or any
/// other signal.
pub fn assert_no_signal_crash(exit_code: i32, context: &str) {
    if exit_code <= -128 {
        let signal = -(exit_code + 128);
        let signal_name = match signal {
            6 => "SIGABRT",
            7 => "SIGBUS",
            11 => "SIGSEGV",
            _ => "unknown signal",
        };
        panic!(
            "{context}: process was killed by {signal_name} (signal {signal}), \
             indicating a crash rather than clean panic propagation"
        );
    }
}

/// Compile and run an Ori program with arguments under Valgrind, checking for memory errors.
///
/// Returns `true` if Valgrind reports no errors, `false` if Valgrind found issues.
/// Returns `None` if Valgrind is not available (test should be skipped).
pub fn compile_and_run_valgrind_with_args(source: &str, args: &[&str]) -> Option<(bool, String)> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    // Check Valgrind availability
    let valgrind_available = Command::new("valgrind")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !valgrind_available {
        return None;
    }

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_vg_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_vg_{id}{}", std::env::consts::EXE_SUFFIX));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return Some((false, format!("compilation failed:\n{stderr}")));
    }

    let mut vg_args = vec![
        "--error-exitcode=42".to_string(),
        "--leak-check=full".to_string(),
        "--errors-for-leak-kinds=definite,possible".to_string(),
        binary_path.to_str().unwrap().to_string(),
    ];
    for arg in args {
        vg_args.push((*arg).to_string());
    }

    let vg_result = Command::new("valgrind")
        .args(&vg_args)
        .output()
        .expect("Failed to execute valgrind");

    let vg_stderr = String::from_utf8_lossy(&vg_result.stderr).to_string();

    // Valgrind uses exit code 42 (our --error-exitcode) for memory errors.
    let vg_exit = vg_result.status.code().unwrap_or(-1);
    let clean = vg_exit != 42;

    Some((clean, vg_stderr))
}
