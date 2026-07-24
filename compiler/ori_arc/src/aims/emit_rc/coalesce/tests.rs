//! Tests for static RC coalescing.

use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcInstr, ArcValue, ArcVarId, LitValue, RcAtomicity, RcStrategy};

use super::coalesce_block_rc;

/// Two adjacent `RcInc(x, 1)` merge into one `RcInc(x, 2)`.
#[test]
fn coalesce_adjacent_inc_inc_same_var() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let strategy = RcStrategy::HeapPointer;

    let mut body = vec![
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        },
    ];

    coalesce_block_rc(&mut body);

    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, count: 2, .. } if var == v0),
        "expected RcInc(v0, 2), got: {:?}",
        body[0]
    );
    assert!(matches!(body[1], ArcInstr::Let { .. }));
}

/// `RcInc(x)` then `RcDec(x)` with no intervening use → both cancelled.
#[test]
fn coalesce_inc_dec_cancellation() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let strategy = RcStrategy::HeapPointer;

    let mut body = vec![
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::RcDec {
            var: v0,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Literal(LitValue::Int(42)),
        },
    ];

    coalesce_block_rc(&mut body);

    // RcInc and RcDec cancel; only Let remains.
    assert_eq!(body.len(), 1, "body: {body:?}");
    assert!(matches!(body[0], ArcInstr::Let { .. }));
}

/// `RcInc(x)`, `Apply(...)`, `RcDec(x)` → preserved (call barrier).
#[test]
fn no_coalesce_across_call() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let strategy = RcStrategy::HeapPointer;

    let mut body = vec![
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::Apply {
            dst: v1,
            ty: Idx::NONE,
            func: Name::new(0, 0),
            args: vec![],
            arg_ownership: vec![],
            mono_instance_id: None,
        },
        ArcInstr::RcDec {
            var: v0,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
    ];

    coalesce_block_rc(&mut body);

    // Call is a barrier — inc flushed before call, dec preserved after.
    assert_eq!(body.len(), 3, "body: {body:?}");
    assert!(matches!(body[0], ArcInstr::RcInc { var, count: 1, .. } if var == v0),);
    assert!(matches!(body[1], ArcInstr::Apply { .. }));
    assert!(matches!(body[2], ArcInstr::RcDec { var, .. } if var == v0));
}

/// `RcInc(x)`, `Let(y, x)`, `RcDec(x)` → preserved (alias/use barrier).
#[test]
fn no_coalesce_across_alias() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let strategy = RcStrategy::HeapPointer;

    let mut body = vec![
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        },
        ArcInstr::RcDec {
            var: v0,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
    ];

    coalesce_block_rc(&mut body);

    // Let uses v0 — acts as barrier. Inc flushed before Let, Dec after.
    assert_eq!(body.len(), 3, "body: {body:?}");
    assert!(matches!(body[0], ArcInstr::RcInc { var, count: 1, .. } if var == v0),);
    assert!(matches!(body[1], ArcInstr::Let { .. }));
    assert!(matches!(body[2], ArcInstr::RcDec { var, .. } if var == v0));
}

/// `RcInc(x); RcInc(x); RcDec(x)` → `RcInc(x, 1)` (net +1).
#[test]
fn coalesce_net_effect_multiple_ops() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let strategy = RcStrategy::HeapPointer;

    let mut body = vec![
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::RcDec {
            var: v0,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        },
    ];

    coalesce_block_rc(&mut body);

    // Net effect: +2 - 1 = +1. Flushed before Let (which uses v0).
    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, count: 1, .. } if var == v0),
        "expected RcInc(v0, 1), got: {:?}",
        body[0]
    );
    assert!(matches!(body[1], ArcInstr::Let { .. }));
}

/// Merged `RcInc` preserves `RcStrategy` from operands.
#[test]
fn coalesce_preserves_strategy() {
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    let strategy = RcStrategy::FatPointer;

    let mut body = vec![
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::RcInc {
            var: v0,
            count: 1,
            strategy,
            atomicity: crate::ir::RcAtomicity::default_atomic(),
        },
        ArcInstr::Let {
            dst: v1,
            ty: Idx::NONE,
            value: ArcValue::Var(v0),
        },
    ];

    coalesce_block_rc(&mut body);

    assert_eq!(body.len(), 2, "body: {body:?}");
    match &body[0] {
        ArcInstr::RcInc {
            var,
            count,
            strategy: s,
            ..
        } => {
            assert_eq!(*var, v0);
            assert_eq!(*count, 2);
            assert_eq!(*s, RcStrategy::FatPointer, "strategy must be preserved");
        }
        other => panic!("expected RcInc, got: {other:?}"),
    }
}

/// A compatible non-atomic physical plan remains non-atomic after merging.
#[test]
fn coalesce_preserves_non_atomic_plan() {
    let var = ArcVarId::new(0);
    let mut body = vec![
        ArcInstr::RcInc {
            var,
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::NonAtomic,
        },
        ArcInstr::RcInc {
            var,
            count: 2,
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::NonAtomic,
        },
    ];

    coalesce_block_rc(&mut body);

    assert_eq!(body.len(), 1, "body: {body:?}");
    assert!(matches!(
        body[0],
        ArcInstr::RcInc {
            var: actual,
            count: 3,
            atomicity: RcAtomicity::NonAtomic,
            ..
        } if actual == var
    ));
}

/// Incompatible physical synchronization modes are a coalescing fence.
#[test]
fn coalesce_does_not_merge_or_cancel_mixed_atomicity() {
    let var = ArcVarId::new(0);
    let mut body = vec![
        ArcInstr::RcInc {
            var,
            count: 1,
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::NonAtomic,
        },
        ArcInstr::RcDec {
            var,
            strategy: RcStrategy::HeapPointer,
            atomicity: RcAtomicity::Atomic,
        },
    ];

    coalesce_block_rc(&mut body);

    assert_eq!(body.len(), 2, "body: {body:?}");
    assert!(matches!(
        body[0],
        ArcInstr::RcInc {
            var: actual,
            count: 1,
            atomicity: RcAtomicity::NonAtomic,
            ..
        } if actual == var
    ));
    assert!(matches!(
        body[1],
        ArcInstr::RcDec {
            var: actual,
            atomicity: RcAtomicity::Atomic,
            ..
        } if actual == var
    ));
}
