//! ARC IR → `GraphViz` DOT visualization.
//!
//! Emits DOT digraphs for ARC IR control-flow graphs. Each function becomes
//! a subgraph with basic blocks as table nodes and control flow as colored
//! edges. RC operations (inc/dec) are color-highlighted in the nodes.
//!
//! Output goes to stderr (consistent with all phase dumps). Users pipe to file:
//!
//! ```bash
//! ORI_EMIT_ARC_DOT=1 ori build file.ori 2> arc.dot
//! dot -Tsvg arc.dot -o arc.svg
//! ```

mod node;

use std::fmt::Write;

use ori_arc::{AnnotatedSig, ArcClassification, ArcFunction, ArcTerminator};
use ori_ir::{Name, StringInterner};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use self::node::{render_block_node, render_param_header};

/// Emit DOT graphs for all functions in the ARC cache to stderr.
///
/// Clones the pre-lowered functions and re-runs the full ARC pipeline (same
/// pattern as `arc_dump::dump_arc_ir`) for accurate RC op display.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
#[expect(
    clippy::implicit_hasher,
    reason = "internal function always called with FxHashMap"
)]
pub fn emit_arc_dot(
    arc_cache: &FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>,
    annotated_sigs: &FxHashMap<Name, AnnotatedSig>,
    classifier: &dyn ArcClassification,
    pool: &Pool,
    interner: &StringInterner,
) {
    let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
    let mut out = String::with_capacity(16384);

    // Collect and sort for deterministic output.
    let mut entries: Vec<_> = arc_cache.iter().collect();
    entries.sort_by_key(|(name, _)| interner.lookup(**name));

    for (_, (parent, lambdas)) in &entries {
        // Clone and run pipeline for accurate RC ops.
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
            std::env::var("ORI_VERIFY_ARC").is_ok(),
        );

        for func in &funcs {
            emit_function_graph(&mut out, func, pool, interner);
            writeln!(out).unwrap();
        }
    }

    eprint!("{out}");
}

/// Emit a single function as a DOT digraph.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn emit_function_graph(
    out: &mut String,
    func: &ArcFunction,
    pool: &Pool,
    interner: &StringInterner,
) {
    let name = interner.lookup(func.name);
    // DOT identifiers can't contain most special characters — sanitize
    let safe_name = sanitize_dot_id(name);

    writeln!(out, "digraph \"{name}\" {{").unwrap();
    writeln!(out, "  rankdir=TB;").unwrap();
    writeln!(
        out,
        "  fontname=\"Courier\"; node [shape=plaintext fontname=\"Courier\"];"
    )
    .unwrap();
    writeln!(out, "  label=\"fn @{name}\"; labelloc=t;").unwrap();
    writeln!(out).unwrap();

    // Parameter header node
    if !func.params.is_empty() {
        render_param_header(out, func, pool, interner);
        writeln!(out).unwrap();
    }

    // Block nodes
    for block in &func.blocks {
        render_block_node(out, block, func, pool, interner);
    }

    writeln!(out).unwrap();

    // Edges from terminators
    for block in &func.blocks {
        let src = block.id.raw();
        emit_edges(out, src, &block.terminator);
    }

    writeln!(out, "}}").unwrap();

    let _ = safe_name; // used for the digraph name
}

/// Emit CFG edges from a block's terminator.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn emit_edges(out: &mut String, src: u32, term: &ArcTerminator) {
    match term {
        ArcTerminator::Jump { target, .. } => {
            writeln!(out, "  bb{src} -> bb{};", target.raw()).unwrap();
        }

        ArcTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            writeln!(
                out,
                "  bb{src} -> bb{} [label=\"true\" color=\"green4\"];",
                then_block.raw()
            )
            .unwrap();
            writeln!(
                out,
                "  bb{src} -> bb{} [label=\"false\" color=\"red3\"];",
                else_block.raw()
            )
            .unwrap();
        }

        ArcTerminator::Switch { cases, default, .. } => {
            for (val, target) in cases {
                writeln!(out, "  bb{src} -> bb{} [label=\"{val}\"];", target.raw()).unwrap();
            }
            writeln!(
                out,
                "  bb{src} -> bb{} [label=\"default\" style=dashed];",
                default.raw()
            )
            .unwrap();
        }

        ArcTerminator::Invoke { normal, unwind, .. } => {
            writeln!(
                out,
                "  bb{src} -> bb{} [label=\"normal\" color=\"green4\"];",
                normal.raw()
            )
            .unwrap();
            writeln!(
                out,
                "  bb{src} -> bb{} [label=\"unwind\" color=\"red3\" style=dashed];",
                unwind.raw()
            )
            .unwrap();
        }

        // No outgoing edges
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
}

/// Sanitize a name for use as a DOT identifier.
///
/// Replaces characters that are invalid in DOT identifiers with underscores.
fn sanitize_dot_id(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
