//! ARC IR pretty-printer for phase dumps.
//!
//! Produces human-readable representations of the ARC IR after the full ARC
//! pipeline (borrow inference -> RC insertion -> RC elimination -> reset/reuse).
//! Intended for compiler debugging via `ORI_DUMP_AFTER_ARC=1`.
//!
//! The core formatting logic lives in `ori_arc::ir::format` (canonical home).
//! This module is a thin wrapper that re-runs the ARC pipeline on cached
//! functions and delegates formatting.

pub(crate) mod instr;

use std::fmt::Write;

use ori_arc::{AnnotatedSig, ArcClassification, ArcFunction};
use ori_ir::{Name, StringInterner};
use ori_types::Pool;
use rustc_hash::FxHashMap;

/// Dump ARC IR to stderr for all functions in the arc cache.
///
/// Clones the pre-lowered functions, runs the full ARC pipeline on each,
/// then pretty-prints the result. The clone + re-run cost is negligible
/// (only fires when `ORI_DUMP_AFTER_ARC=1` is set).
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
#[expect(
    clippy::implicit_hasher,
    reason = "internal function always called with FxHashMap"
)]
pub fn dump_arc_ir(
    arc_cache: &FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>,
    annotated_sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
    pool: &Pool,
    interner: &StringInterner,
    path: &str,
) {
    let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
    let mut out = String::with_capacity(16384);

    // Collect and sort function names for deterministic output order.
    let mut entries: Vec<_> = arc_cache.iter().collect();
    entries.sort_by_key(|(name, _)| interner.lookup(**name));

    let total_funcs: usize = entries
        .iter()
        .map(|(_, (_, lambdas))| 1 + lambdas.len())
        .sum();

    writeln!(out, "=== ARC IR after lowering: {path} ===").unwrap();
    writeln!(out, "  {total_funcs} functions").unwrap();
    writeln!(out).unwrap();

    for (_, (parent, lambdas)) in &entries {
        // Clone and run the full ARC pipeline for accurate RC op display.
        let mut funcs: Vec<ArcFunction> = std::iter::once(parent.clone())
            .chain(lambdas.iter().cloned())
            .collect();
        let problems = ori_arc::run_arc_pipeline_all(
            &mut funcs,
            classifier,
            annotated_sigs,
            interner,
            pool,
            &builtins,
            std::env::var(crate::debug_flags::ORI_VERIFY_ARC).is_ok_and(|v| v != "0"),
        );
        // Log verification ICEs but don't abort the dump -- it's a diagnostic tool.
        if let Err(verify_errors) = &problems {
            for e in verify_errors {
                tracing::error!("ARC IR verification ICE during dump: {e}");
            }
        }
        let _ = problems;

        for func in &funcs {
            // Delegate to ori_arc's canonical formatter.
            out.push_str(&ori_arc::ir::format::format_function(func, pool, interner));
            writeln!(out).unwrap();
        }
    }

    writeln!(out, "=== END ARC IR ===").unwrap();
    eprint!("{out}");
}
