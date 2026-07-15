//! Tests for [`super`]: Phase-7 mechanical burden-lowering and return-block
//! scope-exit dec ordering.
//!
//! Pins [`super::lower_burden_ops_to_rc`]: under the probe
//! (`predicate_stack_rc_disabled`), surviving whole-var `BurdenInc` /
//! `BurdenDec` lower to real `RcInc` / `RcDec`, and the field-grain
//! `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` variants
//! lower by RE-SPELLING to `RcDecPartial` / `RcDecField` / `RcDecVariant`
//! (identical per-field / per-variant drop glue at codegen; out of the
//! Step-11 burden census per RL-comp net-preservation). Also pins
//! [`super::order_return_block_scope_exit_decs`].
//!
//! RC counts use the SSOT `crate::pipeline::rc_count::count_rc_ops`.

use super::lower_burden_ops_to_rc;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind, ValueRepr,
};
use crate::pipeline::rc_count::count_rc_ops;
use ori_types::{Idx, Pool, TypeRegistry};

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
    let mut function = ArcFunction {
        var_types,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    };
    function.replace_variable_representations(var_reprs);
    function
}

fn list_construct(dst: ArcVarId, args: Vec<ArcVarId>) -> ArcInstr {
    ArcInstr::Construct {
        dst,
        ty: Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args,
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

    lower_burden_ops_to_rc(
        &mut func,
        &pool,
        &TypeRegistry::default(),
        &rustc_hash::FxHashSet::default(),
    );

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

    lower_burden_ops_to_rc(
        &mut func,
        &pool,
        &TypeRegistry::default(),
        &rustc_hash::FxHashSet::default(),
    );

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

    lower_burden_ops_to_rc(
        &mut func,
        &pool,
        &TypeRegistry::default(),
        &rustc_hash::FxHashSet::default(),
    );

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

    lower_burden_ops_to_rc(
        &mut func,
        &pool,
        &TypeRegistry::default(),
        &rustc_hash::FxHashSet::default(),
    );

    assert_eq!(
        burden_count(&func),
        1,
        "scalar-repr field-grain dec left burden-spelled (census abort surface)"
    );
}

#[test]
fn lower_leaves_scalar_repr_burden_in_place() {
    let pool = Pool::default();
    // Class-ledger admission excludes scalar reprs. The RE-2 backstop must leave
    // a contract-violating scalar burden op in place rather than synthesize
    // unsound RC.
    let mut func = rc_pointer_func(1, vec![ArcInstr::BurdenInc { var: v(0) }]);
    func.var_reprs[0] = ValueRepr::Scalar;

    lower_burden_ops_to_rc(
        &mut func,
        &pool,
        &TypeRegistry::default(),
        &rustc_hash::FxHashSet::default(),
    );

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

    lower_burden_ops_to_rc(&mut func, &pool, &TypeRegistry::default(), &elidable);

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

    lower_burden_ops_to_rc(&mut func, &pool, &TypeRegistry::default(), &elidable);

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

/// Semantic pin: [`super::order_return_block_scope_exit_decs`] sorts a
/// `Return` block's trailing release run into DESCENDING `ArcVarId` order (the
/// value-semantics teardown order) and keeps each release's span attached to
/// the SAME reordered instruction — never to whichever slot it lands in.
#[test]
fn order_return_block_sorts_descending_var_and_keeps_spans_in_lockstep() {
    let mut func = rc_pointer_func(
        3,
        vec![
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(2) },
            ArcInstr::BurdenDec { var: v(1) },
        ],
    );
    // One span per body instruction, keyed by its ORIGINAL position — var(0)'s
    // release carries span (0,1), var(2)'s carries (10,11), var(1)'s (20,21).
    func.spans = vec![vec![
        Some(ori_ir::Span::new(0, 1)),
        Some(ori_ir::Span::new(10, 11)),
        Some(ori_ir::Span::new(20, 21)),
    ]];

    super::order_return_block_scope_exit_decs(&mut func);

    let body = &func.blocks[0].body;
    let vars: Vec<u32> = body
        .iter()
        .map(|i| match super::release_var(i) {
            Some(var) => var.raw(),
            None => panic!("every body instr is a release op"),
        })
        .collect();
    assert_eq!(
        vars,
        vec![2, 1, 0],
        "the trailing release run sorts into descending var order"
    );

    let spans = &func.spans[0];
    assert_eq!(
        spans,
        &[
            Some(ori_ir::Span::new(10, 11)),
            Some(ori_ir::Span::new(20, 21)),
            Some(ori_ir::Span::new(0, 1)),
        ],
        "each release's span travels WITH its reordered instruction, not with its old slot"
    );
}

/// Semantic pin: the ordering pass is idempotent — running it a second time on
/// its own output reproduces the same body (a non-idempotent reorder would be
/// a bug per `tests.md §Negative Testing Protocol` idempotency testing).
#[test]
fn order_return_block_scope_exit_decs_is_idempotent() {
    let mut func = rc_pointer_func(
        3,
        vec![
            ArcInstr::BurdenDec { var: v(0) },
            ArcInstr::BurdenDec { var: v(2) },
            ArcInstr::BurdenDec { var: v(1) },
        ],
    );

    super::order_return_block_scope_exit_decs(&mut func);
    let once = func.blocks[0].body.clone();

    super::order_return_block_scope_exit_decs(&mut func);
    let twice = func.blocks[0].body.clone();

    assert_eq!(
        once, twice,
        "running the ordering pass twice matches running it once"
    );
}
