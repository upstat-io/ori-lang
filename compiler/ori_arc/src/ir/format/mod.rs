//! Canonical ARC IR formatter.
//!
//! This is the SINGLE canonical ARC IR text formatter. Used by:
//! - `oric::arc_dump` for `ORI_DUMP_AFTER_ARC=1` phase dumps
//! - Snapshot tests for per-pass `.after.arc` baselines
//! - `oric::arc_dot` for DOT graph instruction labels
//!
//! Output follows LLVM IR / Rust MIR conventions:
//! `fn @name(params) -> ret`, `bb0:`, `%var: type = instr`.
//!
//! Deterministic: same `ArcFunction` input always produces the same string.
//! Diffable: no pointers, no addresses, no timestamps — only structural data.

pub mod instr;

#[cfg(test)]
mod tests;

use std::fmt::Write;

use crate::ir::{ArcFunction, ArcVarId, RcAtomicity, RcStrategy, ValueRepr};
use crate::ownership::Ownership;
use ori_ir::StringInterner;
use ori_types::{Idx, Pool};

use self::instr::{fmt_instr, fmt_terminator};

/// Format an `ArcFunction` as human-readable, diffable text.
///
/// This is the SINGLE canonical ARC IR formatter. Used by:
/// - `oric::arc_dump` for `ORI_DUMP_AFTER_ARC=1` phase dumps
/// - Snapshot tests for per-pass `.after.arc` baselines
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub fn format_function(func: &ArcFunction, pool: &Pool, interner: &StringInterner) -> String {
    let mut out = String::with_capacity(1024);

    let name = interner.lookup(func.name);
    let ret_str = format_type(pool, func.return_type, interner);

    // Parameters with ownership and type annotations
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| {
            let pname = format!("%{}", p.var.raw());
            let ty = format_type(pool, p.ty, interner);
            let own = match p.ownership {
                Ownership::Owned => "own",
                Ownership::Borrowed => "borrow",
            };
            format!("{pname}: {ty} [{own}]")
        })
        .collect();

    writeln!(
        &mut out,
        "fn @{name}({}) -> {ret_str} [entry: bb{}]",
        params.join(", "),
        func.entry.raw(),
    )
    .unwrap();

    if func.is_fbip {
        writeln!(out, "  #fbip").unwrap();
    }
    if func.num_captures > 0 {
        writeln!(out, "  captures: {}", func.num_captures).unwrap();
    }

    // Basic blocks
    for block in &func.blocks {
        write!(out, "  bb{}:", block.id.raw()).unwrap();

        // Block parameters (phi-like values)
        if !block.params.is_empty() {
            let bp: Vec<String> = block
                .params
                .iter()
                .map(|(var, ty)| {
                    let ts = format_type(pool, *ty, interner);
                    format!("%{}: {ts}", var.raw())
                })
                .collect();
            write!(out, " ({})", bp.join(", ")).unwrap();
        }
        writeln!(out).unwrap();

        // Instructions
        for instr in &block.body {
            write!(out, "    ").unwrap();
            fmt_instr(&mut out, instr, func, pool, interner);
            writeln!(out).unwrap();
        }

        // Terminator
        write!(out, "    ").unwrap();
        fmt_terminator(&mut out, &block.terminator, func, pool, interner);
        writeln!(out).unwrap();
    }

    out
}

// Formatting helpers

/// Format a type index as a human-readable string.
///
/// Handles `Idx::NONE` gracefully (returns `"<none>"`) since ARC IR
/// functions may have unresolved type slots during testing or early
/// pipeline stages.
pub fn format_type(pool: &Pool, idx: Idx, interner: &StringInterner) -> String {
    if idx == Idx::NONE {
        return "<none>".to_owned();
    }
    pool.format_type_resolved(idx, interner)
}

/// Format a variable reference.
pub fn fmt_var(_func: &ArcFunction, var: ArcVarId) -> String {
    format!("%{}", var.raw())
}

/// Format a variable with its type and repr annotation.
pub fn fmt_var_typed(
    func: &ArcFunction,
    var: ArcVarId,
    pool: &Pool,
    interner: &StringInterner,
) -> String {
    let ty = format_type(pool, func.var_type(var), interner);
    let repr = func
        .var_repr(var)
        .map_or(String::new(), |r| format!(" [{}]", fmt_repr(r)));
    format!("%{}: {ty}{repr}", var.raw())
}

/// Format a value representation tag.
pub fn fmt_repr(repr: ValueRepr) -> &'static str {
    match repr {
        ValueRepr::Scalar => "Scalar",
        ValueRepr::RcPointer => "RcPtr",
        ValueRepr::Aggregate => "Aggregate",
        ValueRepr::FatValue => "FatVal",
    }
}

/// Format an RC strategy tag.
pub fn fmt_strategy(strategy: RcStrategy) -> &'static str {
    match strategy {
        RcStrategy::HeapPointer => "HeapPtr",
        RcStrategy::FatPointer => "FatPtr",
        RcStrategy::Closure => "Closure",
        RcStrategy::AggregateFields => "AggFields",
        RcStrategy::InlineEnum => "InlineEnum",
        RcStrategy::Iterator => "Iterator",
    }
}

/// Format the atomicity suffix for an RC op.
///
/// Returns `""` for [`RcAtomicity::Atomic`] (the construction-site default)
/// so atomic RC ops dump byte-identically to before the carrier landed;
/// returns `" [non-atomic]"` for [`RcAtomicity::NonAtomic`] once the
/// thread-locality dispatch selects it.
pub fn fmt_atomicity_suffix(atomicity: RcAtomicity) -> &'static str {
    match atomicity {
        RcAtomicity::Atomic => "",
        RcAtomicity::NonAtomic => " [non-atomic]",
    }
}
