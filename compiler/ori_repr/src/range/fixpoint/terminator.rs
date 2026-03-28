//! Terminator processing for the fixpoint loop.
//!
//! Handles `Invoke` (defines a variable), `Branch`/`Switch` (produce
//! conditional refinements), and `Return` (accumulates return range).

use ori_arc::ir::{ArcBlock, ArcFunction, ArcTerminator};
use ori_arc::{ArcBlockId, ArcVarId};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::super::conditional::refine_from_branch;
use super::super::transfer::transfer_known_call;
use super::super::{is_int_typed, KnownBuiltins, ValueRange};
use super::update_range;
use ValueRange::{Bottom, Bounded, Top};

/// Process a block's terminator: `Invoke` defines a variable,
/// `Branch`/`Switch` produce refinements, `Return` accumulates return range.
#[expect(
    clippy::too_many_arguments,
    reason = "fixpoint infrastructure passes — bundling would add indirection"
)]
pub(super) fn process_terminator(
    block: &ArcBlock,
    func: &ArcFunction,
    pool: &Pool,
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    block_refinements: &mut FxHashMap<(ArcBlockId, ArcVarId), ValueRange>,
    return_range: &mut ValueRange,
    iteration: usize,
    known_builtins: &KnownBuiltins,
    call_result_narrowings: &FxHashMap<ArcVarId, ValueRange>,
    thresholds: &[i64],
) -> bool {
    let mut changed = false;
    match &block.terminator {
        ArcTerminator::Invoke {
            dst,
            ty,
            func: callee,
            ..
        } => {
            if is_int_typed(*ty, pool) {
                let mut new_range = transfer_known_call(*callee, known_builtins).unwrap_or(Top);
                // TPR-03-034: Apply callee return-range narrowing to Invoke dst,
                // matching the Apply handling in run_forward_iteration(). Without
                // this, Invoke dst vars miss interprocedural return-range facts.
                if let Some(&narrowing) = call_result_narrowings.get(dst) {
                    new_range = new_range.meet(narrowing);
                }
                changed |= update_range(ranges, *dst, new_range, iteration, thresholds);
            }
        }
        ArcTerminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            // TPR-03-020: JOIN refinements per (block, var) — not overwrite.
            // Multiple predecessors may target the same block with different
            // refinements; joining is the sound over-approximation.
            let refinements = refine_from_branch(*cond, ranges, &block.body);
            for r in &refinements {
                block_refinements
                    .entry((*then_block, r.var))
                    .and_modify(|existing| *existing = existing.join(r.true_range))
                    .or_insert(r.true_range);
                block_refinements
                    .entry((*else_block, r.var))
                    .and_modify(|existing| *existing = existing.join(r.false_range))
                    .or_insert(r.false_range);
            }
        }
        ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        } => {
            // TPR-03-017: JOIN case values per (block, scrutinee) — not overwrite.
            for &(case_val, case_block) in cases {
                if let Ok(val) = i64::try_from(case_val) {
                    let exact = Bounded { lo: val, hi: val };
                    block_refinements
                        .entry((case_block, *scrutinee))
                        .and_modify(|existing| *existing = existing.join(exact))
                        .or_insert(exact);
                }
            }
            // TPR-03-018: default block gets complement refinement.
            // TPR-03-036: use JOIN (not meet) when multiple predecessors
            // target the same default — join is the sound over-approximation.
            let scr_range = ranges.get(scrutinee).copied().unwrap_or(Top);
            let complement = switch_default_complement(scr_range, cases);
            if complement != scr_range {
                block_refinements
                    .entry((*default, *scrutinee))
                    .and_modify(|existing| *existing = existing.join(complement))
                    .or_insert(complement);
            }
        }
        ArcTerminator::Return { value } => {
            if is_int_typed(func.return_type, pool) {
                let ret_range = ranges.get(value).copied().unwrap_or(Top);
                *return_range = return_range.join(ret_range);
            }
        }
        ArcTerminator::Jump { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {}
    }
    changed
}

/// Compute the complement range for a Switch default block.
///
/// Trims contiguous case values from the lo/hi edges of the scrutinee range.
/// For example: scrutinee `[0, 10]` with cases `{0, 1, 2}` → `[3, 10]`.
/// Middle gaps (non-contiguous cases) cannot be excluded with a single
/// interval, so they are conservatively kept.
fn switch_default_complement(
    scrutinee_range: ValueRange,
    cases: &[(u64, ArcBlockId)],
) -> ValueRange {
    let Bounded { lo, hi } = scrutinee_range else {
        return scrutinee_range;
    };

    let mut case_vals: Vec<i64> = cases
        .iter()
        .filter_map(|&(v, _)| i64::try_from(v).ok())
        .filter(|&v| v >= lo && v <= hi)
        .collect();

    if case_vals.is_empty() {
        return scrutinee_range;
    }
    case_vals.sort_unstable();
    case_vals.dedup();

    // Trim contiguous cases from the low edge.
    let mut new_lo = lo;
    for &v in &case_vals {
        if v == new_lo {
            match new_lo.checked_add(1) {
                Some(next) => new_lo = next,
                None => return Top,
            }
        } else {
            break;
        }
    }

    // Trim contiguous cases from the high edge.
    let mut new_hi = hi;
    for &v in case_vals.iter().rev() {
        if v == new_hi {
            match new_hi.checked_sub(1) {
                Some(prev) => new_hi = prev,
                None => return Top,
            }
        } else {
            break;
        }
    }

    if new_lo > new_hi {
        Bottom
    } else {
        Bounded {
            lo: new_lo,
            hi: new_hi,
        }
    }
}
