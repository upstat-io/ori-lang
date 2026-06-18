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

/// Positive pin (pre-loop borrow-read): a fresh list borrow-read via `@__index`
/// (`xs[0]`) BEFORE the for-loop's `@iter [own]` consume — the borrow-read does
/// not transfer/duplicate, so the value survives move-once into @iter and the
/// dead-thread lineage is still suppressed. Mirrors the
/// `for_yield_str_with_narrowed_int_list` shape (`if xs[0] != 1 ...` before the
/// loop). A NON-param member borrow-read is benign; only a PARAM-member read
/// (the live post-loop thread) declines.
#[test]
fn for_yield_pre_loop_borrow_read_suppresses() {
    let v = ArcVarId::new;
    let interner = StringInterner::new();
    let iter = interner.intern("iter");
    let index = interner.intern("__index");
    // bb0: %3 = Construct List; %5 = %3; %8 = @__index(%5 [borrow]) (xs[0]);
    //      Invoke @iter(%5 [own]) normal bb1 unwind bb2
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
            // xs[0] borrow-read BEFORE the loop (non-param member, benign).
            ArcInstr::Apply {
                dst: v(8),
                ty: Idx::from_raw(8),
                func: index,
                args: vec![v(5), v(7)],
                arg_ownership: vec![
                    crate::ir::ArgOwnership::Borrowed,
                    crate::ir::ArgOwnership::Borrowed,
                ],
                mono_instance_id: None,
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
    let bb1 = jmp_block(1, 3, vec![v(3)]);
    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: vec![],
        terminator: ArcTerminator::Resume,
    };
    // bb3: (%10) dead loop param; Return.
    let bb3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: vec![(v(10), Idx::from_raw(1))],
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: v(8) },
    };
    let mut var_reprs = vec![ValueRepr::Scalar; 11];
    var_reprs[3] = ValueRepr::RcPointer;
    var_reprs[5] = ValueRepr::RcPointer;
    var_reprs[6] = ValueRepr::RcPointer;
    var_reprs[10] = ValueRepr::RcPointer;
    let func = ArcFunction {
        var_types: (0..11).map(|i| Idx::from_raw(i + 1)).collect(),
        var_reprs,
        blocks: vec![bb0, bb1, bb2, bb3],
        entry: ArcBlockId::new(0),
        name: interner.intern("main"),
        ..ArcFunction::default()
    };
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(3), ArcVarId::new(5), ArcVarId::new(10)]
        .into_iter()
        .collect();

    let out = compute_iter_consume_dead_thread_orphan_inc(&func, &contracts, &owned, &interner);
    assert!(
        out.contains(&ArcVarId::new(3)),
        "fresh list root suppressed despite pre-loop xs[0] borrow-read"
    );
    assert!(
        out.contains(&ArcVarId::new(10)),
        "dead loop param suppressed"
    );
}

/// Build the chained-collect shape (`iter_collect_set_str`):
/// ```text
/// bb0: %4 = Apply @__collect_set(%3 [own])   ; fresh {str}, RC=1 (the root)
///      %6 = %4;
///      Apply @iter(%6 [own]) -> %7            ; move-once iter-consume
///      ... (the second collect chains off %7, separate lineage)
///      Jump bb1(%4)                           ; %4 dead-thread into the tail param
/// bb1: (%19: {str})                           ; DEAD post-loop alias (read iff thread_read)
///      Return %ret
/// ```
/// `thread_read` makes bb1 genuinely READ %19 (the negative live-thread case).
fn collect_set_chain_func(interner: &StringInterner, thread_read: bool) -> ArcFunction {
    let v = ArcVarId::new;
    let iter = interner.intern("iter");
    let collect_set = interner.intern("__collect_set");
    let ret = v(41);

    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            // %4 = @__collect_set(%3 [own]) — fresh {str} root (NOT a Construct).
            ArcInstr::Apply {
                dst: v(4),
                ty: Idx::from_raw(2),
                func: collect_set,
                args: vec![v(3)],
                arg_ownership: vec![crate::ir::ArgOwnership::Owned],
                mono_instance_id: None,
            },
            ArcInstr::Let {
                dst: v(6),
                ty: Idx::from_raw(2),
                value: ArcValue::Var(v(4)),
            },
            // @iter(%6 [own]) — the move-once iter-consume of the collect result.
            ArcInstr::Apply {
                dst: v(7),
                ty: Idx::from_raw(3),
                func: iter,
                args: vec![v(6)],
                arg_ownership: vec![crate::ir::ArgOwnership::Owned],
                mono_instance_id: None,
            },
        ],
        // %4 dead-thread Jump-arg'd into the tail param %19.
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(4)],
        },
    };
    // bb1 post-loop: optionally READ %19 (negative live-thread); else %19 dead.
    let bb1_body = if thread_read {
        vec![ArcInstr::Apply {
            dst: v(50),
            ty: Idx::from_raw(8),
            func: interner.intern("len"),
            args: vec![v(19)],
            arg_ownership: vec![crate::ir::ArgOwnership::Owned],
            mono_instance_id: None,
        }]
    } else {
        Vec::new()
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v(19), Idx::from_raw(2))],
        body: bb1_body,
        terminator: ArcTerminator::Return { value: ret },
    };

    let mut var_reprs = vec![ValueRepr::Scalar; 51];
    var_reprs[3] = ValueRepr::RcPointer; // collect-set source iterator
    var_reprs[4] = ValueRepr::RcPointer; // fresh {str} root
    var_reprs[6] = ValueRepr::RcPointer; // @iter arg alias
    var_reprs[7] = ValueRepr::RcPointer; // second iterator (separate lineage)
    var_reprs[19] = ValueRepr::RcPointer; // dead post-loop thread param

    ArcFunction {
        var_types: (0..51).map(|i| Idx::from_raw(i + 1)).collect(),
        var_reprs,
        blocks: vec![bb0, bb1],
        entry: ArcBlockId::new(0),
        name: interner.intern("main"),
        ..ArcFunction::default()
    }
}

/// Positive pin (collect-builtin root): a fresh `@__collect_set` result
/// move-once iter-consumed and dead-thread Jump-arg'd to a DEAD tail param is
/// suppressed (lineage {%4, %6, %19} removed from `owned_vars_needing_rc`),
/// eliding the orphan funding inc — the `iter_collect_set_str` floor cell.
/// Reverting (Construct-only root set) leaks the intermediate Set (the @iter
/// consume suppresses the scope-exit dec, the funding inc has no paired dec).
#[test]
fn collect_set_chain_dead_thread_suppresses_orphan_inc() {
    let interner = StringInterner::new();
    let func = collect_set_chain_func(&interner, false);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(4), ArcVarId::new(6), ArcVarId::new(19)]
        .into_iter()
        .collect();

    let out = compute_iter_consume_dead_thread_orphan_inc(&func, &contracts, &owned, &interner);
    assert!(
        out.contains(&ArcVarId::new(4)),
        "fresh @__collect_set result root suppressed"
    );
    assert!(
        out.contains(&ArcVarId::new(6)),
        "@iter arg alias suppressed"
    );
    assert!(
        out.contains(&ArcVarId::new(19)),
        "dead post-loop thread param suppressed"
    );
}

/// Negative pin (collect-builtin root, live thread): when the post-iter thread
/// param %19 is GENUINELY READ at an owned position (the collected Set survives
/// downstream — stored / returned / re-consumed), the lineage is NOT
/// dead-threaded -> gate (c)/(d) declines (the funding inc is load-bearing).
/// Guards against eliding a collect result that genuinely survives (dead-end
/// #191 surface for the collect-builtin family).
#[test]
fn collect_set_chain_live_thread_declines() {
    let interner = StringInterner::new();
    let func = collect_set_chain_func(&interner, true);
    let contracts: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(4), ArcVarId::new(6), ArcVarId::new(19)]
        .into_iter()
        .collect();

    let out = compute_iter_consume_dead_thread_orphan_inc(&func, &contracts, &owned, &interner);
    assert!(
        out.is_empty(),
        "a genuine owned-position read of the collect-result thread declines (keep-alive needed)"
    );
}

/// Build the for-yield-result shape whose root is a direct `@ori_list_take`
/// builtin `Apply` (the loop-accumulator finalizer — a FRESH rc=1 buffer), NOT a
/// `Construct` literal. Mirrors `for_yield_func` otherwise:
/// ```text
/// bb0: %3 = Apply @ori_list_take(%2 [borrow]); %5 = %3;
///      Invoke @iter(%5 [own/borrow]) normal bb1 unwind bb2
/// bb1..bb6: identical dead-thread carry as for_yield_func.
/// ```
/// `iter_consumed` controls the `@iter` arg ownership (Owned = iter-consumed,
/// Borrowed = the not-iter-consumed over-fire boundary).
fn for_yield_take_func(interner: &StringInterner, iter_consumed: bool) -> ArcFunction {
    let v = ArcVarId::new;
    let iter = interner.intern("iter");
    let take = interner.intern("ori_list_take");
    let cond = v(16);
    let ret = v(49);
    let iter_own = if iter_consumed {
        crate::ir::ArgOwnership::Owned
    } else {
        crate::ir::ArgOwnership::Borrowed
    };

    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Apply {
                dst: v(3),
                ty: Idx::from_raw(1),
                func: take,
                args: vec![v(2)],
                arg_ownership: vec![crate::ir::ArgOwnership::Borrowed],
                mono_instance_id: None,
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
            arg_ownership: vec![iter_own],
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(2),
            mono_instance_id: None,
        },
    };
    let bb1 = jmp_block(1, 3, vec![v(3)]);
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
    let bb4 = jmp_block(4, 3, vec![v(10)]);
    let bb5 = ArcBlock {
        id: ArcBlockId::new(5),
        params: vec![(v(11), Idx::from_raw(1))],
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: ret },
    };
    let bb6 = jmp_block(6, 5, vec![v(10)]);

    let mut var_reprs = vec![ValueRepr::Scalar; 51];
    var_reprs[3] = ValueRepr::RcPointer; // ori_list_take result root
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

/// Positive pin (for-yield-result root): the `@ori_list_take` finalizer result %3
/// (FRESH rc=1, the `for d in dirs yield d` → `for d in copy do` shape) iter-
/// consumed + dead-threaded is suppressed — the surplus FRESH-site inc is elided.
/// Reverting the take-root admission (`ORI_DISABLE_ITER_CONSUME_FOR_YIELD_TAKE_ROOT=1`)
/// restores the +1 leak (the all-unit-enum `narrowing::test_for_yield_all_unit_enum`
/// AOT cell). The Phase-6 `mark_lineage_rebalance_removals` `touches_loop` gate
/// DEFERS this loop-carried lineage; this back-edge-aware Phase-5 scan owns it.
/// Spec: Annex E §AIMS RL-1 + RL-2.
#[test]
fn for_yield_list_take_root_dead_thread_suppresses_orphan_inc() {
    let interner = StringInterner::new();
    let func = for_yield_take_func(&interner, true);
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
        "ori_list_take for-yield-result root suppressed"
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

/// Negative pin (over-fire boundary): an `@ori_list_take` for-yield-result root
/// whose `@iter` arg is BORROWED (not iter-consumed) declines — gate (b) holds for
/// the take-root exactly as for a `Construct` root. Guards the take-root admission
/// against eliding a take-result that genuinely survives (no `ori_iter_drop`
/// release ⇒ eliding the inc would leak nothing but eliding the dec would UAF).
#[test]
fn for_yield_list_take_root_not_iter_consumed_declines() {
    let interner = StringInterner::new();
    let func = for_yield_take_func(&interner, false);
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
        "a take-root not consumed at @iter [own] declines (gate b — no iter-drop release)"
    );
}
