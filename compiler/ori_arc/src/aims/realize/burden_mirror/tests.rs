//! Tests for the coexistence-mirror scope-exit `BurdenDec` reconciliation.
//!
//! Positive pin: a `burden_emitted` var with a `BurdenInc` and an `RcDec` but
//! no whole-var `BurdenDec` receives a mirrored `BurdenDec` adjacent to the
//! `RcDec`. Negative pins: an already-decked var (dedup) and a non-tracked var
//! are left unchanged.

use super::reconcile_burden_ledger;
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcStrategy};
use ori_ir::Name;
use ori_types::Idx;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Single-block `ArcFunction` with `body` and the given `burden_emitted`
/// bitvector. Allocates `burden_emitted.len()` typed var slots.
fn func_with(body: Vec<ArcInstr>, burden_emitted: Vec<bool>) -> ArcFunction {
    let n = u32::try_from(burden_emitted.len()).unwrap_or(u32::MAX);
    let var_types: Vec<Idx> = (0..n).map(Idx::from_raw).collect();
    ArcFunction {
        name: Name::from_raw(1),
        return_type: Idx::from_raw(0),
        var_types,
        burden_emitted,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    }
}

fn count_burden_dec(func: &ArcFunction, var: ArcVarId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var: w } if *w == var))
        .count()
}

#[test]
fn inc_without_dec_mirrors_burden_dec_adjacent_to_rcdec() {
    let mut func = func_with(
        vec![
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::RcDec {
                var: v(1),
                strategy: RcStrategy::FatPointer,
            },
        ],
        vec![false, true],
    );
    reconcile_burden_ledger(&mut func);

    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 3, "one BurdenDec inserted");
    assert!(
        matches!(body[0], ArcInstr::BurdenInc { var } if var == v(1)),
        "BurdenInc preserved first",
    );
    assert!(
        matches!(body[1], ArcInstr::BurdenDec { var } if var == v(1)),
        "BurdenDec mirrored immediately before the RcDec",
    );
    assert!(
        matches!(body[2], ArcInstr::RcDec { var, .. } if var == v(1)),
        "RcDec preserved last",
    );
}

#[test]
fn already_decked_var_is_not_double_mirrored() {
    let mut func = func_with(
        vec![
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::BurdenDec { var: v(1) },
            ArcInstr::RcDec {
                var: v(1),
                strategy: RcStrategy::FatPointer,
            },
        ],
        vec![false, true],
    );
    reconcile_burden_ledger(&mut func);

    assert_eq!(
        count_burden_dec(&func, v(1)),
        1,
        "dedup: a var already carrying a whole-var BurdenDec gets no mirror",
    );
}

#[test]
fn non_burden_tracked_var_rcdec_is_not_mirrored() {
    let mut func = func_with(
        vec![ArcInstr::RcDec {
            var: v(1),
            strategy: RcStrategy::HeapPointer,
        }],
        vec![false, false],
    );
    reconcile_burden_ledger(&mut func);

    assert_eq!(
        count_burden_dec(&func, v(1)),
        0,
        "a var absent from burden_emitted is never mirrored",
    );
    assert_eq!(func.blocks[0].body.len(), 1, "body unchanged");
}

#[test]
fn burden_emitted_flag_with_eliminated_inc_is_not_mirrored() {
    // Regression: Phase 2.5 elimination removed the `BurdenInc` but left
    // `burden_emitted[v1] = true`. The gate must consult the LIVE inc, not the
    // stale flag — mirroring the `RcDec` here would over-decrement (net < 0).
    let mut func = func_with(
        vec![ArcInstr::RcDec {
            var: v(1),
            strategy: RcStrategy::FatPointer,
        }],
        vec![false, true],
    );
    reconcile_burden_ledger(&mut func);
    assert_eq!(
        count_burden_dec(&func, v(1)),
        0,
        "no live BurdenInc → no mirror despite burden_emitted=true",
    );
    assert_eq!(func.blocks[0].body.len(), 1, "body unchanged");
}

fn count_burden_inc(func: &ArcFunction, var: ArcVarId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var: w } if *w == var))
        .count()
}

#[test]
fn dec_without_inc_pairs_a_balancing_burden_inc_before_the_dec() {
    // Dec-surplus class (e.g. an Invoke-result whose FRESH-site inc the walk
    // missed): a whole-var BurdenDec with no BurdenInc gets a balancing
    // BurdenInc immediately before it.
    let mut func = func_with(vec![ArcInstr::BurdenDec { var: v(1) }], vec![false, true]);
    reconcile_burden_ledger(&mut func);

    let body = &func.blocks[0].body;
    assert_eq!(body.len(), 2, "one BurdenInc inserted");
    assert!(
        matches!(body[0], ArcInstr::BurdenInc { var } if var == v(1)),
        "BurdenInc paired immediately before the dec",
    );
    assert!(
        matches!(body[1], ArcInstr::BurdenDec { var } if var == v(1)),
        "BurdenDec preserved",
    );
    assert_eq!(count_burden_inc(&func, v(1)), 1);
    assert_eq!(count_burden_dec(&func, v(1)), 1, "net 0: one inc, one dec");
}

#[test]
fn empty_burden_emitted_is_noop() {
    let mut func = func_with(
        vec![ArcInstr::RcDec {
            var: v(0),
            strategy: RcStrategy::HeapPointer,
        }],
        Vec::new(),
    );
    reconcile_burden_ledger(&mut func);
    assert_eq!(
        func.blocks[0].body.len(),
        1,
        "no-op when nothing is tracked"
    );
}
