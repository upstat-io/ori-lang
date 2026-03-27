//! Tests for conditional range refinement.

use ori_arc::ir::{ArcInstr, ArcValue, PrimOp};
use ori_arc::ArcVarId;
use ori_ir::BinaryOp;
use rustc_hash::FxHashMap;

use super::*;
use crate::range::ValueRange;
use ValueRange::{Bounded, Top};

/// Helper: build a block body with `v0 = PrimOp(op, [v1, v2])` and call refine.
fn refine_with_op(op: BinaryOp, x_range: ValueRange, c: i64) -> Vec<BranchRefinement> {
    let v0 = ArcVarId::new(0); // condition result
    let v1 = ArcVarId::new(1); // x
    let v2 = ArcVarId::new(2); // constant c

    let body = vec![ArcInstr::Let {
        dst: v0,
        ty: ori_types::Idx::BOOL,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(op),
            args: vec![v1, v2],
        },
    }];

    let mut ranges = FxHashMap::default();
    ranges.insert(v1, x_range);
    ranges.insert(v2, Bounded { lo: c, hi: c });

    refine_from_branch(v0, &ranges, &body)
}

// ─── Lt ────────────────────────────────────────────────────────

/// Semantic pin: x < 100 with x in [0, 200] → true [0, 99], false [100, 200]
#[test]
fn lt_basic() {
    let r = refine_with_op(BinaryOp::Lt, Bounded { lo: 0, hi: 200 }, 100);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].true_range, Bounded { lo: 0, hi: 99 });
    assert_eq!(r[0].false_range, Bounded { lo: 100, hi: 200 });
}

#[test]
fn lt_boundary_i64_min() {
    // x < i64::MIN → true: Bottom (nothing < MIN), false: full range
    let r = refine_with_op(BinaryOp::Lt, Top, i64::MIN);
    assert_eq!(r.len(), 1);
    // c = i64::MIN, c-1 overflows → impossible branch → Bottom.
    // meet(Top, Bottom) = Bottom — correctly marks the branch as unreachable.
    assert_eq!(r[0].true_range, ValueRange::Bottom);
}

// ─── LtEq ──────────────────────────────────────────────────────

#[test]
fn lteq_basic() {
    let r = refine_with_op(BinaryOp::LtEq, Bounded { lo: 0, hi: 200 }, 100);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].true_range, Bounded { lo: 0, hi: 100 });
    assert_eq!(r[0].false_range, Bounded { lo: 101, hi: 200 });
}

#[test]
fn lteq_boundary_i64_max() {
    // x <= i64::MAX → true: [MIN, MAX] (full bounded range), false: c+1 overflows → Bottom
    let r = refine_with_op(BinaryOp::LtEq, Top, i64::MAX);
    assert_eq!(r.len(), 1);
    // meet(Top, Bounded{MIN, MAX}) = Bounded{MIN, MAX} — structurally different from Top
    assert_eq!(
        r[0].true_range,
        Bounded {
            lo: i64::MIN,
            hi: i64::MAX
        }
    );
    // c = i64::MAX, c+1 overflows → impossible false branch → Bottom.
    assert_eq!(r[0].false_range, ValueRange::Bottom);
}

// ─── Gt ────────────────────────────────────────────────────────

#[test]
fn gt_basic() {
    let r = refine_with_op(BinaryOp::Gt, Bounded { lo: 0, hi: 200 }, 100);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].true_range, Bounded { lo: 101, hi: 200 });
    assert_eq!(r[0].false_range, Bounded { lo: 0, hi: 100 });
}

#[test]
fn gt_boundary_i64_max() {
    // x > i64::MAX → true: c+1 overflows → Bottom (nothing > MAX)
    let r = refine_with_op(BinaryOp::Gt, Top, i64::MAX);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].true_range, ValueRange::Bottom);
}

// ─── GtEq ──────────────────────────────────────────────────────

#[test]
fn gteq_basic() {
    let r = refine_with_op(BinaryOp::GtEq, Bounded { lo: 0, hi: 200 }, 100);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].true_range, Bounded { lo: 100, hi: 200 });
    assert_eq!(r[0].false_range, Bounded { lo: 0, hi: 99 });
}

#[test]
fn gteq_non_negative_check() {
    // x >= 0 with x in Top → true: [0, MAX], false: [MIN, -1]
    let r = refine_with_op(BinaryOp::GtEq, Top, 0);
    assert_eq!(r.len(), 1);
    assert_eq!(
        r[0].true_range,
        Bounded {
            lo: 0,
            hi: i64::MAX
        }
    );
    assert_eq!(
        r[0].false_range,
        Bounded {
            lo: i64::MIN,
            hi: -1
        }
    );
}

/// Semantic pin (TPR-03-013): x >= `i64::MIN` → true: full range, false: Bottom
#[test]
fn gteq_boundary_i64_min() {
    // x >= i64::MIN is always true → false branch is impossible → Bottom
    let r = refine_with_op(BinaryOp::GtEq, Top, i64::MIN);
    assert_eq!(r.len(), 1);
    assert_eq!(
        r[0].true_range,
        Bounded {
            lo: i64::MIN,
            hi: i64::MAX
        }
    );
    // c = i64::MIN, c-1 overflows → impossible false branch → Bottom
    assert_eq!(r[0].false_range, ValueRange::Bottom);
}

// ─── Eq ────────────────────────────────────────────────────────

#[test]
fn eq_basic() {
    let r = refine_with_op(BinaryOp::Eq, Bounded { lo: 0, hi: 200 }, 100);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].true_range, Bounded { lo: 100, hi: 100 });
    // false is conservative: meet(current, current) = current
    assert_eq!(r[0].false_range, Bounded { lo: 0, hi: 200 });
}

#[test]
fn eq_i64_min() {
    let r = refine_with_op(BinaryOp::Eq, Top, i64::MIN);
    assert_eq!(r.len(), 1);
    assert_eq!(
        r[0].true_range,
        Bounded {
            lo: i64::MIN,
            hi: i64::MIN
        }
    );
}

// ─── NotEq ─────────────────────────────────────────────────────

#[test]
fn noteq_basic() {
    let r = refine_with_op(BinaryOp::NotEq, Bounded { lo: 0, hi: 200 }, 100);
    assert_eq!(r.len(), 1);
    // true is conservative: current range
    assert_eq!(r[0].true_range, Bounded { lo: 0, hi: 200 });
    // false: x == c
    assert_eq!(r[0].false_range, Bounded { lo: 100, hi: 100 });
}

// ─── Edge cases ────────────────────────────────────────────────

#[test]
fn cond_not_in_body_returns_empty() {
    let v0 = ArcVarId::new(0);
    let ranges = FxHashMap::default();
    let body: Vec<ArcInstr> = vec![];
    let r = refine_from_branch(v0, &ranges, &body);
    assert!(r.is_empty());
}

#[test]
fn non_comparison_returns_empty() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    // cond = IsShared(v1) — not a PrimOp comparison
    let body = vec![ArcInstr::IsShared { dst: v0, var: v1 }];
    let ranges = FxHashMap::default();
    let r = refine_from_branch(v0, &ranges, &body);
    assert!(r.is_empty());
}
