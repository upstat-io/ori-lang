//! L12 production-path pin for the `ori explain idx <n> <file>` provenance verb.
//!
//! Drives the REAL `ori` binary through the new CLI verb — the same tracer entry
//! the `ORI_TRACE_IDX` env knob funnels into. The nested-generic convergence
//! pin asserts that struct `Wrap<Wrap<int>>` and enum `Holder<Holder<int>>`
//! retain their structure and resolution provenance after type checking while
//! carrying no stale generic leaf or scalar drop plan.

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
    /// `(exit_success, combined stderr)`. Negative cases assert a
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

    /// The traced root carries concrete structure and resolution provenance,
    /// with no stale generic leaf or scalar-only drop plan.
    fn assert_converged_dag(trace: &str) {
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
        assert!(
            trace.contains("0 divergence(s)"),
            "a fully materialized nested generic must have no stale generic leaf:\n{trace}"
        );
        assert!(
            !trace.contains(" <> concrete "),
            "a fully materialized nested generic must have no divergence line:\n{trace}"
        );

        // Consumer attribution is available with the ARC-enabled feature.
        // These fixtures contain only scalar leaves, so no structural drop plan
        // exists at any level.
        #[cfg(feature = "llvm")]
        assert!(
            trace.contains("0 consumer edge(s)"),
            "scalar-only nested generics must not invent a drop-plan consumer:\n{trace}"
        );
    }

    /// Struct and enum nested-generic fixtures converge to concrete provenance
    /// chains, matching the monomorphic control.
    #[test]
    fn nested_generic_instantiations_converge() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));

        // Positive leg 1: struct nesting (Wrap<Wrap<int>>).
        let wrap = write_fixture(&dir, "wrap_nested.ori", WRAP_FIXTURE);
        let wrap_root = discover_body_idx(&wrap, "Wrap<Wrap<int>>#");
        assert_converged_dag(&run_explain_idx(wrap_root, &wrap));

        // Positive leg 2: enum-in-enum nesting (Holder<Holder<int>>).
        let holder = write_fixture(&dir, "holder_nested.ori", HOLDER_FIXTURE);
        let holder_root = discover_body_idx(&holder, "Holder<Holder<int>>#");
        assert_converged_dag(&run_explain_idx(holder_root, &holder));

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
