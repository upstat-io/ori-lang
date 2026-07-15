//! L12 production-path pins for the `ORI_TRACE_IDX` provenance-DAG knob.
//!
//! Spawns the REAL `ori` binary against a `Wrap<Wrap<int>>` fixture and drives
//! the wired `ORI_TRACE_IDX` path end-to-end. The walking-skeleton contract:
//! setting `ORI_TRACE_IDX` to the outer body `Idx` emits a NON-EMPTY STRUCTURE
//! DAG to stderr; leaving it unset emits nothing.
//!
//! The target `Idx` is discovered dynamically from the `ORI_DUMP_TYPE_IDX` dump
//! (not hard-coded) so the pin is robust to pool renumbering as the compiler
//! evolves.

use std::path::Path;
use std::process::Command;

const WRAP_FIXTURE: &str = include_str!("fixtures/provenance/wrap_nested.ori");

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

/// Run `ori check <fixture>` with the given extra env, returning combined
/// stdout+stderr (the DAG is written to stderr).
fn run_check(fixture: &Path, env: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("check").arg(fixture);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to run ori: {e}"));
    // Assert clean exit so the no-panic contract is observable: a panic/abort
    // (e.g. an out-of-range ORI_TRACE_IDX) sets a non-success status that a
    // stdout/stderr-only assertion would pass straight through.
    assert!(
        output.status.success(),
        "ori check must exit cleanly (no panic/abort); status={:?}\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined
}

/// Parse the outer body `Idx` from a `Wrap<Wrap<int>>#NNN` annotation in the
/// `ORI_DUMP_TYPE_IDX` dump.
fn outer_body_idx(dump: &str) -> u32 {
    let marker = "Wrap<Wrap<int>>#";
    let start = dump
        .find(marker)
        .unwrap_or_else(|| panic!("dump missing `{marker}`:\n{dump}"))
        + marker.len();
    let digits: String = dump[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse::<u32>()
        .unwrap_or_else(|e| panic!("could not parse outer body idx from `{digits}`: {e}"))
}

#[test]
fn ori_trace_idx_emits_nonempty_structure_dag_on_wrap_nested() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);

    // Discover the outer body `Idx` dynamically (robust to pool renumbering).
    let dump = run_check(
        &fixture,
        &[("ORI_DUMP_AFTER_TYPECK", "1"), ("ORI_DUMP_TYPE_IDX", "1")],
    );
    let idx = outer_body_idx(&dump);

    // Drive the wired ORI_TRACE_IDX path on the discovered root.
    let traced = run_check(&fixture, &[("ORI_TRACE_IDX", &idx.to_string())]);

    assert!(
        traced.contains("Provenance DAG"),
        "ORI_TRACE_IDX must emit a provenance DAG section:\n{traced}"
    );
    // Non-empty: at least one structure edge with an edge arrow.
    assert!(
        traced.contains("-->"),
        "DAG rooted at the outer body Idx must carry >=1 structure edge:\n{traced}"
    );
    assert!(
        !traced.contains("0 structure edge(s)"),
        "DAG rooted at the outer body Idx must NOT be empty:\n{traced}"
    );
    // The production trace emits the FULL edge set (structure + resolution +
    // mono + divergence) AND the diagnostic's core deliverable FIRES: the
    // `Wrap<Wrap<int>>` body carries the `T#141` generic leaf under a concrete
    // instantiation, so the divergence detector flags it (non-zero) — proving
    // the edge SSOT is exercised end-to-end, not header-only.
    assert!(
        traced.contains("~resolves~>"),
        "the trace must carry a real resolution edge (Named -> concrete):\n{traced}"
    );
    assert!(
        traced.contains("divergence(s)") && !traced.contains("0 divergence(s)"),
        "the diagnostic must SURFACE the generic-leaf divergence on Wrap<Wrap<int>> \
         (non-zero), not merely print the section header:\n{traced}"
    );
}

/// Parse the generic-leaf `Idx` from the divergence line
/// (`  generic T#141 <> concrete ...`). The leaf's raw index is the first
/// `#<digits>` in the segment before ` <> `.
fn divergence_generic_idx(trace: &str) -> u32 {
    let line = trace
        .lines()
        .find(|l| l.trim_start().starts_with("generic "))
        .unwrap_or_else(|| panic!("trace missing a divergence line:\n{trace}"));
    let seg = line.split(" <> ").next().unwrap_or(line);
    let hash = seg
        .find('#')
        .unwrap_or_else(|| panic!("divergence generic has no #idx: {line}"))
        + 1;
    let digits: String = seg[hash..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse::<u32>()
        .unwrap_or_else(|e| panic!("could not parse generic leaf idx from `{digits}`: {e}"))
}

#[test]
fn ori_trace_idx_attributes_drop_glue_to_generic_leaf_chain_on_wrap_nested() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);

    // Discover the outer body `Idx` dynamically (robust to pool renumbering).
    let dump = run_check(
        &fixture,
        &[("ORI_DUMP_AFTER_TYPECK", "1"), ("ORI_DUMP_TYPE_IDX", "1")],
    );
    let root = outer_body_idx(&dump);

    let traced = run_check(&fixture, &[("ORI_TRACE_IDX", &root.to_string())]);

    // The CONSUMER-edge section fires and is non-empty.
    assert!(
        traced.contains("consumer edge(s)"),
        "ORI_TRACE_IDX must emit a consumer-edge section:\n{traced}"
    );
    assert!(
        !traced.contains("0 consumer edge(s)"),
        "the Wrap<Wrap<int>> root must carry >=1 consumer edge:\n{traced}"
    );

    // North-star: the generic leaf's logical drop plan is attributed through
    // its provenance chain to the traced outer body.
    let leaf = divergence_generic_idx(&traced);
    let edge_line = traced
        .lines()
        .find(|line| {
            line.contains("drop-plan")
                && line.contains(&format!("#{leaf}"))
                && line.contains("<=consumes=")
        })
        .unwrap_or_else(|| {
            panic!("no consumer edge attributes the generic-leaf drop plan #{leaf}:\n{traced}")
        });
    // The leaf plan descends from a parent Wrap body, not a bare leaf.
    assert!(
        edge_line.contains(" -> "),
        "the generic-leaf drop plan must have a multi-hop walked chain:\n{edge_line}"
    );
    // The chain reaches the generic leaf and roots at the traced outer body.
    assert!(
        edge_line.contains(&format!("#{leaf}")),
        "the walked chain must reach the generic leaf #{leaf}:\n{edge_line}"
    );
    assert!(
        edge_line.contains(&format!("#{root}")),
        "the walked chain must root at the traced outer body #{root}:\n{edge_line}"
    );
}

#[test]
fn unset_ori_trace_idx_emits_no_provenance_dag() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);

    let out = run_check(&fixture, &[]);
    assert!(
        !out.contains("Provenance DAG"),
        "no provenance DAG without ORI_TRACE_IDX:\n{out}"
    );
}

#[test]
fn empty_ori_trace_idx_emits_nothing() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);

    // An empty value is the off state — silent, never a parse-cause diagnostic.
    let out = run_check(&fixture, &[("ORI_TRACE_IDX", "")]);
    assert!(
        !out.contains("Provenance DAG"),
        "an empty ORI_TRACE_IDX is the off state — no DAG:\n{out}"
    );
    assert!(
        !out.contains("not a valid type-pool index"),
        "an empty ORI_TRACE_IDX must stay silent, not name a parse cause:\n{out}"
    );
}

#[test]
fn non_numeric_ori_trace_idx_names_cause_no_dag() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);

    let out = run_check(&fixture, &[("ORI_TRACE_IDX", "not-a-number")]);
    assert!(
        !out.contains("Provenance DAG"),
        "a non-numeric ORI_TRACE_IDX must emit no DAG (no panic):\n{out}"
    );
    // The parse failure is surfaced, not swallowed: name the cause + the fix.
    assert!(
        out.contains("not a valid type-pool index") && out.contains("no trace emitted"),
        "non-numeric ORI_TRACE_IDX must name the cause ('not a valid type-pool index') \
         and that no trace was emitted:\n{out}"
    );
    // The discover-indices hint must name BOTH flags: ORI_DUMP_TYPE_IDX is the
    // typeck --with-idx VIEW and is a no-op without ORI_DUMP_AFTER_TYPECK=1, so
    // the hint must give the actionable pair.
    assert!(
        out.contains("ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_TYPE_IDX=1"),
        "the discover-indices hint must name BOTH flags (the typeck view is gated \
         on ORI_DUMP_AFTER_TYPECK), not ORI_DUMP_TYPE_IDX alone:\n{out}"
    );
}

#[test]
fn out_of_range_ori_trace_idx_emits_no_dag() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);

    let out = run_check(&fixture, &[("ORI_TRACE_IDX", "4000000000")]);
    assert!(
        !out.contains("Provenance DAG"),
        "an out-of-range ORI_TRACE_IDX must emit no DAG (no panic):\n{out}"
    );
    // Pin the friendly diagnostic content (cause + guidance): an out-of-range
    // index names the cause and that nothing was emitted, rather than silently
    // dropping or panicking.
    assert!(
        out.contains("out of range"),
        "out-of-range ORI_TRACE_IDX must name the cause ('out of range'):\n{out}"
    );
    assert!(
        out.contains("valid indices") && out.contains("no trace emitted"),
        "out-of-range diagnostic must state the valid-index range and that no trace was emitted:\n{out}"
    );
}
