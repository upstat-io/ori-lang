//! Conditional range refinement — extracts range information from branch conditions.
//!
//! When `if x < 100` branches, the true branch knows `x ∈ [lo, 99]` and the
//! false branch knows `x ∈ [100, hi]`. This is the most powerful source of
//! narrowing information for loop counters and bounds checks.
//!
//! Called from the fixpoint loop (§03.3) when processing `Branch` terminators.

use ori_arc::ir::{ArcInstr, ArcValue, PrimOp};
use ori_arc::ArcVarId;
use ori_ir::BinaryOp;

use super::ValueRange;
use ValueRange::{Bottom, Bounded, Top};

/// Refinement result for a single variable at a branch point.
///
/// Both `true_range` and `false_range` are intersected with the variable's
/// current range, so they can only narrow (never widen) the existing range.
#[derive(Debug, Clone)]
pub struct BranchRefinement {
    /// The variable being refined.
    pub var: ArcVarId,
    /// Range in the true branch (condition holds).
    pub true_range: ValueRange,
    /// Range in the false branch (condition doesn't hold).
    pub false_range: ValueRange,
}

/// Extract range refinements from a `Branch` terminator's condition.
///
/// `cond_var` is the `ArcVarId` from `Branch { cond, .. }`.
/// `body` is the block's body instructions — we trace `cond_var` back
/// to the `ArcInstr` that produced it (e.g., a `PrimOp` comparison).
///
/// Returns refinements for variables that can be narrowed in each branch.
/// An empty vec means no refinement could be extracted (safe — no narrowing).
#[tracing::instrument(skip_all)]
pub fn refine_from_branch<S: std::hash::BuildHasher>(
    cond_var: ArcVarId,
    ranges: &std::collections::HashMap<ArcVarId, ValueRange, S>,
    body: &[ArcInstr],
) -> Vec<BranchRefinement> {
    // Find the instruction that defined cond_var (search backwards).
    let Some(def_instr) = body
        .iter()
        .rev()
        .find(|i| i.defined_var() == Some(cond_var))
    else {
        return vec![]; // cond defined in a predecessor — can't analyze locally
    };

    // Match: cond = PrimOp(CmpOp, [x, y])
    let ArcInstr::Let {
        value:
            ArcValue::PrimOp {
                op: PrimOp::Binary(cmp_op),
                args,
            },
        ..
    } = def_instr
    else {
        return vec![]; // not a comparison — can't extract
    };

    if args.len() != 2 {
        return vec![];
    }

    let mut x = args[0];
    // Trace through Var copy chains to find the original variable.
    // The ARC pipeline inserts copies like `%6 = %4` at block boundaries.
    // Refining the original (%4) is more useful than the copy (%6) because
    // the original is visible in dominated blocks (loop body) while the copy
    // is local to the comparison block.
    for instr in body.iter().rev() {
        if let ArcInstr::Let {
            dst,
            value: ArcValue::Var(src),
            ..
        } = instr
        {
            if *dst == x {
                x = *src;
                // Continue tracing — there might be multiple levels of copies.
                // Don't break here in case %6 = %5 = %4.
            }
        }
    }

    let x_range = ranges.get(&x).copied().unwrap_or(Top);

    // We need y to be a known constant for the simple refinement.
    let Some(c) = ranges.get(&args[1]).and_then(ValueRange::is_constant) else {
        return vec![];
    };

    refine_comparison(*cmp_op, x, x_range, c)
}

/// Compute refinements for `x <cmp_op> c` where c is a constant.
///
/// Each operator has a true-range and false-range that narrow `x`:
/// - `Lt`:   true `[lo, c-1]`, false `[c, hi]`
/// - `LtEq`: true `[lo, c]`,   false `[c+1, hi]`
/// - `Gt`:   true `[c+1, hi]`, false `[lo, c]`
/// - `GtEq`: true `[c, hi]`,   false `[lo, c-1]`
/// - `Eq`:   true `[c, c]`,    false = current range (conservative)
/// - `NotEq`: true = current range (conservative), false `[c, c]`
fn refine_comparison(
    op: BinaryOp,
    x: ArcVarId,
    x_range: ValueRange,
    c: i64,
) -> Vec<BranchRefinement> {
    let (true_constraint, false_constraint) = match op {
        BinaryOp::Lt => {
            // x < c → true: x ∈ [-∞, c-1], false: x ∈ [c, +∞]
            // When c = i64::MIN, c-1 overflows → true branch is impossible (Bottom).
            let hi = c.checked_sub(1).map_or(Bottom, |h| Bounded {
                lo: i64::MIN,
                hi: h,
            });
            let lo = Bounded {
                lo: c,
                hi: i64::MAX,
            };
            (hi, lo)
        }
        BinaryOp::LtEq => {
            // x <= c → true: x ∈ [-∞, c], false: x ∈ [c+1, +∞]
            let t = Bounded {
                lo: i64::MIN,
                hi: c,
            };
            // When c = i64::MAX, c+1 overflows → false branch is impossible (Bottom).
            let f = c.checked_add(1).map_or(Bottom, |l| Bounded {
                lo: l,
                hi: i64::MAX,
            });
            (t, f)
        }
        BinaryOp::Gt => {
            // x > c → true: x ∈ [c+1, +∞], false: x ∈ [-∞, c]
            // When c = i64::MAX, c+1 overflows → true branch is impossible (Bottom).
            let t = c.checked_add(1).map_or(Bottom, |l| Bounded {
                lo: l,
                hi: i64::MAX,
            });
            let f = Bounded {
                lo: i64::MIN,
                hi: c,
            };
            (t, f)
        }
        BinaryOp::GtEq => {
            // x >= c → true: x ∈ [c, +∞], false: x ∈ [-∞, c-1]
            let t = Bounded {
                lo: c,
                hi: i64::MAX,
            };
            // When c = i64::MIN, c-1 overflows → false branch is impossible (Bottom).
            let f = c.checked_sub(1).map_or(Bottom, |h| Bounded {
                lo: i64::MIN,
                hi: h,
            });
            (t, f)
        }
        BinaryOp::Eq => {
            // x == c → true: x = c, false: current range (conservative)
            (Bounded { lo: c, hi: c }, x_range)
        }
        BinaryOp::NotEq => {
            // x != c → true: current range (conservative), false: x = c
            (x_range, Bounded { lo: c, hi: c })
        }
        _ => return vec![], // not a comparison operator
    };

    // Intersect with the current range to only narrow, never widen.
    let true_range = x_range.meet(true_constraint);
    let false_range = x_range.meet(false_constraint);

    vec![BranchRefinement {
        var: x,
        true_range,
        false_range,
    }]
}

#[cfg(test)]
mod tests;
