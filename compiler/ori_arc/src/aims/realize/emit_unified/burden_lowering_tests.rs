//! Phase-7 mechanical burden-lowering tests (probe path).
//!
//! Pins [`super::lower_burden_ops_to_rc`]: under the probe
//! (`predicate_stack_rc_disabled`), surviving whole-var `BurdenInc` /
//! `BurdenDec` lower to real `RcInc` / `RcDec`, while the field-grain
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` variants are
//! left intact for codegen's per-field / per-variant drop glue.
//!
//! RC counts use the SSOT `crate::pipeline::rc_count::count_rc_ops`.

use super::{
    compute_dead_collection_source_releases, compute_dead_iterator_handle_releases,
    compute_dead_owned_collection_releases, compute_elidable_fresh_self_alloc_incs,
    compute_lineage_alloc_aware_net, emit_for_yield_index_consumed_element_rc,
    emit_iter_element_view_iter_consume_keepalive_inc, lower_burden_ops_to_rc,
    relocate_borrowed_terminator_arg_dec_to_edges, suppress_multi_borrow_iter_consume_source_decs,
    IterHandleRelease,
};
use crate::aims::contract::{MemoryContract, ReturnAliasShape};
use crate::aims::lattice::AccessClass;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership,
    CtorKind, PrimOp, ValueRepr,
};
use crate::pipeline::rc_count::count_rc_ops;
use ori_ir::BinaryOp;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Single-block function with `body`, `num_vars` typed slots all `RcPointer`
/// (heap-backed → `RcStrategy::HeapPointer` under the default `Pool`), and a
/// `Return` of var 0. Mirrors the burden-walk precondition that whole-var
/// burden ops only target RC-bearing (non-scalar) vars.
fn rc_pointer_func(num_vars: u32, body: Vec<ArcInstr>) -> ArcFunction {
    let var_types: Vec<Idx> = (0..num_vars).map(|_| Idx::from_raw(0)).collect();
    let var_reprs: Vec<ValueRepr> = (0..num_vars).map(|_| ValueRepr::RcPointer).collect();
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    }
}

/// Count burden ops of every kind remaining in `func`.
fn burden_count(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| {
            matches!(
                i,
                ArcInstr::BurdenInc { .. }
                    | ArcInstr::BurdenDec { .. }
                    | ArcInstr::BurdenDecPartial { .. }
                    | ArcInstr::BurdenDecField { .. }
                    | ArcInstr::BurdenDecVariant { .. }
            )
        })
        .count()
}

#[test]
fn lower_whole_var_burden_inc_dec_becomes_real_rc() {
    let pool = Pool::default();
    let mut func = rc_pointer_func(
        1,
        vec![
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );

    // Semantic pin (a): pre-lowering there are zero real RC ops — only burden
    // markers exist.
    let before = count_rc_ops(&func);
    assert_eq!(before.inc, 0, "no real RcInc before lowering");
    assert_eq!(before.dec, 0, "no real RcDec before lowering");

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    // Semantic pin (b): the burden path produced real, balanced RC ops.
    let after = count_rc_ops(&func);
    assert_eq!(after.inc, 1, "BurdenInc lowered to exactly one real RcInc");
    assert_eq!(after.dec, 1, "BurdenDec lowered to exactly one real RcDec");
    // Negative pin: no whole-var burden marker survives the lowering.
    assert_eq!(
        burden_count(&func),
        0,
        "whole-var burden ops fully consumed by Phase-7 lowering"
    );
}

#[test]
fn lower_preserves_field_grain_burden_variants() {
    let pool = Pool::default();
    // BurdenDecPartial / BurdenDecField / BurdenDecVariant carry field/variant
    // info codegen consumes directly (instr_dispatch.rs); Phase-7 lowering must
    // NOT rewrite them to a whole-var RcDec (that would double-drop the
    // moved-out / surviving fields).
    let mut func = rc_pointer_func(
        2,
        vec![
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDecPartial {
                var: v(0),
                skip_fields: vec![0],
            },
            ArcInstr::BurdenDecField {
                base: v(1),
                field: 0,
            },
            ArcInstr::BurdenDecVariant { var: v(1) },
        ],
    );

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    // The whole-var BurdenInc lowered; the three field-grain variants survive
    // verbatim for codegen.
    let body = &func.blocks[0].body;
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, count: 1, .. } if var == v(0)),
        "whole-var BurdenInc lowered to RcInc"
    );
    assert!(
        matches!(&body[1], ArcInstr::BurdenDecPartial { var, skip_fields } if *var == v(0) && skip_fields == &[0]),
        "BurdenDecPartial preserved verbatim — NOT rewritten to RcDec"
    );
    assert!(
        matches!(body[2], ArcInstr::BurdenDecField { base, field: 0 } if base == v(1)),
        "BurdenDecField preserved verbatim"
    );
    assert!(
        matches!(body[3], ArcInstr::BurdenDecVariant { var } if var == v(1)),
        "BurdenDecVariant preserved verbatim"
    );

    // Negative pin: the three field-grain variants still count as burden ops.
    assert_eq!(
        burden_count(&func),
        3,
        "field-grain burden variants are NOT consumed by whole-var lowering"
    );
    // Negative pin: lowering whole-var ops to a whole-var RcDec for v(1) would
    // be a double-drop — assert NO whole-var RcDec was synthesized for v(1).
    let v1_whole_rcdec = func
        .blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == v(1)))
        .count();
    assert_eq!(
        v1_whole_rcdec, 0,
        "no spurious whole-var RcDec synthesized for a field-grain-only var"
    );
}

#[test]
fn lower_leaves_scalar_repr_burden_in_place() {
    let pool = Pool::default();
    // Scalars never carry RcStrategy. emit_burden_ops filters them, but the
    // RE-2 backstop must leave a (contract-violating) scalar burden op in place
    // rather than synthesize unsound RC.
    let mut func = rc_pointer_func(1, vec![ArcInstr::BurdenInc { var: v(0) }]);
    func.var_reprs[0] = ValueRepr::Scalar;

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    let after = count_rc_ops(&func);
    assert_eq!(
        after.inc, 0,
        "no RcInc synthesized for a scalar-repr burden op"
    );
    assert_eq!(
        burden_count(&func),
        1,
        "scalar-repr burden op left in place (codegen no-ops it)"
    );
}

// --- M3 + Phase-7 elision pins (broad-shape collection-source freeing) ---

/// Single-block reps map where each var is its own rep (no same-alloc aliasing).
fn identity_reps(n: u32) -> FxHashMap<ArcVarId, ArcVarId> {
    (0..n).map(|i| (v(i), v(i))).collect()
}

fn list_construct(dst: ArcVarId, args: Vec<ArcVarId>) -> ArcInstr {
    ArcInstr::Construct {
        dst,
        ty: Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args,
    }
}

/// Semantic pin: a single-reference FRESH self-alloc whose lineage is read-only
/// (no COW-mutation operand) has alloc-aware net == 1 (alloc +1, fresh inc +1,
/// one dec −1 → counting alloc, the fresh inc is the surplus), so its fresh inc
/// is ELIDABLE — eliding restores the alloc-aware balance to 0, freeing the
/// value exactly once. Mirrors `coll_list_index` (the redundant-inc leak).
#[test]
fn elide_redundant_fresh_inc_for_read_only_self_alloc() {
    // %0 = Construct List()  (fresh self-alloc, +1)
    // burden_inc %0          (paired fresh inc, +1)
    // burden_dec %0          (release, −1)  → alloc-aware net = 1
    let func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    let reps = identity_reps(1);
    let net = compute_lineage_alloc_aware_net(&func, &reps, &ori_ir::StringInterner::new());
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(1),
        "read-only single-ref self-alloc lineage nets +1 (redundant fresh inc surplus)"
    );
    let elidable =
        compute_elidable_fresh_self_alloc_incs(&func, &reps, &ori_ir::StringInterner::new());
    assert!(
        elidable.contains(&v(0)),
        "redundant fresh inc of a read-only single-ref self-alloc is elidable"
    );
}

/// Negative pin: a FRESH self-alloc whose lineage flows into a COW-mutation
/// operand (a `PrimOp Binary` with an `RcPtr` operand — list `+`/concat) KEEPS its
/// fresh inc — the COW helper reads the runtime refcount to choose
/// copy-vs-mutate, so the fresh inc is LOAD-BEARING (raises rc ≥ 2 → copy).
/// Eliding it would mutate the shared value in place. Mirrors
/// `coll_list_cow_concat_shared`.
#[test]
fn keep_fresh_inc_for_cow_mutated_self_alloc() {
    // %0 = Construct List()       (fresh self-alloc xs)
    // burden_inc %0
    // %1 = Construct List()       (rhs)
    // burden_inc %1
    // %2 = %0 + %1                (COW concat — reads %0's refcount)
    // burden_dec %0 ; burden_dec %1
    let func = rc_pointer_func(
        3,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            list_construct(v(1), Vec::new()),
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::Let {
                dst: v(2),
                ty: Idx::from_raw(0),
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(1) },
        ],
    );
    let reps = identity_reps(3);
    let elidable =
        compute_elidable_fresh_self_alloc_incs(&func, &reps, &ori_ir::StringInterner::new());
    assert!(
        !elidable.contains(&v(0)),
        "COW-mutated self-alloc (list + operand) keeps its load-bearing fresh inc"
    );
    assert!(
        !elidable.contains(&v(1)),
        "the other COW `+` operand also keeps its fresh inc"
    );
}

/// Negative pin (double-free protection): a FRESH self-alloc with a move-alias
/// dec but no paired dup inc has alloc-aware net == 0 (alloc +1, fresh inc +1,
/// TWO decs −2 → the fresh inc balances the move-alias's unpaired dec). Net != 1
/// → the fresh inc is NOT elidable: eliding would net −1 = a double-free.
/// Mirrors `coll_list_length_one`.
#[test]
fn keep_fresh_inc_when_net_not_one_move_alias_dec() {
    // %0 = Construct List()  (fresh self-alloc)
    // burden_inc %0          (+1)
    // burden_dec %0          (−1)  ← the move-alias %1=%0 dec, attributed to rep %0
    // burden_dec %0          (−1)  ← the fresh value's own release
    // net counting alloc = 1 + 1 − 2 = 0 (NOT 1) → keep.
    let func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    let reps = identity_reps(1);
    let net = compute_lineage_alloc_aware_net(&func, &reps, &ori_ir::StringInterner::new());
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "move-alias-dec lineage nets 0 (fresh inc balances the unpaired move dec)"
    );
    let elidable =
        compute_elidable_fresh_self_alloc_incs(&func, &reps, &ori_ir::StringInterner::new());
    assert!(
        !elidable.contains(&v(0)),
        "net != 1 → fresh inc kept (eliding would double-free, net −1)"
    );
}

/// The `for x in coll yield expr` comprehension result is finalized by
/// `ori_list_take` (moves the scratch buffer out → a FRESH owned list at rc=1).
/// `ori_list_take` is a no-contract builtin `Apply`, so `fresh_site_burden_inc`
/// gives its result a fresh-site inc; when the result is dup-indexed the surplus
/// fresh inc nets the lineage to +1. `fresh_self_alloc_dst` MUST recognize the
/// `ori_list_take` result as a fresh self-alloc so the net-keyed elision removes
/// that surplus inc — mirrors `for_yield_str_to_lengths`.
#[test]
fn elide_fresh_inc_for_for_yield_list_take_result_dup_indexed() {
    let interner = ori_ir::StringInterner::new();
    let take = interner.intern("ori_list_take");
    // %0 = ori_list_take(scratch)   (FRESH self-alloc — the for_yield result)
    // burden_inc %0                 (fresh-site inc)
    // burden_inc %1                 (dup-alias index 0)
    // burden_dec %1
    // burden_inc %2                 (dup-alias index 1)
    // burden_dec %0                 (move-alias dec on the result)
    // burden_dec %2
    // alloc-aware net counting alloc = 1 + 3 inc − 3 dec = 1 → elide the fresh inc.
    let func = rc_pointer_func(
        3,
        vec![
            ArcInstr::Apply {
                dst: v(0),
                ty: Idx::from_raw(0),
                func: take,
                args: vec![v(1)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenInc { var: v(1) },
            ArcInstr::BurdenDec { var: v(1) },
            ArcInstr::BurdenInc { var: v(2) },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(2) },
        ],
    );
    // The two index aliases fold into the result's lineage (move-alias reps).
    let reps: FxHashMap<ArcVarId, ArcVarId> = [(v(0), v(0)), (v(1), v(0)), (v(2), v(0))]
        .into_iter()
        .collect();
    let net = compute_lineage_alloc_aware_net(&func, &reps, &interner);
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(1),
        "dup-indexed list_take result nets +1 (the surplus fresh inc over alloc)"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(&func, &reps, &interner);
    assert!(
        elidable.contains(&v(0)),
        "the for_yield list_take result's surplus fresh inc is elidable (net == 1)"
    );
}

/// Negative pin: a SINGLE-use `ori_list_take` result (the `for_yield` result used
/// once, e.g. `result.length()`) nets 0 counting alloc (alloc +1, fresh inc +1,
/// one dec −2 via the move-alias) — eliding would double-free. The result must
/// NOT be elided when its lineage net is not exactly 1.
#[test]
fn keep_fresh_inc_for_single_use_list_take_result() {
    let interner = ori_ir::StringInterner::new();
    let take = interner.intern("ori_list_take");
    // %0 = ori_list_take(scratch)
    // burden_inc %0   (+1)
    // burden_dec %0   (−1)  ← the single borrowed-use dec
    // net counting alloc = 1 + 1 − 1 = 1? No: a single use has ONE dec, so the
    // result's own release is the lone dec → net 1 + 1 − 1 = 1. The elision is
    // CORRECT here too — a single-use result also carries the surplus fresh inc.
    // The genuine keep case is when a move-alias adds a SECOND dec (net 0). Pin
    // that shape: a move-alias dec makes the result net 0 → keep.
    let func = rc_pointer_func(
        2,
        vec![
            ArcInstr::Apply {
                dst: v(0),
                ty: Idx::from_raw(0),
                func: take,
                args: vec![v(1)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    let reps = identity_reps(2);
    let net = compute_lineage_alloc_aware_net(&func, &reps, &interner);
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "list_take result with a move-alias dec nets 0 → fresh inc is load-bearing"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(&func, &reps, &interner);
    assert!(
        !elidable.contains(&v(0)),
        "net 0 list_take result keeps its fresh inc (eliding would double-free)"
    );
}

/// JUMP-THREADED `ori_list_take` result (the `for_yield_*_two_call` shape): the
/// fresh-result `%0` flows through a Jump-arg → block-param POSITIONAL rename
/// (`%0` → `%2`) before its TRUE release `BurdenDec %3` fires on a `Let`-alias of
/// the threaded block-param. The fresh-site `BurdenInc %0` / paired `BurdenDec
/// %0` net 0 at the alloc site; the downstream `BurdenDec %3` is the lone
/// genuine release of the lineage.
///
/// `compute_same_alloc_reps` EXCLUDES the Jump-phi edge BY DESIGN, so without
/// phi-aware net attribution rep `%0`'s net counts only the fresh-site pair
/// (alloc(+1) + inc(+1) − dec(−1) = +1) and MISSES the threaded `BurdenDec %3`
/// (attributed to a different rep). The +1 verdict would elide the fresh inc,
/// leaving alloc(+1) − dec %0(−1) − dec %3(−1) = −1 = a double-free.
///
/// The phi-aware lineage net MUST thread the Jump-arg → block-param edge so the
/// whole single-allocation chain (`%0` ≡ `%2` ≡ `%3`) nets 0 (the fresh inc is
/// load-bearing — it balances the SECOND release), keeping the inc.
/// Spec: Annex E §AIMS RL-1 (`rl1_emits_inc = !incElidable`) + RL-2
/// (`RL2_release_exactly_once`).
#[test]
fn keep_fresh_inc_for_jump_threaded_list_take_result() {
    let interner = ori_ir::StringInterner::new();
    let take = interner.intern("ori_list_take");
    // bb0: %0 = ori_list_take(%1)   (FRESH self-alloc, the for_yield result)
    //      burden_inc %0            (fresh-site inc)
    //      burden_dec %0            (paired premature dec — net-0 with the inc)
    //      Jump bb1(%0)             (Jump-arg → block-param phi: %0 → %2)
    // bb1: (%2)
    //      %3 = Let Var(%2)         (the `%44 = %30` Let alias of the threaded param)
    //      burden_dec %3            (the lineage's TRUE single release)
    //      Return %3
    let func = ArcFunction {
        var_types: vec![Idx::from_raw(0); 4],
        var_reprs: vec![ValueRepr::RcPointer; 4],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Apply {
                        dst: v(0),
                        ty: Idx::from_raw(0),
                        func: take,
                        args: vec![v(1)],
                        arg_ownership: vec![ArgOwnership::Borrowed],
                        mono_instance_id: None,
                    },
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(2), Idx::from_raw(0))],
                body: vec![
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::from_raw(0),
                        value: ArcValue::Var(v(2)),
                    },
                    ArcInstr::BurdenDec { var: v(3) },
                ],
                terminator: ArcTerminator::Return { value: v(3) },
            },
        ],
        ..Default::default()
    };
    // `same_alloc_reps` (phi-EXCLUDED, as the production path supplies): the Let
    // alias %3 ↔ %2 unions, but the Jump-phi %0 → %2 does NOT, so %0 is its own
    // rep and %3/%2 are a separate rep.
    let reps: FxHashMap<ArcVarId, ArcVarId> = [(v(0), v(0)), (v(2), v(2)), (v(3), v(2))]
        .into_iter()
        .collect();
    let net = compute_lineage_alloc_aware_net(&func, &reps, &interner);
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "jump-threaded list_take result nets 0 once the phi-threaded downstream \
         release is attributed to the alloc rep (the fresh inc balances the 2nd dec)"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(&func, &reps, &interner);
    assert!(
        !elidable.contains(&v(0)),
        "the jump-threaded result keeps its fresh inc (eliding would net −1 = \
         double-free of the for_yield result)"
    );
}

/// Count `BurdenInc`/`BurdenDec` on a specific var.
fn burden_inc_dec_count(func: &ArcFunction, var: ArcVarId) -> (usize, usize) {
    let mut inc = 0;
    let mut dec = 0;
    for b in &func.blocks {
        for i in &b.body {
            match i {
                ArcInstr::BurdenInc { var: v } if *v == var => inc += 1,
                ArcInstr::BurdenDec { var: v } if *v == var => dec += 1,
                _ => {}
            }
        }
    }
    (inc, dec)
}

/// `for_yield` INDEX-consumed result: `emit_for_yield_index_consumed_element_rc`
/// emits a yield-element `BurdenInc` on the `ori_list_push(scratch, w [own])`
/// element value AND a `BurdenDec` on the `@__index(result [borrow], _) -> view`
/// extracted view. RcPtr/FatVal reprs only. The result is NOT returned + NOT
/// iter-consumed → eligible.
#[test]
fn for_yield_index_consumed_emits_yield_element_inc_and_index_view_dec() {
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("ori_list_push");
    let take = interner.intern("ori_list_take");
    let index = interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());
    // %1 = scratch (ori_list_new result — modeled as a plain RcPtr var)
    // ori_list_push(%1 [borrow], %2 [own], %3 [borrow])  ← %2 = yielded element
    // %4 = ori_list_take(%1)                              ← the result
    // %6 = __index(%4 [borrow], %5 [borrow])              ← indexed view
    // Return %0 (a scalar — result NOT returned)
    let mut func = ArcFunction {
        var_types: vec![Idx::from_raw(0); 8],
        var_reprs: vec![
            ValueRepr::Scalar,    // %0 scalar return
            ValueRepr::RcPointer, // %1 scratch
            ValueRepr::FatValue,  // %2 yielded element (str)
            ValueRepr::Scalar,    // %3 elem_size
            ValueRepr::RcPointer, // %4 result
            ValueRepr::Scalar,    // %5 index
            ValueRepr::FatValue,  // %6 indexed view (str)
            ValueRepr::Scalar,    // %7 push () result
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: v(7),
                    ty: Idx::from_raw(0),
                    func: push,
                    args: vec![v(1), v(2), v(3)],
                    arg_ownership: vec![
                        ArgOwnership::Borrowed,
                        ArgOwnership::Owned,
                        ArgOwnership::Borrowed,
                    ],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: v(4),
                    ty: Idx::from_raw(0),
                    func: take,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: v(6),
                    ty: Idx::from_raw(0),
                    func: index,
                    args: vec![v(4), v(5)],
                    arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    };
    let reps = identity_reps(8);
    emit_for_yield_index_consumed_element_rc(&mut func, &Pool::default(), &interner, &reps);
    let (yield_inc, _) = burden_inc_dec_count(&func, v(2));
    assert_eq!(
        yield_inc, 1,
        "yielded element pushed into an index-consumed result gets a duplicating BurdenInc"
    );
    let (_, view_dec) = burden_inc_dec_count(&func, v(6));
    assert_eq!(
        view_dec, 1,
        "the __index-extracted view of an index-consumed result gets a release BurdenDec"
    );
}

/// Negative: a RETURNED `for_yield` result transfers ownership to the caller, so the
/// yielded element gets NO inc (the caller decides element RC — the
/// `yield_identity_str_list_two_calls` `clone_list` shape).
#[test]
fn for_yield_returned_result_skips_yield_element_inc() {
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("ori_list_push");
    let take = interner.intern("ori_list_take");
    // ori_list_push(%1 [borrow], %2 [own], %3 [borrow])
    // %4 = ori_list_take(%1)
    // Return %4   ← the RESULT is returned → NOT eligible
    let mut func = ArcFunction {
        var_types: vec![Idx::from_raw(0); 5],
        var_reprs: vec![
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
            ValueRepr::FatValue,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: v(0),
                    ty: Idx::from_raw(0),
                    func: push,
                    args: vec![v(1), v(2), v(3)],
                    arg_ownership: vec![
                        ArgOwnership::Borrowed,
                        ArgOwnership::Owned,
                        ArgOwnership::Borrowed,
                    ],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: v(4),
                    ty: Idx::from_raw(0),
                    func: take,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: v(4) },
        }],
        ..Default::default()
    };
    let reps = identity_reps(5);
    emit_for_yield_index_consumed_element_rc(&mut func, &Pool::default(), &interner, &reps);
    let (yield_inc, _) = burden_inc_dec_count(&func, v(2));
    assert_eq!(
        yield_inc, 0,
        "a RETURNED for_yield result transfers element ownership to the caller — no yield-element inc"
    );
}

// Step-B' dead-collection-source freeing (`compute_dead_collection_source_releases`)
//
// The leaked OWNED collection-source of a `m.keys()` / `s.split()` shape: the
// map/set/str is BORROWED by the conversion builtin (survives), loop-carried via
// Jump-args into a post-loop block param, then dead there without a freeing dec.
// The pass emits ONE whole-var `BurdenDec` on the dead-block-param (Phase-7
// lowers to `RcDec { HeapPointer }` → `ori_buffer_rc_dec`, which walks
// `elem_dec_fn`). These pins build the structural shape with a real `Pool` so the
// conversion-source receiver resolves to a `Tag::Map`/`Set`/`List`/`Str`.

use ori_ir::Name;

/// Build the map_keys_str-shaped func (with a real `Pool` carrying a Map type):
/// bb0: `%0 = Construct Map`; `Invoke @keys(%0 [borrow]) -> %1` normal bb1.
/// bb1: `Jump bb2(%0, %1)` (loop-carry both source map + keys result).
/// bb2 (loop header, params `%2:map, %3:keys`): `Branch -> bb3 (body) | bb4 (exit)`.
/// bb3 (body): `Jump bb2(%2, %3)` (back-edge — keeps the lineage live in-loop).
/// bb4 (exit, params `%4:map, %5:keys`): the DEAD SINK — `%4` (map) arrives dead
///   and is never freed; `%5` (keys) is consumed `@iter`-style (owned) so it is
///   excluded. `Return %scalar`.
#[expect(
    clippy::too_many_lines,
    reason = "multi-block ArcFunction IR fixture — the 5-block CFG (entry / \
              iter-create / loop-header / body / dead-sink-exit) is one cohesive \
              test input; splitting it fragments the fixture's CFG shape"
)]
fn map_keys_loop_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let map_ty = pool.map(Idx::INT, Idx::STR);
    let keys_ty = pool.list(Idx::STR);
    let keys_name = interner.intern("keys");
    let iter_name = interner.intern("iter");
    // 6 vars: %0 map src, %1 keys result, %2/%3 loop-header params,
    // %4/%5 exit params, %6 scalar return, %7 iter-drop scalar result.
    let var_types = vec![
        map_ty,
        keys_ty,
        map_ty,
        keys_ty,
        map_ty,
        keys_ty,
        Idx::INT,
        Idx::INT,
    ];
    let mut var_reprs = vec![ValueRepr::RcPointer; 6];
    var_reprs.push(ValueRepr::Scalar); // %6
    var_reprs.push(ValueRepr::Scalar); // %7
    let map_p = |n: u32| (v(n), Idx::INT); // param decl (ty unused by the pass)
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            // bb0: Construct Map %0, Invoke @keys(%0 [borrow]) -> %1.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Construct {
                    dst: v(0),
                    ty: map_ty,
                    ctor: CtorKind::MapLiteral,
                    args: Vec::new(),
                }],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: keys_ty,
                    func: keys_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(5),
                },
            },
            // bb1: consume keys via `@iter(%1 [own])`, Jump loop-header with both.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Apply {
                    dst: v(7),
                    ty: Idx::INT,
                    func: iter_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(2),
                    args: vec![v(0), v(1)],
                },
            },
            // bb2: loop header (params %2 map, %3 keys), branch body/exit-transit.
            ArcBlock {
                id: ArcBlockId::new(2),
                params: vec![map_p(2), map_p(3)],
                body: Vec::new(),
                terminator: ArcTerminator::Branch {
                    cond: v(6),
                    then_block: ArcBlockId::new(3),
                    else_block: ArcBlockId::new(6),
                },
            },
            // bb3: loop body — back-edge re-passing both (keeps lineage live).
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(2),
                    args: vec![v(2), v(3)],
                },
            },
            // bb4: loop EXIT — DEAD SINK for the map lineage (%4 arrives dead,
            // Jump-fed by bb6 so the jump-threaded rep connects %2→%4).
            ArcBlock {
                id: ArcBlockId::new(4),
                params: vec![map_p(4), map_p(5)],
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(6) },
            },
            // bb5: unwind sink.
            ArcBlock {
                id: ArcBlockId::new(5),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
            // bb6: exit-transit — Jumps the loop params into the dead-sink bb4
            // (mirrors the real loop-exit edge `Jump bb_sink(...%map...)`).
            ArcBlock {
                id: ArcBlockId::new(6),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(4),
                    args: vec![v(2), v(3)],
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_collection_source_frees_borrowed_map_at_loop_exit() {
    // The map source (%0 → %2 → %4) is borrowed by `@keys`, loop-carried, and
    // dead at the exit block bb4 — the pass emits exactly ONE freeing BurdenDec
    // on the exit-block map param (%4). Jump-threaded net at bb4 is +1 (the
    // Construct's alloc unbalanced by any release on the normal path).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = map_keys_loop_func(&mut pool, &interner);
    let releases = compute_dead_collection_source_releases(&func, &pool, &interner);
    // Exactly one release: the map param at bb4 (block 4).
    assert_eq!(
        releases.len(),
        1,
        "exactly one dead-collection-source release (the leaked map at the loop exit); got {releases:?}",
    );
    let (block_idx, var) = releases[0];
    assert_eq!(
        block_idx, 4,
        "release at the loop-EXIT block bb4 (the dead sink)"
    );
    assert_eq!(
        var,
        v(4),
        "frees the map exit-block param %4 (the dead-at-entry source)"
    );
}

#[test]
fn dead_collection_source_excludes_iter_consumed_keys_result() {
    // Negative pin: the keys RESULT (%1 → %3 → %5) is consumed by `@iter(%1
    // [own])` (owned) → it is iterator-managed (freed by `ori_iter_drop`), NOT a
    // leaked source. The pass MUST NOT emit a dec on the keys lineage (%5) — that
    // would double-free the buffer the iterator drop owns.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = map_keys_loop_func(&mut pool, &interner);
    let releases = compute_dead_collection_source_releases(&func, &pool, &interner);
    assert!(
        releases
            .iter()
            .all(|&(_, var)| var != v(5) && var != v(3) && var != v(1)),
        "no release on the @iter-consumed keys-result lineage (%1/%3/%5); got {releases:?}",
    );
}

/// Build a for-loop-managed collection func with NO conversion-builtin borrow:
/// `%0 = Construct List`; `@iter(%0 [own]) -> %1`; loop; `ori_iter_drop`. The
/// list is consumed owned by `@iter` (iterator-managed), never borrowed by a
/// conversion builtin — the pass must emit NOTHING (it is freed by the iterator
/// drop; a dead-source dec would double-free — the for-loop-cluster guard).
fn for_loop_managed_list_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::STR);
    let iter_name = interner.intern("iter");
    let drop_name = interner.intern("ori_iter_drop");
    let var_types = vec![list_ty, Idx::INT, list_ty, Idx::INT, Idx::INT];
    let mut var_reprs = vec![ValueRepr::RcPointer]; // %0
    var_reprs.push(ValueRepr::Scalar); // %1 iter handle
    var_reprs.push(ValueRepr::RcPointer); // %2 loop param (list)
    var_reprs.push(ValueRepr::Scalar); // %3 drop result
    var_reprs.push(ValueRepr::Scalar); // %4 return
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: list_ty,
                        ctor: CtorKind::ListLiteral,
                        args: Vec::new(),
                    },
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::INT,
                        func: iter_name,
                        args: vec![v(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![v(0)],
                },
            },
            // bb1: loop exit param %2 (the list), drops the iterator. The list
            // arrives but is owned-consumed by `@iter` upstream → excluded.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(v(2), Idx::INT)],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::INT,
                    func: drop_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                }],
                terminator: ArcTerminator::Return { value: v(4) },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_collection_source_skips_for_loop_managed_list() {
    // Negative pin (the for-loop-cluster guard): a for-loop-managed list consumed
    // by `@iter` (owned) and freed by `ori_iter_drop` is NOT a
    // conversion-borrowed source — zero releases (a dec here double-frees).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = for_loop_managed_list_func(&mut pool, &interner);
    let releases = compute_dead_collection_source_releases(&func, &pool, &interner);
    assert!(
        releases.is_empty(),
        "for-loop-managed (iter-consumed) list gets NO dead-source dec; got {releases:?}",
    );
}

// Dead OWNED-COLLECTION / mutation-result freeing
// (`compute_dead_owned_collection_releases`).
//
// A FRESH owned collection bound as a body-local (read-only `let m = {..};
// m.contains_key(..)` or a mutation RESULT `let ys = xs.sort()`), last-used at a
// BORROWED builtin position then dead at function scope exit, leaks its allocation
// under sole-emitter lowering (the duplicating-use fresh-site `BurdenInc` + the
// per-path scope-exit `BurdenDec` net the EXPLICIT ops to 0, leaving the alloc
// `+1` unreleased). The pass emits ONE additional whole-var `BurdenDec` at the
// alloc-aware-net-positive last-use sink. These pins build the shape with a real
// `Pool` so the receiver resolves to a `Tag::Map`/`List`.

/// Build the read-only-map leak shape: `%0 = Construct Map(non-empty)`;
/// `%1 = %0`; `burden_inc %1`; `burden_dec %1`; `Invoke @contains_key(%1
/// [borrow], %key [borrow]) -> %2` normal bb1 unwind bb2. bb1: `Return %scalar`.
/// The map `%0`/`%1` is borrowed-read then dead at scope exit — alloc-aware net
/// `+1` (the Construct alloc + the dup-use inc, unbalanced by the single dec) at
/// the last-use sink bb0, so the pass frees `%1` at the END of bb0.
fn read_only_map_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let map_ty = pool.map(Idx::INT, Idx::INT);
    let contains_name = interner.intern("contains_key");
    // %0 map, %1 map alias, %2 bool result, %3 scalar return, %4 scalar key.
    let var_types = vec![map_ty, map_ty, Idx::INT, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    // Non-empty Construct (the alloc the candidate owns).
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: map_ty,
                        ctor: CtorKind::MapLiteral,
                        args: vec![v(4)],
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: map_ty,
                        value: ArcValue::Var(v(0)),
                    },
                    // The dup-use fresh-site inc + the single scope-exit dec
                    // (net 0 on explicit ops; the alloc +1 leaks pre-cure).
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::BurdenDec { var: v(1) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::INT,
                    func: contains_name,
                    args: vec![v(1), v(4)],
                    arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(3) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_owned_collection_frees_read_only_map_at_scope_exit() {
    // The owned map (%0 → %1) is borrowed by `@contains_key`, dead at scope exit —
    // alloc-aware net `+1` at the last-use sink bb0. The pass emits exactly ONE
    // freeing dec on the map's live SSA value (%1) at bb0.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = read_only_map_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(&func, &pool, &interner);
    assert_eq!(
        releases.len(),
        1,
        "exactly one dead-owned-collection release (the leaked map at scope exit); got {releases:?}",
    );
    let (block_idx, var) = releases[0];
    assert_eq!(
        block_idx, 0,
        "release at the last-use sink bb0 (the borrowed read block)"
    );
    assert_eq!(
        var,
        v(1),
        "frees the map's live SSA value %1 at the borrowed-read sink"
    );
}

/// Build the same map shape but RETURN the map (transfer): the terminator of bb1
/// returns the map `%0` instead of a scalar. A returned collection is an RL-2
/// transfer (caller inherits) — the pass MUST emit nothing.
fn returned_map_func(pool: &mut ori_types::Pool, interner: &ori_ir::StringInterner) -> ArcFunction {
    let mut func = read_only_map_func(pool, interner);
    // Re-type %3 as the map and return it from bb1 (transfer).
    func.var_types[3] = func.var_types[0];
    func.var_reprs[3] = ValueRepr::RcPointer;
    func.blocks[1].body = vec![ArcInstr::Let {
        dst: v(3),
        ty: func.var_types[0],
        value: ArcValue::Var(v(0)),
    }];
    func.blocks[1].terminator = ArcTerminator::Return { value: v(3) };
    func
}

#[test]
fn dead_owned_collection_skips_returned_map() {
    // Negative pin: the map is RETURNED (RL-2 transfer — the caller inherits the
    // release). The pass MUST NOT free it (a dec here double-frees with the
    // caller's release).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = returned_map_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(&func, &pool, &interner);
    assert!(
        releases.is_empty(),
        "a RETURNED (transferred) collection gets NO scope-exit dec; got {releases:?}",
    );
}

/// Build a map passed to a USER function: `%0 = Construct Map`; `Apply
/// @user_fn(%0 [borrow]) -> %2`; `Return scalar`. A collection passed to a
/// non-builtin call is the callee's concern (the arg ownership flips at Phase 7) —
/// the pass MUST emit nothing.
fn map_to_user_fn_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let map_ty = pool.map(Idx::INT, Idx::INT);
    // A non-builtin user function name (not in any builtin ownership set).
    let user_name = interner.intern("my_user_helper_fn_zzz");
    let var_types = vec![map_ty, Idx::INT, Idx::INT];
    let var_reprs = vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: map_ty,
                    ctor: CtorKind::MapLiteral,
                    args: vec![v(2)],
                },
                ArcInstr::BurdenInc { var: v(0) },
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: user_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::BurdenDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_owned_collection_skips_user_function_arg() {
    // Negative pin (the cow_push_use_after / user-call guard): a collection passed
    // to a NON-BUILTIN user function is the callee's concern (a "borrowed"
    // Phase-6.8 arg may lower to an owned transfer) — the pass MUST emit nothing.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = map_to_user_fn_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(&func, &pool, &interner);
    assert!(
        releases.is_empty(),
        "a collection passed to a user function gets NO scope-exit dec; got {releases:?}",
    );
}

/// Build a CHAINED COW-mutation: `%0 = Construct List(%5)`; `%1 = @push(%0, %6)`;
/// `%2 = @push(%1, %7)`; `@length(%2 [borrow])`; `Return scalar`. The SECOND push's
/// receiver `%1` is the FIRST push RESULT (not a `Construct`), so the chain tail
/// `%2` is freeable only when the fresh-local-equivalence transitive closure marks
/// `%1` fresh-local-equivalent. `%0` and `%1` are owned-consumed by the next push
/// (excluded); `%2` is the borrowed-read scope-exit sink.
fn push_chain_list_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let push_name = interner.intern("push");
    let length_name = interner.intern("length");
    // %0 Construct, %1 push1, %2 push2, %3 bool, %4 scalar return, %5/%6/%7 scalar elems.
    let var_types = vec![
        list_ty,
        list_ty,
        list_ty,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
        Idx::INT,
    ];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: list_ty,
                        ctor: CtorKind::ListLiteral,
                        args: vec![v(5)],
                    },
                    // %1 = @push(%0, %6) — receiver %0 owned-consumed.
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: list_ty,
                        func: push_name,
                        args: vec![v(0), v(6)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                    // %2 = @push(%1, %7) — receiver %1 is the first push RESULT.
                    ArcInstr::Apply {
                        dst: v(2),
                        ty: list_ty,
                        func: push_name,
                        args: vec![v(1), v(7)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                    // The chain tail %2 carries the dup-use fresh inc + a scope-exit
                    // dec (net 0 on explicit ops; alloc +1 leaks pre-cure).
                    ArcInstr::BurdenInc { var: v(2) },
                    ArcInstr::BurdenDec { var: v(2) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(3),
                    ty: Idx::INT,
                    func: length_name,
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(4) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_owned_collection_frees_cow_mutator_chain_tail() {
    // The fresh-local-equivalence transitive closure over a COW-mutator chain: the
    // second push's receiver is the first push RESULT, so the chain tail %2 is the
    // freeable scope-exit value. Exactly ONE release, on %2 (not %0 / %1 — both
    // owned-consumed by the next push).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = push_chain_list_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(&func, &pool, &interner);
    assert_eq!(
        releases.len(),
        1,
        "exactly one release on the COW-mutator chain TAIL; got {releases:?}",
    );
    let (block_idx, var) = releases[0];
    assert_eq!(
        block_idx, 0,
        "release at the chain tail's borrowed-read sink bb0"
    );
    assert_eq!(
        var,
        v(2),
        "frees the chain TAIL %2 (not the owned-consumed %0/%1)"
    );
}

/// Build a collection-CONVERSION result: `%0 = Construct Map(%4, %5)`; `%1 =
/// @values(%0 [borrow]) -> [int]`; `@length(%1 [borrow])`; `Return scalar`. The
/// `@values` RESULT `%1` is a fresh owned list the runtime allocates from the map;
/// it is borrowed-read then dead at scope exit — a freeable conversion result. The
/// map SOURCE `%0` is the dedicated conversion-source pass's domain.
fn values_result_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let map_ty = pool.map(Idx::INT, Idx::INT);
    let list_ty = pool.list(Idx::INT);
    let values_name = interner.intern("values");
    let length_name = interner.intern("length");
    // %0 map, %1 values result list, %2 bool, %3 scalar return, %4/%5 scalar k/v.
    let var_types = vec![map_ty, list_ty, Idx::INT, Idx::INT, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: map_ty,
                        ctor: CtorKind::MapLiteral,
                        args: vec![v(4), v(5)],
                    },
                    // %1 = @values(%0 [borrow]) — fresh owned list result.
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: list_ty,
                        func: values_name,
                        args: vec![v(0)],
                        arg_ownership: vec![ArgOwnership::Borrowed],
                        mono_instance_id: None,
                    },
                    // The result %1 carries the dup-use fresh inc + a scope-exit dec
                    // (net 0 on explicit ops; alloc +1 leaks pre-cure).
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::BurdenDec { var: v(1) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::INT,
                    func: length_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(3) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_owned_collection_frees_conversion_result() {
    // A collection-conversion result (`m.values()`) is a fresh owned collection,
    // borrowed-read then dead at scope exit. Exactly ONE release, on the result %1.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = values_result_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(&func, &pool, &interner);
    assert_eq!(
        releases.len(),
        1,
        "exactly one release on the conversion RESULT; got {releases:?}",
    );
    assert_eq!(releases[0].1, v(1), "frees the @values result list %1");
}

/// Single-block-then-successors function where the map SOURCE `%0` carries an
/// INLINE scope-exit `BurdenDec %0` placed before the borrowed conversion
/// terminator `Invoke @values(%0 [borrow]) normal bb1 unwind bb2`. Models the
/// `map_values` no-loop shape: the inline dec frees the map before the
/// conversion reads it. `%0` is dead on both successors (only the result `%1` is
/// used). `func` for [`relocate_conversion_source_terminator_dec_to_edges`].
fn conversion_source_inline_dec_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let map_ty = pool.map(Idx::INT, Idx::INT);
    let list_ty = pool.list(Idx::INT);
    let values_name = interner.intern("values");
    // %0 map source, %1 values result, %2 scalar return, %3/%4 scalar k/v.
    let var_types = vec![map_ty, list_ty, Idx::INT, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: map_ty,
                        ctor: CtorKind::MapLiteral,
                        args: vec![v(3), v(4)],
                    },
                    // The misplaced inline scope-exit dec for the map source.
                    ArcInstr::BurdenDec { var: v(0) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: list_ty,
                    func: values_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(2) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn conversion_source_terminator_dec_relocated_to_edges() {
    // The inline `BurdenDec %0` before `Invoke @values(%0 [borrow])` is removed
    // from the conversion block and re-emitted at the front of BOTH successors
    // (RL-4 edge release) — the map is freed AFTER the borrowed conversion reads
    // it, exactly once per path.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = conversion_source_inline_dec_func(&mut pool, &interner);
    let inline_dec_pre = |f: &ArcFunction| {
        f.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)))
    };
    assert!(
        inline_dec_pre(&func),
        "precondition: inline dec on %0 present"
    );

    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    assert!(
        !inline_dec_pre(&func),
        "the inline pre-terminator BurdenDec %0 must be removed",
    );
    let normal_dec = matches!(
        func.blocks[1].body.first(),
        Some(ArcInstr::BurdenDec { var }) if *var == v(0)
    );
    let unwind_dec = matches!(
        func.blocks[2].body.first(),
        Some(ArcInstr::BurdenDec { var }) if *var == v(0)
    );
    assert!(
        normal_dec,
        "the release must land at the front of the normal successor"
    );
    assert!(
        unwind_dec,
        "the release must land at the front of the unwind successor"
    );
}

#[test]
fn conversion_source_relocation_skips_owned_receiver() {
    // Negative pin: when the conversion receiver is at an OWNED position (the
    // conversion CONSUMES it — not the borrowed-source shape), the relocation
    // leaves the inline dec untouched (no edge release, no removal). Guards
    // against over-firing on a transfer.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = conversion_source_inline_dec_func(&mut pool, &interner);
    // Flip the receiver to an OWNED position.
    if let ArcTerminator::Invoke { arg_ownership, .. } = &mut func.blocks[0].terminator {
        arg_ownership[0] = ArgOwnership::Owned;
    }

    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline_dec = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(
        inline_dec,
        "owned-receiver conversion: inline dec stays (no relocation)"
    );
    assert!(
        func.blocks[1].body.is_empty(),
        "no edge release on owned receiver"
    );
    assert!(
        func.blocks[2].body.is_empty(),
        "no unwind release on owned receiver"
    );
}

/// `Invoke @<callee>(%0 [borrow])` with an inline `BurdenDec %0`, mirroring the
/// conversion-source func but with a NON-conversion `callee` name — for the
/// contract-gated escape relocation tests. `%0` is an owned map; `%1` the result.
fn borrowed_arg_inline_dec_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    callee: &str,
) -> ArcFunction {
    let map_ty = pool.map(Idx::INT, Idx::INT);
    let var_types = vec![map_ty, Idx::INT, Idx::INT, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: map_ty,
                        ctor: CtorKind::MapLiteral,
                        args: vec![v(3), v(4)],
                    },
                    ArcInstr::BurdenDec { var: v(0) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::INT,
                    func: interner.intern(callee),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(2) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

fn one_param_contract(
    access: AccessClass,
    return_alias: Option<ReturnAliasShape>,
) -> MemoryContract {
    let mut c = MemoryContract::conservative(1);
    c.params[0].access = access;
    c.params[0].return_alias = return_alias;
    c.params[0].return_payload_contains_param = false;
    c
}

/// Contract for a Borrowed-access scalar-returning user fn whose arg is used ONCE
/// (`access=Borrowed`, `cardinality=Once`, `consumption=Affine`) — the CASE (b)
/// boundary: the iter-consume (`@sum_values`) and borrow-read (`@sum_list` fold)
/// callees present this IDENTICAL contract, so neither is relocated.
fn once_affine_borrow_contract() -> MemoryContract {
    use crate::aims::lattice::{Cardinality, Consumption};
    let mut c = MemoryContract::conservative(1);
    c.params[0].access = AccessClass::Borrowed;
    c.params[0].cardinality = Cardinality::Once;
    c.params[0].consumption = Consumption::Affine;
    c.params[0].return_alias = None;
    c.params[0].return_payload_contains_param = false;
    c
}

#[test]
fn borrowed_arg_dec_kept_inline_for_once_affine_borrow_user_fn() {
    // CASE (b) negative pin: a Borrowed-access scalar-returning user fn with
    // `cardinality=Once`, `consumption=Affine` is NOT relocated. This contract is
    // shared by the iter-consume case (`@sum_values`: `for x in coll do` → `@iter`
    // frees the collection → caller SHOULD suppress) AND the borrow-read case
    // (`@sum_list`: `xs.fold(..)` borrows, does NOT free → caller MUST keep). No
    // contract field records the inward-transfer-into-iter-and-drop, so relocating
    // would over-fire on the fold case (the `rc_matrix`/`fat_matrix` regressions).
    // Keep the inline dec until the next-leaf per-param signal lands.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_arg_inline_dec_func(&mut pool, &interner, "sum_list");
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(interner.intern("sum_list"), once_affine_borrow_contract());

    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(
        inline,
        "once-affine-borrow user fn: inline dec stays (case-b boundary)"
    );
    assert!(
        func.blocks[1].body.is_empty(),
        "no normal-edge release on once-affine-borrow user fn"
    );
    assert!(
        func.blocks[2].body.is_empty(),
        "no unwind-edge release on once-affine-borrow user fn"
    );
}

/// Like [`borrowed_arg_inline_dec_func`] but with the Invoke result `dst`
/// repr flipped to `repr` (default scalar; pass `RcPointer` for a heap result).
fn borrowed_arg_inline_dec_func_with_dst(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    callee: &str,
    dst_repr: ValueRepr,
) -> ArcFunction {
    let mut func = borrowed_arg_inline_dec_func(pool, interner, callee);
    func.var_reprs[1] = dst_repr;
    func
}

#[test]
fn borrowed_arg_dec_relocated_to_both_edges_for_builtin_scalar_read() {
    // A BUILTIN borrowing read (`@len`) whose scalar result cannot alias the
    // receiver: the source survives the call, dead on each successor → relocate the
    // inline dec to BOTH the normal AND unwind edges (RL-4). `len` is in the
    // builtin set so placement is `Both`.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_arg_inline_dec_func(&mut pool, &interner, "len");
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();

    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(!inline, "builtin scalar read: inline dec removed");
    assert!(
        matches!(func.blocks[1].body.first(), Some(ArcInstr::BurdenDec { var }) if *var == v(0)),
        "release on the normal successor edge",
    );
    assert!(
        matches!(func.blocks[2].body.first(), Some(ArcInstr::BurdenDec { var }) if *var == v(0)),
        "release on the unwind successor edge",
    );
}

#[test]
fn borrowed_arg_dec_relocated_to_unwind_only_for_owned_consume_contract() {
    // A callee whose contract upgraded the arg to Owned (a true consume at an owned
    // position — the Lean `ownParamsUsingArgs` transfer): the callee owns/frees it
    // on the normal path; the caller releases ONLY on the unwind edge (RL-2 transfer
    // no-dec on normal + RL-4 unwind cleanup). Scalar result → escape-safe.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_arg_inline_dec_func(&mut pool, &interner, "consume_fn");
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(
        interner.intern("consume_fn"),
        one_param_contract(AccessClass::Owned, None),
    );

    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(!inline, "owned-consume contract: inline dec removed");
    assert!(
        func.blocks[1].body.is_empty(),
        "NO release on the normal edge — the callee consumes/frees on normal return",
    );
    assert!(
        matches!(func.blocks[2].body.first(), Some(ArcInstr::BurdenDec { var }) if *var == v(0)),
        "release on the unwind edge only",
    );
}

#[test]
fn borrowed_arg_dec_kept_inline_for_borrowed_access_user_fn_scalar() {
    // Negative pin (guardrail-clean narrowing): a non-builtin scalar-returning
    // callee with Borrowed-access contract is NOT relocated — the iter-consume case
    // (`@sum_values`) and the borrow-mutate-through-borrow case (`@check`+`push`)
    // are contract-indistinguishable (both `access=Borrowed`, `may_deallocate=false`),
    // so relocating the former over-fires on the latter (the COW-through-borrow
    // blocker — a borrowed param COW-realloc'd in place then re-read). Keep the
    // inline dec.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_arg_inline_dec_func(&mut pool, &interner, "borrow_fn");
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(
        interner.intern("borrow_fn"),
        one_param_contract(AccessClass::Borrowed, None),
    );

    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(
        inline,
        "borrowed-access user fn: inline dec stays (escape-over-fire guard)"
    );
    assert!(
        func.blocks[1].body.is_empty(),
        "no normal-edge release on borrowed-access user fn"
    );
    assert!(
        func.blocks[2].body.is_empty(),
        "no unwind-edge release on borrowed-access user fn"
    );
}

#[test]
fn borrowed_arg_dec_kept_inline_for_nonscalar_result() {
    // Negative pin (the escape-over-fire guard): a non-conversion
    // callee whose result is a HEAP value (`@get_first -> str` returning
    // `match xs.first() { Some(s) -> s }`, an element VIEW into the arg) may alias
    // the receiver — `return_alias` does NOT capture the `first()`-then-unwrap
    // shape, so the SCALAR-RESULT gate is what excludes it. Keep the inline dec.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_arg_inline_dec_func_with_dst(
        &mut pool,
        &interner,
        "get_first",
        ValueRepr::RcPointer,
    );
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();

    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(
        inline,
        "non-scalar (heap) result: inline dec stays (escape-over-fire guard)"
    );
    assert!(
        func.blocks[1].body.is_empty(),
        "no normal-edge release on non-scalar result"
    );
    assert!(
        func.blocks[2].body.is_empty(),
        "no unwind-edge release on non-scalar result"
    );
}

#[test]
fn borrowed_arg_dec_kept_inline_for_return_alias_contract() {
    // Negative pin: even with a scalar result, a contract that records a
    // `return_alias = Some(Project)` on the arg keeps the inline dec (defensive —
    // the callee threads the arg into the result).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_arg_inline_dec_func(&mut pool, &interner, "alias_fn");
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(
        interner.intern("alias_fn"),
        one_param_contract(
            AccessClass::Borrowed,
            Some(ReturnAliasShape::Project { field: 0 }),
        ),
    );

    relocate_borrowed_terminator_arg_dec_to_edges(&mut func, &interner, &contracts);

    let inline = func.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    assert!(inline, "return-alias contract: inline dec stays");
    assert!(
        func.blocks[1].body.is_empty(),
        "no normal-edge release on return-alias contract"
    );
    assert!(
        func.blocks[2].body.is_empty(),
        "no unwind-edge release on return-alias contract"
    );
}

// In-function ITERATOR-HANDLE freeing pins (`compute_dead_iterator_handle_releases`).
//
// An `@iter`-family result is a FRESH owned `Tag::DoubleEndedIterator` handle with
// no RC header (the source buffer moved into the iterator state). It carries no
// `BURDEN_TABLE` burden, so the Phase-5 walk emits zero ops on it — under sole-
// emitter lowering it leaks. The pass emits one whole-var `BurdenDec` on the bare
// handle at its dead-at-scope-exit sink (lowered `RcStrategy::Iterator` →
// `ori_iter_drop`). SEED-not-reuse excludes for-loop-managed (`ori_iter_drop`-arg)
// and returned handles.

/// Build the bare-unused-iterator leak shape: `%0 = Construct List(non-empty)`;
/// `%1 = Apply @iter(%0 [own])` (a `DoubleEndedIterator` handle); `Return %2`. The
/// handle `%1` is born + dies in bb0, never re-read — its only reference is the
/// `@iter` dst, dead at scope exit. The pass frees `%1` at the END of bb0.
fn bare_iter_handle_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let iter_ty = pool.double_ended_iterator(Idx::INT);
    let iter_name = interner.intern("iter");
    // %0 list, %1 iterator handle, %2 scalar return, %3 scalar elem.
    let var_types = vec![list_ty, iter_ty, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: v(0),
                    ty: list_ty,
                    ctor: CtorKind::ListLiteral,
                    args: vec![v(3)],
                },
                ArcInstr::Apply {
                    dst: v(1),
                    ty: iter_ty,
                    func: iter_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_iterator_handle_frees_bare_unused_iter_at_scope_exit() {
    // The fresh owned iterator handle (%1, `@iter` result) is dead at scope exit —
    // it carries no RC burden so the Phase-5 walk emits nothing; the pass frees it
    // with exactly one `BurdenDec` on %1 at the END of bb0 (lowered to
    // `RcDec [Iterator]` = `ori_iter_drop`).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = bare_iter_handle_func(&mut pool, &interner);
    let releases = compute_dead_iterator_handle_releases(&func, &pool, &interner);
    assert_eq!(
        releases.len(),
        1,
        "exactly one bare-iterator-handle release at scope exit; got {releases:?}",
    );
    match &releases[0] {
        IterHandleRelease::EndOfBody { block_idx, var } => {
            assert_eq!(*block_idx, 0, "release at the defining block bb0");
            assert_eq!(*var, v(1), "frees the iterator handle %1");
        }
        other @ IterHandleRelease::SuccessorFront { .. } => {
            panic!("expected EndOfBody release at bb0; got {other:?}")
        }
    }
}

#[test]
fn dead_iterator_handle_skips_for_loop_managed_handle() {
    // Negative pin: a handle CONSUMED by an `@ori_iter_drop` Apply is for-loop-
    // managed — the loop lowering already frees it on every exit path
    // (`compute_iter_drop_handle_lineages` holds it). The pass MUST emit nothing
    // (a dec here double-frees the iterator-owned buffer).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = bare_iter_handle_func(&mut pool, &interner);
    // Append `@ori_iter_drop(%1 [own])` (the for-loop drop) before the return,
    // mirroring a for-loop's explicit handle drop.
    let iter_drop_name = interner.intern("ori_iter_drop");
    func.blocks[0].body.push(ArcInstr::Apply {
        dst: v(2),
        ty: Idx::UNIT,
        func: iter_drop_name,
        args: vec![v(1)],
        arg_ownership: vec![ArgOwnership::Owned],
        mono_instance_id: None,
    });
    let releases = compute_dead_iterator_handle_releases(&func, &pool, &interner);
    assert!(
        releases.is_empty(),
        "a for-loop-managed (ori_iter_drop'd) handle gets NO extra dec; got {releases:?}",
    );
}

#[test]
fn dead_iterator_handle_skips_returned_handle() {
    // Negative pin: a RETURNED iterator handle is an RL-2 transfer (the caller
    // inherits the release). The pass MUST emit nothing (a dec here double-frees
    // with the caller's release).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = bare_iter_handle_func(&mut pool, &interner);
    // Return the handle %1 instead of the scalar.
    func.blocks[0].terminator = ArcTerminator::Return { value: v(1) };
    let releases = compute_dead_iterator_handle_releases(&func, &pool, &interner);
    assert!(
        releases.is_empty(),
        "a RETURNED iterator handle gets NO scope-exit dec; got {releases:?}",
    );
}

/// Single-param `MemoryContract` with `iter_consumes` set as given (Borrowed
/// access, the iter-consume callee's contract shape).
fn iter_consume_contract(iter_consumes: bool) -> MemoryContract {
    let mut c = MemoryContract::conservative(1);
    c.params[0].access = AccessClass::Borrowed;
    c.params[0].iter_consumes = iter_consumes;
    c
}

/// Build the multi-borrow iter-consume `@main` shape: `%0 = Construct List(%5)`;
/// `BurdenInc %0`; `%2 = %0`; `Apply @cons(%2 [borrow])`; `BurdenDec %0`;
/// `%3 = %0`; `Apply @cons(%3 [borrow])`; terminator branches on `%0`-derived
/// scalar (keeping `%0` live across the first call). `@cons` is the named
/// iter-consuming callee. `n_calls` controls how many consuming calls (2 or 1).
fn multi_borrow_iter_consume_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    n_calls: usize,
) -> (ArcFunction, ori_ir::Name) {
    let list_ty = pool.list(Idx::INT);
    let cons_name = interner.intern("cons_iter_consumer_zzz");
    // %0 source list; %1/%2.. scalar results + move-aliases; element %elem.
    // Vars: %0 list, %1 scalar result1, %2 alias1, %3 scalar result2, %4 alias2,
    // %5 element scalar, %6 scalar terminator value.
    let var_types = vec![
        list_ty,
        Idx::INT,
        list_ty,
        Idx::INT,
        list_ty,
        Idx::INT,
        Idx::INT,
    ];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    let mut body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: list_ty,
            ctor: CtorKind::ListLiteral,
            args: vec![v(5)],
        },
        ArcInstr::BurdenInc { var: v(0) },
        // call 1: %2 = %0; @cons(%2)
        ArcInstr::Let {
            dst: v(2),
            ty: list_ty,
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::Apply {
            dst: v(1),
            ty: Idx::INT,
            func: cons_name,
            args: vec![v(2)],
            arg_ownership: vec![ArgOwnership::Borrowed],
            mono_instance_id: None,
        },
        // The spurious move-alias source dec the Phase-5 walk emits.
        ArcInstr::BurdenDec { var: v(0) },
    ];
    if n_calls >= 2 {
        // call 2: %4 = %0; @cons(%4)
        body.push(ArcInstr::Let {
            dst: v(4),
            ty: list_ty,
            value: ArcValue::Var(v(0)),
        });
        body.push(ArcInstr::Apply {
            dst: v(3),
            ty: Idx::INT,
            func: cons_name,
            args: vec![v(4)],
            arg_ownership: vec![ArgOwnership::Borrowed],
            mono_instance_id: None,
        });
    }
    let func = ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            // Return scalar %1 — the source %0 is dead after the calls (its
            // multi-borrow keep-alive is the only obligation; the callees free).
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    };
    (func, cons_name)
}

/// Build a function where the `[T]` source `%0` is iter-consumed by `n_calls`
/// INLINE `@iter(arg [own])` protocol-builtin positions (the for-loop iterator
/// signature) — NO user `MemoryContract`. Mirrors [`multi_borrow_iter_consume_func`]
/// but the iter-consume positions are `@iter [own]` Apply calls, exercising the
/// inline-for-loop recognizer path.
fn inline_iter_multi_borrow_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    n_calls: usize,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let iter_name = interner.intern("iter");
    let var_types = vec![
        list_ty,
        Idx::INT,
        list_ty,
        Idx::INT,
        list_ty,
        Idx::INT,
        Idx::INT,
    ];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
    ];
    let mut body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: list_ty,
            ctor: CtorKind::ListLiteral,
            args: vec![v(5)],
        },
        ArcInstr::BurdenInc { var: v(0) },
        // loop 1: %2 = %0; @iter(%2 [own])
        ArcInstr::Let {
            dst: v(2),
            ty: list_ty,
            value: ArcValue::Var(v(0)),
        },
        ArcInstr::Apply {
            dst: v(1),
            ty: Idx::INT,
            func: iter_name,
            args: vec![v(2)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
        // The spurious move-alias source dec the Phase-5 walk emits.
        ArcInstr::BurdenDec { var: v(0) },
    ];
    if n_calls >= 2 {
        // loop 2: %4 = %0; @iter(%4 [own])
        body.push(ArcInstr::Let {
            dst: v(4),
            ty: list_ty,
            value: ArcValue::Var(v(0)),
        });
        body.push(ArcInstr::Apply {
            dst: v(3),
            ty: Idx::INT,
            func: iter_name,
            args: vec![v(4)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        });
    }
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

fn source_dec_count(func: &ArcFunction, src: ArcVarId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == src))
        .count()
}

fn source_inc_count(func: &ArcFunction, src: ArcVarId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == src))
        .count()
}

#[test]
fn multi_borrow_iter_consume_suppresses_source_dec_keeps_keep_alive_inc() {
    // Two iter-consuming `[own]` calls on the same source: the spurious
    // normal-path source `BurdenDec` is removed (the callees free via iter-drop)
    // and exactly (N-1)=1 keep-alive `BurdenInc` survives (RL-1 duplicating use).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (mut func, cons) = multi_borrow_iter_consume_func(&mut pool, &interner, 2);
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(cons, iter_consume_contract(true));

    assert_eq!(
        source_dec_count(&func, v(0)),
        1,
        "precondition: one spurious source dec present"
    );
    suppress_multi_borrow_iter_consume_source_decs(&mut func, &pool, &interner, &contracts);
    assert_eq!(
        source_dec_count(&func, v(0)),
        0,
        "the spurious normal-path source dec must be removed (callees free)"
    );
    // (N-1) = 1 keep-alive inc survives, on the consumed arg of the first call.
    let total_inc = source_inc_count(&func, v(0)) + source_inc_count(&func, v(2));
    assert_eq!(
        total_inc, 1,
        "exactly one keep-alive inc (N-1) on the source lineage; got {total_inc}",
    );
}

#[test]
fn multi_borrow_iter_consume_skips_single_call() {
    // Negative pin (the multi-borrow lower boundary): a SINGLE iter-consuming call
    // (the 6.65 single-borrow `Suppress` shape) must NOT be touched by this pass —
    // emitting a keep-alive inc here would leak (the callee's iter-drop is the sole
    // release).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (mut func, cons) = multi_borrow_iter_consume_func(&mut pool, &interner, 1);
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(cons, iter_consume_contract(true));

    let dec_before = source_dec_count(&func, v(0));
    let inc_before = source_inc_count(&func, v(0));
    suppress_multi_borrow_iter_consume_source_decs(&mut func, &pool, &interner, &contracts);
    assert_eq!(
        source_dec_count(&func, v(0)),
        dec_before,
        "single-call source ops must be untouched (6.65 owns the single-borrow case)"
    );
    assert_eq!(
        source_inc_count(&func, v(0)),
        inc_before,
        "single-call source ops must be untouched"
    );
}

#[test]
fn multi_borrow_iter_consume_skips_borrow_read_callee() {
    // Negative pin (the iter-consume-vs-borrow-read boundary): a callee whose
    // `iter_consumes` is FALSE (a borrow-read like `xs.fold(..)`) presents an
    // otherwise-identical Borrowed contract, but the source decs must NOT be
    // suppressed (the caller is responsible for the release). Guards against the
    // borrow-read over-fire trap.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (mut func, cons) = multi_borrow_iter_consume_func(&mut pool, &interner, 2);
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(cons, iter_consume_contract(false));

    let dec_before = source_dec_count(&func, v(0));
    suppress_multi_borrow_iter_consume_source_decs(&mut func, &pool, &interner, &contracts);
    assert_eq!(
        source_dec_count(&func, v(0)),
        dec_before,
        "borrow-read callee (iter_consumes=false): source decs MUST stay (no suppression)"
    );
}

#[test]
fn multi_borrow_inline_iter_own_suppresses_source_dec_keeps_keep_alive_inc() {
    // INLINE for-loop iter-consume: the source `%0` flows to TWO `@iter(arg [own])`
    // protocol-builtin positions (no user contract). The recognizer counts the
    // `@iter [own]` positions as iter-consumes, suppresses the spurious source dec,
    // and keeps exactly (N-1)=1 keep-alive inc — identical to the user-callee path.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = inline_iter_multi_borrow_func(&mut pool, &interner, 2);
    // No user contract — the inline `@iter` path does not consult `contracts`.
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();

    assert_eq!(
        source_dec_count(&func, v(0)),
        1,
        "precondition: one spurious source dec present"
    );
    suppress_multi_borrow_iter_consume_source_decs(&mut func, &pool, &interner, &contracts);
    assert_eq!(
        source_dec_count(&func, v(0)),
        0,
        "the spurious source dec must be removed (the for-loop iter-drops free the buffer)"
    );
    let total_inc = source_inc_count(&func, v(0)) + source_inc_count(&func, v(2));
    assert_eq!(
        total_inc, 1,
        "exactly one keep-alive inc (N-1) on the source lineage; got {total_inc}",
    );
}

#[test]
fn multi_borrow_inline_iter_own_skips_single_loop() {
    // Negative pin (the inline-`@iter` recognizer lower boundary): a SINGLE
    // `@iter(arg [own])` (one for-loop, dead-after-call) must NOT be touched — a
    // spurious keep-alive inc with no matching drop would leak the source buffer.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = inline_iter_multi_borrow_func(&mut pool, &interner, 1);
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();

    let dec_before = source_dec_count(&func, v(0));
    let inc_before = source_inc_count(&func, v(0));
    suppress_multi_borrow_iter_consume_source_decs(&mut func, &pool, &interner, &contracts);
    assert_eq!(
        source_dec_count(&func, v(0)),
        dec_before,
        "single inline `@iter [own]` loop: source ops untouched (N < 2 multi-borrow gate)"
    );
    assert_eq!(
        source_inc_count(&func, v(0)),
        inc_before,
        "single inline `@iter [own]` loop: source ops untouched"
    );
}

/// Build a nested-loop func: `%0 [[int]]` source, outer `@iter(%0 [own])`,
/// `%2 = @__iter_next(%1, %m)`, `%3 = Project %2.1` (inner `[int]` view),
/// `%4 = %3` (Let alias), inner `@iter(%4 [own])`. The inner element view %3/%4
/// is the iter-element-view that needs a keep-alive before the inner `@iter`.
fn nested_iter_element_keepalive_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let inner_ty = pool.list(Idx::INT);
    let outer_ty = pool.list(inner_ty);
    let iter_name = interner.intern(ProtocolBuiltin::Iter.name());
    let iter_next = interner.intern(ProtocolBuiltin::IterNext.name());
    // %0 outer [[int]], %1 outer iter handle, %2 next-result, %3 marker [int],
    // %4 inner [int] view, %5 inner [int] alias, %6 inner iter handle.
    let var_types = vec![
        outer_ty,
        Idx::INT,
        Idx::INT,
        inner_ty,
        inner_ty,
        inner_ty,
        Idx::INT,
    ];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
    ];
    let body = vec![
        ArcInstr::Construct {
            dst: v(0),
            ty: outer_ty,
            ctor: CtorKind::ListLiteral,
            args: Vec::new(),
        },
        // outer @iter(%0 [own]) — %0 is the GENUINELY-OWNED source (not an
        // element view), so it must NOT get a keep-alive.
        ArcInstr::Apply {
            dst: v(1),
            ty: Idx::INT,
            func: iter_name,
            args: vec![v(0)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
        ArcInstr::Apply {
            dst: v(2),
            ty: Idx::INT,
            func: iter_next,
            args: vec![v(1), v(3)],
            arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
            mono_instance_id: None,
        },
        // %4 = Project %2.1 — the inner [int] element view (iter-element-view).
        ArcInstr::Project {
            dst: v(4),
            ty: inner_ty,
            value: v(2),
            field: 1,
        },
        // %5 = Let %4 — alias of the inner view.
        ArcInstr::Let {
            dst: v(5),
            ty: inner_ty,
            value: ArcValue::Var(v(4)),
        },
        // inner @iter(%5 [own]) — %5 is a borrow-view of the outer source's
        // element, so it MUST get a keep-alive inc.
        ArcInstr::Apply {
            dst: v(6),
            ty: Idx::INT,
            func: iter_name,
            args: vec![v(5)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
    ];
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn nested_iter_element_view_consumed_by_inner_iter_gets_keepalive_inc() {
    // The inner `@iter(%5 [own])` arg is an iter-element-view of the outer source
    // (a borrow into the outer buffer). Phase 6.67 emits exactly ONE keep-alive
    // `BurdenInc` on it (RL-1 duplicating consume) so the inner `ori_iter_drop`
    // and the outer `elem_dec_fn` each release once.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = nested_iter_element_keepalive_func(&mut pool, &interner);
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    emit_iter_element_view_iter_consume_keepalive_inc(&mut func, &interner, &contracts, &pool);
    // Exactly one keep-alive inc on the inner element-view lineage (%4 or %5).
    let inner_inc = source_inc_count(&func, v(4)) + source_inc_count(&func, v(5));
    assert_eq!(
        inner_inc, 1,
        "the inner iter-element-view consumed by the inner @iter MUST get exactly \
         one keep-alive inc; got {inner_inc}; body={:?}",
        func.blocks[0].body,
    );
    // NEGATIVE direction: the genuinely-owned outer source %0 (NOT an element
    // view) MUST receive zero keep-alive (a keep-alive there orphans a +1 -> leak).
    assert_eq!(
        source_inc_count(&func, v(0)),
        0,
        "the top-level owned source (not an element view) MUST NOT get a \
         keep-alive inc; body={:?}",
        func.blocks[0].body,
    );
}
