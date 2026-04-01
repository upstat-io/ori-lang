//! Widening and narrowing primitives for fixpoint iteration.

use rustc_hash::FxHashMap;

use super::ValueRange;
use ori_arc::ArcVarId;
use ValueRange::{Bottom, Bounded, Top};

/// Widening threshold — start widening after this many iterations.
pub(super) const WIDEN_THRESHOLD: usize = 3;

/// Standard widening: if a bound grew since the previous iteration,
/// push it to infinity. This guarantees termination by ensuring the
/// ascending chain reaches `Top` in at most 2 widening steps per bound.
#[must_use]
/// Widen with optional comparison thresholds.
///
/// Standard VRP widening: when a bound is growing, jump to the NEXT threshold
/// instead of ±MAX. This ensures loop counters converge to their comparison
/// bounds (e.g., `i < 10` → widen to `[0, 10]` instead of `[0, MAX]`).
///
/// Without thresholds, falls back to the classic widening (jump to ±MAX).
pub fn widen_with_thresholds(
    previous: ValueRange,
    current: ValueRange,
    thresholds: &[i64],
) -> ValueRange {
    match (previous, current) {
        (Bottom, x) => x,
        (_, Bottom) => Bottom,
        (Top, _) | (_, Top) => Top,
        (Bounded { lo: p_lo, hi: p_hi }, Bounded { lo: c_lo, hi: c_hi }) => {
            let new_lo = if c_lo < p_lo {
                // Lower bound is shrinking — find the next threshold below c_lo
                thresholds
                    .iter()
                    .rev()
                    .find(|&&t| t <= c_lo)
                    .copied()
                    .unwrap_or(i64::MIN)
            } else {
                c_lo
            };
            let new_hi = if c_hi > p_hi {
                // Upper bound is growing — find the next threshold above c_hi
                thresholds
                    .iter()
                    .find(|&&t| t >= c_hi)
                    .copied()
                    .unwrap_or(i64::MAX)
            } else {
                c_hi
            };
            if new_lo == i64::MIN && new_hi == i64::MAX {
                Top
            } else {
                Bounded {
                    lo: new_lo,
                    hi: new_hi,
                }
            }
        }
    }
}

pub fn widen(previous: ValueRange, current: ValueRange) -> ValueRange {
    widen_with_thresholds(previous, current, &[])
}

/// Narrowing: intersect widened result with transfer function output
/// to recover precision lost during widening. Always tightens (or
/// preserves) the widened bound — never widens it.
#[must_use]
pub fn narrow(widened: ValueRange, computed: ValueRange) -> ValueRange {
    widened.meet(computed)
}

/// Merge or widen a variable's range, returning whether it changed.
pub(super) fn update_range(
    ranges: &mut FxHashMap<ArcVarId, ValueRange>,
    var: ArcVarId,
    new_range: ValueRange,
    iteration: usize,
    thresholds: &[i64],
) -> bool {
    let old = ranges.get(&var).copied().unwrap_or(Bottom);
    let merged = if iteration > WIDEN_THRESHOLD {
        widen_with_thresholds(old, old.join(new_range), thresholds)
    } else {
        old.join(new_range)
    };
    if merged == old {
        false
    } else {
        tracing::trace!(var = var.index(), ?old, ?merged, "range updated");
        ranges.insert(var, merged);
        true
    }
}
