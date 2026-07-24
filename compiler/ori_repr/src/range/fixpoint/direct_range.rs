//! Direct-range induction recognition and semantic interval recovery.

use ori_arc::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, PrimOp};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_ir::BinaryOp;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::{is_int_typed, ValueRange};
use ValueRange::{Bottom, Top};

/// Recover the two intervals carried by a directly lowered `Range` loop.
///
/// The header parameter merges the range start with `header_param + step`.
/// The failed bound may overshoot by one step, so the header retains a
/// conservative interval while the body edge receives the semantic interval.
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
            if !is_int_typed(param_ty, pool) {
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
    let mut seen = FxHashSet::default();
    while seen.insert(var) {
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

/// Derives loop-header and loop-body ranges for a bounded direct range.
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
