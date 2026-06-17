//! Unit pins for the RL-1 + RL-2 iter-consume dead-thread orphaned-inc elision
//! `compute_iter_consume_dead_thread_orphan_inc`.
//!
//! Positive pin — a fresh list iter-consumed via an `Invoke @iter` terminator and
//! dead-thread Jump-arg'd to a dead post-loop block-param suppresses the lineage.
//! Negative pins — a post-loop genuine read of the thread declines; a list NOT
//! iter-consumed declines. Spec: Annex E §AIMS RL-1 + RL-2.

use ori_ir::{Name, StringInterner};
use ori_types::Idx;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::aims::contract::MemoryContract;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind,
    ValueRepr,
};

use super::compute_iter_consume_dead_thread_orphan_inc;

/// A bodyless block with the given ID + terminator (the trivial thread blocks).
fn jmp_block(id: u32, target: u32, args: Vec<ArcVarId>) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(target),
            args,
        },
    }
}

/// Build the `for_yield` iter-source shape (block IDs == vec positions):
/// ```text
/// bb0: %3 = Construct List; %5 = %3;
///      Invoke @iter(%5 [own]) normal bb1 unwind bb2
/// bb1: Jump bb3(%3)                  ; xs dead-thread into the loop body param
/// bb2: Resume
/// bb3: (%10: [int])                  ; loop-carried xs alias (DEAD)
///      Branch %cond ? bb4 : bb5
/// bb4: Jump bb3(%10)                 ; back-edge re-thread
/// bb5: (%11: [int])                  ; post-loop xs alias (DEAD unless read)
///      Return %ret
/// ```
/// `thread_read` makes bb5 genuinely READ %11 (the negative live-thread case).
fn for_yield_func(interner: &StringInterner, thread_read: bool) -> ArcFunction {
    let v = ArcVarId::new;
    let iter = interner.intern("iter");
    let cond = v(16);
    let ret = v(49);

    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Construct {
                dst: v(3),
                ty: Idx::from_raw(1),
                ctor: CtorKind::ListLiteral,
                args: Vec::new(),
            },
            ArcInstr::Let {
                dst: v(5),
                ty: Idx::from_raw(1),
                value: ArcValue::Var(v(3)),
            },
        ],
        terminator: ArcTerminator::Invoke {
            dst: v(6),
            ty: Idx::from_raw(2),
            func: iter,
            args: vec![v(5)],
            arg_ownership: vec![crate::ir::ArgOwnership::Owned],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
            mono_instance_id: None,
        },
    };
    let bb1 = jmp_block(1, 3, vec![v(3)]); // xs dead-thread into the loop body param
    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Resume,
    };
    let bb3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![(v(10), Idx::from_raw(1))],
        body: vec![ArcInstr::Let {
            dst: cond,
            ty: Idx::from_raw(9),
            value: ArcValue::Literal(crate::ir::LitValue::Bool(true)),
        }],
        terminator: ArcTerminator::Branch {
            cond,
            then_block: ArcBlockId::new(4),
            else_block: ArcBlockId::new(6),
        },
    };
    let bb4 = jmp_block(4, 3, vec![v(10)]); // back-edge re-thread
                                            // bb5 post-loop: optionally READ %11 (the negative live-thread case) via a
                                            // borrowed @len-like Apply; otherwise %11 is dead.
    let bb5_body = if thread_read {
        vec![ArcInstr::Apply {
            dst: v(50),
            ty: Idx::from_raw(8),
            func: interner.intern("len"),
            args: vec![v(11)],
            arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
            mono_instance_id: None,
        }]
    } else {
        Vec::new()
    };
    let bb5 = ArcBlock {
        id: ArcBlockId::new(5),
        params: vec![(v(11), Idx::from_raw(1))],
        body: bb5_body,
        terminator: ArcTerminator::Return { value: ret },
    };
    // bb6: the loop-exit forward edge threading %10 into bb5.%11 (mirrors the
    // real for-loop lowering's post-loop branch-split block).
    let bb6 = jmp_block(6, 5, vec![v(10)]);

    let mut var_reprs = vec![ValueRepr::Scalar; 51];
    var_reprs[3] = ValueRepr::RcPointer; // xs root
    var_reprs[5] = ValueRepr::RcPointer; // @iter arg alias
    var_reprs[6] = ValueRepr::RcPointer; // iterator
    var_reprs[10] = ValueRepr::RcPointer; // loop-carried dead-thread param
    var_reprs[11] = ValueRepr::RcPointer; // post-loop dead-thread param

    ArcFunction {
        var_types: (0..51).map(|i| Idx::from_raw(i + 1)).collect(),
        var_reprs,
        blocks: vec![bb0, bb1, bb2, bb3, bb4, bb5, bb6],
        entry: ArcBlockId::new(0),
        name: interner.intern("main"),
        ..ArcFunction::default()
    }
}

/// Positive pin: the fresh list %3 iter-consumed via `Invoke @iter` + dead-thread
/// Jump-arg'd through %10 (back-edge) -> %11 (dead post-loop) is suppressed
/// (lineage {%3, %5, %10, %11} removed from `owned_vars_needing_rc`), eliding the
/// orphan FRESH-site inc. Reverting leaks (the @iter consume suppresses the
/// scope-exit dec, the orphan inc has no paired dec).
#[test]
fn for_yield_dead_thread_suppresses_orphan_inc() {
    let interner = StringInterner::new();
    let func = for_yield_func(&interner, false);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let owned: FxHashSet<ArcVarId> = [
        ArcVarId::new(3),
        ArcVarId::new(5),
        ArcVarId::new(10),
        ArcVarId::new(11),
    ]
    .into_iter()
    .collect();

    let out = compute_iter_consume_dead_thread_orphan_inc(&func, &contracts, &owned, &interner);

    assert!(
        out.contains(&ArcVarId::new(3)),
        "fresh list root suppressed"
    );
    assert!(
        out.contains(&ArcVarId::new(5)),
        "@iter arg alias suppressed"
    );
    assert!(
        out.contains(&ArcVarId::new(10)),
        "loop-carried dead-thread param suppressed"
    );
    assert!(
        out.contains(&ArcVarId::new(11)),
        "post-loop dead-thread param suppressed"
    );
}

/// Negative pin (live-thread guard): when the post-loop param %11 is genuinely
/// READ (`xs.len()` after the loop — the `str_list_explicit_last_owner` shape), the
/// lineage is NOT dead-threaded -> gate (c) declines (the keep-alive is needed).
#[test]
fn for_yield_live_post_loop_read_declines() {
    let interner = StringInterner::new();
    let func = for_yield_func(&interner, true);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let owned: FxHashSet<ArcVarId> = [
        ArcVarId::new(3),
        ArcVarId::new(5),
        ArcVarId::new(10),
        ArcVarId::new(11),
    ]
    .into_iter()
    .collect();

    let out = compute_iter_consume_dead_thread_orphan_inc(&func, &contracts, &owned, &interner);

    assert!(
        out.is_empty(),
        "a genuine post-loop read of the thread declines (keep-alive needed)"
    );
}

/// Negative pin (not-iter-consumed guard): a fresh list whose `@iter` arg is
/// BORROWED (not owned — not iter-consumed) declines (gate b).
#[test]
fn for_yield_not_iter_consumed_declines() {
    let v = ArcVarId::new;
    let interner = StringInterner::new();
    let iter = interner.intern("iter");
    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst: v(3),
            ty: Idx::from_raw(1),
            ctor: CtorKind::ListLiteral,
            args: Vec::new(),
        }],
        terminator: ArcTerminator::Invoke {
            dst: v(6),
            ty: Idx::from_raw(2),
            func: iter,
            args: vec![v(3)],
            arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(1),
            mono_instance_id: None,
        },
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Resume,
    };
    let mut var_reprs = vec![ValueRepr::Scalar; 7];
    var_reprs[3] = ValueRepr::RcPointer;
    var_reprs[6] = ValueRepr::RcPointer;
    let func = ArcFunction {
        var_types: (0..7).map(|i| Idx::from_raw(i + 1)).collect(),
        var_reprs,
        blocks: vec![bb0, bb1],
        entry: ArcBlockId::new(0),
        name: interner.intern("main"),
        ..ArcFunction::default()
    };
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(3)].into_iter().collect();

    let out = compute_iter_consume_dead_thread_orphan_inc(&func, &contracts, &owned, &interner);
    assert!(
        out.is_empty(),
        "borrowed @iter arg (not iter-consumed) declines"
    );
}
