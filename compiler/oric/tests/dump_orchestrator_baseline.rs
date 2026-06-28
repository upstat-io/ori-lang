//! Golden baseline pins for the oric phase-dump DRIVERS.
//!
//! The four `ORI_DUMP_AFTER_*` drivers (`ast_dump`, `ir_dump`, `arc_dump`,
//! `llvm_dump`) carry no output tests of their own. This module pins each
//! driver's default (unfiltered) stderr block on the nested-generic
//! `Wrap<Wrap<int>>` fixture against a committed golden file, so a later
//! refactor of the driver surface can be verified byte-identical against the
//! captured baseline.
//!
//! Phase → command mapping:
//! - `ORI_DUMP_AFTER_PARSE`  / `ORI_DUMP_AFTER_TYPECK` fire on `ori check`
//!   (frontend).
//! - `ORI_DUMP_AFTER_ARC`    / `ORI_DUMP_AFTER_LLVM`   fire on `ori build`
//!   (codegen pipeline).
//!
//! Each driver brackets its output with a stable `=== <phase> ===` … `=== END
//! <phase> ===` pair; only that block is extracted (the surrounding tracing
//! lines, the `ori_llvm` emit-path ARC dump, and any downstream build error are
//! deliberately excluded — they are not the oric driver surface under test).
//!
//! The `ori build` legs on this fixture currently exit non-zero (the
//! generated `_ori_drop$<idx>` call on a scalar payload trips LLVM module
//! verification); the driver block is emitted before that abort, so the pin
//! captures driver output regardless of the build's exit status.
//!
//! Regenerate the goldens with `ORI_BLESS=1 cargo test -p oric --test
//! dump_orchestrator_baseline`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/provenance/wrap_nested.ori")
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dump_baseline")
        .join(format!("{name}.golden"))
}

/// Run the real `ori` binary with exactly ONE `ORI_DUMP_AFTER_*` flag set and
/// return combined stdout+stderr (driver dumps go to stderr). `out_dir`, when
/// supplied, is passed as the `ori build` `-o` target so no binary lands in the
/// working tree.
fn run_with_flag(subcommand: &str, flag: &str, out_dir: Option<&Path>) -> String {
    run_with_envs(subcommand, &[(flag, "1")], out_dir)
}

/// Like `run_with_flag` but sets several env vars at once (for the multi-flag
/// `ORI_DUMP_AFTER_TYPECK` + `ORI_DUMP_TYPE_IDX` idx-filter view).
fn run_with_envs(subcommand: &str, envs: &[(&str, &str)], out_dir: Option<&Path>) -> String {
    let fixture = fixture_path();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg(subcommand).arg(&fixture);
    if let Some(dir) = out_dir {
        cmd.arg("-o").arg(dir.join("out"));
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to run ori {subcommand}: {e}"));
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined
}

/// Extract the inclusive driver block spanning the line containing
/// `begin_marker` through the line containing `end_marker`, LF-normalize it,
/// and replace the absolute fixture path with `<FIXTURE>` so the golden is
/// machine-independent.
fn extract_block(output: &str, begin_marker: &str, end_marker: &str) -> String {
    let normalized = output.replace("\r\n", "\n");
    let begin = normalized.find(begin_marker).unwrap_or_else(|| {
        panic!("dump output missing begin marker `{begin_marker}`:\n{normalized}")
    });
    let line_start = normalized[..begin].rfind('\n').map_or(0, |i| i + 1);
    let from_begin = &normalized[line_start..];
    let end_rel = from_begin
        .find(end_marker)
        .unwrap_or_else(|| panic!("dump output missing end marker `{end_marker}`:\n{normalized}"));
    let after_end = end_rel + end_marker.len();
    let block_len = from_begin[after_end..]
        .find('\n')
        .map_or(from_begin.len(), |i| after_end + i + 1);
    let block = &from_begin[..block_len];

    let fixture_str = fixture_path().display().to_string();
    block.replace(&fixture_str, "<FIXTURE>")
}

/// Assert the extracted block matches its committed golden, or (re)write the
/// golden under `ORI_BLESS=1`. Goldens are stored LF; both sides are normalized
/// before comparison per the cross-platform discipline.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_path(name);
    if std::env::var("ORI_BLESS").as_deref() == Ok("1") {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                panic!("failed to create golden dir {}: {e}", parent.display())
            });
        }
        std::fs::write(&path, actual)
            .unwrap_or_else(|e| panic!("failed to write golden {}: {e}", path.display()));
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| {
            panic!(
                "missing golden {} (run `ORI_BLESS=1 cargo test -p oric --test \
                 dump_orchestrator_baseline` to create): {e}",
                path.display()
            )
        })
        .replace("\r\n", "\n");
    assert_eq!(
        actual, expected,
        "phase-dump driver `{name}` output drifted from its committed baseline; \
         a refactor of the driver surface must keep this byte-identical (re-bless \
         only after confirming the change against the spec)"
    );
}

#[test]
fn ori_dump_after_parse_pins_ast_driver_block() {
    let out = run_with_flag("check", "ORI_DUMP_AFTER_PARSE", None);
    let block = extract_block(&out, "=== AST after parse:", "=== END AST ===");

    // Load-bearing structure: the nested struct literal + double field access.
    assert!(
        block.contains("Type Wrap"),
        "AST dump must list the Wrap type:\n{block}"
    );
    assert!(
        block.matches("Struct Wrap").count() >= 2,
        "AST dump must show the nested `Wrap {{ inner: Wrap {{ ... }} }}` literal:\n{block}"
    );
    assert!(
        block.contains("Int(7)"),
        "AST dump must show the inner literal:\n{block}"
    );
    assert!(
        block.matches("Field .inner").count() >= 2,
        "AST dump must show the double `.inner.inner` projection:\n{block}"
    );

    assert_golden("parse", &block);
}

#[test]
fn ori_dump_after_typeck_pins_typed_ir_driver_block() {
    let out = run_with_flag("check", "ORI_DUMP_AFTER_TYPECK", None);
    let block = extract_block(&out, "=== Typed IR after typeck:", "=== END Typed IR ===");

    // Load-bearing structure: nested-generic instantiation resolved on nodes.
    assert!(
        block.contains("Wrap<Wrap<int>>"),
        "typed-IR dump must carry the outer nested-generic type:\n{block}"
    );
    assert!(
        block.contains("Wrap<int>"),
        "typed-IR dump must carry the inner instantiation:\n{block}"
    );
    assert!(
        block.contains("@main () -> int"),
        "typed-IR dump must carry the resolved signature:\n{block}"
    );

    assert_golden("typeck", &block);
}

#[test]
fn ori_dump_after_arc_pins_arc_ir_driver_block() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let out = run_with_flag("build", "ORI_DUMP_AFTER_ARC", Some(dir.path()));
    // Scope to the oric `arc_dump` driver block, NOT the `ori_llvm` emit-path
    // `=== ARC IR (emit path, prepared) ===` block.
    let block = extract_block(&out, "=== ARC IR after lowering:", "=== END ARC IR ===");

    // Load-bearing structure: the nested Construct chain + projection + dec.
    assert!(
        block.matches("Construct Struct(Wrap)").count() >= 2,
        "ARC dump must show the nested `Construct Struct(Wrap)` chain:\n{block}"
    );
    assert!(
        block.contains("Project") && block.contains("RcDec"),
        "ARC dump must show the projection read and the RcDec placement:\n{block}"
    );
    assert!(
        block.contains("Wrap<Wrap<int>>"),
        "ARC dump must carry the nested-generic aggregate type:\n{block}"
    );

    assert_golden("arc", &block);
}

#[test]
fn ori_dump_after_llvm_pins_llvm_ir_driver_block() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let out = run_with_flag("build", "ORI_DUMP_AFTER_LLVM", Some(dir.path()));
    let block = extract_block(&out, "=== LLVM IR after codegen:", "=== END LLVM IR ===");

    // Load-bearing structure: the demangled entry point + the generic-leaf drop
    // glue the diagnostic surface exists to attribute.
    assert!(
        block.contains("@_ori_main"),
        "LLVM dump must carry the demangled entry point:\n{block}"
    );
    assert!(
        block.contains("ori_rc_dec") && block.contains("_ori_drop$"),
        "LLVM dump must carry the generic-leaf RC dec + drop glue:\n{block}"
    );
    assert!(
        block.contains("; RC--"),
        "LLVM dump must carry the oric driver's RC annotation:\n{block}"
    );

    assert_golden("llvm", &block);
}

/// The typeck idx-filter view (`ORI_DUMP_AFTER_TYPECK` + `ORI_DUMP_TYPE_IDX`)
/// routed through the dump orchestrator annotates every rendered type with its
/// pool `Idx` -- each gains a `#<idx>` suffix.
#[test]
fn ori_dump_type_idx_typeck_view_annotates_types_with_pool_idx() {
    let out = run_with_envs(
        "check",
        &[("ORI_DUMP_AFTER_TYPECK", "1"), ("ORI_DUMP_TYPE_IDX", "1")],
        None,
    );
    let block = extract_block(&out, "=== Typed IR after typeck:", "=== END Typed IR ===");

    // The idx-filter view annotates the nested-generic type with its pool Idx.
    let marker = "Wrap<Wrap<int>>#";
    let pos = block
        .find(marker)
        .unwrap_or_else(|| panic!("idx-filter view must annotate types with `#<idx>`:\n{block}"));
    // The annotation is a real numeric pool index, not a bare `#`.
    let next = block[pos + marker.len()..].chars().next();
    assert!(
        next.is_some_and(|c| c.is_ascii_digit()),
        "the `#` annotation must be a numeric pool Idx:\n{block}"
    );
}

/// The flag-off path -- with `ORI_DUMP_AFTER_TYPECK` alone (the idx-filter OFF),
/// the typeck dump carries NO idx noise: types render name-only with no `#<idx>`
/// suffix. The unfiltered default output unchanged is pinned byte-for-byte by
/// `ori_dump_after_typeck_pins_typed_ir_driver_block`.
#[test]
fn ori_dump_after_typeck_without_idx_filter_emits_no_idx_noise() {
    let out = run_with_flag("check", "ORI_DUMP_AFTER_TYPECK", None);
    let block = extract_block(&out, "=== Typed IR after typeck:", "=== END Typed IR ===");

    // Name-only types are present...
    assert!(
        block.contains("Wrap<Wrap<int>>") && block.contains("Wrap<int>"),
        "the typeck dump must carry name-resolved types:\n{block}"
    );
    // ...and NO type carries a `#<idx>` annotation when the idx-filter is off.
    assert!(
        !block.contains('#'),
        "flag-off typeck dump must carry no idx noise (`#<idx>`):\n{block}"
    );
}
