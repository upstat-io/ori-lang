//! CLI entry point for the `aims-proof-checker` binary.
//!
//! Per
//! `the design-validation gate`
//! Implementation Item 113 + 119, the CLI signature is:
//!
//! ```text
//! aims-proof-checker check <proof-file> [--engine <name>] [--json]
//! ```
//!
//! Future subcommands per the proof-checker design sec-Architecture-Sketch +
//! sec-Cross-references (each scaffolded by a later Agent dispatch):
//!
//! - `coverage-corpus` — drive `scripts/run-coverage-corpus.sh` parity
//! from inside the checker for local debugging.
//! - `smoke-test` — drive `scripts/run-smoke-test.sh` parity.
//!
//! Lean proofs are hand-authored at `aims-proof/lean/AimsProof/*.lean`
//! and cross-validated via the dual-discharge gate
//! (`aims-proof/scripts/dual-discharge.sh`); the checker emits no Lean
//! source.
//!
//! Exit codes (cross-reference §00 FAIL-branch enum + central routing
//! table at `scripts/plan_corpus/exit_reasons.py:229`):
//!
//! - 0 — Valid (`smoke_passes_in_ori_checker`) OR
//! UnimplementedEngineShape (`proof_checker_scope_inadequate`) at
//! scaffold-time. The §00 PASS-gate (e) accepts both verdicts; the
//! `status` field of the emitted JSON carries the actual verdict for
//! downstream consumers.
//! - 1 — generic CLI failure (bad args, missing file before parse).
//! - 2 — Fail (`checker_smoke_failed`) — engine genuinely rejected
//! the obligation.
//! - 3 — ParseFailure / infrastructure (`checker_build_failed`).
//!
//! On `--json`, emits structured diagnostic to stdout per the
//! Implementation Item 122 JSON contract:
//! `{status: "fail"|"valid"|"unimplemented_engine_shape",
//! failing_engine: <name>, reason: <text>}` (failing_engine + reason
//! populated only on fail / unimplemented; failing_corpus_entry added
//! when failure surfaces inside coverage-corpus driver).

use crate::checker::{check_proof_file, CheckResult};
use std::path::PathBuf;

/// CLI subcommand surface — closed enum per the proof-checker design sec-Cross-references.
#[derive(Debug, Clone)]
pub enum Subcommand {
    /// `check <proof-file> [--engine <name>] [--json]`.
    Check {
        /// Path to the canonical-notation proof file.
        path: PathBuf,
        /// Optional engine override (forces dispatch through this engine
        /// even if its `accepts` predicate returns `false`). Used by
        /// engine unit tests at `tests/smoke.rs`.
        engine: Option<String>,
        /// Emit structured JSON to stdout vs human-readable diagnostic.
        json: bool,
    },
    /// `coverage-corpus` — placeholder; full impl in run-coverage-corpus.sh.
    CoverageCorpus,
    /// `smoke-test` — placeholder; full impl in run-smoke-test.sh.
    SmokeTest,
}

/// Parsed CLI invocation — either a resolved subcommand or a structured
/// failure carrying the diagnostic to print before exit.
#[derive(Debug, Clone)]
enum ParsedArgs {
    Cmd(Subcommand),
    Error(String),
}

/// CLI entry point. Parses `args`, dispatches the requested subcommand,
/// returns an OS exit code per §00 FAIL-branch enum.
///
/// `args[0]` is the binary name (per stdlib `std::env::args`
/// convention); `args[1..]` are the user-supplied arguments.
pub fn run(args: Vec<String>) -> i32 {
    match parse_args(&args) {
        ParsedArgs::Cmd(Subcommand::Check { path, engine: _, json }) => {
            let result = check_proof_file(&path);
            if json {
                println!("{}", format_json(&result));
            } else {
                println!("{}", format_human(&result));
            }
            result.exit_code()
        }
        ParsedArgs::Cmd(Subcommand::CoverageCorpus) => {
            eprintln!(
                "coverage-corpus subcommand not yet implemented; \
                 use scripts/run-coverage-corpus.sh"
            );
            1
        }
        ParsedArgs::Cmd(Subcommand::SmokeTest) => {
            eprintln!(
                "smoke-test subcommand not yet implemented; \
                 use scripts/run-smoke-test.sh"
            );
            1
        }
        ParsedArgs::Error(msg) => {
            eprintln!("aims-proof-checker: {}", msg);
            eprintln!("usage: aims-proof-checker check <proof-file> [--engine <name>] [--json]");
            1
        }
    }
}

/// Parse `args` (incl. `args[0]` binary name) into a `Subcommand` or a
/// structured error message.
fn parse_args(args: &[String]) -> ParsedArgs {
    if args.len() < 2 {
        return ParsedArgs::Error("missing subcommand".to_string());
    }
    match args[1].as_str() {
        "check" => parse_check(&args[2..]),
        "coverage-corpus" => ParsedArgs::Cmd(Subcommand::CoverageCorpus),
        "smoke-test" => ParsedArgs::Cmd(Subcommand::SmokeTest),
        other => ParsedArgs::Error(format!("unknown subcommand {:?}", other)),
    }
}

/// Parse the `check` subcommand body: positional `<proof-file>` plus
/// optional `--engine <name>` and `--json` flags.
fn parse_check(rest: &[String]) -> ParsedArgs {
    let mut path: Option<PathBuf> = None;
    let mut engine: Option<String> = None;
    let mut json = false;
    let mut i = 0;
    while i < rest.len() {
        let a = &rest[i];
        match a.as_str() {
            "--json" => {
                json = true;
                i += 1;
            }
            "--engine" => {
                if i + 1 >= rest.len() {
                    return ParsedArgs::Error("--engine requires a value".to_string());
                }
                engine = Some(rest[i + 1].clone());
                i += 2;
            }
            other if other.starts_with("--") => {
                return ParsedArgs::Error(format!("unknown flag {:?}", other));
            }
            other => {
                if path.is_some() {
                    return ParsedArgs::Error(format!(
                        "unexpected positional argument {:?}",
                        other
                    ));
                }
                path = Some(PathBuf::from(other));
                i += 1;
            }
        }
    }
    match path {
        Some(p) => ParsedArgs::Cmd(Subcommand::Check { path: p, engine, json }),
        None => ParsedArgs::Error("check requires <proof-file>".to_string()),
    }
}

/// Format a `CheckResult` as the Implementation Item 122 JSON object.
pub fn format_json(result: &CheckResult) -> String {
    result.to_json()
}

/// Human-readable rendering of `CheckResult` for non-JSON mode.
fn format_human(result: &CheckResult) -> String {
    match result {
        CheckResult::Valid => "valid: every theorem discharged constructively".to_string(),
        CheckResult::Fail {
            failing_engine,
            failing_theorem,
            reason,
        } => format!(
            "fail: theorem {} rejected by engine {}: {}",
            failing_theorem, failing_engine, reason
        ),
        CheckResult::UnimplementedEngineShape {
            engine,
            theorem,
            reason,
        } => format!(
            "unimplemented_engine_shape: theorem {} not dischargeable by engine {}: {}",
            theorem, engine, reason
        ),
        CheckResult::ParseFailure(err) => format!("parse_failure: {}", err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p
    }

    #[test]
    fn cli_check_smoke_json_matches_expected() {
        let proof = workspace_root().join("proofs/00-smoke-test/cn-1-bidirectional.proof");
        let expected = workspace_root().join("proofs/00-smoke-test/cn-1-bidirectional.expected");
        let result = check_proof_file(&proof);
        let json = format_json(&result);
        let expected_text =
            std::fs::read_to_string(&expected).expect("read expected smoke file");
        assert_eq!(json.trim(), expected_text.trim());
    }

    #[test]
    fn parse_args_check_with_json_flag() {
        let args = vec![
            "aims-proof-checker".to_string(),
            "check".to_string(),
            "some.proof".to_string(),
            "--json".to_string(),
        ];
        match parse_args(&args) {
            ParsedArgs::Cmd(Subcommand::Check { path, json, .. }) => {
                assert_eq!(path, PathBuf::from("some.proof"));
                assert!(json);
            }
            other => panic!("expected Check; got {:?}", other),
        }
    }

    #[test]
    fn parse_args_missing_subcommand_errors() {
        let args = vec!["aims-proof-checker".to_string()];
        assert!(matches!(parse_args(&args), ParsedArgs::Error(_)));
    }

    #[test]
    fn parse_args_unknown_subcommand_errors() {
        let args = vec!["aims-proof-checker".to_string(), "bogus".to_string()];
        assert!(matches!(parse_args(&args), ParsedArgs::Error(_)));
    }

    #[test]
    fn run_check_smoke_returns_exit_code_0() {
        let proof =
            workspace_root().join("proofs/00-smoke-test/cn-1-bidirectional.proof");
        let args = vec![
            "aims-proof-checker".to_string(),
            "check".to_string(),
            proof.to_string_lossy().to_string(),
            "--json".to_string(),
        ];
        let code = run(args);
        // `UnimplementedEngineShape` -> exit 0 at scaffold-time per §00
        // PASS-gate (e) — scripts/run-smoke-test.sh + run-coverage-corpus.sh
        // accept the verdict and diff the JSON against .expected.
        assert_eq!(code, 0);
    }
}
