//! The `explain idx` command: emit a read-only provenance DAG for one type-pool `Idx`.
//!
//! Funnels into the same tracer entry as the `ORI_TRACE_IDX` env knob
//! ([`super::provenance::trace_idx_provenance`]); the verb supplies the index + file and
//! prints the rendered DAG to stdout, never re-implementing the walk or render.

use std::path::PathBuf;

use ori_diagnostic::emitter::{ColorMode, TerminalEmitter};
use oric::{CompilerDb, Db, SourceFile};

use crate::debug_flags::{ORI_DUMP_AFTER_TYPECK, ORI_DUMP_TYPE_IDX, ORI_TRACE_IDX};

use super::provenance::{idx_out_of_range_detail, trace_idx_provenance, IdxTrace};
use super::{read_file, report_frontend_errors};

/// Usage text for `ori explain idx`, shown on `--help` and on a usage error.
/// Flag names flow from the `debug_flags` constants (SSOT) so a flag rename never
/// leaves the help text pointing at a dead env var.
fn usage() -> String {
    format!(
        "Usage: ori explain idx <index> <file.ori>\n\
         \n\
         Emit a read-only provenance DAG for one type-pool index: STRUCTURE + RESOLUTION +\n\
         MONO edges, generic-leaf DIVERGENCE verdicts, and drop-glue CONSUMER attribution.\n\
         \n\
         Discover indices with:\n\
         \x20\x20{ORI_DUMP_AFTER_TYPECK}=1 {ORI_DUMP_TYPE_IDX}=1 ori check <file.ori>\n\
         \n\
         The same trace is available via the {ORI_TRACE_IDX}=<index> env knob during `ori check`."
    )
}

/// Handle `ori explain idx ...` — `args` are the tokens AFTER `explain idx`.
///
/// `--help` / `-h` prints usage and exits 0. Otherwise expects two positionals:
/// `<index> <file.ori>`. Runs the frontend pipeline on the file, then renders the
/// provenance DAG for the index via the shared tracer SSOT.
pub fn explain_idx(args: &[String]) {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", usage());
        return;
    }

    let positionals: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let [idx_str, path] = positionals.as_slice() else {
        eprintln!("error: `ori explain idx` expects <index> <file.ori>");
        eprintln!();
        eprintln!("{}", usage());
        std::process::exit(1);
    };

    let Ok(idx_raw) = idx_str.parse::<u32>() else {
        eprintln!(
            "ori: explain idx: {idx_str:?} is not a valid type-pool index \
             (expected a non-negative integer; discover indices with \
             {ORI_DUMP_AFTER_TYPECK}=1 {ORI_DUMP_TYPE_IDX}=1 ori check {path})."
        );
        std::process::exit(1);
    };

    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(*path), content);

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(*path);

    // Run the frontend far enough to obtain the type pool + monomorphization
    // instances the tracer reads. Frontend errors abort with a non-zero status.
    let Some(frontend) = report_frontend_errors(&db, file, &mut emitter) else {
        std::process::exit(1);
    };

    match trace_idx_provenance(
        idx_raw,
        &frontend.pool,
        &frontend.type_result.typed.mono_instances,
        db.interner(),
    ) {
        IdxTrace::Rendered(rendered) => print!("{rendered}"),
        IdxTrace::OutOfRange { idx, pool_len } => {
            eprintln!(
                "ori: explain idx: index {idx} {}",
                idx_out_of_range_detail(pool_len)
            );
            std::process::exit(1);
        }
    }
}
