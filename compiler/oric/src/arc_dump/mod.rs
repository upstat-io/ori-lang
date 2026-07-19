//! ARC IR pretty-printer for phase dumps.
//!
//! Produces human-readable representations of the ARC IR after the full ARC
//! pipeline (borrow inference -> RC insertion -> RC elimination -> reset/reuse).
//! Intended for compiler debugging via `ORI_DUMP_AFTER_ARC=1`.
//!
//! The core formatting logic lives in `ori_arc::ir::format` (canonical home).
//! This module is a thin renderer over the already-realized executable artifact.

pub(crate) mod instr;

use std::fmt::Write;

use ori_arc::ArcFunction;
use ori_ir::StringInterner;
use ori_types::Pool;

/// Dump ARC IR to stderr for all functions in the closed artifact.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(crate) fn dump_arc_ir(
    functions: &[ArcFunction],
    pool: &Pool,
    interner: &StringInterner,
    path: &str,
) {
    let mut out = String::with_capacity(16384);

    writeln!(out, "=== ARC IR after lowering: {path} ===").unwrap();
    writeln!(out, "  {} functions", functions.len()).unwrap();
    writeln!(out).unwrap();

    for func in functions {
        // Delegate to ori_arc's canonical formatter.
        out.push_str(&ori_arc::ir::format::format_function(func, pool, interner));
        writeln!(out).unwrap();
    }

    writeln!(out, "=== END ARC IR ===").unwrap();
    eprint!("{out}");
}
