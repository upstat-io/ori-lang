//! Multi-module synthesized-glue linkage cells.
//!
//! Every per-module synthesized helper (`_ori_Error_ctor`, `_ori_drop$<idx>`,
//! `_ori_elem_dec$<idx>`, the eq/hash/partial/catch-thunk families) is
//! referenced only within its emitting module, so it must carry INTERNAL
//! linkage: an external definition collides at the multi-module link step
//! (`ld: multiple definition`) and, because per-module pools can assign the
//! same idx to different types, any weak dedup would be memory corruption.

use std::process::Command;

use crate::util::{assert_multifile_cell_output, compile_and_run_capture, ori_binary, stdlib_path};

const ERRLIB: &str = include_str!("fixtures/multi_module_glue/errlib.ori");
const CATCHLIB: &str = include_str!("fixtures/multi_module_glue/catchlib.ori");

// Minimal isolation of the collision class: both modules construct an
// `Error`, so both pools materialize the builtin and both objects would
// define the ctor (plus its drop/elem-dec glue).
#[test]
fn error_ctor_in_both_modules_links_and_runs() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/multi_module_glue/host_error_both.ori"),
            ),
            ("errlib.ori", ERRLIB),
        ],
        "error_ctor_in_both_modules",
        "from-lib local",
    );
}

// Catch + lambdas + drop glue in both modules — the thunk/wrapper families
// ride the same per-module emission.
#[test]
fn catch_and_closures_in_both_modules_link_and_run() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/multi_module_glue/host_catch_both.ori"),
            ),
            ("catchlib.ori", CATCHLIB),
        ],
        "catch_and_closures_in_both_modules",
        "10 4",
    );
}

// In-module regression guard: internal linkage must not break first-class
// `Error` use within the emitting module.
#[test]
fn single_module_first_class_error_still_works() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/multi_module_glue/first_class_error.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "first_class_error: expected clean exit, got {exit_code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "boom", "first_class_error: wrong stdout");
}

// Namespace audit: every synthesized-glue definition in the emitted LLVM IR
// carries `internal` linkage. Clamps the families the multi-module cells do
// not happen to exercise (eq/hash/partial/catch-thunk) alongside the ones
// they do.
#[test]
fn glue_symbols_are_module_local() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let src = temp.path().join("glue_families.ori");
    std::fs::write(
        &src,
        include_str!("fixtures/multi_module_glue/glue_families.ori"),
    )
    .expect("write fixture");
    let out = temp.path().join("glue_families.bin");

    let dump = Command::new(ori_binary())
        .args(["build", src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .env("ORI_STDLIB", stdlib_path())
        .env("ORI_DUMP_AFTER_LLVM", "1")
        .output()
        .expect("ori build");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&dump.stdout),
        String::from_utf8_lossy(&dump.stderr)
    );
    assert!(dump.status.success(), "glue_families build failed:\n{text}");

    let glue_prefixes = [
        "_ori_Error_ctor",
        "_ori_drop$",
        "_ori_elem_dec$",
        "_ori_eq_",
        "_ori_hash_",
        "_ori_partial_",
        "_ori_catch_thunk",
    ];
    let mut audited = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("define ") {
            continue;
        }
        if glue_prefixes
            .iter()
            .any(|p| trimmed.contains(&format!("@{p}")) || trimmed.contains(&format!("@\"{p}")))
        {
            audited += 1;
            assert!(
                trimmed.starts_with("define internal "),
                "synthesized glue defined without internal linkage: {trimmed}"
            );
        }
    }
    assert!(
        audited >= 3,
        "glue audit exercised too few families ({audited}); fixture must materialize \
         Error ctor + drop glue + elem-dec at minimum\n{text}"
    );
}
