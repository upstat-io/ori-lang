//! Post-fixpoint narrowing and recomputation passes.
//!
//! After the forward fixpoint converges (possibly with widened ranges),
//! these passes recover precision:
//! - **Narrowing**: intersects widened ranges with transfer function output
//! - **Field summary recomputation**: clears and rebuilds from final ranges
//! - **Return range recomputation**: rebuilds from final narrowed variables

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, PrimOp};
use ori_arc::ArcVarId;
use ori_ir::BinaryOp;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::super::field_summary::{
    update_element_summaries, update_element_summaries_from_terminator, update_field_summaries,
    ElementSummaryTable, FieldSummaryTable,
};
use super::super::transfer::{transfer, transfer_known_call, TransferContext};
use super::super::{is_int_typed, ValueRange};
use super::{apply_block_refinements, narrow, restore_block_refinements};
use ori_arc::ArcBlockId;
use ValueRange::{Bottom, Top};

/// Recover the two intervals carried by a directly lowered `Range` loop.
///
/// A loop header sees both body values and the final value that failed the
/// bounds test. The latter may overshoot the endpoint by one step, so using a
/// single interval either loses the body bound or becomes unsound. ARC range
/// lowering has an explicit SSA shape: the header parameter receives the
/// range's start from the preheader and `header_param + range.step` from the
/// latch. Recognizing that shape lets analysis retain a conservative header
/// interval while attaching the tighter semantic interval to the body edge.
pub(super) fn refine_direct_range_inductions(
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    predecessors: &[Vec<usize>],
    block_refinements: &mut FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
) {
    let definitions: FxHashMap<_, _> = func
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instr| instr.defined_var().map(|var| (var, instr)))
        .collect();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Branch { then_block, .. } = block.terminator else {
            continue;
        };

        for (param_idx, &(param, param_ty)) in block.params.iter().enumerate() {
            if !super::super::is_int_typed(param_ty, pool) {
                continue;
            }
            let incoming: Vec<_> = predecessors[block_idx]
                .iter()
                .filter_map(|&pred_idx| match &func.blocks[pred_idx].terminator {
                    ArcTerminator::Jump { target, args } if *target == block.id => {
                        args.get(param_idx).copied()
                    }
                    _ => None,
                })
                .collect();
            if incoming.len() != 2 {
                continue;
            }

            let Some((range_value, construct_args)) = incoming.iter().find_map(|&candidate| {
                let (range_value, field) = project_origin(candidate, &definitions)?;
                if field != 0 {
                    return None;
                }
                let ArcInstr::Construct { ty, args, .. } = definitions.get(&range_value)? else {
                    return None;
                };
                (pool.tag(pool.resolve_fully(*ty)) == ori_types::Tag::Range && args.len() >= 4)
                    .then_some((range_value, args.as_slice()))
            }) else {
                continue;
            };

            let latch_matches = incoming.iter().any(|&candidate| {
                let Some((lhs, rhs)) = add_operands(candidate, &definitions) else {
                    return false;
                };
                [(lhs, rhs), (rhs, lhs)]
                    .into_iter()
                    .any(|(induction, step)| {
                        resolve_alias(induction, &definitions) == param
                            && project_origin(step, &definitions) == Some((range_value, 2))
                    })
            });
            if !latch_matches {
                continue;
            }

            let start = ranges.get(&construct_args[0]).copied().unwrap_or(Top);
            let end = ranges.get(&construct_args[1]).copied().unwrap_or(Top);
            let step = ranges.get(&construct_args[2]).copied().unwrap_or(Top);
            let inclusive = ranges
                .get(&construct_args[3])
                .and_then(ValueRange::is_constant);
            let Some((header_range, body_range)) =
                direct_range_intervals(start, end, step, inclusive)
            else {
                continue;
            };

            ranges
                .entry(param)
                .and_modify(|current| *current = current.meet(header_range))
                .or_insert(header_range);
            block_refinements
                .entry((then_block, param))
                .and_modify(|current| *current = current.meet(body_range))
                .or_insert(body_range);
        }
    }
}

fn resolve_alias(mut var: ArcVarId, definitions: &FxHashMap<ArcVarId, &ArcInstr>) -> ArcVarId {
    for _ in 0..32 {
        let Some(ArcInstr::Let {
            value: ArcValue::Var(next),
            ..
        }) = definitions.get(&var)
        else {
            break;
        };
        if *next == var {
            break;
        }
        var = *next;
    }
    var
}

fn project_origin(
    var: ArcVarId,
    definitions: &FxHashMap<ArcVarId, &ArcInstr>,
) -> Option<(ArcVarId, u32)> {
    let var = resolve_alias(var, definitions);
    let ArcInstr::Project { value, field, .. } = definitions.get(&var)? else {
        return None;
    };
    Some((resolve_alias(*value, definitions), *field))
}

fn add_operands(
    var: ArcVarId,
    definitions: &FxHashMap<ArcVarId, &ArcInstr>,
) -> Option<(ArcVarId, ArcVarId)> {
    let var = resolve_alias(var, definitions);
    let ArcInstr::Let {
        value:
            ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Add),
                args,
            },
        ..
    } = definitions.get(&var)?
    else {
        return None;
    };
    Some((*args.first()?, *args.get(1)?))
}

pub(super) fn direct_range_intervals(
    start: ValueRange,
    end: ValueRange,
    step: ValueRange,
    inclusive: Option<i64>,
) -> Option<(ValueRange, ValueRange)> {
    let (
        ValueRange::Bounded {
            lo: start_lo,
            hi: start_hi,
        },
        ValueRange::Bounded {
            lo: end_lo,
            hi: end_hi,
        },
        ValueRange::Bounded {
            lo: step_lo,
            hi: step_hi,
        },
        inclusive @ (0 | 1),
    ) = (start, end, step, inclusive?)
    else {
        return None;
    };

    if step_lo > 0 {
        let body_hi = if inclusive == 1 {
            end_hi
        } else {
            end_hi.checked_sub(1)?
        };
        let body = bounded_or_bottom(start_lo, body_hi);
        let overshoot = if inclusive == 1 {
            end_hi.saturating_add(step_hi)
        } else {
            end_hi.saturating_add(step_hi).saturating_sub(1)
        };
        let header = ValueRange::Bounded {
            lo: start_lo,
            hi: start_hi.max(overshoot),
        };
        Some((header, body))
    } else if step_hi < 0 {
        let body_lo = if inclusive == 1 {
            end_lo
        } else {
            end_lo.checked_add(1)?
        };
        let body = bounded_or_bottom(body_lo, start_hi);
        let overshoot = if inclusive == 1 {
            end_lo.saturating_add(step_lo)
        } else {
            end_lo.saturating_add(step_lo).saturating_add(1)
        };
        let header = ValueRange::Bounded {
            lo: start_lo.min(overshoot),
            hi: start_hi,
        };
        Some((header, body))
    } else {
        None
    }
}

fn bounded_or_bottom(lo: i64, hi: i64) -> ValueRange {
    if lo <= hi {
        ValueRange::Bounded { lo, hi }
    } else {
        Bottom
    }
}

/// Run one narrowing pass over all blocks to recover precision lost to widening.
///
/// also re-merges block parameters from predecessors, applies
/// block refinements (branch/switch), and narrows invoke terminators.
/// This allows widened loop-header parameters to recover bounded ranges.
#[expect(
    clippy::too_many_arguments,
    reason = "fixpoint infrastructure passes — bundling would add indirection"
)]
pub(super) fn run_narrowing_pass(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &FieldSummaryTable,
    direct_field_sources: &FxHashMap<(ArcVarId, u32), ArcVarId>,
    predecessors: &[Vec<usize>],
    block_refinements: &FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    known_builtins: &super::super::KnownBuiltins,
    call_result_narrowings: &FxHashMap<ArcVarId, super::super::ValueRange>,
) {
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];

        // Narrow block parameters from predecessor jump args.
        // Skip entry block parameters with no predecessors — they may be seeded
        // from interprocedural analysis, and narrowing against Bottom
        // (which means "no info from predecessors") would destroy those seeds.
        for (param_idx, (param_var, _)) in block.params.iter().enumerate() {
            if predecessors[block_idx].is_empty() {
                continue; // Entry block — preserve interprocedural seeds.
            }
            let mut merged = Bottom;
            for &pred_idx in &predecessors[block_idx] {
                let pred = &func.blocks[pred_idx];
                if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
                    if target.index() == block_idx {
                        if let Some(&arg_var) = args.get(param_idx) {
                            merged = merged.join(ranges.get(&arg_var).copied().unwrap_or(Bottom));
                        }
                    }
                }
            }
            if let Some(&refinement) = block_refinements.get(&(block.id, *param_var)) {
                merged = merged.meet(refinement);
            }
            if let Some(&widened) = ranges.get(param_var) {
                let narrowed = narrow(widened, merged);
                if narrowed != widened {
                    ranges.insert(*param_var, narrowed);
                }
            }
        }

        // Apply block-entry refinements temporarily (same as forward pass).
        let saved = apply_block_refinements(block, ranges, block_refinements);

        // Narrow body instructions.
        // Apply updates immediately so later instructions see narrowed values
        // from earlier instructions in the same block. This is critical for
        // loop body copy chains: %18 = %4 (narrowed via refinement) must be
        // visible when computing %20 = %18 + 1.
        let field_summaries = field_summary_table.as_map();
        for instr in &block.body {
            let computed = {
                let ctx = TransferContext {
                    ranges: &*ranges,
                    pool,
                    var_types: &func.var_types,
                    field_summaries,
                    direct_field_sources,
                    known_builtins,
                };
                transfer(instr, &ctx)
            };
            let Some(var) = instr.defined_var() else {
                continue;
            };
            if let Some(&widened) = ranges.get(&var) {
                let narrowed = narrow(widened, computed);
                if narrowed != widened {
                    ranges.insert(var, narrowed);
                }
            }
        }

        // Restore temporary refinements.
        restore_block_refinements(ranges, saved);

        // Narrow invoke terminator.
        // also apply call_result_narrowings for Invoke dst (same
        // as forward pass), so return-range feedback reaches Invoke paths.
        if let ArcTerminator::Invoke {
            dst,
            ty,
            func: callee,
            ..
        } = &block.terminator
        {
            if is_int_typed(*ty, pool) {
                let mut computed = transfer_known_call(*callee, known_builtins).unwrap_or(Top);
                if let Some(&crn) = call_result_narrowings.get(dst) {
                    computed = computed.meet(crn);
                }
                if let Some(&widened) = ranges.get(dst) {
                    let narrowed = narrow(widened, computed);
                    if narrowed != widened {
                        ranges.insert(*dst, narrowed);
                    }
                }
            }
        }
    }
}

/// Recompute field summaries from final ranges (post-narrowing).
///
/// During the fixpoint loop, field summaries may accumulate wider ranges
/// from pre-convergence iterations. This clears and recomputes from the
/// converged ranges.
pub(super) fn recompute_field_summaries(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
    field_summary_table: &mut FieldSummaryTable,
) {
    field_summary_table.clear();
    for &block_idx in rpo {
        for instr in &func.blocks[block_idx].body {
            update_field_summaries(instr, ranges, &func.var_types, pool, field_summary_table);
        }
    }
}

/// Recompute element summaries from final (post-narrowing) variable ranges.
///
/// Same rationale as `recompute_field_summaries`.
pub(super) fn recompute_element_summaries(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
    element_summary_table: &mut ElementSummaryTable,
) {
    element_summary_table.clear();
    for &block_idx in rpo {
        for instr in &func.blocks[block_idx].body {
            update_element_summaries(instr, ranges, &func.var_types, pool, element_summary_table);
        }
        // also check terminators for Invoke calls returning collections.
        update_element_summaries_from_terminator(
            &func.blocks[block_idx].terminator,
            pool,
            element_summary_table,
        );
    }
}

/// Recompute `return_range` from the final narrowed variable ranges.
///
/// During forward iterations, `return_range` accumulates pre-narrowing values.
/// After narrowing recovers precision for loop variables, `return_range` must
/// be recomputed so the interprocedural handoff uses the tightened ranges.
///
/// Only iterates reachable blocks (via `rpo`). Unreachable blocks
/// contain variables that were never analyzed, so `ranges.get()` returns `None`
/// and the `unwrap_or(Top)` fallback would pollute the return range.
pub(super) fn recompute_return_range(
    rpo: &[usize],
    func: &ArcFunction,
    pool: &Pool,
    ranges: &FxHashMap<ArcVarId, ValueRange>,
) -> ValueRange {
    if !is_int_typed(func.return_type, pool) {
        return Bottom;
    }
    let mut result = Bottom;
    for &block_idx in rpo {
        let block = &func.blocks[block_idx];
        if let ArcTerminator::Return { value } = &block.terminator {
            let ret_range = ranges.get(value).copied().unwrap_or(Top);
            result = result.join(ret_range);
        }
    }
    result
}
