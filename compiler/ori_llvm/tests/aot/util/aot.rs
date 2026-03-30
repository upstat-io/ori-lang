//! AOT compile-and-run helpers and IR inspection utilities.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};

use tempfile::TempDir;

// AOT compile-and-run helpers

/// Extract exit code from a process status, distinguishing signal kills from normal exits.
///
/// On Unix, processes killed by a signal (e.g., SIGSEGV=11, SIGABRT=6) have
/// `status.code() == None`. This function maps signal-killed processes to
/// `-(128 + signal)` (e.g., SIGSEGV → -139, SIGABRT → -134), matching the
/// bash convention of 128+N for signal exits.
///
/// This is critical for panic-path tests: a post-panic crash (SIGSEGV) must be
/// distinguishable from a clean panic exit (non-zero code).
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

/// Get the workspace root directory (contains `Cargo.toml` + `compiler/`).
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("compiler").exists())
        .map_or_else(|| PathBuf::from("/workspace"), Path::to_path_buf)
}

/// Get the path to the standard library (`library/` in workspace root).
pub fn stdlib_path() -> PathBuf {
    workspace_root().join("library")
}

/// Get the path to an LLVM-enabled `ori` binary.
///
/// Picks the binary matching the current build profile (`cargo test` = debug,
/// `cargo test --release` = release). Falls back to the other profile only if
/// the matching one has no LLVM support. Panics if neither has LLVM support.
///
/// **Why not mtime?** The previous mtime-based selection silently picked stale
/// release binaries when `cargo build` (debug) was newer, causing tests to run
/// against code without recent fixes. Profile-matching ensures `cargo build &&
/// cargo test` always tests the binary you just built.
pub fn ori_binary() -> PathBuf {
    let workspace_root = workspace_root();

    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let debug_path = workspace_root.join("target/debug").join(&exe);
    let release_path = workspace_root.join("target/release").join(&exe);

    let debug_llvm = debug_path.exists() && has_llvm_support(&debug_path);
    let release_llvm = release_path.exists() && has_llvm_support(&release_path);

    // Pick the binary that matches the current build profile.
    // `cargo test` compiles tests in debug mode → use debug ori binary.
    // `cargo test --release` compiles tests in release → use release ori binary.
    let (preferred, fallback) = if cfg!(debug_assertions) {
        ((debug_llvm, &debug_path), (release_llvm, &release_path))
    } else {
        ((release_llvm, &release_path), (debug_llvm, &debug_path))
    };

    if preferred.0 {
        return preferred.1.clone();
    }
    if fallback.0 {
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        eprintln!(
            "warning: {profile} ori binary has no LLVM support, falling back to other profile"
        );
        return fallback.1.clone();
    }

    panic!(
        "No LLVM-enabled ori binary found.\n\
         AOT tests require `ori` built with LLVM (enabled by default).\n\
         Run `cargo build` (debug) or `cargo build --release` (release) first,\n\
         or use `./llvm-test.sh` which builds automatically.\n\
         \n\
         Checked:\n  \
           debug:   {} (exists: {})\n  \
           release: {} (exists: {})",
        debug_path.display(),
        debug_path.exists(),
        release_path.display(),
        release_path.exists(),
    );
}

/// Check whether an `ori` binary has LLVM/AOT support by running `ori build`
/// with no arguments and checking for the E5004 "requires LLVM backend" error.
fn has_llvm_support(binary: &Path) -> bool {
    Command::new(binary)
        .args(["build", "--help"])
        .output()
        .map(|o| {
            // A non-LLVM binary prints E5004 to stderr; an LLVM binary
            // prints "cannot find file" or usage info (but no E5004).
            let stderr = String::from_utf8_lossy(&o.stderr);
            !stderr.contains("E5004")
        })
        .unwrap_or(false)
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
/// other signal. Use this in panic-path tests to distinguish clean panic
/// propagation from post-panic crashes.
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
/// The program is expected to exit abnormally (e.g., panic) — only memory errors matter.
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
    // The program may exit with any code (e.g., panic exits non-zero) —
    // we only care whether Valgrind itself detected errors.
    let vg_exit = vg_result.status.code().unwrap_or(-1);
    let clean = vg_exit != 42;

    Some((clean, vg_stderr))
}

/// Return the debug `ori` binary for IR capture.
///
/// IR dump (`ORI_DEBUG_LLVM=1`) is only available in debug builds — the release
/// binary compiles out phase dumps via `dbg_set!`. IR-quality tests must use the
/// debug binary to capture IR, regardless of the test harness build profile.
///
/// Panics if no debug binary exists. This is intentional: IR-quality tests are
/// semantic pins that must never silently degrade to no-ops. If you see this
/// panic in `cargo test --release`, run `cargo build` first to produce the
/// debug binary.
fn ir_capture_binary() -> PathBuf {
    let workspace_root = workspace_root();
    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let debug_path = workspace_root.join("target/debug").join(&exe);

    if debug_path.exists() && has_llvm_support(&debug_path) {
        return debug_path;
    }
    panic!(
        "No debug ori binary found for IR capture.\n\
         IR-quality tests require the debug binary (release compiles out IR dumps).\n\
         Run `cargo build` to build the debug binary first."
    );
}

/// Compile an Ori program and capture its LLVM IR (via `ORI_DEBUG_LLVM=1`).
///
/// Uses the debug `ori` binary for IR capture — the release binary compiles out
/// phase dumps. Returns the IR string from compilation stderr. Panics if
/// compilation fails.
pub fn compile_and_capture_ir(source: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_ir_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_ir_{id}{}", std::env::consts::EXE_SUFFIX));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ir_capture_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .env("ORI_DEBUG_LLVM", "1")
        .output()
        .expect("Failed to execute ori build");

    assert!(
        compile_result.status.success(),
        "Compilation failed:\n{}",
        String::from_utf8_lossy(&compile_result.stderr)
    );

    String::from_utf8_lossy(&compile_result.stderr).to_string()
}

// IR inspection helpers

/// Extract a single function's LLVM IR from a full module dump.
///
/// Finds `define ... @func_name(` and returns everything up to the next
/// `define` or end of IR. Panics if the function is not found.
pub fn extract_function_ir<'a>(full_ir: &'a str, func_name: &str) -> &'a str {
    let search = format!("@{func_name}(");
    let start = full_ir.find(&search).unwrap_or_else(|| {
        panic!(
            "function {func_name} not found in IR.\n\
             Available functions: {:?}",
            full_ir
                .lines()
                .filter(|l| l.starts_with("define "))
                .collect::<Vec<_>>()
        );
    });

    // Find the "define" line containing this function
    let define_start = full_ir[..start].rfind("define ").unwrap_or(start);

    // Find the next "define" or end of IR section
    let rest = &full_ir[define_start..];
    let end = rest[1..]
        .find("\ndefine ")
        .map_or(rest.len(), |pos| pos + 1);

    &full_ir[define_start..define_start + end]
}

/// Count "bridge-only" blocks in a function's LLVM IR.
///
/// A bridge-only block is one whose only non-comment, non-blank instruction
/// is `br label %target` — no phi, no computation, no conditional branch.
/// These represent redundant blocks from invoke splitting that should have
/// been merged by the ARC block merge pass.
///
/// Parses the function IR structurally (label lines → next label or `}`),
/// handling LLVM output variations (predecessor comments, spacing, label
/// formats).
pub fn count_bridge_blocks(function_ir: &str) -> usize {
    let mut bridge_count = 0;
    let mut in_block = false;
    let mut block_instrs: Vec<&str> = Vec::new();

    for line in function_ir.lines() {
        let trimmed = line.trim();

        // Detect block start: a label line (e.g., "bb1:", "entry:", "4:")
        // or the opening "{" of the function.
        let is_label =
            trimmed.ends_with(':') && !trimmed.starts_with(';') && !trimmed.contains("  ; preds");

        // Also match "labelname: ; preds = ..." pattern
        let is_label = is_label
            || trimmed.contains(':') && !trimmed.starts_with(';') && {
                let colon_pos = trimmed.find(':');
                colon_pos.is_some_and(|p| {
                    let before = &trimmed[..p];
                    // Label is alphanumeric/underscore/dot, possibly with numeric prefix
                    !before.is_empty()
                        && before
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '%')
                })
            };

        if is_label || trimmed == "{" {
            // Flush previous block.
            if in_block && is_bridge_only(&block_instrs) {
                bridge_count += 1;
            }
            block_instrs.clear();
            in_block = is_label;
            continue;
        }

        if trimmed == "}" {
            // End of function — flush.
            if in_block && is_bridge_only(&block_instrs) {
                bridge_count += 1;
            }
            break;
        }

        if in_block {
            block_instrs.push(trimmed);
        }
    }

    bridge_count
}

/// Check if a block's instructions consist of only an unconditional `br label`.
fn is_bridge_only(instrs: &[&str]) -> bool {
    let meaningful: Vec<&&str> = instrs
        .iter()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty() && !l.starts_with(';')
        })
        .collect();

    meaningful.len() == 1 && meaningful[0].starts_with("br label ")
}

/// Count phi nodes with exactly one incoming edge in a function's IR.
///
/// A single-predecessor phi `%x = phi T [ %v, %bb ]` with one incoming
/// edge is redundant — equivalent to using `%v` directly. These should
/// be eliminated by the ARC block merge pass (Phase 5).
pub fn count_single_pred_phis(function_ir: &str) -> usize {
    function_ir
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with(';') {
                return false;
            }
            if !trimmed.contains(" = phi ") {
                return false;
            }
            // Single incoming edge = exactly one `[` bracket pair.
            trimmed.matches('[').count() == 1
        })
        .count()
}

/// Count phi nodes whose result is never used elsewhere in the function.
///
/// A dead phi `%vN = phi T [...]` defines a value that is never referenced
/// as an operand in any other instruction. These arise from dead block params
/// on multi-predecessor exit blocks (e.g., mutable loop variables not used
/// after the loop).
pub fn count_dead_phis(function_ir: &str) -> usize {
    let lines: Vec<&str> = function_ir.lines().collect();

    lines
        .iter()
        .enumerate()
        .filter(|(i, line)| {
            let trimmed = line.trim();
            if !trimmed.contains(" = phi ") || trimmed.starts_with(';') {
                return false;
            }
            let name = match trimmed.split_whitespace().next() {
                Some(n) if n.starts_with('%') => n,
                _ => return false,
            };
            // Dead if name never appears in any OTHER non-comment line.
            !lines.iter().enumerate().any(|(j, other)| {
                if i == &j {
                    return false;
                }
                let ot = other.trim();
                if ot.starts_with(';') {
                    return false;
                }
                is_ssa_var_used_in(name, ot)
            })
        })
        .count()
}

/// Check whether `%name` appears as a standalone SSA variable in `line`.
///
/// Handles the `%v3` vs `%v34` ambiguity by requiring a word boundary
/// (non-alphanumeric, non-underscore) after the variable name.
fn is_ssa_var_used_in(var_name: &str, line: &str) -> bool {
    let mut search_from = 0;
    while let Some(pos) = line[search_from..].find(var_name) {
        let abs_pos = search_from + pos;
        let after = abs_pos + var_name.len();
        let next_char = line[after..].chars().next();
        let is_boundary = next_char.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        if is_boundary {
            return true;
        }
        search_from = abs_pos + 1;
    }
    false
}

/// Compile an Ori program to LLVM IR and return the IR text.
///
/// Returns `Ok(ir_text)` on success, `Err(stderr)` on compilation failure.
pub fn compile_to_llvm_ir(source: &str) -> Result<String, String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("ir_test_{id}.ori"));
    let ir_path = temp_dir.path().join(format!("ir_test_{id}.ll"));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "--emit=llvm-ir",
            "-o",
            ir_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        return Err(String::from_utf8_lossy(&compile_result.stderr).to_string());
    }

    fs::read_to_string(&ir_path).map_err(|e| format!("Failed to read IR file: {e}"))
}
