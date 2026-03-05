//! Phase 4: Merge single-predecessor Jump chains until fixed point.
//!
//! For each block A with terminator `Jump { target: B, args }` where:
//! - A != B (self-loop guard)
//! - B has exactly one predecessor (A)
//! - B is not the entry block
//!
//! Lower B's params as Let bindings (parallel-copy semantics), then
//! merge B's body and spans into A.
//!
//! Runs to fixed point for transitive chains (A → B → C all merge into A).
//! After fixed point, runs a final compaction to remove dead blocks.

use rustc_hash::FxHashSet;

use crate::graph::compute_pred_counts;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::compact::compact_blocks;

/// Merge single-predecessor Jump chains until fixed point.
pub(crate) fn merge_jump_chains(func: &mut ArcFunction) {
    let mut dead: FxHashSet<usize> = FxHashSet::default();

    loop {
        let mut changed = false;
        let pred_counts = compute_pred_counts(func);

        for a_idx in 0..func.blocks.len() {
            if dead.contains(&a_idx) {
                continue;
            }

            let (b_idx, jump_args) = {
                let ArcTerminator::Jump { target, args } = &func.blocks[a_idx].terminator else {
                    continue;
                };
                let b_idx = target.index();

                // Self-loop guard.
                if a_idx == b_idx {
                    continue;
                }
                // B must have exactly one predecessor.
                if pred_counts[b_idx] != 1 {
                    continue;
                }
                // B must not be the entry block.
                if b_idx == func.entry.index() {
                    continue;
                }
                // B must not already be dead.
                if dead.contains(&b_idx) {
                    continue;
                }

                (b_idx, args.clone())
            };

            let b_params = func.blocks[b_idx].params.clone();

            // Arity check: Jump args must match target block params.
            debug_assert_eq!(
                b_params.len(),
                jump_args.len(),
                "Jump args/params arity mismatch: block {a_idx} → block {b_idx}",
            );
            if b_params.len() != jump_args.len() {
                continue;
            }

            // Lower parallel-copy semantics: block params → Let bindings.
            lower_parallel_copy(func, a_idx, &b_params, &jump_args);

            // Remap COW annotations: B's entries → A's coordinates.
            let offset = func.blocks[a_idx].body.len();
            func.cow_annotations.remap_block_merge(b_idx, a_idx, offset);

            // Merge B's body into A.
            let b_body: Vec<ArcInstr> = func.blocks[b_idx].body.drain(..).collect();
            func.blocks[a_idx].body.extend(b_body);

            // Merge B's spans into A.
            let b_spans: Vec<Option<ori_ir::Span>> = func.spans[b_idx].drain(..).collect();
            func.spans[a_idx].extend(b_spans);

            // Replace A's terminator with B's.
            let b_term = std::mem::replace(
                &mut func.blocks[b_idx].terminator,
                ArcTerminator::Unreachable,
            );
            func.blocks[a_idx].terminator = b_term;

            // Mark B as dead.
            dead.insert(b_idx);
            changed = true;
        }

        if !changed {
            break;
        }
    }

    // Final compaction: remove dead blocks.
    if !dead.is_empty() {
        compact_blocks(func);
    }
}

/// Lower block-param parallel-copy semantics to sequential Let bindings.
///
/// Jump args are parallel phi inputs — all args are read before any param
/// is written. When no arg aliases a target param, direct Let is safe.
/// When overlap exists (e.g., swap: `Jump { args: [p1, p0] }` → params
/// `[p0, p1]`), we use fresh temps to avoid clobbering.
pub(super) fn lower_parallel_copy(
    func: &mut ArcFunction,
    block_idx: usize,
    params: &[(ArcVarId, ori_types::Idx)],
    args: &[ArcVarId],
) {
    if params.is_empty() {
        return;
    }

    // Check for overlap: does any arg alias a target param?
    let param_vars: FxHashSet<ArcVarId> = params.iter().map(|(v, _)| *v).collect();
    let has_overlap = args.iter().any(|a| param_vars.contains(a));

    if has_overlap {
        // Slow path: copy all args to fresh temps first, then temps to params.
        // Use fresh_var_repr to preserve repr metadata for ref-typed params.
        let temps: Vec<ArcVarId> = args
            .iter()
            .zip(params.iter())
            .map(|(arg, (_, ty))| {
                let repr = func.var_repr(*arg).unwrap_or(ValueRepr::Scalar);
                func.fresh_var_repr(*ty, repr)
            })
            .collect();

        // Phase 1: args → temps.
        for ((&arg, temp), (_, ty)) in args.iter().zip(temps.iter()).zip(params.iter()) {
            func.blocks[block_idx].body.push(ArcInstr::Let {
                dst: *temp,
                ty: *ty,
                value: ArcValue::Var(arg),
            });
            func.spans[block_idx].push(None);
        }

        // Phase 2: temps → params.
        for ((param_var, param_ty), temp) in params.iter().zip(temps.iter()) {
            func.blocks[block_idx].body.push(ArcInstr::Let {
                dst: *param_var,
                ty: *param_ty,
                value: ArcValue::Var(*temp),
            });
            func.spans[block_idx].push(None);
        }
    } else {
        // Fast path: no aliasing, direct Let is safe.
        for ((param_var, param_ty), &arg) in params.iter().zip(args.iter()) {
            func.blocks[block_idx].body.push(ArcInstr::Let {
                dst: *param_var,
                ty: *param_ty,
                value: ArcValue::Var(arg),
            });
            func.spans[block_idx].push(None);
        }
    }
}
