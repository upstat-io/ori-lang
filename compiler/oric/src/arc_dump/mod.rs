//! ARC IR pretty-printer for phase dumps.
//!
//! Produces human-readable representations of the ARC IR after the full ARC
//! pipeline (borrow inference → RC insertion → RC elimination → reset/reuse).
//! Intended for compiler debugging via `ORI_DUMP_AFTER_ARC=1`.
//!
//! Unlike `ast_dump` (tree-walking parse tree) and `ir_dump` (tree-walking typed
//! IR), ARC IR is a basic-block SSA-like form. The output follows LLVM IR / Rust
//! MIR conventions: `fn @name(params) -> ret`, `bb0:`, `%var: type = instr`.

pub(crate) mod instr;

use std::fmt::Write;

use ori_arc::{AnnotatedSig, ArcClassification, ArcFunction, Ownership, RcStrategy, ValueRepr};
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use self::instr::{fmt_instr, fmt_terminator};

/// Dump ARC IR to stderr for all functions in the arc cache.
///
/// Clones the pre-lowered functions, runs the full ARC pipeline on each,
/// then pretty-prints the result. The clone + re-run cost is negligible
/// (only fires when `ORI_DUMP_AFTER_ARC=1` is set).
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
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
        let _problems = ori_arc::run_arc_pipeline_all(
            &mut funcs,
            classifier,
            annotated_sigs,
            interner,
            pool,
            &builtins,
        );

        for func in &funcs {
            dump_function(&mut out, func, pool, interner);
            writeln!(out).unwrap();
        }
    }

    writeln!(out, "=== END ARC IR ===").unwrap();
    eprint!("{out}");
}

/// Pretty-print a single ARC function.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn dump_function(out: &mut String, func: &ArcFunction, pool: &Pool, interner: &StringInterner) {
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
        out,
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
            fmt_instr(out, instr, func, pool, interner);
            writeln!(out).unwrap();
        }

        // Terminator
        write!(out, "    ").unwrap();
        fmt_terminator(out, &block.terminator, func, pool, interner);
        writeln!(out).unwrap();
    }
}

// Formatting helpers

/// Format a type index as a human-readable string.
pub(crate) fn format_type(pool: &Pool, idx: Idx, interner: &StringInterner) -> String {
    pool.format_type_resolved(idx, interner)
}

/// Format a variable reference with its type and repr annotation.
pub(crate) fn fmt_var(_func: &ArcFunction, var: ori_arc::ArcVarId) -> String {
    format!("%{}", var.raw())
}

/// Format a variable with its type annotation.
pub(crate) fn fmt_var_typed(
    func: &ArcFunction,
    var: ori_arc::ArcVarId,
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
pub(crate) fn fmt_repr(repr: ValueRepr) -> &'static str {
    match repr {
        ValueRepr::Scalar => "Scalar",
        ValueRepr::RcPointer => "RcPtr",
        ValueRepr::Aggregate => "Aggregate",
        ValueRepr::FatValue => "FatVal",
    }
}

/// Format an RC strategy tag.
pub(crate) fn fmt_strategy(strategy: RcStrategy) -> &'static str {
    match strategy {
        RcStrategy::HeapPointer => "HeapPtr",
        RcStrategy::FatPointer => "FatPtr",
        RcStrategy::Closure => "Closure",
        RcStrategy::AggregateFields => "AggFields",
        RcStrategy::InlineEnum => "InlineEnum",
    }
}
