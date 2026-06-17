//! Phase-7 mechanical burden-lowering tests (probe path).
//!
//! Pins [`super::lower_burden_ops_to_rc`]: under the probe
//! (`predicate_stack_rc_disabled`), surviving whole-var `BurdenInc` /
//! `BurdenDec` lower to real `RcInc` / `RcDec`, and the field-grain
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` variants
//! lower by RE-SPELLING to `RcDecPartial` / `RcDecField` / `RcDecVariant`
//! (identical per-field / per-variant drop glue at codegen; out of the
//! Step-11 burden census per RL-comp net-preservation).
//!
//! RC counts use the SSOT `crate::pipeline::rc_count::count_rc_ops`.

use super::{
    borrow_survives_transform_names, borrowed_arg_release_verdict, collection_conversion_names,
    collection_set_algebra_names, compute_borrowed_terminator_aggregate_relocations,
    compute_branch_dead_value_releases, compute_comparison_operand_keepalive_strips,
    compute_cow_mutated_lineage_reps, compute_dead_collection_source_releases,
    compute_dead_iterator_handle_candidates, compute_dead_iterator_handle_releases,
    compute_dead_no_use_aggregate_releases, compute_dead_owned_collection_releases,
    compute_elidable_fresh_self_alloc_incs, compute_fresh_owned_collection_reps,
    compute_jump_threaded_reps, compute_lineage_alloc_aware_net,
    compute_redundant_project_borrowed_view_dec_strips,
    compute_returned_collection_surplus_inc_strips, emit_cow_inc_terminator_edge_release,
    emit_for_yield_index_consumed_element_rc,
    emit_iter_element_pushed_into_returned_collection_keepalive_inc,
    emit_iter_element_view_iter_consume_keepalive_inc, emit_single_iter_consume_reuse_keepalive,
    fresh_str_producing_method_names, is_burden_carrying_aggregate,
    iterator_consumer_collection_names, lineage_genuinely_read_outside_call,
    lower_burden_ops_to_rc, relocate_borrowed_terminator_arg_dec_to_edges,
    set_algebra_relocation_names, sharing_view_relocation_names,
    suppress_multi_borrow_iter_consume_source_decs,
    suppress_single_borrowed_invoke_iter_consume_source, user_callee_iter_consume_uses_of_rep,
    EdgeRelease, EscapeSafeBorrowedNames, IterHandleRelease,
};
use crate::aims::contract::{MemoryContract, ParamContract, ReturnAliasShape};
use crate::aims::lattice::AccessClass;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, PrimOp, ValueRepr,
};
use crate::ownership::Ownership;
use crate::pipeline::rc_count::count_rc_ops;
use ori_ir::BinaryOp;
use ori_types::{EnumVariant, Idx, Pool};
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
fn lower_respells_field_grain_decs_to_realized_forms() {
    let pool = Pool::default();
    // BurdenDecPartial / BurdenDecField / BurdenDecVariant carry field/variant
    // info codegen consumes directly (instr_dispatch.rs); Phase-7 lowering
    // RE-SPELLS them to RcDecPartial / RcDecField / RcDecVariant — same drop
    // glue, OUT of the Step-11 burden census (a mechanically-lowered op must
    // leave the burden stream with its pair partner per RL-comp; a surviving
    // half-pair nets -1 and aborts gated runs). NEVER a whole-var RcDec (that
    // would double-drop the moved-out / surviving fields).
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

    // The whole-var BurdenInc lowered; the three field-grain variants
    // re-spelled to their realized forms with payloads preserved.
    let body = &func.blocks[0].body;
    assert!(
        matches!(body[0], ArcInstr::RcInc { var, count: 1, .. } if var == v(0)),
        "whole-var BurdenInc lowered to RcInc"
    );
    assert!(
        matches!(&body[1], ArcInstr::RcDecPartial { var, skip_fields } if *var == v(0) && skip_fields == &[0]),
        "BurdenDecPartial re-spelled to RcDecPartial with skip_fields preserved"
    );
    assert!(
        matches!(body[2], ArcInstr::RcDecField { base, field: 0 } if base == v(1)),
        "BurdenDecField re-spelled to RcDecField"
    );
    assert!(
        matches!(body[3], ArcInstr::RcDecVariant { var } if var == v(1)),
        "BurdenDecVariant re-spelled to RcDecVariant"
    );

    // Semantic pin: ZERO burden ops survive a complete Phase-7 lowering — the
    // Step-11 census sees the empty stream (RL3_elision_net_preserving shape).
    assert_eq!(
        burden_count(&func),
        0,
        "no burden op survives Phase-7 lowering"
    );
    // Negative pin: lowering field-grain ops to a whole-var RcDec for v(1)
    // would be a double-drop — assert NO whole-var RcDec was synthesized.
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

/// Semantic pin (the family-C VF-1 shape): an aggregate call-result whose
/// acquire inc lowers to `RcInc` while its per-path release is a
/// `BurdenDecPartial` nets 0 on the Step-11 whole-var ledger AFTER the
/// re-spelling — the matched pair leaves the burden census TOGETHER
/// (RL-comp net-preservation; RL-1 duplication pair, RL-2 exactly-once
/// release). Pre-respelling this exact stream aborted gated runs at
/// `net=-1 ops=p1`.
#[test]
fn lower_partial_dec_pair_nets_zero_on_vf1_ledger_after_respelling() {
    let pool = Pool::default();
    let mut func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDecPartial {
                var: v(0),
                skip_fields: vec![1],
            },
        ],
    );
    func.burden_emitted = vec![true];

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    assert_eq!(
        burden_count(&func),
        0,
        "pair fully out of the burden census"
    );
    assert!(
        crate::aims::verify::burden_balance::verify_burden_balance(&func).is_empty(),
        "VF-1 whole-var ledger nets 0 once the partial dec is re-spelled"
    );
}

/// RE-2 backstop pin: a field-grain dec on a Scalar-repr subject is an
/// upstream admission contract violation — the re-spelling refuses it and
/// leaves the burden op in place so the Step-11 census surfaces it.
#[test]
fn lower_leaves_scalar_repr_field_grain_dec_in_place() {
    let pool = Pool::default();
    let mut func = rc_pointer_func(
        1,
        vec![ArcInstr::BurdenDecPartial {
            var: v(0),
            skip_fields: vec![0],
        }],
    );
    func.var_reprs[0] = ValueRepr::Scalar;

    lower_burden_ops_to_rc(&mut func, &pool, &rustc_hash::FxHashSet::default());

    assert_eq!(
        burden_count(&func),
        1,
        "scalar-repr field-grain dec left burden-spelled (census abort surface)"
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

/// Semantic pin: the elided fresh-site inc is REMOVED from the op stream —
/// the VF-1 whole-var ledger counts surviving burden ops, so the lowered
/// function verifies net-0 at every exit. A retained no-op marker would net
/// `+1` and abort gated (`ORI_VERIFY_ARC=1`) compilation.
#[test]
fn lower_elided_fresh_inc_is_removed_and_vf1_ledger_nets_zero() {
    let pool = Pool::default();
    let mut func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    func.burden_emitted = vec![true];
    let elidable: rustc_hash::FxHashSet<ArcVarId> = std::iter::once(v(0)).collect();

    lower_burden_ops_to_rc(&mut func, &pool, &elidable);

    // The elided inc is GONE (not a surviving no-op marker); the release
    // lowered to a real RcDec.
    assert_eq!(
        burden_count(&func),
        0,
        "elided fresh-site inc removed; no burden op survives"
    );
    let after = count_rc_ops(&func);
    assert_eq!(after.inc, 0, "elided fresh inc never lowers to RcInc");
    assert_eq!(after.dec, 1, "release lowered to exactly one real RcDec");
    assert!(
        crate::aims::verify::burden_balance::verify_burden_balance(&func).is_empty(),
        "VF-1 whole-var ledger nets 0 after elision-by-removal"
    );
}

/// Semantic pin: ONLY the first fresh-site inc per elidable var is removed —
/// a subsequent `BurdenInc` on the same var is a genuine dup-alias acquire
/// and still lowers to a real `RcInc`.
#[test]
fn lower_elided_var_second_inc_still_lowers_to_rcinc() {
    let pool = Pool::default();
    let mut func = rc_pointer_func(
        1,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    let elidable: rustc_hash::FxHashSet<ArcVarId> = std::iter::once(v(0)).collect();

    lower_burden_ops_to_rc(&mut func, &pool, &elidable);

    let after = count_rc_ops(&func);
    assert_eq!(
        after.inc, 1,
        "second (dup-alias) inc lowers to RcInc; only the first is elided"
    );
    assert_eq!(after.dec, 1, "release lowered to one real RcDec");
    assert_eq!(burden_count(&func), 0, "no burden op survives");
}

/// Negative pin (mutation-verify shape): a SURVIVING no-op marker inc — the
/// legacy elision form — fails VF-1 with `net=+1` and carries the
/// classification attribution (`def_kind` / `exit_kind` / `residual_ops`).
#[test]
fn verify_flags_surviving_marker_inc_with_attribution() {
    let mut func = rc_pointer_func(1, vec![list_construct(v(0), Vec::new())]);
    func.blocks[0].body.push(ArcInstr::BurdenInc { var: v(0) });
    func.burden_emitted = vec![true];

    let errors = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert_eq!(errors.len(), 1, "surviving marker inc nets +1 at exit");
    let e = &errors[0];
    assert_eq!(e.observed_net, 1);
    assert_eq!(e.def_kind, "construct");
    assert_eq!(e.exit_kind, "return");
    assert_eq!(e.var_repr, "rc-pointer");
    assert_eq!(
        (
            e.residual_ops.inc,
            e.residual_ops.dec,
            e.residual_ops.dec_partial,
            e.residual_ops.dec_variant
        ),
        (1, 0, 0, 0)
    );
}

/// Negative ledger pin (mutation-verify shape): a genuinely-unbalanced
/// SURVIVING `BurdenDecPartial` — spurious residue the Phase-7 re-spelling
/// never consumed (hand-built IR; equivalently the
/// `ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1` legacy half-pair) — still fails
/// VF-1 with `net=-1` and a `dec_partial` census. The verifier's whole-var
/// counting (`whole_var_dec_target`) is UNTOUCHED by the family-C cure; the
/// cure is lowering-completeness, never verifier widening.
#[test]
fn verify_flags_surviving_partial_dec_with_attribution() {
    let mut func = rc_pointer_func(1, vec![list_construct(v(0), Vec::new())]);
    func.blocks[0].body.push(ArcInstr::BurdenDecPartial {
        var: v(0),
        skip_fields: vec![1],
    });
    func.burden_emitted = vec![true];

    let errors = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert_eq!(errors.len(), 1, "surviving partial dec nets -1 at exit");
    let e = &errors[0];
    assert_eq!(e.observed_net, -1);
    assert_eq!(e.def_kind, "construct");
    assert_eq!(e.exit_kind, "return");
    assert_eq!(e.var_repr, "rc-pointer");
    assert_eq!(
        (
            e.residual_ops.inc,
            e.residual_ops.dec,
            e.residual_ops.dec_partial,
            e.residual_ops.dec_variant
        ),
        (0, 0, 1, 0)
    );
}

/// Negative ledger pin twin: a spurious SURVIVING `BurdenDecVariant` still
/// fails VF-1 with `net=-1` and a `dec_variant` census.
#[test]
fn verify_flags_surviving_variant_dec_with_attribution() {
    let mut func = rc_pointer_func(1, vec![list_construct(v(0), Vec::new())]);
    func.blocks[0]
        .body
        .push(ArcInstr::BurdenDecVariant { var: v(0) });
    func.burden_emitted = vec![true];

    let errors = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert_eq!(errors.len(), 1, "surviving variant dec nets -1 at exit");
    let e = &errors[0];
    assert_eq!(e.observed_net, -1);
    assert_eq!(
        (
            e.residual_ops.inc,
            e.residual_ops.dec,
            e.residual_ops.dec_partial,
            e.residual_ops.dec_variant
        ),
        (0, 0, 0, 1)
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
    // The dec IS the lineage's release, so the function exit must not read the
    // member (the aliveness guard correctly rejects a read-after-release):
    // return %1, outside the lineage.
    let mut func = rc_pointer_func(
        2,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
    );
    func.blocks[0].terminator = ArcTerminator::Return { value: v(1) };
    let reps = identity_reps(1);
    let net = compute_lineage_alloc_aware_net(
        &func,
        &reps,
        &ori_ir::StringInterner::new(),
        &rustc_hash::FxHashSet::default(),
    );
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(1),
        "read-only single-ref self-alloc lineage nets +1 (redundant fresh inc surplus)"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &ori_ir::StringInterner::new(),
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
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
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &ori_ir::StringInterner::new(),
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
    assert!(
        !elidable.contains(&v(0)),
        "COW-mutated self-alloc (list + operand) keeps its load-bearing fresh inc"
    );
    assert!(
        !elidable.contains(&v(1)),
        "the other COW `+` operand also keeps its fresh inc"
    );
}

/// One-block func: `%0 = Construct List(); %1 = Apply callee(%0 [borrow])` —
/// an `RcPtr` collection at a borrowed user-call arg position, the
/// interprocedural may-COW shape `compute_cow_mutated_lineage_reps` vets.
fn borrowed_user_call_arg_func(callee: ori_ir::Name) -> ArcFunction {
    rc_pointer_func(
        2,
        vec![
            list_construct(v(0), Vec::new()),
            ArcInstr::Apply {
                dst: v(1),
                ty: Idx::from_raw(0),
                func: callee,
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
        ],
    )
}

/// Contract for a 1-param callee whose param is `Borrowed` with the given
/// `borrowed_read_only` fact.
fn borrowed_param_contract(read_only: bool) -> MemoryContract {
    let mut param = ParamContract::CONSERVATIVE;
    param.access = AccessClass::Borrowed;
    param.borrowed_read_only = read_only;
    MemoryContract {
        params: vec![param],
        ..MemoryContract::conservative(1)
    }
}

/// Narrowing pin: a user callee whose contract proves the param a pure
/// borrow-read (`access == Borrowed && borrowed_read_only`) cannot COW the
/// arg — the lineage is NOT counted COW-mutated, so the fresh inc stays
/// elidable downstream.
#[test]
fn cow_lineage_excludes_contract_proven_borrowed_read_only_callee_arg() {
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("pure_reader");
    let func = borrowed_user_call_arg_func(callee);
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> =
        [(callee, borrowed_param_contract(true))]
            .into_iter()
            .collect();
    let reps = compute_cow_mutated_lineage_reps(&func, &identity_reps(2), &interner, &contracts);
    assert!(
        !reps.contains(&v(0)),
        "a contract-proven pure borrow-read position is NOT may-COW; reps = {reps:?}"
    );
}

/// Funding-direction pin: an UNKNOWN user callee (no contract) stays
/// conservatively may-COW — the fresh inc is kept (when in doubt, fund).
#[test]
fn cow_lineage_keeps_unknown_contract_user_callee_arg() {
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("unknown_user_fn");
    let func = borrowed_user_call_arg_func(callee);
    let reps = compute_cow_mutated_lineage_reps(
        &func,
        &identity_reps(2),
        &interner,
        &FxHashMap::default(),
    );
    assert!(
        reps.contains(&v(0)),
        "an unknown-contract user callee stays conservatively may-COW"
    );
}

/// Funding-direction pin: a contract whose Borrowed param lacks the
/// `borrowed_read_only` fact (the COW-through-borrowed-param risk —
/// `@check` doing `list.push(..)` on a borrowed param) stays may-COW.
#[test]
fn cow_lineage_keeps_borrowed_non_read_only_callee_arg() {
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("cow_pusher");
    let func = borrowed_user_call_arg_func(callee);
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> =
        [(callee, borrowed_param_contract(false))]
            .into_iter()
            .collect();
    let reps = compute_cow_mutated_lineage_reps(&func, &identity_reps(2), &interner, &contracts);
    assert!(
        reps.contains(&v(0)),
        "a Borrowed param WITHOUT the read-only fact stays may-COW (funding direction)"
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
    let net = compute_lineage_alloc_aware_net(
        &func,
        &reps,
        &ori_ir::StringInterner::new(),
        &rustc_hash::FxHashSet::default(),
    );
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "move-alias-dec lineage nets 0 (fresh inc balances the unpaired move dec)"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &ori_ir::StringInterner::new(),
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
    assert!(
        !elidable.contains(&v(0)),
        "net != 1 → fresh inc kept (eliding would double-free, net −1)"
    );
}

/// Build a fresh self-alloc result on an `Invoke` TERMINATOR (the `s.insert(..)`
/// COW-insert shape): `%0` is owned-consumed by `Invoke @insert`, producing a
/// FRESH owned set result `%1` on the terminator (`normal` bb1, where Phase-5
/// prepends `%1`'s fresh-site inc); `%1` is borrow-read by `@len` and dead after.
/// The result `%1` is the borrow-read-only fresh COW-result whose surplus
/// fresh-site inc must be elided.
///
/// Linear normal path (bb0 → bb1 → bb2 Return) with ONE alloc-reachable terminal,
/// so the alloc-aware per-path net is unambiguously `+1` (alloc + fresh-inc −
/// one-dec). The `@insert` / `@len` unwind edges go to bb3, an alloc-UNREACHABLE
/// `Resume` pad (reached only on the pre-alloc bb0 unwind), so it carries no `%1`
/// net and is excluded by the alloc-reachable filter. Spec: Annex E §AIMS RL-1.
fn invoke_insert_result_func(interner: &ori_ir::StringInterner) -> ArcFunction {
    let insert_name = interner.intern("insert");
    let len_name = interner.intern("len");
    // %0 source set, %1 insert result (fresh self-alloc), %2 len-alias,
    // %3 scalar len, %4 inserted key (borrow arg).
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0 source set
        ValueRepr::RcPointer, // %1 insert result (fresh self-alloc)
        ValueRepr::RcPointer, // %2 %1 alias for @len
        ValueRepr::Scalar,    // %3 len result
        ValueRepr::Scalar,    // %4 inserted key (borrow arg)
    ];
    let var_types: Vec<Idx> = (0..var_reprs.len()).map(|_| Idx::from_raw(0)).collect();
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            // bb0: `Invoke @insert(%0 [own], %4 [borrow])` -> fresh result %1.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::BurdenInc { var: v(0) }],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::from_raw(0),
                    func: insert_name,
                    args: vec![v(0), v(4)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    // Pre-alloc unwind pad (bb3): %1 not yet allocated here.
                    unwind: ArcBlockId::new(3),
                },
            },
            // bb1: fresh-site inc on the result, borrow-read (@len), result dies.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::from_raw(0),
                        value: ArcValue::Var(v(1)),
                    },
                    ArcInstr::BurdenDec { var: v(1) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(3),
                    ty: Idx::from_raw(0),
                    func: len_name,
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(2),
                    // The `@len` unwind takes the freeing `BurdenDec %1` already
                    // emitted in bb1's body (it precedes the terminator), so the
                    // unwind pad bb4 sees `%1` released too — a balanced unwind
                    // edge, not a leaked one.
                    unwind: ArcBlockId::new(4),
                },
            },
            // bb2: exit.
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(3) },
            },
            // bb3: bb0's pre-alloc unwind pad (alloc-UNREACHABLE; carries no %1).
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
            // bb4: bb1's post-dec unwind pad (alloc-reachable; %1 already released).
            ArcBlock {
                id: ArcBlockId::new(4),
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

/// Semantic pin (a fresh-COW-result borrow-read-then-dead leak): a FRESH self-alloc
/// produced on an `Invoke` TERMINATOR (`s.insert(..)` COW-result), borrow-read
/// and dead, has alloc-aware net == 1 — its fresh-site inc is the surplus over
/// balance. `fresh_collection_source_apply_dst` MUST recognize the `Invoke`-form
/// fresh self-alloc (not only the `Apply`-instruction form) so the net-keyed
/// elision removes that surplus inc; without it the result leaks (alloc(+1) +
/// fresh-inc(+1) − one-dec(−1) = +1). Mirrors
/// `narrowing::set_insert_with_narrowed_list_context`. Spec: Annex E §AIMS RL-1.
#[test]
fn elide_fresh_inc_for_invoke_terminator_self_alloc_result() {
    let interner = ori_ir::StringInterner::new();
    let func = invoke_insert_result_func(&interner);
    let reps = identity_reps(5);
    let net =
        compute_lineage_alloc_aware_net(&func, &reps, &interner, &rustc_hash::FxHashSet::default());
    assert_eq!(
        net.get(&v(1)).copied(),
        Some(1),
        "Invoke-terminator fresh result lineage nets +1 (surplus fresh-site inc) \
         once the result is recognized as a fresh self-alloc"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &interner,
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
    assert!(
        elidable.contains(&v(1)),
        "the surplus fresh inc of a borrow-read-only Invoke-terminator self-alloc \
         result is elidable (eliding restores the alloc-aware balance to 0)"
    );
}

/// Negative pin (Invoke-path over-fire clamp): an `Invoke`-terminator fresh
/// self-alloc result whose value is then OWNED-CONSUMED by a downstream COW
/// mutator (a value-mutation builtin at an owned position) KEEPS its fresh inc —
/// the cow-mutated classifier flags it and the elision defers it (RL-1: the inc
/// is load-bearing for the COW copy-vs-mutate read). Eliding would corrupt the
/// shared buffer. Clamps the `Invoke`-recognition extension to the
/// borrow-read-only shape. Spec: Annex E §AIMS RL-1.
#[test]
fn keep_fresh_inc_for_invoke_terminator_result_owned_consumed() {
    let interner = ori_ir::StringInterner::new();
    let insert_name = interner.intern("insert");
    // bb0: `Invoke @insert(%0 [own])` -> fresh result %1.
    // bb1: burden_inc %1 ; `Invoke @insert(%1 [own])` -> %2 (owned-consume of %1).
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0 source
        ValueRepr::RcPointer, // %1 first insert result (fresh self-alloc)
        ValueRepr::RcPointer, // %2 second insert result
        ValueRepr::Scalar,    // %3 key
    ];
    let var_types: Vec<Idx> = (0..var_reprs.len()).map(|_| Idx::from_raw(0)).collect();
    let func = ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::BurdenInc { var: v(0) }],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::from_raw(0),
                    func: insert_name,
                    args: vec![v(0), v(3)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::BurdenInc { var: v(1) }],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::from_raw(0),
                    func: insert_name,
                    args: vec![v(1), v(3)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(3),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    };
    let reps = identity_reps(4);
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &interner,
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
    assert!(
        !elidable.contains(&v(1)),
        "Invoke-terminator fresh result that is owned-consumed downstream (COW) \
         keeps its load-bearing fresh inc"
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
    // The scratch arg %3 and the returned %3 sit OUTSIDE the lineage (the
    // final dec is the lineage's release; the aliveness guard correctly
    // rejects a member read after it).
    let mut func = rc_pointer_func(
        4,
        vec![
            ArcInstr::Apply {
                dst: v(0),
                ty: Idx::from_raw(0),
                func: take,
                args: vec![v(3)],
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
    func.blocks[0].terminator = ArcTerminator::Return { value: v(3) };
    // The two index aliases fold into the result's lineage (move-alias reps).
    let reps: FxHashMap<ArcVarId, ArcVarId> = [(v(0), v(0)), (v(1), v(0)), (v(2), v(0))]
        .into_iter()
        .collect();
    let net =
        compute_lineage_alloc_aware_net(&func, &reps, &interner, &rustc_hash::FxHashSet::default());
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(1),
        "dup-indexed list_take result nets +1 (the surplus fresh inc over alloc)"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &interner,
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
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
    let net =
        compute_lineage_alloc_aware_net(&func, &reps, &interner, &rustc_hash::FxHashSet::default());
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "list_take result with a move-alias dec nets 0 → fresh inc is load-bearing"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &interner,
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
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
    let net =
        compute_lineage_alloc_aware_net(&func, &reps, &interner, &rustc_hash::FxHashSet::default());
    assert_eq!(
        net.get(&v(0)).copied(),
        Some(0),
        "jump-threaded list_take result nets 0 once the phi-threaded downstream \
         release is attributed to the alloc rep (the fresh inc balances the 2nd dec)"
    );
    let elidable = compute_elidable_fresh_self_alloc_incs(
        &func,
        &reps,
        &interner,
        &rustc_hash::FxHashMap::default(),
        &rustc_hash::FxHashSet::default(),
    );
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
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
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
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
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
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
    assert!(
        releases.is_empty(),
        "a collection passed to a user function gets NO scope-exit dec; got {releases:?}",
    );
}

// === Dead-no-use INLINE-AGGREGATE pass (M1) ===

/// Build a bare dead-no-use struct: `%1 = Construct Struct(Doc)(%0)` where `Doc =
/// { content: str }` (an `Aggregate` repr, `NonTrivial` triviality), with ZERO
/// uses. The single-block function returns a scalar (`%2`). The pass MUST emit
/// exactly one scope-exit dec on `%1` (the heap str field leaks otherwise).
fn dead_no_use_struct_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let content = interner.intern("content");
    let doc_name = interner.intern("Doc");
    let doc_ty = pool.struct_type(doc_name, &[(content, Idx::STR)]);
    // %0 = the str field arg (FatValue), %1 = the Doc struct (Aggregate), %2 = scalar.
    ArcFunction {
        var_types: vec![Idx::STR, doc_ty, Idx::INT],
        var_reprs: vec![ValueRepr::FatValue, ValueRepr::Aggregate, ValueRepr::Scalar],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: v(1),
                ty: doc_ty,
                ctor: CtorKind::Struct(doc_name),
                args: vec![v(0)],
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn dead_no_use_aggregate_frees_struct_with_str_at_scope_exit() {
    // The dead-no-use `Doc { content: str }` struct (Aggregate repr, NonTrivial)
    // has ZERO uses. The pass emits exactly ONE scope-exit dec on `%1` at the END
    // of its defining block (bb0) — Phase 7 lowers it to `RcDec [AggFields]`
    // walking the heap str field. Spec: Annex E §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = dead_no_use_struct_func(&mut pool, &interner);
    // The gate recognises the burden-carrying aggregate.
    assert!(
        is_burden_carrying_aggregate(v(1), &func, &pool),
        "a struct with a str field is a burden-carrying aggregate"
    );
    let releases =
        compute_dead_no_use_aggregate_releases(&func, &pool, &interner, &FxHashMap::default());
    assert_eq!(
        releases.len(),
        1,
        "exactly one dead-no-use aggregate release (the leaked struct at scope exit); got {releases:?}",
    );
    let (block_idx, var) = releases[0];
    assert_eq!(
        block_idx, 0,
        "release at the defining block bb0 (scope exit)"
    );
    assert_eq!(var, v(1), "frees the struct's SSA value %1");
}

#[test]
fn dead_no_use_aggregate_skips_scalar_only_struct() {
    // NEGATIVE pin: a scalar-only struct `{ x: int, y: int }` is `Trivial` (no heap
    // field, no drop-glue) -> `is_burden_carrying_aggregate` is false -> the pass
    // MUST emit nothing (a `RcDec [AggFields]` on a null-drop-glue struct is a
    // spurious release). Spec: Annex E §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let x = interner.intern("x");
    let y = interner.intern("y");
    let point_name = interner.intern("Point");
    let point_ty = pool.struct_type(point_name, &[(x, Idx::INT), (y, Idx::INT)]);
    let func = ArcFunction {
        var_types: vec![Idx::INT, Idx::INT, point_ty, Idx::INT],
        var_reprs: vec![
            ValueRepr::Scalar,
            ValueRepr::Scalar,
            ValueRepr::Aggregate,
            ValueRepr::Scalar,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: v(2),
                ty: point_ty,
                ctor: CtorKind::Struct(point_name),
                args: vec![v(0), v(1)],
            }],
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    };
    assert!(
        !is_burden_carrying_aggregate(v(2), &func, &pool),
        "a scalar-only struct is NOT a burden-carrying aggregate"
    );
    let releases =
        compute_dead_no_use_aggregate_releases(&func, &pool, &interner, &FxHashMap::default());
    assert!(
        releases.is_empty(),
        "a scalar-only (Trivial) aggregate gets NO scope-exit dec; got {releases:?}",
    );
}

#[test]
fn dead_no_use_aggregate_skips_returned_struct() {
    // NEGATIVE pin: a fresh aggregate that is RETURNED is an RL-2 transfer (the
    // caller inherits the release). The pass MUST emit nothing (a dec here
    // double-frees the heap field against the caller's release). Spec: Annex E
    // §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let content = interner.intern("content");
    let doc_name = interner.intern("Doc");
    let doc_ty = pool.struct_type(doc_name, &[(content, Idx::STR)]);
    // %0 = str field, %1 = Doc struct — RETURNED from bb0.
    let func = ArcFunction {
        var_types: vec![Idx::STR, doc_ty],
        var_reprs: vec![ValueRepr::FatValue, ValueRepr::Aggregate],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: v(1),
                ty: doc_ty,
                ctor: CtorKind::Struct(doc_name),
                args: vec![v(0)],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    };
    let releases =
        compute_dead_no_use_aggregate_releases(&func, &pool, &interner, &FxHashMap::default());
    assert!(
        releases.is_empty(),
        "a RETURNED (transferred) aggregate gets NO scope-exit dec; got {releases:?}",
    );
}

/// Build a project-borrowed-view double-free shape: `%0 = "..."` (str field),
/// `%1 = Construct Struct(Wrapper)(%0)` (single-ref aggregate), `%2 = Project
/// %1.0` (the borrow-view), `burden_dec %1` (the aggregate `[AggFields]` drop —
/// frees `%0`), `burden_dec %2` (the SPURIOUS view dec — frees `%0` again),
/// borrowed-read `@length(%2)`, return scalar. `paired_inc` controls whether the
/// aggregate is bumped by a keep-alive `burden_inc %1` (shared -> KEEP) or stays
/// single-ref (-> STRIP). `aggregate_dec` controls whether the aggregate's
/// freeing dec is present (no dec -> the view dec is the only release -> KEEP).
fn project_borrowed_view_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    paired_inc: bool,
    aggregate_dec: bool,
) -> ArcFunction {
    let s = interner.intern("s");
    let wrapper_name = interner.intern("Wrapper");
    let length_name = interner.intern("length");
    let wrapper_ty = pool.struct_type(wrapper_name, &[(s, Idx::STR)]);
    // %0 str field (bare FatValue slot, consumed by Construct), %1 Wrapper struct,
    // %2 Project view, %3 length result (scalar).
    let mut body = vec![ArcInstr::Construct {
        dst: v(1),
        ty: wrapper_ty,
        ctor: CtorKind::Struct(wrapper_name),
        args: vec![v(0)],
    }];
    if paired_inc {
        body.push(ArcInstr::BurdenInc { var: v(1) });
    }
    body.push(ArcInstr::Project {
        dst: v(2),
        ty: Idx::STR,
        value: v(1),
        field: 0,
    });
    if aggregate_dec {
        body.push(ArcInstr::BurdenDec { var: v(1) });
    }
    body.push(ArcInstr::BurdenDec { var: v(2) });
    body.push(ArcInstr::Apply {
        dst: v(3),
        ty: Idx::INT,
        func: length_name,
        args: vec![v(2)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    });
    ArcFunction {
        var_types: vec![Idx::STR, wrapper_ty, Idx::STR, Idx::INT],
        var_reprs: vec![
            ValueRepr::FatValue,
            ValueRepr::Aggregate,
            ValueRepr::FatValue,
            ValueRepr::Scalar,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

/// Build the USED-and-compared aggregate `a == b` / `a != c` shape (the f13
/// canonical). `%1` is the compared aggregate (`Doc { content: str }`), bumped by
/// a construct keep-alive `BurdenInc %1` for its two comparisons.
///
/// bb0 builds a / b / c, the keep-alive, the operand alias `%9 = %1` with a
/// spurious `BurdenInc %9`, the `%9 == %10` `PrimOp`, then `BurdenDec %9` /
/// `BurdenDec %10`; branches to bb1 / bb2. bb1 (then, re-compares `%1`) holds the
/// operand alias `%12 = %1` with a spurious `BurdenInc %12`, the `%12 != %13`
/// `PrimOp`, the misplaced whole-var `BurdenDec %1` (M4), and the operand decs
/// `BurdenDec %12` / `BurdenDec %13`. bb2 (else) holds the genuine
/// complement-branch `BurdenDec %1`. `%10` / `%13` alias the other operands (`%4`
/// is b, `%7` is c). With `keepalive` false the construct `BurdenInc %1` is
/// omitted (single-use shape, nothing to strip, M4 must not fire).
/// Var tables for `comparison_operand_func`: aggregates at %1/%4/%7/%9/%10/%12/%13,
/// bool comparison results at %11/%14.
fn comparison_operand_var_tables(doc_ty: Idx) -> (Vec<Idx>, Vec<ValueRepr>) {
    let mut var_types = vec![Idx::STR; 15];
    let mut var_reprs = vec![ValueRepr::FatValue; 15];
    for &agg in &[1u32, 4, 7, 9, 10, 12, 13] {
        var_types[agg as usize] = doc_ty;
        var_reprs[agg as usize] = ValueRepr::Aggregate;
    }
    for &cmp in &[11u32, 14] {
        var_types[cmp as usize] = Idx::BOOL;
        var_reprs[cmp as usize] = ValueRepr::Scalar;
    }

    (var_types, var_reprs)
}

fn comparison_operand_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    keepalive: bool,
) -> ArcFunction {
    let content = interner.intern("content");
    let doc_name = interner.intern("Doc");
    let doc_ty = pool.struct_type(doc_name, &[(content, Idx::STR)]);
    let cmp = |dst: u32, a: u32, b: u32, op: BinaryOp| ArcInstr::Let {
        dst: v(dst),
        ty: Idx::BOOL,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(op),
            args: vec![v(a), v(b)],
        },
    };
    let mk = |dst: u32, ty: Idx, name: ori_ir::Name| ArcInstr::Construct {
        dst: v(dst),
        ty,
        ctor: CtorKind::Struct(name),
        args: vec![v(dst.saturating_sub(1))],
    };
    // bb0: build a (%1), b (%4), c (%7); construct keep-alive on %1; `a == b`.
    let mut bb0 = vec![
        mk(1, doc_ty, doc_name),
        mk(4, doc_ty, doc_name),
        mk(7, doc_ty, doc_name),
    ];
    if keepalive {
        bb0.push(ArcInstr::BurdenInc { var: v(1) });
    }
    bb0.push(ArcInstr::Let {
        dst: v(9),
        ty: doc_ty,
        value: ArcValue::Var(v(1)),
    });
    bb0.push(ArcInstr::Let {
        dst: v(10),
        ty: doc_ty,
        value: ArcValue::Var(v(4)),
    });
    bb0.push(ArcInstr::BurdenInc { var: v(9) }); // spurious operand inc (M3 target)
    bb0.push(cmp(11, 9, 10, BinaryOp::Eq));
    bb0.push(ArcInstr::BurdenDec { var: v(9) });
    bb0.push(ArcInstr::BurdenDec { var: v(10) });

    // bb1 (then): `a != c`; misplaced whole-var dec of %1 (M4 target).
    let bb1 = vec![
        ArcInstr::Let {
            dst: v(12),
            ty: doc_ty,
            value: ArcValue::Var(v(1)),
        },
        ArcInstr::Let {
            dst: v(13),
            ty: doc_ty,
            value: ArcValue::Var(v(7)),
        },
        ArcInstr::BurdenInc { var: v(12) }, // spurious operand inc (M3 target)
        cmp(14, 12, 13, BinaryOp::NotEq),
        ArcInstr::BurdenDec { var: v(1) }, // misplaced (M4 target)
        ArcInstr::BurdenDec { var: v(12) },
        ArcInstr::BurdenDec { var: v(13) },
    ];

    // bb2 (else): genuine complement-branch %1 release (KEEP).
    let bb2 = vec![ArcInstr::BurdenDec { var: v(1) }];

    let (var_types, var_reprs) = comparison_operand_var_tables(doc_ty);
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: bb0,
                terminator: ArcTerminator::Branch {
                    cond: v(11),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: bb1,
                terminator: ArcTerminator::Return { value: v(14) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: bb2,
                terminator: ArcTerminator::Return { value: v(14) },
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn comparison_operand_strips_spurious_inc_and_misplaced_dec() {
    // POSITIVE: a multi-use compared aggregate `%1` (keep-alive inc present). The
    // operand aliases `%9`/`%12` (sole use = `==`/`!=` operand) get their spurious
    // `BurdenInc` stripped (M3); the misplaced bb1 `BurdenDec %1` (the operand dec
    // already releases it there) gets stripped (M4). The bb2 `BurdenDec %1`
    // (complement branch) stays. Spec: Annex E §AIMS RL-1 + RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = comparison_operand_func(&mut pool, &interner, true);
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(&func, &FxHashMap::default());
    let strips = compute_comparison_operand_keepalive_strips(&func, &pool, &same_alloc_reps);
    assert!(
        strips.inc_strips.contains(&v(9)) && strips.inc_strips.contains(&v(12)),
        "both comparison-operand keep-alive incs (%9, %12) are stripped; got {:?}",
        strips.inc_strips,
    );
    assert!(
        strips.dec_strips.contains(&(1usize, v(1))),
        "the misplaced bb1 whole-var BurdenDec %1 is stripped (M4); got {:?}",
        strips.dec_strips,
    );
    assert!(
        !strips.dec_strips.contains(&(2usize, v(1))),
        "the bb2 complement-branch BurdenDec %1 is KEPT (the genuine release); got {:?}",
        strips.dec_strips,
    );
}

#[test]
fn comparison_operand_keeps_single_use_no_keepalive() {
    // NEGATIVE: a single-comparison compared aggregate (NO construct keep-alive
    // inc). `%1` is used once at the comparison, so its operand dec is the sole
    // release. With no keep-alive inc on the `%1` lineage, the M4 whole-var
    // strip must NOT fire (no redundant branch dec to remove). M3 still strips the
    // operand incs, but the dec_strips set must be empty. Spec: Annex E §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = comparison_operand_func(&mut pool, &interner, false);
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(&func, &FxHashMap::default());
    let strips = compute_comparison_operand_keepalive_strips(&func, &pool, &same_alloc_reps);
    assert!(
        strips.dec_strips.is_empty(),
        "with no construct keep-alive inc, no whole-var dec is stripped; got {:?}",
        strips.dec_strips,
    );
}

#[test]
fn comparison_operand_keeps_projected_field_no_compare() {
    // NEGATIVE (the Config boundary): a struct whose field is PROJECTED (`Project
    // %1.0`) and read, with NO `==`/`!=` comparison operand. The projected view is
    // not a comparison operand, so no inc is stripped and no dec is stripped — the
    // M3+M4 cure never touches the inline-struct-projected shape. Spec: Annex E
    // §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    // Reuse the project-borrowed-view shape: %2 = Project %1.0 (a field read, not a
    // comparison operand).
    let func = project_borrowed_view_func(&mut pool, &interner, false, true);
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(&func, &FxHashMap::default());
    let strips = compute_comparison_operand_keepalive_strips(&func, &pool, &same_alloc_reps);
    assert!(
        strips.inc_strips.is_empty() && strips.dec_strips.is_empty(),
        "a projected-field (non-comparison) shape is never touched by the comparison-operand \
         cure; got inc={:?} dec={:?}",
        strips.inc_strips,
        strips.dec_strips,
    );
}

#[test]
fn comparison_operand_same_root_excluded_no_strip() {
    // CRITICAL NEGATIVE (same-root guard): a comparison whose TWO operands
    // (%9, %10) both alias the SAME `Construct` %1 -> one `same_alloc` rep. The
    // M3/M4 net reasoning holds only for DISTINCT-root operands; a same-root
    // comparison's two operand decs release the SAME ref, so an added M4 whole-var
    // strip over-releases (double-free). The guard MUST exclude both operands ->
    // empty inc_strips AND empty dec_strips. Spec: Annex E §AIMS RL-2
    // (`RL2_release_exactly_once`).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = comparison_operand_same_root_func(&mut pool, &interner);
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(&func, &FxHashMap::default());
    let strips = compute_comparison_operand_keepalive_strips(&func, &pool, &same_alloc_reps);
    assert!(
        strips.inc_strips.is_empty() && strips.dec_strips.is_empty(),
        "a same-root comparison (both operands one allocation) is never stripped; \
         got inc={:?} dec={:?}",
        strips.inc_strips,
        strips.dec_strips,
    );
}

/// Same-root comparison shape for [`comparison_operand_same_root_excluded_no_strip`]:
/// ONE `Construct` %1, two Let-Var aliases %9/%10 of it, `%9 == %10` (both operands
/// trace to the single allocation). Mirrors `a == b` where `b = a`.
fn comparison_operand_same_root_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let content = interner.intern("content");
    let doc_name = interner.intern("Doc");
    let doc_ty = pool.struct_type(doc_name, &[(content, Idx::STR)]);
    let bb0 = vec![
        ArcInstr::Construct {
            dst: v(1),
            ty: doc_ty,
            ctor: CtorKind::Struct(doc_name),
            args: vec![v(0)],
        },
        ArcInstr::BurdenInc { var: v(1) }, // construct keep-alive
        // %9 = %1 and %10 = %1 — both alias the single allocation.
        ArcInstr::Let {
            dst: v(9),
            ty: doc_ty,
            value: ArcValue::Var(v(1)),
        },
        ArcInstr::Let {
            dst: v(10),
            ty: doc_ty,
            value: ArcValue::Var(v(1)),
        },
        ArcInstr::BurdenInc { var: v(9) },
        ArcInstr::BurdenInc { var: v(10) },
        ArcInstr::Let {
            dst: v(11),
            ty: Idx::BOOL,
            value: ArcValue::PrimOp {
                op: PrimOp::Binary(BinaryOp::Eq),
                args: vec![v(9), v(10)],
            },
        },
        ArcInstr::BurdenDec { var: v(9) },
        ArcInstr::BurdenDec { var: v(10) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    let mut var_types = vec![Idx::UNIT; 12];
    var_types[1] = doc_ty;
    var_types[9] = doc_ty;
    var_types[10] = doc_ty;
    var_types[11] = Idx::BOOL;
    let mut var_reprs = vec![ValueRepr::Scalar; 12];
    var_reprs[1] = ValueRepr::Aggregate;
    var_reprs[9] = ValueRepr::Aggregate;
    var_reprs[10] = ValueRepr::Aggregate;
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: bb0,
            terminator: ArcTerminator::Return { value: v(11) },
        }],
        ..Default::default()
    }
}

#[test]
fn project_borrowed_view_strips_single_ref_str_field_dec() {
    // The str-field view `%2 = Project %1.0` of a SINGLE-REF aggregate `%1` whose
    // `[AggFields]` drop (`burden_dec %1`) frees the field: the view's
    // `burden_dec %2` is the redundant SECOND release -> STRIP. Spec: Annex E
    // §AIMS RL-2 + RL-4.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = project_borrowed_view_func(&mut pool, &interner, false, true);
    let pbd = crate::aims::emit_rc::borrowed_defs::collect_project_borrowed_defs(&func, &pool);
    assert!(pbd.contains(&v(2)), "%2 is a non-take Project borrow-view");
    let strips = compute_redundant_project_borrowed_view_dec_strips(&func, &pool, &pbd);
    assert!(
        strips.contains(&v(2)),
        "the single-ref str-field view dec is redundant (the aggregate drop frees the field); \
         got {strips:?}",
    );
}

#[test]
fn project_borrowed_view_keeps_paired_inc_shared_aggregate_dec() {
    // NEGATIVE pin: the aggregate `%1` IS bumped by a keep-alive `burden_inc %1`
    // (shared, rc >= 2 at the projection point). The view's `burden_dec %2`
    // releases the EXTRA reference, NOT a redundant second release of a single-ref
    // field -> KEEP. Spec: Annex E §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = project_borrowed_view_func(&mut pool, &interner, true, true);
    let pbd = crate::aims::emit_rc::borrowed_defs::collect_project_borrowed_defs(&func, &pool);
    let strips = compute_redundant_project_borrowed_view_dec_strips(&func, &pool, &pbd);
    assert!(
        strips.is_empty(),
        "a paired-inc shared-aggregate projection dec releases the extra ref and is KEPT; \
         got {strips:?}",
    );
}

#[test]
fn project_borrowed_view_keeps_view_dec_when_no_aggregate_dec() {
    // NEGATIVE pin: the aggregate `%1` carries NO freeing `burden_dec` (the field
    // release is carried by the view dec alone — the no-scope-exit-aggregate-dec
    // tuple shape). Stripping the view dec would LEAK the field -> KEEP. Spec:
    // Annex E §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = project_borrowed_view_func(&mut pool, &interner, false, false);
    let pbd = crate::aims::emit_rc::borrowed_defs::collect_project_borrowed_defs(&func, &pool);
    let strips = compute_redundant_project_borrowed_view_dec_strips(&func, &pool, &pbd);
    assert!(
        strips.is_empty(),
        "with no aggregate freeing dec, the view dec is the field's only release and is KEPT; \
         got {strips:?}",
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
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
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
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
    assert_eq!(
        releases.len(),
        1,
        "exactly one release on the conversion RESULT; got {releases:?}",
    );
    assert_eq!(releases[0].1, v(1), "frees the @values result list %1");
}

/// Single-block-then-successors function modelling `a.union(b)`: two fresh Set
/// Constructs `%0`(a)/`%1`(b), a `union` Invoke producing the fresh owned Set
/// result `%2`, borrowed-read by `@len` then dead at scope exit. The result %2
/// carries the dup-use fresh inc + a paired scope-exit dec (net 0 on explicit
/// ops; the alloc `+1` leaks pre-cure). Models the set-algebra fresh-owned-result
/// shape for [`compute_dead_owned_collection_releases`].
fn set_union_result_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let set_ty = pool.set(Idx::INT);
    let union_name = interner.intern("union");
    let len_name = interner.intern("len");
    // %0 set a, %1 set b, %2 union result set, %3 scalar len, %4 scalar return,
    // %5/%6 scalar elements for the two literals.
    let var_types = vec![
        set_ty,
        set_ty,
        set_ty,
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
                        ty: set_ty,
                        ctor: CtorKind::SetLiteral,
                        args: vec![v(5)],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: set_ty,
                        ctor: CtorKind::SetLiteral,
                        args: vec![v(6)],
                    },
                    // The result %2 carries the dup-use fresh inc + a scope-exit dec
                    // (net 0 on explicit ops; alloc +1 leaks pre-cure).
                    ArcInstr::BurdenInc { var: v(2) },
                    ArcInstr::BurdenDec { var: v(2) },
                ],
                // %2 = @union(%0 [own], %1 [borrow]) — fresh owned Set result.
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: set_ty,
                    func: union_name,
                    args: vec![v(0), v(1)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                // @len(%2 [borrow]) — borrowed read; %2 dead afterward.
                terminator: ArcTerminator::Invoke {
                    dst: v(3),
                    ty: Idx::INT,
                    func: len_name,
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(3),
                    unwind: ArcBlockId::new(4),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(4) },
            },
            ArcBlock {
                id: ArcBlockId::new(4),
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
fn fresh_owned_collection_reps_includes_set_union_result() {
    // The set-algebra result (`a.union(b)`, the Invoke-terminator result %2) is a
    // FRESH owned Set the recognizer MUST classify so the alloc-aware net frees
    // it at its borrowed-read scope-exit sink (RL-2). Pre-cure the recognizer
    // omits set-algebra producers (only conversion / iter-consumer / COW results)
    // and %2 leaks; post-cure %2's jump-threaded rep is in the candidate set.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = set_union_result_func(&mut pool, &interner);
    let jt_reps = compute_jump_threaded_reps(&func, None);
    let rep_of = |x: ArcVarId| jt_reps.get(&x).copied().unwrap_or(x);
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let reps =
        compute_fresh_owned_collection_reps(&func, &pool, &jt_reps, &same_alloc_reps, &interner);
    assert!(
        reps.contains(&rep_of(v(2))),
        "the @union result %2 must be a fresh-owned-collection candidate; got {reps:?}",
    );
}

#[test]
fn set_algebra_names_covers_union_difference_intersection() {
    // The set-algebra name set is the SSOT for the three fresh-Set producers.
    let interner = ori_ir::StringInterner::new();
    let names = collection_set_algebra_names(&interner);
    for n in ["union", "difference", "intersection"] {
        assert!(
            names.contains(&interner.intern(n)),
            "set-algebra name set must contain {n}",
        );
    }
    // A non-producer (`to_list` is a conversion, not set-algebra) is NOT in this set.
    assert!(
        !names.contains(&interner.intern("to_list")),
        "to_list is a conversion, not a set-algebra producer",
    );
}

/// Single-block-then-successors function where a USER-FUNCTION call returns a
/// fresh owned `str` (`%2 = Apply @make_label(%0 [own])`), the result carries a
/// dup-use keep-alive `BurdenInc` + scope-exit `BurdenDec` (net 0 on explicit
/// ops; alloc +1 leaks pre-cure), then `%2` is borrowed-read by
/// `@contains(%2 [borrow])` and dead. `%0` is a scalar seed arg (NOT same-alloc
/// with `%2`), so the result is a genuine fresh str — not a Direct-transfer
/// forwarder. `func` for [`compute_fresh_owned_collection_reps`].
fn user_call_str_result_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let _ = pool;
    let make_label = interner.intern("make_label");
    let contains = interner.intern("contains");
    // %0 scalar arg, %1 scalar substr-arg slot, %2 fresh str result, %3 scalar
    // bool result, %4 scalar return.
    let var_types = vec![Idx::INT, Idx::STR, Idx::STR, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::Scalar,
        ValueRepr::FatValue,
        ValueRepr::FatValue,
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
                    // %2 = @make_label(%0 [own]) — fresh owned str result.
                    ArcInstr::Apply {
                        dst: v(2),
                        ty: Idx::STR,
                        func: make_label,
                        args: vec![v(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                    // dup-use keep-alive inc + scope-exit dec (net 0 explicit; alloc +1).
                    ArcInstr::BurdenInc { var: v(2) },
                    ArcInstr::BurdenDec { var: v(2) },
                ],
                // @contains(%2 [borrow], %1 [borrow]) — borrowed read; %2 dead after.
                terminator: ArcTerminator::Invoke {
                    dst: v(3),
                    ty: Idx::INT,
                    func: contains,
                    args: vec![v(2), v(1)],
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
fn fresh_owned_collection_reps_includes_user_call_str_result() {
    // The fresh owned `str` returned by a non-builtin user call (`%2 =
    // @make_label(..)`, dup-read then dead) MUST be a fresh-owned candidate so the
    // alloc-aware net frees it at its borrowed-read scope-exit sink (RL-2). Pre-cure
    // the user-call recognizer arm gated on `is_collection_dst` (List/Map/Set only)
    // and a str result was omitted -> leak. Post-cure the gate is
    // `is_collection_or_str_dst` and %2's jump-threaded rep is in the candidate set.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = user_call_str_result_func(&mut pool, &interner);
    let jt_reps = compute_jump_threaded_reps(&func, None);
    let rep_of = |x: ArcVarId| jt_reps.get(&x).copied().unwrap_or(x);
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let reps =
        compute_fresh_owned_collection_reps(&func, &pool, &jt_reps, &same_alloc_reps, &interner);
    assert!(
        reps.contains(&rep_of(v(2))),
        "the fresh user-call str result %2 must be a fresh-owned candidate; got {reps:?}",
    );
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

/// Build a TWO-BLOCK inline-`@iter` multi-borrow func where the FIRST
/// `@iter(%2 [own])` in bb0 is FOLLOWED by burden ops on the source `%0`
/// (`BurdenInc; BurdenDec; BurdenDec` — the Phase-5 fresh-inc + multi-use move
/// decs), and the source threads to bb1 (`Jump bb1(%0)` → param `%4`) where the
/// SECOND `@iter(%5 [own])` lives. The source lineage rep unifies `%0/%2/%4/%5`
/// via Let + Jump-arg threading. Mirrors the real `for s in items do ..; for s in
/// items do ..` shape: the first-use index points PAST the bb0 body once the
/// burden-op `retain` strips the trailing source ops, so a stale-index
/// keep-alive resolution silently emits nothing (the index-invalidation bug).
fn inline_iter_two_block_trailing_burden_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let iter_name = interner.intern("iter");
    // %0 source [int]-list, %1/%3 iter handles (scalar), %2 bb0 Let alias,
    // %4 bb1 threaded param, %5 bb1 Let alias.
    let var_types = vec![list_ty, Idx::INT, list_ty, Idx::INT, list_ty, list_ty];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
    ];
    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Construct {
                dst: v(0),
                ty: list_ty,
                ctor: CtorKind::ListLiteral,
                args: Vec::new(),
            },
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::Let {
                dst: v(2),
                ty: list_ty,
                value: ArcValue::Var(v(0)),
            },
            // First iter-consume (recorded at this index against the un-mutated body).
            ArcInstr::Apply {
                dst: v(1),
                ty: Idx::INT,
                func: iter_name,
                args: vec![v(2)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
            // Trailing source burden ops the `retain` strips → shift the recorded
            // first-use index PAST the (post-retain) bb0 body.
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(0) },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(0)],
        },
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v(4), list_ty)],
        body: vec![
            ArcInstr::Let {
                dst: v(5),
                ty: list_ty,
                value: ArcValue::Var(v(4)),
            },
            // Second iter-consume on the threaded source.
            ArcInstr::Apply {
                dst: v(3),
                ty: Idx::INT,
                func: iter_name,
                args: vec![v(5)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
        ],
        terminator: ArcTerminator::Return { value: v(3) },
    };
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![bb0, bb1],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn multi_borrow_iter_consume_keep_alive_survives_index_shift_from_burden_strip() {
    // Regression pin (index-invalidation): the first iter-consume use is recorded
    // at a bb0 index that points PAST the body once the burden-op `retain` strips
    // the trailing source `BurdenInc`/`BurdenDec` ops. A stale-index keep-alive
    // resolution returns None at that out-of-bounds index → emits ZERO keep-alive
    // → the source buffer (rc=1) is freed by BOTH iter-drops = double-free. The
    // keep-alive arg MUST be resolved pre-retain and re-located post-retain so
    // exactly (N-1)=1 keep-alive inc survives on the source lineage.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func = inline_iter_two_block_trailing_burden_func(&mut pool, &interner);
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();

    assert_eq!(
        source_dec_count(&func, v(0)),
        2,
        "precondition: two spurious source decs present in bb0"
    );
    suppress_multi_borrow_iter_consume_source_decs(&mut func, &pool, &interner, &contracts);
    assert_eq!(
        source_dec_count(&func, v(0)),
        0,
        "all normal-path source decs removed (the for-loop iter-drops free the buffer)"
    );
    // (N-1)=1 keep-alive inc must SURVIVE — the index-shift must not drop it.
    // The lineage's incs live on its SSA-alias members (%0/%2/%4/%5).
    let total_inc: usize = [v(0), v(2), v(4), v(5)]
        .iter()
        .map(|&m| source_inc_count(&func, m))
        .sum();
    assert_eq!(
        total_inc, 1,
        "exactly one keep-alive inc (N-1) survives the burden-strip index shift; got {total_inc}",
    );
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

#[test]
fn single_borrowed_invoke_iter_consume_dead_source_strips_all_burden_ops() {
    // Positive pin (Phase 6.66c): an owned FRESH collection (`%0 = Construct`)
    // passed at a BORROWED arg to a USER callee whose `iter_consumes` is true, then
    // DEAD (Returns the scalar result `%1`), gets a spurious FRESH `BurdenInc` +
    // misplaced scope-exit `BurdenDec` from the Phase-5 walk. The callee's
    // `ori_iter_drop` is the single release (RL2_iter_consuming_no_caller_dec), so
    // ALL caller burden ops on the source lineage must be stripped.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (mut func, cons) = multi_borrow_iter_consume_func(&mut pool, &interner, 1);
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(cons, iter_consume_contract(true));

    assert_eq!(
        source_inc_count(&func, v(0)),
        1,
        "precondition: one spurious FRESH inc on the source"
    );
    assert_eq!(
        source_dec_count(&func, v(0)),
        1,
        "precondition: one spurious source dec"
    );
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    suppress_single_borrowed_invoke_iter_consume_source(
        &mut func,
        &pool,
        &interner,
        &contracts,
        &same_alloc_reps,
    );
    assert_eq!(
        source_inc_count(&func, v(0)),
        0,
        "the spurious FRESH inc must be stripped (callee iter-drop is the single release)"
    );
    assert_eq!(
        source_dec_count(&func, v(0)),
        0,
        "the spurious source dec must be stripped"
    );
}

#[test]
fn single_borrowed_invoke_iter_consume_declines_two_consume_source() {
    // Negative pin (the N >= 2 upper boundary): a source with TWO user-callee
    // iter-consume uses is the multi-borrow case (Phase 6.66's domain). Phase 6.66c
    // must DECLINE — stripping the FRESH inc here would leave the source freed by
    // both callee iter-drops with no keep-alive = double-free. The `uses.len() != 1`
    // gate is the boundary.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let (mut func, cons) = multi_borrow_iter_consume_func(&mut pool, &interner, 2);
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(cons, iter_consume_contract(true));

    // Confirm the discriminator sees TWO user-callee iter-consume uses.
    let jt_reps = compute_jump_threaded_reps(&func, None);
    let rep_of = |x: ArcVarId| jt_reps.get(&x).copied().unwrap_or(x);
    assert_eq!(
        user_callee_iter_consume_uses_of_rep(&func, &contracts, v(0), &rep_of).len(),
        2,
        "precondition: two user-callee iter-consume uses (multi-borrow)"
    );
    let inc_before = source_inc_count(&func, v(0));
    let dec_before = source_dec_count(&func, v(0));
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    suppress_single_borrowed_invoke_iter_consume_source(
        &mut func,
        &pool,
        &interner,
        &contracts,
        &same_alloc_reps,
    );
    assert_eq!(
        source_inc_count(&func, v(0)),
        inc_before,
        "two-consume source must be untouched (N >= 2 is Phase 6.66's multi-borrow domain)"
    );
    assert_eq!(
        source_dec_count(&func, v(0)),
        dec_before,
        "two-consume source decs untouched"
    );
}

#[test]
fn single_borrowed_invoke_iter_consume_declines_genuinely_read_source() {
    // Negative pin (the over-fire boundary `lineage_genuinely_read_outside_call`):
    // a source iter-consumed once by a user callee BUT genuinely re-consumed by a
    // downstream inline `@iter(%alias [own])` (`for w in words` after the call) is
    // a 2-consume shape whose second consume is reached through a `Let { Var }`
    // alias the jump-rep map does not connect. Stripping the keep-alive here =
    // double-free. The Let-Var lineage closure MUST detect the second consume so
    // the pass DECLINES.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let iter_name = interner.intern("iter");
    let cons = interner.intern("user_iter_cons_zzz");
    let list_ty = pool.list(Idx::INT);
    // %5 is the (scalar) element of the NON-empty source Construct — an empty-literal
    // Construct is excluded from `compute_fresh_owned_collection_reps` (no backing
    // buffer), which would make the pass skip the source for the wrong reason and
    // render this negative pin vacuous.
    let var_types = vec![list_ty, Idx::INT, list_ty, list_ty, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0 source Construct (non-empty)
        ValueRepr::Scalar,    // %1 user-callee result
        ValueRepr::RcPointer, // %2 Let-Var alias for the user-callee borrowed arg
        ValueRepr::RcPointer, // %3 Let-Var alias for the second inline @iter
        ValueRepr::Scalar,    // %4 second @iter handle
        ValueRepr::Scalar,    // %5 source element
    ];
    let func = ArcFunction {
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
                    args: vec![v(5)],
                },
                ArcInstr::BurdenInc { var: v(0) },
                // user-callee iter-consume at a borrowed arg (the recorded use).
                ArcInstr::Let {
                    dst: v(2),
                    ty: list_ty,
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: cons,
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::BurdenDec { var: v(0) },
                // SECOND consume: inline `@iter(%3 [own])` reached via a Let-Var
                // alias `%3 = %0` — a genuine downstream read of the source.
                ArcInstr::Let {
                    dst: v(3),
                    ty: list_ty,
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::Apply {
                    dst: v(4),
                    ty: Idx::INT,
                    func: iter_name,
                    args: vec![v(3)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    };
    let mut contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    contracts.insert(cons, iter_consume_contract(true));

    // The genuine-read check MUST see the second inline `@iter` consume (reached via
    // the `%3 = Var(%0)` Let-Var alias) so the pass declines.
    let jt_reps = compute_jump_threaded_reps(&func, None);
    let rep_of = |x: ArcVarId| jt_reps.get(&x).copied().unwrap_or(x);
    assert!(
        lineage_genuinely_read_outside_call(&func, v(0), &rep_of, 0, Some(3)),
        "the second inline @iter consume (via Let-Var alias) is a genuine read"
    );

    let mut func = func;
    let inc_before = source_inc_count(&func, v(0));
    let same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    suppress_single_borrowed_invoke_iter_consume_source(
        &mut func,
        &pool,
        &interner,
        &contracts,
        &same_alloc_reps,
    );
    assert_eq!(
        source_inc_count(&func, v(0)),
        inc_before,
        "genuinely-read source must be untouched (the second @iter consume keeps the keep-alive)"
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

/// Build the element-escape shape `@collect(words: [str] [borrow]) -> [str]`:
/// `for w in words do { result = result.push(value: w) }; result`. Var layout —
/// %0 borrowed `[str]` param, %1 result `[str]` (`Construct List()`), %2 iterator
/// handle, %3 elem-marker phantom, %4 `__iter_next` result, %5 element view
/// (`Project %4.1`, the iter-element borrow-view of `words`), %6 push result.
/// bb0 builds the result + iterates; bb1 is the loop header projecting the element
/// and PUSHING it `[own]` into the result at the terminator (`Invoke @push(result
/// [own], view [own])`); bb2 is the loop body re-binding the push result;
/// `returned` controls whether bb3 RETURNS the result (the element-escape case) or
/// iterates it IN-SCOPE then returns a scalar (the benign over-fire-boundary case).
#[expect(
    clippy::too_many_lines,
    reason = "one cohesive 4-block CFG fixture builder; splitting mid-CFG \
              fragments the var-layout documented above"
)]
fn iter_element_pushed_collection_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    returned: bool,
) -> ArcFunction {
    let list_ty = pool.list(Idx::STR);
    let iter_name = interner.intern("iter");
    let iter_next = interner.intern("__iter_next");
    let push_name = interner.intern("push");
    let iter_drop = interner.intern("ori_iter_drop");

    // %0 borrowed param, %1 result list, %2 iter handle, %3 phantom, %4 next
    // result, %5 element view, %6 push result.
    let var_types = vec![
        list_ty,
        list_ty,
        Idx::INT,
        Idx::STR,
        Idx::INT,
        Idx::STR,
        list_ty,
    ];
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0 words
        ValueRepr::RcPointer, // %1 result
        ValueRepr::Scalar,    // %2 iter handle
        ValueRepr::FatValue,  // %3 phantom marker
        ValueRepr::Scalar,    // %4 next result
        ValueRepr::FatValue,  // %5 element view (str)
        ValueRepr::RcPointer, // %6 push result
    ];

    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![
            ArcInstr::Construct {
                dst: v(1),
                ty: list_ty,
                ctor: CtorKind::ListLiteral,
                args: Vec::new(),
            },
            // @iter(%0 [own]) — iterate the BORROWED source param.
            ArcInstr::Apply {
                dst: v(2),
                ty: Idx::INT,
                func: iter_name,
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
        ],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(1)],
        },
    };
    // bb1 loop header: project the element view; push it [own] into the result at
    // the terminator Invoke (re-binds the result lineage to %6, normal -> bb2).
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v(1), list_ty)],
        body: vec![
            ArcInstr::Apply {
                dst: v(4),
                ty: Idx::INT,
                func: iter_next,
                args: vec![v(2), v(3)],
                arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                mono_instance_id: None,
            },
            // %5 = Project %4.1 — the iter-element borrow-view of `words`.
            ArcInstr::Project {
                dst: v(5),
                ty: Idx::STR,
                value: v(4),
                field: 1,
            },
        ],
        terminator: ArcTerminator::Invoke {
            dst: v(6),
            ty: list_ty,
            func: push_name,
            args: vec![v(1), v(5)],
            arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
            mono_instance_id: None,
            normal: ArcBlockId::new(2),
            unwind: ArcBlockId::new(4),
        },
    };
    // bb2 loop body: re-bind and jump back to the loop header.
    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(6)],
        },
    };
    // bb3 loop exit: RETURN the result (element-escape) OR iterate it in-scope
    // (`@iter(result [own])`) then return a scalar (benign over-fire boundary).
    let bb3 = if returned {
        ArcBlock {
            id: ArcBlockId::new(3),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: v(4),
                ty: Idx::INT,
                func: iter_drop,
                args: vec![v(2)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }
    } else {
        ArcBlock {
            id: ArcBlockId::new(3),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: v(4),
                    ty: Idx::INT,
                    func: iter_drop,
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                // @iter(result [own]) — in-scope iterate; the result NEVER escapes.
                ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::INT,
                    func: iter_name,
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: v(4) },
        }
    };
    let bb4 = ArcBlock {
        id: ArcBlockId::new(4),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Resume,
    };

    ArcFunction {
        var_types,
        var_reprs,
        params: vec![ArcParam {
            var: v(0),
            ty: list_ty,
            ownership: Ownership::Borrowed,
        }],
        blocks: vec![bb0, bb1, bb2, bb3, bb4],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn iter_element_pushed_into_returned_collection_gets_keepalive_inc() {
    // POSITIVE: a borrowed iter-element view (`Project %4.1` of `words`) pushed
    // `[own]` into a `result` collection that is RETURNED. Phase 6.68b emits exactly
    // ONE keep-alive `BurdenInc` on the element view (RL-1 duplication) so the
    // source's in-callee `ori_iter_drop` and the caller's `elem_dec_fn` over the
    // returned `result` each release once instead of double-freeing the rc-1 backing.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func =
        iter_element_pushed_collection_func(&mut pool, &interner, /*returned=*/ true);
    emit_iter_element_pushed_into_returned_collection_keepalive_inc(&mut func, &interner, &pool);
    let elem_inc = source_inc_count(&func, v(5));
    assert_eq!(
        elem_inc, 1,
        "a borrowed iter-element view pushed into a RETURNED collection MUST get \
         exactly one keep-alive inc; got {elem_inc}; blocks={:?}",
        func.blocks,
    );
    // The receiver result-list lineage MUST NOT itself get the element keep-alive.
    assert_eq!(
        source_inc_count(&func, v(1)),
        0,
        "the receiver collection lineage MUST NOT get the element keep-alive inc; \
         blocks={:?}",
        func.blocks,
    );
}

#[test]
fn iter_element_pushed_into_in_scope_collection_gets_no_keepalive() {
    // NEGATIVE over-fire boundary: the SAME borrowed iter-element push, but the
    // `result` collection is iterated IN-SCOPE (`@iter(result [own])`) and NEVER
    // returned. The source iter-drop and the in-scope `result` drop are sequenced
    // within one function, so the base accounting already balances them — a
    // keep-alive here orphans a +1 -> leak. Phase 6.68b MUST decline (the
    // `collection_receiver_returned` gate is false).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let mut func =
        iter_element_pushed_collection_func(&mut pool, &interner, /*returned=*/ false);
    emit_iter_element_pushed_into_returned_collection_keepalive_inc(&mut func, &interner, &pool);
    assert_eq!(
        source_inc_count(&func, v(5)),
        0,
        "a borrowed iter-element view pushed into an IN-SCOPE (non-returned) \
         collection MUST NOT get a keep-alive inc; blocks={:?}",
        func.blocks,
    );
}

/// `MaybeIter = Empty | Holds(it: Iterator<int>)` constructed via a `Construct
/// Variant`, then the iterator PROJECTED OUT on the `Holds` arm. Var layout:
/// %0 list, %1 iterator handle, %2 `MaybeIter` source enum, %3 projected iterator
/// (`Project %2.1`), %4 scalar elem. Single-block; the source enum is the
/// take-project source; %3 is the projected iterator handle.
fn take_project_source_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let iter_ty = pool.double_ended_iterator(Idx::INT);
    let enum_name = interner.intern("MaybeIter");
    let empty_name = interner.intern("Empty");
    let holds_name = interner.intern("Holds");
    let enum_ty = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: empty_name,
                field_types: vec![],
            },
            EnumVariant {
                name: holds_name,
                field_types: vec![iter_ty],
            },
        ],
    );
    let iter_name = interner.intern("iter");
    // %0 list, %1 iter handle, %2 enum, %3 projected iter, %4 scalar.
    let var_types = vec![list_ty, iter_ty, enum_ty, iter_ty, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Aggregate,
        ValueRepr::RcPointer,
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
                    args: vec![v(4)],
                },
                ArcInstr::Apply {
                    dst: v(1),
                    ty: iter_ty,
                    func: iter_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Construct {
                    dst: v(2),
                    ty: enum_ty,
                    ctor: CtorKind::EnumVariant {
                        enum_name,
                        variant: 1,
                    },
                    args: vec![v(1)],
                },
                // `Project %2.1` — the iterator projected OUT of the enum (a
                // take-project site: source `Tag::Enum`, dst `Tag::Iterator`).
                ArcInstr::Project {
                    dst: v(3),
                    ty: iter_ty,
                    value: v(2),
                    field: 1,
                },
            ],
            terminator: ArcTerminator::Return { value: v(4) },
        }],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn take_project_candidate_includes_projected_iterator_handle() {
    // Phase-6.9 candidate case (c): an iterator PROJECTED OUT of an enum
    // (`%3 = Project %2.1`, `Tag::Iterator`) is a dead-iterator-handle candidate —
    // on a take-project UNUSED arm it is the freeing value (the source enum's own
    // dec is suppressed by the Phase-6.10 strip). Without this the unused-binding
    // projected iterator leaks.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = take_project_source_func(&mut pool, &interner);
    let jt_reps = compute_jump_threaded_reps(&func, None);
    let candidates = compute_dead_iterator_handle_candidates(&func, &pool, &jt_reps);
    assert!(
        candidates.contains(&v(3)),
        "the projected iterator handle %3 must be a dead-iterator-handle candidate; \
         got {candidates:?}",
    );
}

#[test]
fn take_project_source_plan_strips_source_enum_not_projected_handle() {
    // Phase-6.10 strip-var classification: the take-project SOURCE enum (%2,
    // InlineEnum) is in the strip set (its spurious copy/last-use ops are removed);
    // the iterator PROJECTED OUT of it (%3, `is_iterator_handle_dst`) is NOT — it is
    // a separate freeing value owned by Phase 6.9. A full plan needs the per-block
    // entry-state map; this pin verifies the source-rep + iterator-handle-exclusion
    // classification the plan is built on.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = take_project_source_func(&mut pool, &interner);
    let take_move_facts = crate::aims::emit_rc::take_project::analyze(&func, &pool);

    // The source enum %2 IS a take-project source (the `value` of the `Project`).
    let sites = crate::aims::emit_rc::take_project::collect_take_project_sites(&func, &pool);
    assert!(
        sites.iter().any(|&(_, src)| src == v(2)),
        "the source enum %2 must be a take-project site source; got {sites:?}",
    );
    // %2 is in-class (the take-project membership set).
    assert!(
        take_move_facts.is_in_class(v(2)),
        "the source enum %2 must be in the take-project class",
    );
    // %2 is NOT a bare iterator handle (it is the InlineEnum source -> stripped).
    assert!(
        !super::is_iterator_handle_dst(v(2), &func, &pool),
        "the source enum %2 is an Aggregate-repr enum, NOT a bare iterator handle",
    );
    // %3 (the projected iterator) IS a bare iterator handle -> excluded from the
    // strip set (Phase 6.9 owns its freeing).
    assert!(
        super::is_iterator_handle_dst(v(3), &func, &pool),
        "the projected iterator %3 must be a bare iterator handle (excluded from strip)",
    );
}

#[test]
fn borrow_survives_transform_set_and_verdict_classification() {
    // The borrow-survives set covers filter/map (fresh result) + clone (rc-inc
    // alias) — all relocate the borrowed source dec to BOTH successor edges
    // (`EdgeRelease::Both`). Seamless-slice / shared-buffer methods (slice/take/
    // substring) are EXCLUDED: empirically they over-fire (a relocated source dec
    // double-frees on slice/take shapes where the source is not single-dead-after-
    // call). The clone inclusion resolves the clone-vs-buffer-sharing
    // contract-indistinguishability via the escape-gated relocation.
    let interner = ori_ir::StringInterner::new();
    let set = borrow_survives_transform_names(&interner);
    let conversion = collection_conversion_names(&interner);
    let accessor = crate::borrow::accessor_retain_builtin_names(&interner);
    let sharing_view = sharing_view_relocation_names(&interner);
    let fresh_str = fresh_str_producing_method_names(&interner);
    let set_algebra = set_algebra_relocation_names(&interner);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(&interner);
    let contracts: FxHashMap<ori_ir::Name, MemoryContract> = FxHashMap::default();
    let names = EscapeSafeBorrowedNames {
        conversion: &conversion,
        survives_transform: &set,
        accessor_retain: &accessor,
        sharing_view: &sharing_view,
        fresh_str: &fresh_str,
        set_algebra: &set_algebra,
        builtins: &builtins,
    };

    // Positive set membership.
    for name in ["filter", "map", "clone"] {
        assert!(
            set.contains(&interner.intern(name)),
            "{name} must be in the borrow-survives transform set",
        );
    }
    // Negative set membership: sharing-view methods stay OUT (they over-fire).
    for name in ["slice", "take", "substring"] {
        assert!(
            !set.contains(&interner.intern(name)),
            "{name} must NOT be in the borrow-survives transform set (over-fires)",
        );
    }

    // Verdict: a NON-scalar (`[int]`) result of a borrow-survives transform yields
    // `Both` (relocate to both edges) DESPITE the scalar gate the verdict applies
    // to other non-conversion callees.
    for name in ["filter", "map", "clone"] {
        assert_eq!(
            borrowed_arg_release_verdict(
                interner.intern(name),
                0,
                false, // non-scalar [int] result
                &names,
                &contracts,
            ),
            Some(EdgeRelease::Both),
            "{name} with a non-scalar result must relocate to Both edges",
        );
    }
    // Verdict: a sharing-view method (`slice`/`substring`/`take`/`drop`) with a
    // non-scalar shared-buffer result relocates to ONE post-dominating edge
    // (`PostDominator`), NOT `Both` (which over-fires when the result is read
    // across `&&` branches). Checked for every sharing-view producer.
    for name in ["slice", "substring", "take", "drop"] {
        assert_eq!(
            borrowed_arg_release_verdict(interner.intern(name), 0, false, &names, &contracts),
            Some(EdgeRelease::PostDominator),
            "{name} (sharing view) must relocate to one post-dominating edge",
        );
    }
    // Verdict: a set-algebra op (`union`/`intersection`/`difference`) with a
    // non-scalar `{T}` Set result yields `Both` (the borrowed `other` arg's
    // elements are rc-inc'd into a FRESH result, so `other` survives the call and
    // is dead on each successor). Checked for every set-algebra op.
    for name in ["union", "intersection", "difference"] {
        assert_eq!(
            borrowed_arg_release_verdict(interner.intern(name), 1, false, &names, &contracts),
            Some(EdgeRelease::Both),
            "{name} (set-algebra) with a non-scalar Set result must relocate to Both edges",
        );
    }
}

#[test]
fn set_algebra_relocation_names_covers_union_intersection_difference() {
    // The set-algebra relocation set is EXACTLY the 3 element-retaining ops whose
    // borrowed `other` arg has its surviving elements rc-inc'd into a fresh result
    // (`inc_copied_set_elements`). TIGHT: COW mutators / comparison-only ops
    // (`remove`/`contains`) and fresh-buffer producers stay OUT (no element-retain
    // into a borrowed-arg-survives result).
    let interner = ori_ir::StringInterner::new();
    let set = set_algebra_relocation_names(&interner);
    for name in ["union", "intersection", "difference"] {
        assert!(
            set.contains(&interner.intern(name)),
            "{name} must be in the set-algebra relocation set",
        );
    }
    for name in ["remove", "contains", "insert", "filter", "map", "to_list"] {
        assert!(
            !set.contains(&interner.intern(name)),
            "{name} must NOT be in the set-algebra relocation set",
        );
    }
}

#[test]
fn sharing_view_relocation_names_covers_slice_substring_take_drop() {
    // The sharing-view set extends the `crate::borrow::sharing_builtin_names`
    // SSOT (`slice`/`substring`) with `take`/`drop` (also `make_slice_cap` slice
    // views) and excludes COW-mutator / conversion producers (those allocate a
    // FRESH buffer, NOT a shared view).
    let interner = ori_ir::StringInterner::new();
    let set = sharing_view_relocation_names(&interner);
    for name in ["slice", "substring", "take", "drop"] {
        assert!(
            set.contains(&interner.intern(name)),
            "{name} must be in the sharing-view relocation set",
        );
    }
    for name in [
        "filter", "map", "clone", "keys", "values", "union", "to_list",
    ] {
        assert!(
            !set.contains(&interner.intern(name)),
            "{name} (fresh-buffer producer) must NOT be in the sharing-view set",
        );
    }
}

#[test]
fn iterator_consumer_collection_names_covers_collect_not_adapters() {
    // The iterator-consumer set covers `collect` / `collect_set` (fresh owned
    // collection results) and EXCLUDES the iterator adapters / sources (`iter` /
    // `map` / `filter` produce iterator HANDLES freed by `ori_iter_drop`, not
    // collections) and the conversion builtins (handled by their own set).
    let interner = ori_ir::StringInterner::new();
    let set = iterator_consumer_collection_names(&interner);
    for name in ["collect", "collect_set"] {
        assert!(
            set.contains(&interner.intern(name)),
            "{name} must be in the iterator-consumer collection set",
        );
    }
    for name in ["iter", "map", "filter", "keys", "values"] {
        assert!(
            !set.contains(&interner.intern(name)),
            "{name} must NOT be in the iterator-consumer collection set",
        );
    }
}

/// Build an iter-chain collect shape: a fresh `Construct List` source (`%0`) is
/// consumed at the OWNED arg of `@collect` (`%0` -> iterator-drop machinery), which
/// produces a FRESH owned `[int]` result `%1`; `%1` is aliased to `%2` and
/// borrowed-read by `@length`, then dead at scope exit. The collect result is the
/// leaked fresh-owned collection (the Phase-5 walk emits zero ops on it).
fn iter_collect_result_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let collect_name = interner.intern("collect");
    let length_name = interner.intern("length");
    // %0 source list, %1 collect result, %2 result alias, %3 scalar len, %4 elem.
    let var_types = vec![list_ty, list_ty, list_ty, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
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
                        args: vec![v(4)],
                    },
                    // `@collect(%0 [own])` -> fresh `[int]` result %1.
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: list_ty,
                        func: collect_name,
                        args: vec![v(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: list_ty,
                        value: ArcValue::Var(v(1)),
                    },
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
fn dead_owned_collection_frees_iter_collect_result_at_scope_exit() {
    // The `@collect` result (%1 -> %2) is a FRESH owned `[int]` the runtime
    // allocates (alloc-aware net `+1`), borrowed-read by `@length`, dead at scope
    // exit. The pass emits exactly ONE freeing dec on the result's live SSA value
    // (%2) at the borrowed-read sink bb0 (RL-2 scope-exit dec). The source list %0
    // is owned-consumed by `@collect` (excluded — the iterator-drop frees it).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = iter_collect_result_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
    assert_eq!(
        releases.len(),
        1,
        "exactly one dead-owned-collection release (the leaked collect result); got {releases:?}",
    );
    let (block_idx, var) = releases[0];
    assert_eq!(block_idx, 0, "release at the borrowed-read sink bb0");
    assert_eq!(
        var,
        v(2),
        "frees the collect result's live SSA value %2 at the borrowed-read sink"
    );
}

/// A `transfers_through_return ∧ Direct` forwarder result shape: `%0 = Construct
/// List` (bb0), `Invoke @id(%0 [own]) -> %1` (bb0 terminator), the result `%1`
/// Let-aliased to `%2` and borrowed-read by `@len` (bb1), dead at scope exit
/// (Return scalar bb3). The result `%1` IS the same allocation as `%0` — the
/// apply-Direct merge is supplied via `same_alloc_reps = {%1 → %0, %2 → %1}`.
fn forwarder_result_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    let list_ty = pool.list(Idx::INT);
    let id_name = interner.intern("id");
    let len_name = interner.intern("len");
    // %0 fresh list, %1 forwarder result, %2 result alias, %3 scalar len, %4 elem.
    let var_types = vec![list_ty, list_ty, list_ty, Idx::INT, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
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
                    ArcInstr::Let {
                        dst: v(4),
                        ty: Idx::INT,
                        value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                    },
                    // Non-empty Construct (the buffer alloc the lineage owns).
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: list_ty,
                        ctor: CtorKind::ListLiteral,
                        args: vec![v(4)],
                    },
                ],
                // The forwarder: `@id(%0 [own])` returns its owned arg Direct, so
                // %1 IS %0's allocation.
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: list_ty,
                    func: id_name,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: list_ty,
                    value: ArcValue::Var(v(1)),
                }],
                // Borrowed read of the result, then dead at scope exit.
                terminator: ArcTerminator::Invoke {
                    dst: v(3),
                    ty: Idx::INT,
                    func: len_name,
                    args: vec![v(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(3),
                    unwind: ArcBlockId::new(4),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return { value: v(3) },
            },
            ArcBlock {
                id: ArcBlockId::new(4),
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
fn dead_owned_collection_frees_forwarder_result_with_apply_direct_seed() {
    // With the apply-Direct seed (`same_alloc_reps` merges %1 → %0), the forwarder
    // result %1 joins %0's fresh-owned-collection lineage. The owned-position arg %0
    // to `@id` is a Direct pass-through (the result carries it forward), so it is NOT
    // excluded as a user-call arg / owned-consume. The lineage's single unbalanced
    // allocation `+1` is released at the result's one borrowed-read dead sink (bb1):
    // exactly one freeing dec on the result's live SSA value (%2).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = forwarder_result_func(&mut pool, &interner);
    let mut same_alloc_reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    same_alloc_reps.insert(v(1), v(0));
    same_alloc_reps.insert(v(2), v(0));
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &same_alloc_reps,
    );
    assert_eq!(
        releases.len(),
        1,
        "exactly one forwarder-result release (the leaked allocation at scope exit); got {releases:?}",
    );
    let (block_idx, var) = releases[0];
    assert_eq!(
        block_idx, 1,
        "release at the result's borrowed-read sink bb1"
    );
    assert_eq!(
        var,
        v(2),
        "frees the forwarder result's live SSA value %2 at the borrowed-read sink"
    );
}

#[test]
fn fresh_owned_collection_reps_classifies_user_call_result_and_excludes_direct_transfer() {
    // The recognizer admits a user-function call returning a collection as a
    // fresh-owned candidate. With NO same-alloc merge (empty map), the `@id` result
    // %1 is a genuine-fresh user-call result → IN the candidate set. With the
    // apply-Direct merge (%1 → %0), the result is a Direct-transfer pass-through →
    // recognized via %0's Construct (the user-call arm excludes it because it
    // same-allocs an arg), so the candidate is %0's rep, never a phantom %1 fresh
    // alloc. Spec: Annex E §AIMS RL-2.
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = forwarder_result_func(&mut pool, &interner);
    let jt_reps = compute_jump_threaded_reps(&func, None);
    let rep_of = |x: ArcVarId| jt_reps.get(&x).copied().unwrap_or(x);

    // Empty same-alloc: the `@id` result %1 reads as a genuine-fresh user-call result.
    let no_merge: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let reps_no_merge =
        compute_fresh_owned_collection_reps(&func, &pool, &jt_reps, &no_merge, &interner);
    assert!(
        reps_no_merge.contains(&rep_of(v(1))),
        "user-call result %1 must be a fresh-owned candidate without a same-alloc merge; got {reps_no_merge:?}",
    );

    // Direct-transfer merge: %1 → %0 → the user-call arm excludes %1 (same-allocs the
    // arg %0); the Construct %0 is the candidate, not a separate fresh %1.
    let mut merged: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    merged.insert(v(1), v(0));
    merged.insert(v(2), v(0));
    let reps_merged =
        compute_fresh_owned_collection_reps(&func, &pool, &jt_reps, &merged, &interner);
    assert!(
        reps_merged.contains(&rep_of(v(0))),
        "the Construct %0 lineage must be the candidate under the Direct-transfer merge; got {reps_merged:?}",
    );
}

#[test]
fn dead_owned_collection_frees_user_call_result_without_apply_direct_seed() {
    // Without the apply-Direct seed (empty `same_alloc_reps`), the forwarder result
    // %1 is a DISTINCT rep from %0, but the user-call-fresh-result path recognizes
    // %1 as a fresh owned `[int]` returned by the non-builtin `@id` (no same-alloc
    // merge with any arg → treated as a genuine fresh allocation). The lineage nets
    // `+1` and the pass emits EXACTLY ONE release of the result at its borrowed-read
    // dead sink (bb1, %2) — the allocation is freed exactly once either way. With the
    // seed (the companion `_with_apply_direct_seed` test), the same single release
    // fires via the merged `%0` Construct lineage with the user-call `+1` suppressed
    // (Direct-transfer detected); the two recognition paths converge on one release,
    // never double-fire. Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
    let mut pool = ori_types::Pool::new();
    let interner = ori_ir::StringInterner::new();
    let func = forwarder_result_func(&mut pool, &interner);
    let releases = compute_dead_owned_collection_releases(
        &func,
        &pool,
        &interner,
        &FxHashMap::default(),
        &FxHashMap::default(),
    );
    assert_eq!(
        releases.len(),
        1,
        "exactly one user-call-result release without the seed (the result is the freed allocation); got {releases:?}",
    );
    assert_eq!(
        releases[0],
        (1, v(2)),
        "frees the user-call result's live SSA value %2 at the borrowed-read sink bb1",
    );
}

/// Build a function exercising the branch-dead-value RL-4 edge shape: a fresh
/// heap `str` `%0` defined in bb0, the function branches `%1 ? bb1 : bb2`; bb1
/// reads `%0` borrowed (its release lives on this surviving path), bb2 is an
/// early-return where `%0` is dead. `bb0` dominates bb2; the dead successor bb2 is
/// a single-predecessor NORMAL block. When `return_str_on_dead_branch` is true,
/// bb2 RETURNS `%0` (an RL-2 transfer) instead — the over-fire boundary.
fn branch_dead_str_func(
    interner: &ori_ir::StringInterner,
    return_str_on_dead_branch: bool,
) -> ArcFunction {
    let lit = interner.intern("a heap string past the SSO inline threshold of 23!");
    let read = interner.intern("__some_read");
    // %0 = fresh str literal, %1 = branch scalar cond, %2 = scalar read result,
    // %3 = scalar early-return value.
    let bb2_term = if return_str_on_dead_branch {
        ArcTerminator::Return { value: v(0) }
    } else {
        ArcTerminator::Return { value: v(3) }
    };
    ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::INT, Idx::INT],
        var_reprs: vec![
            ValueRepr::FatValue,
            ValueRepr::Scalar,
            ValueRepr::Scalar,
            ValueRepr::Scalar,
        ],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: v(0),
                        ty: Idx::STR,
                        value: ArcValue::Literal(crate::ir::LitValue::String(lit)),
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(crate::ir::LitValue::Int(1)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            // bb1: surviving path — borrowed read of %0 keeps the lineage alive.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::INT,
                    func: read,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
            // bb2: early-return — %0 dead (or returned in the negative case).
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: v(3),
                    ty: Idx::INT,
                    value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
                }],
                terminator: bb2_term,
            },
        ],
        entry: ArcBlockId::new(0),
        name: ori_ir::Name::from_raw(0),
        ..Default::default()
    }
}

#[test]
fn branch_dead_value_frees_str_on_early_exit_branch() {
    // POSITIVE: a fresh heap str `%0` defined in bb0, read (released) on the bb1
    // surviving path, DEAD on the bb2 early-return path. RL-4 edge cleanup emits
    // exactly ONE dec at the FRONT of bb2 (the dead single-pred normal successor
    // that bb0 dominates). Spec: Annex E §AIMS RL-4.
    let interner = ori_ir::StringInterner::new();
    let pool = ori_types::Pool::new();
    let func = branch_dead_str_func(&interner, false);
    let releases =
        compute_branch_dead_value_releases(&func, &pool, &interner, &FxHashMap::default());
    assert_eq!(
        releases.len(),
        1,
        "exactly one branch-dead edge release (the str dead on bb2); got {releases:?}",
    );
    assert_eq!(
        releases[0],
        (2, v(0)),
        "frees the str %0 at the dead early-return successor bb2",
    );
}

#[test]
fn branch_dead_value_skips_str_returned_on_dead_branch() {
    // NEGATIVE (the over-fire boundary): the str `%0` is RETURNED on bb2 (an RL-2
    // ownership transfer — the caller releases it). The `compute_returned_lineages`
    // exclusion drops it, so NO edge dec fires; a dec here would double-free
    // against the caller's release. Spec: Annex E §AIMS RL-2 + RL-4.
    let interner = ori_ir::StringInterner::new();
    let pool = ori_types::Pool::new();
    let func = branch_dead_str_func(&interner, true);
    let releases =
        compute_branch_dead_value_releases(&func, &pool, &interner, &FxHashMap::default());
    assert!(
        releases.is_empty(),
        "a str returned (transferred) on the dead-looking branch gets NO edge dec; got {releases:?}",
    );
}

/// Build the fresh-aggregate-into-borrowed-call shape (the f06/f12 canonical).
///
/// A `Wrapper { s: str }` aggregate `%1` carries the coalesce-doomed `BurdenInc %1`
/// then `BurdenDec %1` pair in bb0, whose terminator is an `Invoke` of `@desc_len`
/// with `%1` at the receiver position (Borrowed, or Owned when `recv_owned`), normal
/// successor bb1, unwind successor bb2. bb1 returns a scalar (the receiver is dead
/// after the call); bb2 resumes (unwind). A struct holding a str field is the same
/// `is_burden_carrying_aggregate` shape as a heap-payload sum variant; both lower the
/// whole-var `BurdenDec` to a field-walking `RcDec` over the aggregate fields.
fn borrowed_terminator_aggregate_func(
    pool: &mut ori_types::Pool,
    interner: &ori_ir::StringInterner,
    recv_owned: bool,
) -> ArcFunction {
    let s = interner.intern("s");
    let wrapper_name = interner.intern("Wrapper");
    let desc_len = interner.intern("desc_len");
    let wrapper_ty = pool.struct_type(wrapper_name, &[(s, Idx::STR)]);
    // %0 str field (consumed by Construct), %1 Wrapper aggregate, %2 scalar result.
    let recv_ownership = if recv_owned {
        ArgOwnership::Owned
    } else {
        ArgOwnership::Borrowed
    };
    ArcFunction {
        var_types: vec![Idx::STR, wrapper_ty, Idx::INT],
        var_reprs: vec![ValueRepr::FatValue, ValueRepr::Aggregate, ValueRepr::Scalar],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: wrapper_ty,
                        ctor: CtorKind::Struct(wrapper_name),
                        args: vec![v(0)],
                    },
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::BurdenDec { var: v(1) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::INT,
                    func: desc_len,
                    args: vec![v(1)],
                    arg_ownership: vec![recv_ownership],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
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

/// Borrow-read user-fn contract: `desc_len(i: Item) -> int` borrows the receiver
/// (Borrowed access, no return-view aliasing, not iter-consume) so the relocation
/// fires. Returns a one-param `MemoryContract` keyed by the callee name.
fn borrow_read_contracts(
    interner: &ori_ir::StringInterner,
) -> FxHashMap<ori_ir::Name, MemoryContract> {
    let desc_len = interner.intern("desc_len");
    let mut c = MemoryContract::conservative(1);
    c.params[0].access = AccessClass::Borrowed;
    c.params[0].return_alias = None;
    c.params[0].return_payload_contains_param = false;
    c.params[0].iter_consumes = false;
    let mut map = FxHashMap::default();
    map.insert(desc_len, c);
    map
}

#[test]
fn borrowed_terminator_aggregate_relocates_dec_to_edges() {
    // POSITIVE (the f06/f12 shape): a fresh `Wrapper { s: str }` aggregate `%1`
    // carrying the coalesce-doomed `BurdenInc %1`/`BurdenDec %1` pair, passed
    // BORROWED to a borrow-read callee at the bb0 `Invoke` terminator, dead after.
    // The relocation fires ONE entry (block 0, recv %1, normal bb1, unwind bb2) so
    // the moved-in str field is freed at the variant's scope-exit drop on the
    // successor edges. Spec: Annex E §AIMS RL-2 + RL-4.
    let interner = ori_ir::StringInterner::new();
    let mut pool = ori_types::Pool::new();
    let func = borrowed_terminator_aggregate_func(&mut pool, &interner, false);
    let contracts = borrow_read_contracts(&interner);
    let relocations =
        compute_borrowed_terminator_aggregate_relocations(&func, &pool, &interner, &contracts);
    assert_eq!(
        relocations.len(),
        1,
        "exactly one fresh-aggregate borrowed-call relocation; got {relocations:?}",
    );
    assert_eq!(
        relocations[0],
        (0, v(1), 1, 2),
        "relocates recv %1's dec from bb0 to successors bb1 (normal) + bb2 (unwind)",
    );
}

#[test]
fn borrowed_terminator_aggregate_skips_owned_transfer() {
    // NEGATIVE (the over-fire boundary — variant TRANSFERRED OWNED): the same
    // aggregate passed at an OWNED `Invoke` position is an RL-2 ownership transfer
    // (the callee frees it). The Borrowed-position gate excludes it — NO relocation,
    // or the str double-frees against the callee's release. Spec: Annex E §AIMS RL-2.
    let interner = ori_ir::StringInterner::new();
    let mut pool = ori_types::Pool::new();
    let func = borrowed_terminator_aggregate_func(&mut pool, &interner, true);
    let contracts = borrow_read_contracts(&interner);
    let relocations =
        compute_borrowed_terminator_aggregate_relocations(&func, &pool, &interner, &contracts);
    assert!(
        relocations.is_empty(),
        "an owned-position aggregate transfer gets NO relocation; got {relocations:?}",
    );
}

/// Build a borrowed-`[str]`-param function whose single block iter-consumes the
/// param (`@iter(%0 [own])` -> `@ori_iter_drop`) and OPTIONALLY reuses it at a
/// non-iter `@__index(%0 [borrow])` position afterward (`with_reuse`). Mirrors the
/// `borrowed_param_iterate_then_index` AOT fixture's `@process` shape.
fn borrowed_iter_then_index_func(
    pool: &mut Pool,
    interner: &ori_ir::StringInterner,
    with_reuse: bool,
) -> ArcFunction {
    let list_ty = pool.list(Idx::STR);
    let iter_name = interner.intern("iter");
    let iter_drop_name = interner.intern("ori_iter_drop");
    let index_name = interner.intern("__index");
    // %0 borrowed [str] param, %1 iter-handle (scalar), %2 iter-drop result
    // (scalar), %3 index-key (scalar), %4 index result (str fat-val).
    let var_types = vec![list_ty, Idx::INT, Idx::INT, Idx::INT, Idx::STR];
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0
        ValueRepr::Scalar,    // %1
        ValueRepr::Scalar,    // %2
        ValueRepr::Scalar,    // %3
        ValueRepr::FatValue,  // %4
    ];
    let mut body = vec![
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
            func: iter_drop_name,
            args: vec![v(1)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
    ];
    if with_reuse {
        // Reuse %0 at a NON-iter borrowed position after the iter-drop.
        body.push(ArcInstr::Apply {
            dst: v(4),
            ty: Idx::STR,
            func: index_name,
            args: vec![v(0), v(3)],
            arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
            mono_instance_id: None,
        });
    }
    ArcFunction {
        var_types,
        var_reprs,
        params: vec![ArcParam {
            var: v(0),
            ty: list_ty,
            ownership: Ownership::Borrowed,
        }],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        ..Default::default()
    }
}

/// Count `(BurdenInc, BurdenDec)` ops targeting `var` in `func`.
fn burden_inc_dec_for(func: &ArcFunction, var: ArcVarId) -> (usize, usize) {
    let mut inc = 0;
    let mut dec = 0;
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var: w } if *w == var => inc += 1,
                ArcInstr::BurdenDec { var: w } if *w == var => dec += 1,
                _ => {}
            }
        }
    }
    (inc, dec)
}

#[test]
fn single_iter_consume_then_reuse_gets_keepalive_inc_and_paired_dec() {
    // Semantic pin: a borrowed [str] param iter-consumed ONCE then reused at a
    // non-iter `@__index` gets a keep-alive `BurdenInc(%0)` before the `@iter`
    // (so `ori_iter_drop` decs the keep-alive copy, not the live borrow the reuse
    // needs) + a paired `BurdenDec(%0)` after the reuse — `RL1_duplication_balanced`.
    let mut pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_iter_then_index_func(&mut pool, &interner, /* with_reuse */ true);
    let contracts = FxHashMap::default();

    let (inc_before, dec_before) = burden_inc_dec_for(&func, v(0));
    assert_eq!(
        (inc_before, dec_before),
        (0, 0),
        "no burden ops on the borrowed param before the keep-alive pass"
    );

    emit_single_iter_consume_reuse_keepalive(&mut func, &pool, &interner, &contracts);

    let (inc_after, dec_after) = burden_inc_dec_for(&func, v(0));
    assert_eq!(
        inc_after, 1,
        "exactly one keep-alive BurdenInc on the reused param"
    );
    assert_eq!(
        dec_after, 1,
        "exactly one paired BurdenDec on the reused param"
    );

    // Placement pin: the keep-alive inc precedes the `@iter`, the paired dec
    // follows the lineage's last non-iter use (the `@__index` reuse).
    let body = &func.blocks[0].body;
    let inc_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == v(0)));
    let iter_pos = body.iter().position(
        |i| matches!(i, ArcInstr::Apply { func: f, .. } if *f == interner.intern("iter")),
    );
    let dec_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == v(0)));
    let index_pos = body.iter().position(
        |i| matches!(i, ArcInstr::Apply { func: f, .. } if *f == interner.intern("__index")),
    );
    assert!(
        inc_pos < iter_pos,
        "keep-alive BurdenInc precedes the @iter (inc={inc_pos:?}, iter={iter_pos:?})"
    );
    assert!(
        dec_pos > index_pos,
        "paired BurdenDec follows the @__index reuse (dec={dec_pos:?}, index={index_pos:?})"
    );

    // burden_emitted marks %0 so VF-1's per-var balance check sees the pair.
    assert!(
        func.burden_emitted
            .get(v(0).index())
            .copied()
            .unwrap_or(false),
        "the reused param is marked in burden_emitted for VF-1"
    );
}

#[test]
fn single_iter_consume_without_reuse_gets_no_keepalive() {
    // Negative / over-fire pin: a borrowed [str] param iter-consumed ONCE and NOT
    // reused after (the no-reuse canary shape, `borrowed_str_list_single_call`)
    // gets NO keep-alive — `lineage_live_out_after_use` is false, so the pass
    // declines. Adding a keep-alive here would unbalance the no-reuse case.
    let mut pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_iter_then_index_func(&mut pool, &interner, /* with_reuse */ false);
    let contracts = FxHashMap::default();

    emit_single_iter_consume_reuse_keepalive(&mut func, &pool, &interner, &contracts);

    let (inc_after, dec_after) = burden_inc_dec_for(&func, v(0));
    assert_eq!(
        (inc_after, dec_after),
        (0, 0),
        "no keep-alive on a single-iter-consume param with no reuse (over-fire guard)"
    );
}

// === Phase 6.68c — N>=2 callee-returned scalar-list cross-call surplus-inc strip ===

/// Build a `@main`-shaped function mirroring the `for w in words yield w` two-call
/// returned-`[int]` leak. `n_acquires` user-callee `[int]`-returning `Invoke`s;
/// the FIRST result (`%0`) is live across the second call + iter-consumed. The
/// spurious cross-call `BurdenInc %0` lands in the block carrying the second call.
///
/// Block layout (the leaking shape, minus the post-iter loop which is irrelevant
/// to the explicit-op net since the lineage rep does not span the phi):
///   bb0:  %0 = Invoke @`clone_list(%2` [borrow]) normal bb1
///   bb1:  `burden_inc` %0          // SPURIOUS cross-call inc (the surplus)
///         %1 = Invoke @`clone_list(%2` [borrow]) normal bb2
///   bb2:  `burden_inc` %0          // genuine keep-alive before the @iter consume
///         %9 = @iter(%0 [own])   // iter-consume -> the acquired ref's release
///         `burden_dec` %0          // paired release of the keep-alive
///         Return %0
/// `%2` is the borrowed source (params), `elem_ty` = `int` (scalar).
fn returned_int_list_two_call_func(
    pool: &mut Pool,
    interner: &ori_ir::StringInterner,
    elem: Idx,
    callee_name: &str,
) -> ArcFunction {
    let list_ty = pool.list(elem);
    let callee = interner.intern(callee_name);
    let iter_name = interner.intern("iter");
    // vars: %0 first result, %1 second result, %2 source (all `[elem]` RcPtr), %3
    // iter handle (scalar). Sized to cover every referenced index.
    let var_types = vec![list_ty, list_ty, list_ty, Idx::INT];
    let var_reprs = vec![
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::RcPointer,
        ValueRepr::Scalar,
    ];
    let bb0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Invoke {
            dst: v(0),
            ty: list_ty,
            func: callee,
            args: vec![v(2)],
            arg_ownership: vec![ArgOwnership::Borrowed],
            mono_instance_id: None,
            normal: ArcBlockId::new(1),
            unwind: ArcBlockId::new(3),
        },
    };
    let bb1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: Vec::new(),
        body: vec![ArcInstr::BurdenInc { var: v(0) }],
        terminator: ArcTerminator::Invoke {
            dst: v(1),
            ty: list_ty,
            func: callee,
            args: vec![v(2)],
            arg_ownership: vec![ArgOwnership::Borrowed],
            mono_instance_id: None,
            normal: ArcBlockId::new(2),
            unwind: ArcBlockId::new(3),
        },
    };
    let bb2 = ArcBlock {
        id: ArcBlockId::new(2),
        params: Vec::new(),
        body: vec![
            ArcInstr::BurdenInc { var: v(0) },
            ArcInstr::Apply {
                dst: v(3),
                ty: Idx::INT,
                func: iter_name,
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
            ArcInstr::BurdenDec { var: v(0) },
        ],
        terminator: ArcTerminator::Return { value: v(0) },
    };
    let bb3 = ArcBlock {
        id: ArcBlockId::new(3),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Resume,
    };
    ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![bb0, bb1, bb2, bb3],
        ..Default::default()
    }
}

/// Positive pin: the spurious cross-call `BurdenInc %0` (in bb1, the block
/// carrying the SECOND acquire where `%0` is not an argument) is the ONE surplus
/// inc selected for stripping. The genuine keep-alive inc (bb2, before the
/// `@iter` consume) is NOT selected.
#[test]
fn strip_surplus_cross_call_inc_for_returned_int_list_two_calls() {
    let mut pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    let func = returned_int_list_two_call_func(&mut pool, &interner, Idx::INT, "clone_list");
    let reps = identity_reps(4);
    let strips = compute_returned_collection_surplus_inc_strips(&func, &pool, &interner, &reps);
    // Exactly the bb1 inc (block 1, instr 0) — the cross-call surplus.
    assert_eq!(
        strips,
        vec![(1usize, 0usize)],
        "the spurious cross-call inc (bb1) is the sole surplus; the bb2 keep-alive is kept"
    );
}

/// Negative over-fire pin: a `[str]` (heap-element) returned-list two-call shape
/// MUST NOT be stripped — the buffer dec walks `elem_dec_fn` over shared source
/// element strings, so removing the buffer keep-alive inc double-frees them. The
/// scalar-element gate (`ArcClass::Scalar`) declines `str`.
#[test]
fn no_strip_for_returned_str_list_two_calls_heap_elements() {
    let mut pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    let func = returned_int_list_two_call_func(&mut pool, &interner, Idx::STR, "clone_list");
    let reps = identity_reps(4);
    let strips = compute_returned_collection_surplus_inc_strips(&func, &pool, &interner, &reps);
    assert!(
        strips.is_empty(),
        "heap-element ([str]) returned list declines the strip (scalar-element gate)"
    );
}

/// Negative over-fire pin: a known BUILTIN-callee return (`@map`/`@filter` — here
/// the protocol-builtin-shaped `__map`) is NOT a user-callee acquire; its result
/// is a compiler-modelled self-alloc the base path balances. MUST NOT be stripped.
#[test]
fn no_strip_for_builtin_callee_returned_list() {
    let mut pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    // `__`-prefixed callee = protocol builtin -> not a user-callee acquire.
    let func = returned_int_list_two_call_func(&mut pool, &interner, Idx::INT, "__map");
    let reps = identity_reps(4);
    let strips = compute_returned_collection_surplus_inc_strips(&func, &pool, &interner, &reps);
    assert!(
        strips.is_empty(),
        "a builtin-callee return is not a user-callee acquire (over-fire guard)"
    );
}

/// MUTATION-VERIFY of the scalar-element gate: the `[str]` negative pin
/// ([`no_strip_for_returned_str_list_two_calls_heap_elements`]) is GUARDED by the
/// scalar-element check. This pin proves the gate is load-bearing: the ONLY
/// difference between the firing `[int]` shape and the declining `[str]` shape is
/// the element type — same block layout, same surplus inc, same live-across +
/// iter-consume + net-+1. If the scalar-element gate were removed (forced to treat
/// `str` as scalar), the `[str]` shape WOULD select the same bb1 surplus inc that
/// the `[int]` shape does, double-freeing the shared element strings at runtime.
/// Asserting the `[int]` shape DOES select `(1, 0)` while the `[str]` shape selects
/// NOTHING pins that the element-type discriminator is the firing boundary.
#[test]
fn scalar_element_gate_is_the_firing_boundary_int_vs_str() {
    let interner = ori_ir::StringInterner::new();
    let reps = identity_reps(4);

    let mut pool_int = Pool::default();
    let func_int =
        returned_int_list_two_call_func(&mut pool_int, &interner, Idx::INT, "clone_list");
    let strips_int =
        compute_returned_collection_surplus_inc_strips(&func_int, &pool_int, &interner, &reps);

    let mut pool_str = Pool::default();
    let func_str =
        returned_int_list_two_call_func(&mut pool_str, &interner, Idx::STR, "clone_list");
    let strips_str =
        compute_returned_collection_surplus_inc_strips(&func_str, &pool_str, &interner, &reps);

    // Identical IR shape modulo element type: int fires the same surplus the str
    // shape would, but the scalar-element gate makes str decline. Forcing the gate
    // true (treating str as scalar) would make `strips_str == strips_int` ->
    // double-free; this asymmetry pins the gate as load-bearing.
    assert_eq!(strips_int, vec![(1usize, 0usize)], "int (scalar) fires");
    assert!(
        strips_str.is_empty(),
        "str (heap) declines via the scalar gate"
    );
    assert_ne!(
        strips_int, strips_str,
        "the scalar-element gate is the firing boundary between [int] and [str]"
    );
}

/// Build the borrowed-param COW-push shape: a borrowed param `%0`, COW-inc'd
/// alias `%1 = %0` consumed `[own]` at an `Invoke @push` terminator, then —
/// when `live_after` — a LATER block re-reads the same allocation via `%9 = %0`
/// at a borrowed `@len`. Mirrors `@check(list) = { list.push(99); list.len() }`
/// (`live_after = true`) vs `@extend_list(items) = items.push(..)` returning the
/// result (`live_after = false`).
fn borrowed_param_cow_push_func(
    interner: &ori_ir::StringInterner,
    live_after: bool,
) -> ArcFunction {
    let push_name = interner.intern("push");
    let len_name = interner.intern("len");
    // %0 borrowed param, %1 push receiver alias, %2 elem, %3 push result,
    // %4 push-result alias, %5 scalar len, %6 re-read alias of %0, %7 scalar len2.
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0 borrowed param
        ValueRepr::RcPointer, // %1 = %0 (push receiver)
        ValueRepr::Scalar,    // %2 push elem
        ValueRepr::RcPointer, // %3 push result (fresh)
        ValueRepr::RcPointer, // %4 = %3 (len-alias)
        ValueRepr::Scalar,    // %5 len(result)
        ValueRepr::RcPointer, // %6 = %0 (re-read of the borrowed param)
        ValueRepr::Scalar,    // %7 len(param)
    ];
    let var_types: Vec<Idx> = (0..var_reprs.len()).map(|_| Idx::from_raw(0)).collect();
    let mut blocks = vec![
        // bb0: `%1 = %0`; BurdenInc %1; `Invoke @push(%1 [own], %2 [own]) -> %3`.
        ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::from_raw(0),
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::BurdenInc { var: v(1) },
            ],
            terminator: ArcTerminator::Invoke {
                dst: v(3),
                ty: Idx::from_raw(0),
                func: push_name,
                args: vec![v(1), v(2)],
                arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                mono_instance_id: None,
                normal: ArcBlockId::new(1),
                unwind: ArcBlockId::new(2),
            },
        },
        // bb1: `%4 = %3`; borrow-read `@len(%4)` -> %5; then exit (or re-read).
        ArcBlock {
            id: ArcBlockId::new(1),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: v(4),
                ty: Idx::from_raw(0),
                value: ArcValue::Var(v(3)),
            }],
            terminator: ArcTerminator::Invoke {
                dst: v(5),
                ty: Idx::from_raw(0),
                func: len_name,
                args: vec![v(4)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
                normal: ArcBlockId::new(if live_after { 3 } else { 4 }),
                unwind: ArcBlockId::new(2),
            },
        },
        // bb2: unwind pad.
        ArcBlock {
            id: ArcBlockId::new(2),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Resume,
        },
    ];
    blocks.push(borrowed_param_cow_push_tail_block(live_after, len_name));
    let params = vec![ArcParam {
        var: v(0),
        ty: Idx::from_raw(0),
        ownership: Ownership::Borrowed,
    }];
    ArcFunction {
        params,
        blocks,
        var_types,
        var_reprs,
        ..Default::default()
    }
}

/// The tail block of [`borrowed_param_cow_push_func`]: bb3 re-reads the borrowed
/// param (`%6 = %0`; `Apply @len(%6 [borrow])`) when `live_after`, else bb4 is a
/// bare exit (the push result is returned, the receiver not re-read).
fn borrowed_param_cow_push_tail_block(live_after: bool, len_name: Name) -> ArcBlock {
    if live_after {
        ArcBlock {
            id: ArcBlockId::new(3),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: v(6),
                    ty: Idx::from_raw(0),
                    value: ArcValue::Var(v(0)),
                },
                ArcInstr::Apply {
                    dst: v(7),
                    ty: Idx::from_raw(0),
                    func: len_name,
                    args: vec![v(6)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return { value: v(3) },
        }
    } else {
        ArcBlock {
            id: ArcBlockId::new(4),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return { value: v(3) },
        }
    }
}

/// Same-alloc reps over the function's `Let { Var }` aliases (the test-local
/// projection of `compute_same_alloc_reps`'s Let-edge unions).
fn let_alias_reps(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    let mut reps: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let root = reps.get(src).copied().unwrap_or(*src);
                reps.insert(*dst, root);
            }
        }
    }
    reps
}

/// Positive pin: a borrowed-param COW-push receiver that is LIVE-AFTER the call
/// (re-read through a same-alloc alias) gets NO callee-side edge `BurdenDec` on
/// its lineage. The step-1 COW-inc is balanced by the COW helper's own slow-path
/// dec; the caller owns + drops the allocation. A callee edge-dec would free the
/// caller's still-owned reference before the later read (UAF) and double-free
/// against the caller's drop. Spec: Annex E §AIMS RL-1 + RL-2.
#[test]
fn borrowed_param_cow_push_live_after_emits_no_edge_release() {
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_param_cow_push_func(&interner, true);
    let reps = let_alias_reps(&func);
    emit_cow_inc_terminator_edge_release(&mut func, &interner, &reps);
    let dec_on_lineage = func.blocks.iter().any(|b| {
        b.body.iter().any(|i| {
            matches!(i, ArcInstr::BurdenDec { var }
                if reps.get(var).copied().unwrap_or(*var) == v(0) || *var == v(1))
        })
    });
    assert!(
        !dec_on_lineage,
        "live-after borrowed-param COW receiver must get NO callee edge release"
    );
}

/// Negative pin: a borrowed-param COW-push receiver that is DEAD after the call
/// (the push result is returned, the receiver not re-read) DOES get the
/// both-edge `BurdenDec` — the inc'd reference is dead on each successor and the
/// caller provides the matching ref (str-list shape). Clamps the live-after
/// suppression from below: dropping the gate would leak this shape's receiver.
#[test]
fn borrowed_param_cow_push_dead_after_keeps_edge_release() {
    let interner = ori_ir::StringInterner::new();
    let mut func = borrowed_param_cow_push_func(&interner, false);
    let reps = let_alias_reps(&func);
    emit_cow_inc_terminator_edge_release(&mut func, &interner, &reps);
    let dec_on_lineage = func.blocks.iter().any(|b| {
        b.body.iter().any(|i| {
            matches!(i, ArcInstr::BurdenDec { var }
                if reps.get(var).copied().unwrap_or(*var) == v(0) || *var == v(1))
        })
    });
    assert!(
        dec_on_lineage,
        "dead-after borrowed-param COW receiver must keep the both-edge release"
    );
}
