//! End-to-end non-empty-stream assertion for the RC-remarks walking skeleton.
//!
//! Compiles a fixture that produces >=1 surviving (non-elidable) RC op on the
//! burden-sole path (`ORI_DISABLE_PREDICATE_STACK_RC=1`) with `ORI_RC_REMARKS`
//! set, then asserts the JSONL remark stream is non-empty and well-formed —
//! proving the emit -> file -> read-back pipe end to end
//! (Spec: Annex E §AIMS — observability changes no RC-emission semantics).

use std::fs;
use std::process::Command;

use tempfile::TempDir;

use crate::util::{compile_and_run_with_build_env, ori_binary, stdlib_path};

#[test]
fn rc_remarks_survivor_fixture_emits_nonempty_stream() {
    let temp = TempDir::new().expect("create temp dir for rc-remarks stream");
    let remarks_path = temp.path().join("rc-remarks.jsonl");
    let remarks_path_str = remarks_path.to_str().expect("utf-8 temp path");

    // Compile the survivor fixture on the burden-sole path with remark emission
    // enabled. The run step is incidental; the remark emits during codegen.
    let _ = compile_and_run_with_build_env(
        include_str!("fixtures/rc_remarks/survivor.ori"),
        &[
            ("ORI_RC_REMARKS", remarks_path_str),
            ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
        ],
    );

    let stream = fs::read_to_string(&remarks_path).unwrap_or_default();
    let remark_lines: Vec<&str> = stream.lines().filter(|l| !l.trim().is_empty()).collect();

    // Positive pin: the survivor fixture produces a non-empty remark stream.
    assert!(
        !remark_lines.is_empty(),
        "expected >=1 surviving-RC-op remark on the burden-sole path; stream was empty.\n\
         stream path: {remarks_path_str}\nstream:\n{stream}"
    );
    // Negative pin: every emitted line is a well-formed `missed` remark object.
    for line in &remark_lines {
        assert!(
            line.contains("\"kind\":\"missed\""),
            "remark line is not a well-formed missed-remark object: {line}"
        );
    }
}

/// Production-path (L12): the real `ori build --emit-rc-remarks <path>` CLI flag
/// (not the `ORI_RC_REMARKS` env seam) writes the remark stream to the named
/// file on a real build of a real target. The flag ALONE — with NO burden-sole
/// gating env set — must auto-compose the gating so the stream is a valid
/// verdict surface: the header's `burden_path` is `true`. Pins both the CLI
/// entry point (subcommand) and the gating composition wired by cli-producer,
/// distinct from the env-driven dev surface above.
#[test]
fn rc_remarks_cli_flag_emits_stream_to_file() {
    let temp = TempDir::new().expect("create temp dir for rc-remarks CLI build");
    let source_path = temp.path().join("survivor.ori");
    let remarks_path = temp.path().join("rc-remarks-cli.jsonl");
    let binary_path = temp
        .path()
        .join(format!("survivor{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source_path,
        include_str!("fixtures/rc_remarks/survivor.ori"),
    )
    .expect("write survivor fixture");

    // Drive the real production CLI: `ori build <file> --emit-rc-remarks <path>`.
    // No gating env is set — the flag must auto-compose the burden-sole path.
    let build = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().expect("utf-8 source path"),
            "--emit-rc-remarks",
            remarks_path.to_str().expect("utf-8 remarks path"),
            "-o",
            binary_path.to_str().expect("utf-8 binary path"),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("execute ori build --emit-rc-remarks");

    // The header + remarks emit during codegen (pre-link); the stream must exist.
    let stream = fs::read_to_string(&remarks_path).unwrap_or_default();
    let lines: Vec<&str> = stream.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        !lines.is_empty(),
        "the --emit-rc-remarks CLI flag produced no stream.\n\
         build stderr:\n{}\nstream path: {}\nstream:\n{stream}",
        String::from_utf8_lossy(&build.stderr),
        remarks_path.display()
    );

    // First line is the schema header; its burden_path proves the gating was
    // auto-composed (no ORI_DISABLE_PREDICATE_STACK_RC was set by the test).
    let header = lines[0];
    assert!(
        header.contains("\"record\":\"header\""),
        "first stream line is not the schema header: {header}"
    );
    assert!(
        header.contains("\"burden_path\":true"),
        "--emit-rc-remarks did not auto-compose the burden-sole gating \
         (header burden_path must be true for a valid verdict surface): {header}"
    );

    // Every subsequent line is a well-formed missed remark.
    let remark_lines = &lines[1..];
    assert!(
        !remark_lines.is_empty(),
        "expected >=1 surviving-RC-op remark after the header.\nstream:\n{stream}"
    );
    for line in remark_lines {
        assert!(
            line.contains("\"kind\":\"missed\""),
            "CLI-flag remark line is not a well-formed missed-remark object: {line}"
        );
    }
}

/// Recursively collect every `.ori` fixture under `dir` into `out`.
fn collect_ori_fixtures(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_ori_fixtures(&path, out);
        } else if path.extension().is_some_and(|x| x == "ori") {
            out.push(path);
        }
    }
}

/// The producer runs against the whole AIMS fixture corpus (`tests/aims/**`),
/// not just the single survivor fixture: every scenario produces a valid
/// verdict-surface stream (auto-composed burden-sole header). Enumerates the
/// corpus by recursive discovery + a self-verifying visited-count assertion so
/// a silently-skipped fixture fails the test (per tests.md matrix-completeness).
#[test]
fn rc_remarks_producer_runs_over_aims_corpus() {
    let corpus_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/aims")
        .canonicalize()
        .expect("canonicalize tests/aims corpus dir");

    let mut fixtures = Vec::new();
    collect_ori_fixtures(&corpus_dir, &mut fixtures);
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "AIMS fixture corpus is empty at {corpus_dir:?}"
    );

    let temp = TempDir::new().expect("create temp dir for corpus producer run");
    let mut visited = 0usize;
    for (i, fixture) in fixtures.iter().enumerate() {
        let remarks_path = temp.path().join(format!("rc-{i}.jsonl"));
        let binary_path = temp
            .path()
            .join(format!("bin-{i}{}", std::env::consts::EXE_SUFFIX));
        // Producer over each corpus fixture; flag alone auto-composes the gating.
        let build = Command::new(ori_binary())
            .args([
                "build",
                fixture.to_str().expect("utf-8 fixture path"),
                "--emit-rc-remarks",
                remarks_path.to_str().expect("utf-8 remarks path"),
                "-o",
                binary_path.to_str().expect("utf-8 binary path"),
            ])
            .env("ORI_STDLIB", stdlib_path())
            .output()
            .expect("execute ori build --emit-rc-remarks over corpus fixture");

        // Header emits during codegen (pre-link); a missing stream means the
        // producer did not run against this corpus fixture — a coverage gap.
        let stream = fs::read_to_string(&remarks_path).unwrap_or_default();
        let first = stream.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        assert!(
            first.contains("\"record\":\"header\"") && first.contains("\"burden_path\":true"),
            "producer did not emit a valid verdict-surface header for corpus fixture {}.\n\
             build stderr:\n{}\nheader line: {first:?}",
            fixture.display(),
            String::from_utf8_lossy(&build.stderr)
        );
        visited += 1;
    }

    // Self-verifying completeness: every discovered fixture was exercised.
    assert_eq!(
        visited,
        fixtures.len(),
        "producer-corpus enumeration skipped fixtures: visited {visited} of {}",
        fixtures.len()
    );
}
