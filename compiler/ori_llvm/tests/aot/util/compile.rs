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

/// Assert that `exit_code` represents a panic (not compile failure, not clean
/// exit, not a non-panic crash signal).
///
/// Valid panic termination paths, in order of "correctness":
///
/// 1. Exit code 1 — `ori_run_main` / LLVM-generated `main()` caught the
///    panic cleanly via `invoke`/`landingpad` (Itanium) or SEH (MSVC).
///    This is the designed flow; §Unwinding ABI and
///    `ori_run_main` doc comment: "1: panic".
/// 2. SIGABRT (exit code 134 / signal -134) — panic bubbled past the
///    main wrapper and hit `abort()` (e.g., uncaught unwind on an
///    older toolchain path, or when the caller path lacks a landingpad).
/// 3. Windows `STATUS_STACK_BUFFER_OVERRUN` (`0xC0000409` /
///    `-1_073_740_791`) — MSVC `abort()` triggering
///    `__fastfail(FAST_FAIL_FATAL_APP_EXIT)`.
/// 4. Windows exit code 3 — traditional MSVC `abort()` exit code.
///
/// Rejects compile failures (`exit_code == -1`), clean exits (0), and
/// other crash signals (SIGSEGV -139/139, SIGBUS -135/135) that would
/// indicate a memory-safety bug rather than a controlled panic.
pub fn assert_panic_exit(exit_code: i32, label: &str, stderr: &str) {
    const STATUS_STACK_BUFFER_OVERRUN: i32 = -1_073_740_791; // 0xC0000409
    assert_ne!(exit_code, -1, "{label}: compilation failed:\n{stderr}");
    assert_ne!(exit_code, 0, "{label}: should panic, but exited 0");
    // Accept the four valid panic-termination codes enumerated above.
    let is_panic_exit = exit_code == 1
        || exit_code == -134
        || exit_code == 134
        || exit_code == STATUS_STACK_BUFFER_OVERRUN
        || exit_code == 3;
    assert!(
        is_panic_exit,
        "{label}: expected panic exit (1, SIGABRT 134/-134, Windows 0xC0000409/3), \
         got exit code {exit_code}:\n{stderr}",
    );
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

/// Compile and run an Ori program with extra environment variables set on the
/// child process, capturing output.
///
/// `extra_env` is a list of `(name, value)` pairs applied to the run step (the
/// compiled binary), in addition to the always-on `ORI_CHECK_LEAKS=1`. Use this
/// to capture `ORI_TRACE_RC=1` RC event traces (`[RC] alloc/inc/dec/free`
/// lines, per `ori_rt::rc::debug`) for refcount-balance assertions.
///
/// Returns `(exit_code, stdout, stderr)`.
pub fn compile_and_run_with_env(source: &str, extra_env: &[(&str, &str)]) -> (i32, String, String) {
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

    let mut run_cmd = Command::new(&binary_path);
    run_cmd.env("ORI_CHECK_LEAKS", "1");
    for (name, value) in extra_env {
        run_cmd.env(name, value);
    }
    let run_result = run_cmd.output().expect("Failed to execute binary");

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

/// Compile and run an Ori program with extra environment variables set on the
/// COMPILE (`ori build`) step, capturing output.
///
/// `build_env` is a list of `(name, value)` pairs applied to the `ori build`
/// invocation, in addition to the always-on `ORI_STDLIB`. The run step still
/// sets `ORI_CHECK_LEAKS=1`. Use this for compile-time flags that gate the
/// AIMS pipeline (e.g. `ORI_DISABLE_BURDEN_OPS=1` skips Step 4b burden-op
/// emission); the run-step `compile_and_run_with_env`
/// cannot reach a compile-time flag because it sets env on the child binary.
///
/// Returns `(exit_code, stdout, stderr)`.
pub fn compile_and_run_with_build_env(
    source: &str,
    build_env: &[(&str, &str)],
) -> (i32, String, String) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_{id}{}", std::env::consts::EXE_SUFFIX));

    fs::write(&source_path, source).expect("Failed to write source");

    let mut build_cmd = Command::new(ori_binary());
    build_cmd
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path());
    for (name, value) in build_env {
        build_cmd.env(name, value);
    }
    let compile_result = build_cmd.output().expect("Failed to execute ori build");

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
