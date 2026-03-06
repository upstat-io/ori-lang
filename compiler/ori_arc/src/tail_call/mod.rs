//! Tail call detection pass for ARC IR.
//!
//! Identifies self-recursive tail calls and annotates them on
//! [`ArcFunction::tail_calls`] for the loop-lowering rewrite pass (§09.2).
//!
//! # Pipeline placement
//!
//! Runs AFTER `rc_elim` (all RC operations are in final positions) and
//! BEFORE `block_merge` (which cleans up dead blocks after TCO rewrite).
//!
//! # Detection algorithm
//!
//! Finds the cross-block tail call pattern:
//! ```text
//! bb_call:
//!   %result = Apply @func_name(args...)
//!   [optional RcDec ops — verified safe to hoist]
//!   Jump bb_merge([result])
//!
//! bb_merge: (%param)
//!   Return %param
//! ```
//!
//! Also detects direct tail calls (Apply + Return in same block).
//!
//! A tail call is **eligible** when:
//! 1. `Apply.func == func.name` (self-recursion only)
//! 2. No `ApplyIndirect` (callee identity unknown at compile time)
//! 3. All instructions between Apply and the terminator are `RcDec`
//! 4. No `RcDec` target is among the Apply's arguments (safe to hoist)
//!
//! Mutual recursion (A calls B calls A) is excluded — callee stack frame
//! compatibility cannot be verified at compile time.

mod rewrite;

pub(crate) use rewrite::rewrite_tail_calls;

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::Name;
use rustc_hash::FxHashSet;

/// A detected self-recursive tail call site.
///
/// Stores the block and the kind of call (body `Apply` or terminator
/// `Invoke`). The rewrite pass (§09.2) uses this to replace the call
/// with a loop back-edge.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TailCallSite {
    /// Block containing the tail-recursive call.
    pub call_block: crate::ir::ArcBlockId,
    /// Whether the call is a body `Apply` instruction or an `Invoke` terminator.
    pub kind: TailCallKind,
}

/// Distinguishes body `Apply` instructions from `Invoke` terminators.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TailCallKind {
    /// Body `Apply` instruction at this index.
    Apply { instr_idx: usize },
    /// `Invoke` terminator — the call is the block's terminator, not a body
    /// instruction. User function calls are lowered as `Invoke` because they
    /// may unwind (panic).
    Invoke,
}

/// Detect self-recursive tail calls in an ARC function.
///
/// Returns a list of tail call sites. The rewrite pass (§09.2) consumes
/// these to perform loop lowering.
///
/// Only self-recursion is detected (`Apply.func == func.name`). Mutual
/// recursion and `ApplyIndirect` are excluded.
#[tracing::instrument(skip_all, fields(func = ?func.name))]
pub(crate) fn detect_tail_calls(func: &ArcFunction) -> Vec<TailCallSite> {
    let func_name = func.name;
    let mut sites = Vec::new();

    for block in &func.blocks {
        let ArcTerminator::Return { value: ret_val } = &block.terminator else {
            continue;
        };

        // Check if ret_val is a block parameter (cross-block merge pattern).
        let param_pos = block.params.iter().position(|(var, _)| *var == *ret_val);

        if let Some(pos) = param_pos {
            // Cross-block pattern: find predecessors that Jump here
            // and pass a tail-call result at this parameter position.
            find_cross_block_tail_calls(func, block, pos, func_name, &mut sites);
        } else {
            // Direct pattern: Apply and Return in same block.
            if let Some(site) = find_tail_apply_in_block(block, *ret_val, func_name) {
                sites.push(site);
            }
        }
    }

    // Also check Invoke terminators — user function calls are lowered as
    // Invoke (not body Apply) because they may unwind. The pattern is:
    //   bb_call: Invoke @self(args) → normal: bb_normal, unwind: bb_unwind
    //   bb_normal: [RcDec only] → Jump bb_merge([dst]) or Return dst
    //   bb_merge: (%param) → Return %param
    find_invoke_tail_calls(func, func_name, &mut sites);

    tracing::debug!(count = sites.len(), "tail call detection complete");
    sites
}

/// Find cross-block tail calls: predecessors that Jump to `merge_block`,
/// where the jump argument at `param_pos` came from an `Apply` to `func_name`.
fn find_cross_block_tail_calls(
    func: &ArcFunction,
    merge_block: &ArcBlock,
    param_pos: usize,
    func_name: Name,
    sites: &mut Vec<TailCallSite>,
) {
    let merge_id = merge_block.id;

    for pred_block in &func.blocks {
        if pred_block.id == merge_id {
            continue;
        }

        let ArcTerminator::Jump { target, args } = &pred_block.terminator else {
            continue;
        };

        if *target != merge_id {
            continue;
        }

        if param_pos >= args.len() {
            continue;
        }

        let jump_arg = args[param_pos];

        if let Some(site) = find_tail_apply_in_block(pred_block, jump_arg, func_name) {
            sites.push(site);
        }
    }
}

/// Scan a block for a self-recursive `Apply` whose result is `result_var`.
///
/// Verifies:
/// - The `Apply` calls `func_name` (self-recursion)
/// - All instructions after the `Apply` (before the terminator) are `RcDec`
/// - No `RcDec` target is among the `Apply`'s arguments (safe to hoist)
fn find_tail_apply_in_block(
    block: &ArcBlock,
    result_var: ArcVarId,
    func_name: Name,
) -> Option<TailCallSite> {
    for (instr_idx, instr) in block.body.iter().enumerate() {
        let ArcInstr::Apply {
            dst, func, args, ..
        } = instr
        else {
            continue;
        };

        if *dst != result_var {
            continue;
        }

        // Self-recursion only — exclude mutual recursion.
        if *func != func_name {
            continue;
        }

        // All instructions after Apply must be RcDec (or nothing).
        let remaining = &block.body[instr_idx + 1..];
        let all_rc_decs = remaining
            .iter()
            .all(|i| matches!(i, ArcInstr::RcDec { .. }));

        if !all_rc_decs {
            tracing::trace!(
                block = ?block.id,
                instr_idx,
                "tail call rejected: non-RcDec instructions after Apply"
            );
            continue;
        }

        // Safety check: no RcDec target may be an argument to the recursive
        // call. Hoisting such a dec before the call would be use-after-free.
        let arg_set: FxHashSet<ArcVarId> = args.iter().copied().collect();
        let has_unsafe_dec = remaining
            .iter()
            .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if arg_set.contains(var)));

        if has_unsafe_dec {
            tracing::trace!(
                block = ?block.id,
                instr_idx,
                "tail call rejected: RcDec target used as recursive arg"
            );
            continue;
        }

        return Some(TailCallSite {
            call_block: block.id,
            kind: TailCallKind::Apply { instr_idx },
        });
    }

    None
}

/// Find tail call sites where the self-recursive call is an `Invoke` terminator.
///
/// User function calls are lowered as `Invoke` (not body `Apply`) because they
/// may unwind. This function detects the pattern:
///
/// ```text
/// bb_call:
///   [arg computation]
///   Invoke @self(args) → normal: bb_normal, unwind: bb_unwind
///
/// bb_normal:
///   [optional RcDec ops]
///   Jump bb_merge([dst])   — or —   Return dst
///
/// bb_merge: (%param)
///   Return %param
/// ```
fn find_invoke_tail_calls(func: &ArcFunction, func_name: Name, sites: &mut Vec<TailCallSite>) {
    for block in &func.blocks {
        let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            normal,
            ..
        } = &block.terminator
        else {
            continue;
        };

        // Self-recursion only.
        if *callee != func_name {
            continue;
        }

        let normal_block = &func.blocks[normal.index()];

        // Normal block body must be empty or only RcDec instructions.
        let all_rc_decs = normal_block
            .body
            .iter()
            .all(|i| matches!(i, ArcInstr::RcDec { .. }));
        if !all_rc_decs {
            tracing::trace!(
                block = ?block.id,
                "invoke tail call rejected: non-RcDec instructions in normal block"
            );
            continue;
        }

        // Safety: no RcDec target may be an argument to the recursive call.
        // These RcDecs will be hoisted before the back-edge jump that passes
        // the args, so decrementing an arg would be use-after-free.
        let arg_set: FxHashSet<ArcVarId> = args.iter().copied().collect();
        let has_unsafe_dec = normal_block
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::RcDec { var, .. } if arg_set.contains(var)));
        if has_unsafe_dec {
            tracing::trace!(
                block = ?block.id,
                "invoke tail call rejected: RcDec target used as recursive arg"
            );
            continue;
        }

        // Direct pattern: normal block returns dst.
        if let ArcTerminator::Return { value } = &normal_block.terminator {
            if *value == *dst {
                sites.push(TailCallSite {
                    call_block: block.id,
                    kind: TailCallKind::Invoke,
                });
                continue;
            }
        }

        // Cross-block pattern: normal block jumps to a merge block that
        // returns the invoke result.
        if let ArcTerminator::Jump {
            target,
            args: jump_args,
        } = &normal_block.terminator
        {
            let dst_pos = jump_args.iter().position(|v| *v == *dst);
            if let Some(pos) = dst_pos {
                let merge_block = &func.blocks[target.index()];
                if let ArcTerminator::Return { value } = &merge_block.terminator {
                    let param = merge_block.params.get(pos).map(|(v, _)| *v);
                    if param == Some(*value) {
                        sites.push(TailCallSite {
                            call_block: block.id,
                            kind: TailCallKind::Invoke,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
