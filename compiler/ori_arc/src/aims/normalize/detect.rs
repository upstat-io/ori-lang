//! Structural TRMC candidate detection (pre-analysis).
//!
//! Identifies `Construct` instructions where at least one field argument is
//! produced by a recursive call to the same function. This is a purely
//! structural analysis — no dataflow information is needed.
//!
//! The post-convergence `detect_trmc_candidates()` in `intraprocedural/mod.rs`
//! additionally checks the `Uniqueness` dimension (soundness gate). This
//! pre-analysis detection provides the structural metadata that
//! `analyze_function()` uses to record `ContextOpen`/`ContextClose` events.

use rustc_hash::FxHashMap;

use crate::aims::contract::ContextRegion;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};

/// Detect TRMC constructor-context regions in a function.
///
/// Scans for `Construct` instructions with at least one field argument
/// defined by a recursive call (`Apply` or `Invoke` where callee ==
/// current function). Returns a [`ContextRegion`] for each match.
///
/// # Self-recursive only (v1)
///
/// Only detects direct self-recursion (callee name == function name).
/// Mutual recursion is out of scope for v1.
pub(super) fn detect_context_regions(func: &ArcFunction) -> Vec<ContextRegion> {
    let self_name = func.name;

    // Collect variables defined by recursive calls, with their definition site.
    let recursive_defs = collect_recursive_call_sites(func);
    if recursive_defs.is_empty() {
        return Vec::new();
    }

    let mut regions = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            else {
                continue;
            };

            // Only struct/enum constructors are TRMC candidates.
            if !matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. }) {
                continue;
            }

            // Find the first field argument that was produced by a recursive call.
            for (field_idx, arg) in args.iter().enumerate() {
                if let Some(call_site) = recursive_defs.get(arg) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "ARC IR field counts fit in u32"
                    )]
                    let hole_field = field_idx as u32;

                    regions.push(ContextRegion {
                        open_block: blk,
                        open_instr: instr_idx,
                        context_var: *dst,
                        hole_field,
                        close_block: call_site.block,
                        close_instr: call_site.instr,
                        hole_var: *arg,
                    });

                    tracing::trace!(
                        func = ?self_name,
                        context_var = dst.raw(),
                        hole_field,
                        hole_var = arg.raw(),
                        open_block = blk.raw(),
                        close_block = call_site.block.raw(),
                        "detected TRMC context region"
                    );

                    // One region per Construct (use first recursive arg).
                    break;
                }
            }
        }
    }

    regions
}

/// Location of a recursive call definition.
#[derive(Clone, Copy, Debug)]
struct CallSite {
    block: ArcBlockId,
    instr: usize,
}

/// Collect variables defined by recursive calls, mapping each to its call site.
///
/// Scans both `Apply` instructions (body) and `Invoke` terminators for
/// calls where the callee name matches `func.name`.
fn collect_recursive_call_sites(func: &ArcFunction) -> FxHashMap<ArcVarId, CallSite> {
    let mut sites = FxHashMap::default();
    let self_name = func.name;

    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == self_name {
                    sites.insert(
                        *dst,
                        CallSite {
                            block: blk,
                            instr: instr_idx,
                        },
                    );
                }
            }
        }

        // Invoke terminators also define a dst from a call.
        // Invoke instr index = block.body.len() (the terminator position).
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if *callee == self_name {
                sites.insert(
                    *dst,
                    CallSite {
                        block: blk,
                        instr: block.body.len(),
                    },
                );
            }
        }
    }

    sites
}
