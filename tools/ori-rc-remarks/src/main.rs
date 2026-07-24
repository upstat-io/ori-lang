//! `ori-rc-remarks` — layer-2 analyzer CLI for the AIMS RC-survivor remark stream.
//!
//! Usage:
//!   `ori-rc-remarks <stream.jsonl>`             — one-line stream summary
//!   `ori-rc-remarks --stats <stream.jsonl>`     — opt-stats cause-cluster worklist
//!   `ori-rc-remarks --view <stream.jsonl>`      — opt-viewer source-annotated view
//!   `ori-rc-remarks --diff <base> <candidate>`  — opt-diff two-build comparison
//!
//! All modes build on the [`ori_rc_remarks::ingest`] foundation +
//! [`ori_rc_remarks::stats`] / [`ori_rc_remarks::view`] / [`ori_rc_remarks::diff`].

use std::process::ExitCode;

use ori_rc_remarks::diff::diff_streams;
use ori_rc_remarks::stats::rank_cause_clusters;
use ori_rc_remarks::view::source_annotated_view;
use ori_rc_remarks::{ingest, Stream};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, base, candidate] if flag == "--diff" => run_diff(base, candidate),
        [flag, path] if flag == "--stats" => run_single(path, Mode::Stats),
        [flag, path] if flag == "--view" => run_single(path, Mode::View),
        [path] => run_single(path, Mode::Summary),
        _ => {
            eprintln!("usage: ori-rc-remarks [--stats | --view] <stream.jsonl>");
            eprintln!("       ori-rc-remarks --diff <baseline.jsonl> <candidate.jsonl>");
            ExitCode::from(2)
        }
    }
}

/// Single-stream output mode.
#[derive(Clone, Copy)]
enum Mode {
    /// One-line stream summary (default).
    Summary,
    /// opt-stats cause-cluster worklist.
    Stats,
    /// opt-viewer source-annotated survivor view.
    View,
}

/// Load + ingest a stream file, mapping IO / parse failures to a CLI exit code.
fn load(path: &str) -> Result<Stream, ExitCode> {
    let contents = std::fs::read_to_string(path).map_err(|error| {
        eprintln!("ori-rc-remarks: cannot read {path}: {error}");
        ExitCode::FAILURE
    })?;
    ingest(&contents).map_err(|error| {
        eprintln!("ori-rc-remarks: {error}");
        ExitCode::FAILURE
    })
}

/// Single-stream modes: summary (default), `--stats` worklist, or `--view`.
fn run_single(path: &str, mode: Mode) -> ExitCode {
    let stream = match load(path) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    match mode {
        Mode::Summary => print_summary(path, &stream),
        Mode::Stats => print_stats(path, &stream),
        Mode::View => print_view(path, &stream),
    }
    ExitCode::SUCCESS
}

/// opt-viewer view: surviving RC ops grouped by source location (function-level
/// when the span is absent).
fn print_view(path: &str, stream: &Stream) {
    let groups = source_annotated_view(stream);
    let source = stream
        .header
        .as_ref()
        .map_or(path, |header| header.source_file.as_str());
    println!("{source}: {} location group(s)", groups.len());
    for group in &groups {
        println!("  {} ({} survivor(s))", group.location, group.survivors.len());
        for survivor in &group.survivors {
            let dim = survivor.lattice_dim.as_deref().unwrap_or("-");
            let pf = survivor.proof_failure.as_deref().unwrap_or("-");
            println!("    {} #{:?}  {} / {}", survivor.rc_op, survivor.ssa_value, dim, pf);
        }
    }
}

/// `--diff` mode: two-build comparison (regressions / improvements / persisted).
fn run_diff(baseline_path: &str, candidate_path: &str) -> ExitCode {
    let baseline = match load(baseline_path) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    let candidate = match load(candidate_path) {
        Ok(stream) => stream,
        Err(code) => return code,
    };
    let diff = diff_streams(&baseline, &candidate);
    println!(
        "diff {baseline_path} -> {candidate_path}: {} regression(s), {} improvement(s), {} persisted",
        diff.added.len(),
        diff.removed.len(),
        diff.persisted
    );
    for remark in &diff.added {
        println!("  + {} #{:?}", function_of(remark), remark.ssa_value);
    }
    for remark in &diff.removed {
        println!("  - {} #{:?}", function_of(remark), remark.ssa_value);
    }
    ExitCode::SUCCESS
}

/// Function attribution for a remark (`<unknown>` when absent).
fn function_of(remark: &ori_rc_remarks::Remark) -> &str {
    remark.function.as_deref().unwrap_or("<unknown>")
}

/// One-line stream summary (header facts + survivor count).
fn print_summary(path: &str, stream: &Stream) {
    match &stream.header {
        Some(header) => println!(
            "{}: schema v{}, burden_path={}, {} surviving RC ops (compiler {})",
            header.source_file,
            header.schema_version,
            header.burden_path,
            stream.remarks.len(),
            header.compiler_sha,
        ),
        None => println!(
            "{path}: no header (raw dev stream), {} surviving RC ops",
            stream.remarks.len()
        ),
    }
}

/// opt-stats worklist: cause clusters ranked by population, biggest first.
fn print_stats(path: &str, stream: &Stream) {
    let clusters = rank_cause_clusters(stream);
    let source = stream
        .header
        .as_ref()
        .map_or(path, |header| header.source_file.as_str());
    println!("{source}: {} cause cluster(s), ranked by survivors", clusters.len());
    for cluster in &clusters {
        println!(
            "  {:>5}  {} / {}",
            cluster.count, cluster.lattice_dim, cluster.proof_failure
        );
    }
}
