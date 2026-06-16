//! L12 production-path pins for `ori emit-scip` (walking-skeleton SCIP slice).
//!
//! Spawns the REAL `ori` binary against a temp `.ori` fixture and asserts the
//! emitted `index.scip` carries the `SymbolInformation` for the first
//! top-level function. The CLI-handler layer is otherwise unit-untested, so
//! this end-to-end pin is the load-bearing coverage for the emit pipe.

use std::path::Path;
use std::process::Command;

/// Single-function fixture: the emitted index must carry exactly one symbol.
const SINGLE_FN_FIXTURE: &str = include_str!("fixtures/emit_scip/greet.ori");

/// Two-function fixture: functions sort by name, so `add` (not `zebra`) is the
/// first top-level function the skeleton emits.
const TWO_FN_FIXTURE: &str = include_str!("fixtures/emit_scip/two_fn.ori");

/// Type-error fixture: `name` is `str`; adding an `int` is a type error, so the
/// frontend rejects it and no index is written.
const TYPE_ERROR_FIXTURE: &str = include_str!("fixtures/emit_scip/type_error.ori");

/// Function-less fixture: a type-only file type-checks cleanly but has no
/// top-level function, so the emitted index carries zero symbols.
const NO_FN_FIXTURE: &str = include_str!("fixtures/emit_scip/no_fn.ori");

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

fn run_emit_scip(fixture: &Path, output: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("emit-scip")
        .arg(fixture)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ori emit-scip: {e}"))
}

#[test]
fn emit_scip_writes_symbol_information_for_top_level_function() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "greet.ori", SINGLE_FN_FIXTURE);
    let output = dir.path().join("index.scip");

    let out = run_emit_scip(&fixture, &output);
    assert!(
        out.status.success(),
        "emit-scip exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let index =
        std::fs::read_to_string(&output).unwrap_or_else(|e| panic!("index.scip written: {e}"));
    let parsed: serde_json::Value =
        serde_json::from_str(&index).unwrap_or_else(|e| panic!("index.scip is valid JSON: {e}"));

    assert_eq!(parsed["format"], "ori-scip-minimal/v1");
    let symbols = parsed["documents"][0]["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("symbols array"));
    assert_eq!(
        symbols.len(),
        1,
        "exactly one symbol for the single function"
    );

    let sym = &symbols[0];
    assert_eq!(sym["display_name"], "greet");
    assert_eq!(sym["kind"], "Function");
    assert_eq!(
        sym["symbol"], "ori . . . greet().",
        "SCIP moniker carries the function name as a function descriptor"
    );
}

#[test]
fn emit_scip_emits_first_function_by_sort_order() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "two.ori", TWO_FN_FIXTURE);
    let output = dir.path().join("index.scip");

    let out = run_emit_scip(&fixture, &output);
    assert!(out.status.success(), "emit-scip exited non-zero");

    let index =
        std::fs::read_to_string(&output).unwrap_or_else(|e| panic!("index.scip written: {e}"));
    let parsed: serde_json::Value =
        serde_json::from_str(&index).unwrap_or_else(|e| panic!("valid JSON: {e}"));
    let symbols = parsed["documents"][0]["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("symbols array"));

    assert_eq!(symbols.len(), 1);
    // Negative pin: NOT `zebra` — the first-by-name function is the deliverable.
    assert_eq!(symbols[0]["display_name"], "add");
    assert_ne!(symbols[0]["display_name"], "zebra");
}

#[test]
fn emit_scip_emits_empty_index_for_function_less_file() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "no_fn.ori", NO_FN_FIXTURE);
    let output = dir.path().join("index.scip");

    let out = run_emit_scip(&fixture, &output);
    // A function-less file type-checks cleanly, so emit-scip succeeds.
    assert!(
        out.status.success(),
        "emit-scip exited non-zero on a function-less file: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let index =
        std::fs::read_to_string(&output).unwrap_or_else(|e| panic!("index.scip written: {e}"));
    let parsed: serde_json::Value =
        serde_json::from_str(&index).unwrap_or_else(|e| panic!("index.scip is valid JSON: {e}"));

    assert_eq!(parsed["format"], "ori-scip-minimal/v1");
    let symbols = parsed["documents"][0]["symbols"]
        .as_array()
        .unwrap_or_else(|| panic!("symbols array"));
    // The deliverable: a structurally-valid index with zero symbols, NOT a
    // missing file or a non-zero exit.
    assert_eq!(symbols.len(), 0, "a function-less file emits zero symbols");
}

#[test]
fn emit_scip_rejects_type_error_file() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "bad.ori", TYPE_ERROR_FIXTURE);
    let output = dir.path().join("index.scip");

    let out = run_emit_scip(&fixture, &output);
    assert!(
        !out.status.success(),
        "emit-scip must exit non-zero on a type-error file"
    );
    assert!(
        !output.exists(),
        "no index.scip is written when the frontend errors"
    );
}
