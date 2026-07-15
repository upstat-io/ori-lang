//! L12 production-path pin for the `ori explain idx <n> <file>` provenance verb.
//!
//! Drives the REAL `ori` binary through the new CLI verb — the same tracer entry
//! the `ORI_TRACE_IDX` env knob funnels into. The north-star
//! `nested_generic_divergence` pin asserts the diagnostic DISCRIMINATES a true
//! nested-generic divergence (struct `Wrap<Wrap<int>>` AND enum
//! `Holder<Holder<int>>`) from a clean monomorphic type (`Pair`) that yields a
//! consistent concrete chain with NO divergent node — proving it does not flag
//! everything.

use std::path::Path;
use std::process::Command;

const WRAP_FIXTURE: &str = include_str!("fixtures/provenance/wrap_nested.ori");
const HOLDER_FIXTURE: &str = include_str!("fixtures/provenance/holder_nested.ori");
const MONO_FIXTURE: &str = include_str!("fixtures/provenance/mono_clean.ori");

mod idx_provenance {
    use super::*;

    fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
        path
    }

    /// Run `ori check <fixture>` with the given extra env; combined stdout+stderr.
    fn run_check(fixture: &Path, env: &[(&str, &str)]) -> String {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
        cmd.arg("check").arg(fixture);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("failed to run ori check: {e}"));
        let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        combined
    }

    /// Run the real `ori explain idx <idx> <fixture>` verb; returns stdout (where
    /// the DAG is printed). Asserts a clean exit so a panic/abort is observable.
    fn run_explain_idx(idx: u32, fixture: &Path) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_ori"))
            .arg("explain")
            .arg("idx")
            .arg(idx.to_string())
            .arg(fixture)
            .output()
            .unwrap_or_else(|e| panic!("failed to run ori explain idx: {e}"));
        assert!(
            output.status.success(),
            "`ori explain idx` must exit cleanly; status={:?}\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Run `ori explain idx <args...>` for an EXPECTED-failure path; returns
    /// `(exit_success, combined stderr)`. The negative pins below assert a
    /// non-zero exit AND a named-cause diagnostic — vs [`run_explain_idx`], which
    /// asserts a clean exit and returns stdout.
    fn run_explain_idx_args(args: &[&str]) -> (bool, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_ori"))
            .arg("explain")
            .arg("idx")
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run ori explain idx: {e}"));
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// Discover the body `Idx` for a type annotation `<marker>#NNN` from the
    /// `ORI_DUMP_TYPE_IDX` dump (robust to pool renumbering as the compiler evolves).
    fn discover_body_idx(fixture: &Path, marker: &str) -> u32 {
        let dump = run_check(
            fixture,
            &[("ORI_DUMP_AFTER_TYPECK", "1"), ("ORI_DUMP_TYPE_IDX", "1")],
        );
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
            .unwrap_or_else(|e| panic!("could not parse idx after `{marker}` from `{digits}`: {e}"))
    }

    /// Parse the generic-leaf raw `Idx` from the divergence line
    /// (`  generic T#NNN <> concrete ...`).
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

    /// Positive leg: the traced root surfaces ONE DAG carrying the generic-vs-concrete
    /// divergence and the generic leaf's logical drop-plan attribution.
    fn assert_divergent_dag(trace: &str, root: u32) {
        assert!(
            trace.contains("Provenance DAG"),
            "verb must emit a provenance DAG:\n{trace}"
        );
        assert!(
            trace.contains("-->"),
            "DAG must carry >=1 structure edge:\n{trace}"
        );
        assert!(
            trace.contains("~resolves~>"),
            "DAG must carry a real resolution edge (Named -> concrete):\n{trace}"
        );
        // The generic-leaf divergence FIRES (non-zero) — the diagnostic's core deliverable.
        assert!(
            trace.contains("divergence(s)") && !trace.contains("0 divergence(s)"),
            "DAG must SURFACE the generic-leaf divergence (non-zero):\n{trace}"
        );
        assert!(
            trace.contains(" <> concrete "),
            "DAG must carry a `generic ... <> concrete ...` divergence line:\n{trace}"
        );

        // Consumer attribution is currently built with the ARC-enabled feature.
        #[cfg(feature = "llvm")]
        {
            assert!(
                trace.contains("consumer edge(s)") && !trace.contains("0 consumer edge(s)"),
                "DAG must carry at least one drop-plan consumer edge:\n{trace}"
            );
            let leaf = divergence_generic_idx(trace);
            let edge_line = trace
                .lines()
                .find(|line| {
                    line.contains("drop-plan")
                        && line.contains(&format!("#{leaf}"))
                        && line.contains("<=consumes=")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "no consumer edge attributes the generic-leaf drop plan #{leaf}:\n{trace}"
                    )
                });
            // The leaf plan descends from a parent body rooted at the trace.
            assert!(
                edge_line.contains(" -> "),
                "the generic-leaf drop plan must have a multi-hop walked chain:\n{edge_line}"
            );
            assert!(
                edge_line.contains(&format!("#{leaf}")),
                "the walked chain must reach the generic leaf #{leaf}:\n{edge_line}"
            );
            assert!(
                edge_line.contains(&format!("#{root}")),
                "the walked chain must root at the traced outer body #{root}:\n{edge_line}"
            );
        }
    }

    /// North-star pin (= plan-wide functional criterion probe). Positive legs:
    /// the struct `Wrap<Wrap<int>>` AND enum `Holder<Holder<int>>` nested-generic
    /// fixtures each surface the generic-vs-concrete divergence (+ llvm-gated
    /// drop-glue consumer attribution) as ONE legible DAG. Negative control: a
    /// clean monomorphic `Pair` yields a single consistent concrete chain with NO
    /// divergent node — proving the diagnostic DISCRIMINATES true divergence from
    /// clean monomorphization rather than flagging everything.
    #[test]
    fn nested_generic_divergence() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));

        // Positive leg 1: struct nesting (Wrap<Wrap<int>>).
        let wrap = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);
        let wrap_root = discover_body_idx(&wrap, "Wrap<Wrap<int>>#");
        assert_divergent_dag(&run_explain_idx(wrap_root, &wrap), wrap_root);

        // Positive leg 2: enum-in-enum nesting (Holder<Holder<int>>).
        let holder = write_fixture(&dir, "holder_nested.ori", HOLDER_FIXTURE);
        let holder_root = discover_body_idx(&holder, "Holder<Holder<int>>#");
        assert_divergent_dag(&run_explain_idx(holder_root, &holder), holder_root);

        // Negative control: clean monomorphic struct (Pair) — concrete chain, NO divergence.
        let mono = write_fixture(&dir, "mono_clean.ori", MONO_FIXTURE);
        let mono_root = discover_body_idx(&mono, "Pair#");
        let mono_trace = run_explain_idx(mono_root, &mono);
        assert!(
            mono_trace.contains("Provenance DAG"),
            "negative control must still emit a provenance DAG:\n{mono_trace}"
        );
        assert!(
            mono_trace.contains("-->"),
            "negative control must carry a concrete structure chain:\n{mono_trace}"
        );
        assert!(
            mono_trace.contains("0 divergence(s)"),
            "negative control (clean monomorphic type) must have ZERO divergences:\n{mono_trace}"
        );
        assert!(
            !mono_trace.contains(" <> concrete "),
            "negative control must carry NO generic-leaf-divergent node:\n{mono_trace}"
        );
    }

    /// `ori explain idx --help` documents the verb and exits 0 (feature-independent).
    #[test]
    fn help_exits_zero() {
        let output = Command::new(env!("CARGO_BIN_EXE_ori"))
            .arg("explain")
            .arg("idx")
            .arg("--help")
            .output()
            .unwrap_or_else(|e| panic!("failed to run ori explain idx --help: {e}"));
        assert!(
            output.status.success(),
            "`ori explain idx --help` must exit 0; status={:?}",
            output.status,
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("ori explain idx"),
            "--help must document the verb:\n{stdout}"
        );
    }

    /// Negative pin: a non-numeric index exits non-zero and names the cause +
    /// the discover-indices fix (the `ori explain idx` verb's twin of the
    /// `ORI_TRACE_IDX` non-numeric pin in `provenance_trace.rs`).
    #[test]
    fn non_numeric_index_names_cause() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let fixture = write_fixture(&dir, "mono_clean.ori", MONO_FIXTURE);
        let path = fixture.to_string_lossy().into_owned();

        let (success, stderr) = run_explain_idx_args(&["not-a-number", &path]);
        assert!(
            !success,
            "a non-numeric index must exit non-zero:\n{stderr}"
        );
        assert!(
            stderr.contains("is not a valid type-pool index"),
            "non-numeric index must name the cause:\n{stderr}"
        );
        assert!(
            stderr.contains("ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_TYPE_IDX=1"),
            "the discover-indices hint must name BOTH flags:\n{stderr}"
        );
    }

    /// Negative pin: an out-of-range index exits non-zero and names the cause +
    /// the valid range, never silently dropping or panicking.
    #[test]
    fn out_of_range_index_names_cause() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let fixture = write_fixture(&dir, "mono_clean.ori", MONO_FIXTURE);
        let path = fixture.to_string_lossy().into_owned();

        let (success, stderr) = run_explain_idx_args(&["4000000000", &path]);
        assert!(
            !success,
            "an out-of-range index must exit non-zero:\n{stderr}"
        );
        assert!(
            stderr.contains("out of range") && stderr.contains("valid indices"),
            "out-of-range index must name the cause and the valid range:\n{stderr}"
        );
    }

    /// Negative pin: missing positionals exit non-zero and show the usage text.
    #[test]
    fn missing_args_shows_usage() {
        let (success, stderr) = run_explain_idx_args(&[]);
        assert!(!success, "missing args must exit non-zero:\n{stderr}");
        assert!(
            stderr.contains("expects <index> <file.ori>"),
            "missing args must name the expected positionals:\n{stderr}"
        );
        assert!(
            stderr.contains("Usage: ori explain idx"),
            "missing args must show the usage text:\n{stderr}"
        );
    }
}
