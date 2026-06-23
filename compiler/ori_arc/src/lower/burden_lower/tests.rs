//! Tests for `emit_burden_ops` walker. Ships boundary + iteration pin;
//! owned-filter + matrix coverage added once the `DerivedOwnership`
//! access path is wired.

use ori_ir::Name;
use ori_types::{Idx, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{emit_burden_ops as emit_burden_ops_impl, BurdenLowerCtx};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, LitValue, PrimOp, ValueRepr,
};
use crate::lower::test_utils::{entry_block, project_first, set_first};
use crate::ownership::{DerivedOwnership, Ownership};

/// Test wrapper forwarding to the real `emit_burden_ops` with a fresh empty
/// `StringInterner`. The iterator-element exclusion (`collect_iter_element_defs`)
/// resolves the `__iter_next` protocol-builtin name through the interner; tests
/// that exercise iterator-element exclusion pass their own interner via
/// [`emit_burden_ops_with_interner`]. Synthetic-`Name` fixtures never collide
/// with the interned `__iter_next`, so the fresh interner is a no-op for them —
/// preserving the existing call shape across the suite without 78-site churn.
fn emit_burden_ops<'a>(
    func: &mut ArcFunction,
    type_registry: &'a TypeRegistry,
    derived_ownership: &[DerivedOwnership],
    immortals: &[bool],
    contracts: &FxHashMap<Name, crate::aims::contract::MemoryContract>,
    predicate_stack_rc_disabled: bool,
) -> BurdenLowerCtx<'a> {
    let interner = ori_ir::StringInterner::new();
    emit_burden_ops_impl(
        func,
        type_registry,
        derived_ownership,
        immortals,
        contracts,
        &FxHashMap::default(),
        predicate_stack_rc_disabled,
        &interner,
    )
}

/// Test wrapper threading a caller-supplied interner — used by the
/// iterator-element-exclusion pins which must intern `__iter_next` so the
/// `Apply` callee `Name` matches `collect_iter_element_defs`.
fn emit_burden_ops_with_interner<'a>(
    func: &mut ArcFunction,
    type_registry: &'a TypeRegistry,
    derived_ownership: &[DerivedOwnership],
    immortals: &[bool],
    contracts: &FxHashMap<Name, crate::aims::contract::MemoryContract>,
    predicate_stack_rc_disabled: bool,
    interner: &ori_ir::StringInterner,
) -> BurdenLowerCtx<'a> {
    emit_burden_ops_impl(
        func,
        type_registry,
        derived_ownership,
        immortals,
        contracts,
        &FxHashMap::default(),
        predicate_stack_rc_disabled,
        interner,
    )
}

fn empty_func() -> ArcFunction {
    ArcFunction::default()
}

fn func_with_n_vars(n: u32) -> ArcFunction {
    ArcFunction {
        var_types: (0..n).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn empty_function_collects_no_burdens() {
    let registry = TypeRegistry::new();
    let mut func = empty_func();
    let ctx: BurdenLowerCtx<'_> =
        emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        ctx.collected_burdens().is_empty(),
        "empty fn yields zero burden lookups",
    );
}

#[test]
fn construct_emits_one_transfer_point_per_owned_arg() {
    // Burden-emission contract: "For each transfer point that consumes v, emit
    // BurdenInc(v) immediately before." Construct with 1 arg ⇒ 1 transfer-
    // point entry. Semantic pin: would FAIL if Construct walk is reverted to
    // no-op or if transfer_points field is not populated.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(0)],
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let tp_vars: Vec<ArcVarId> = ctx.transfer_points().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        tp_vars,
        vec![ArcVarId::new(0)],
        "Construct with 1 owned arg MUST emit exactly 1 transfer-point entry for that arg",
    );
}

#[test]
fn apply_with_one_owned_arg_emits_one_transfer_point() {
    // success_criterion 1 enumerates Apply with Owned param as a
    // transfer point per `ArcInstr::is_owned_position`. generic
    // walk via `used_vars` + `is_owned_position` mechanically extends
    // this coverage. Semantic pin: would FAIL if Apply branch in
    // is_owned_position is reverted to `_ => false`.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: Name::from_raw(99),
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let tp_vars: Vec<ArcVarId> = ctx.transfer_points().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        tp_vars,
        vec![ArcVarId::new(0)],
        "Apply with 1 Owned arg MUST emit exactly 1 transfer-point entry for that arg",
    );
}

#[test]
fn set_emits_one_transfer_point_for_owned_value() {
    // success_criterion 1: "Set with Owned value (per
    // TF-15 — value.access:= Owned unconditional via IA-5 step (1); NOT
    // covered by is_owned_position per the _ => false catch-all)". Semantic
    // pin: would FAIL if Set carve-out is reverted (Set falls through the
    // generic walk because is_owned_position returns false). Pin asserts
    // exactly 1 transfer-point entry for `value`; `base` is direct demand
    // only (NOT a transfer point per TF-15).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Set {
                base: ArcVarId::new(0),
                field: 0,
                value: ArcVarId::new(1),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let tp_vars: Vec<ArcVarId> = ctx.transfer_points().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        tp_vars,
        vec![ArcVarId::new(1)],
        "Set MUST emit exactly 1 transfer-point entry for the Owned value (var 1); base (var 0) is direct demand only per TF-15",
    );
}

#[test]
fn borrowed_params_skipped_owned_params_collected() {
    // Burden-walk contract: "For each owned ArcVarId v in the function...".
    // Semantic pin: would FAIL if filter is reverted to walk all vars
    // unconditionally — Borrowed param at var(1) MUST be absent from
    // collected_burdens; Owned param at var(0) MUST be present.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::INT,
                ownership: Ownership::Borrowed,
            },
        ],
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let collected_vars: Vec<ArcVarId> = ctx.collected_burdens().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        collected_vars,
        vec![ArcVarId::new(0)],
        "Borrowed param at var(1) MUST be filtered out; only Owned param at var(0) remains",
    );
}

#[test]
fn construct_emits_burden_inc_immediately_before_consuming_construct() {
    // success_criterion 1 +: Construct with Owned arg
    // gets BurdenInc(arg) emitted immediately before — UNLESS the arg's
    // last-use is at this Construct (the matching Dec would be transfer-
    // suppressed per `RL-2`, producing VF-1 imbalance).
    // This test pins the owned-pos Inc emission for the case where arg(0)
    // has a follow-up use (Let-Var alias keeps it alive past the Construct).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Tuple,
                    args: vec![ArcVarId::new(0)],
                },
                // Follow-up alias keeps var(0) live past the Construct.
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                // TWO follow-up aliases keep the Construct result var(1)
                // GENUINELY live (use-count >= 2 => a DUPLICATION, not a
                // single-use move): the FRESH-site BurdenInc IS emitted. A
                // single-use move-alias to a dead var is a MOVE (RL-2 ownership
                // transfer): the source's inc is suppressed and the lineage's
                // single inc+dec lands at the dst — a DEAD/move result is
                // correctly inc-suppressed per the proven CH-comp case-3
                // coexistence handshake.
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert!(
        inc_vars.contains(&ArcVarId::new(0)),
        "expected BurdenInc(arg=0) before Construct (last-use at Let-Var keeps arg alive past Construct); body={body:?}",
    );
    assert!(
        inc_vars.contains(&ArcVarId::new(1)),
        "expected BurdenInc(dst=1) FRESH-site for Construct (TF-3); body={body:?}",
    );
    let construct_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Construct { .. }))
        .unwrap_or_else(|| panic!("Construct MUST appear in body"));
    let first_inc_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)))
        .unwrap_or_else(|| panic!("BurdenInc(arg=0) MUST appear in body"));
    assert!(
        first_inc_pos < construct_pos,
        "BurdenInc(arg=0) MUST precede Construct; body={body:?}",
    );
}

#[test]
fn apply_emits_burden_inc_immediately_before_consuming_apply() {
    // success_criterion 1 +: Apply with Owned arg gets
    // BurdenInc(arg) emitted immediately before — UNLESS the arg's last-use
    // is at this Apply, in which case the matching Dec would be transfer-
    // suppressed per `RL-2` and emitting the Inc would
    // produce a `Σ Inc - Σ Dec = +1` VF-1 imbalance per
    // `aims/verify/burden_balance.rs`. This test pins the owned-pos Inc
    // emission for the case where arg(0) has a follow-up use (Let-Var
    // alias keeps it alive past the Apply), so Inc IS emitted. Uses
    // Idx::STR (heap-burden) per VF-1 RcOnScalar mirror.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                // Follow-up alias keeps var(0) live past the Apply so its
                // last-use is at the Let, not at the Apply. The Apply's
                // owned-pos Inc on var(0) IS emitted rule.
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                // TWO follow-up aliases keep result var(1) GENUINELY live
                // (use-count >= 2 = duplication, not a single-use move), so its
                // FRESH-site BurdenInc emits. A single-use move-alias is an RL-2
                // ownership transfer whose source inc is correctly suppressed.
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    // [BurdenInc(dst=1) FRESH-site, BurdenInc(arg=0) owned-pos, Apply,
    // Let-Var, BurdenDec(0) at last-use]. ITEM-4 paired-elim may further
    // reduce this if all states allow.
    assert!(
        inc_vars.contains(&ArcVarId::new(0)),
        "expected BurdenInc(arg=0) before Apply (last-use at Let-Var keeps arg alive past Apply); body={body:?}",
    );
    assert!(
        inc_vars.contains(&ArcVarId::new(1)),
        "expected BurdenInc(dst=1) FRESH-site for Apply with no contract (TF-5 CONSERVATIVE); body={body:?}",
    );
    let apply_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Apply { .. }))
        .unwrap_or_else(|| panic!("Apply MUST appear in body"));
    let first_inc_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)))
        .unwrap_or_else(|| panic!("BurdenInc(arg=0) MUST appear in body"));
    assert!(
        first_inc_pos < apply_pos,
        "BurdenInc(arg=0) MUST precede Apply; body={body:?}",
    );
}

#[test]
fn set_emits_burden_inc_before_and_skips_burden_dec_at_value_last_use() {
    //  + RL-2 symmetric pin for TF-15 carve-out:
    // (a) Set with Owned non-scalar value gets BurdenInc(value) emitted
    //     immediately before.
    // (b) Set value as last use does NOT receive BurdenDec after (
    //     transfer_vars carve-out half — value is ownership-transferring
    //     per -2; emitting BurdenDec would double-release).
    //  audit conclusion: instruction-level transfer
    // suppression preserved. The owned-position BurdenInc is a VF-1
    // accounting marker; codegen's predicate-stack realize walk owns
    // physical RC for vars consumed at instruction-level owned positions
    // (Set.value via the TF-15 carve-out). Adding a symmetric BurdenDec
    // would mark the var in `func.burden_emitted`, propagate through
    // `populate_class_covered`, and suppress predicate-stack RC emission —
    // causing real-world RC leaks. Test uses Idx::STR (heap-burden) for
    // value — Idx::INT's lookup_burden returns Some(EMPTY_SPEC) per
    // BURDEN_TABLE, so the `burden_carries_rc` filter at
    // owned_vars_needing_rc rejects it.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
        ],
        var_types: vec![Idx::INT, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Set {
                base: ArcVarId::new(0),
                field: 0,
                value: ArcVarId::new(1),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Expected: BurdenInc(var(1)) before Set (TF-15 value carve-out), Set,
    // and possibly BurdenDec(var(0)) after (var(0) is `base` — non-transfer
    // last-use per RL-2; only Set `value` is in the ownership-transferring
    // list, NOT Set `base`). The RL-2 pin is value-specific:
    // BurdenDec(var(1)) MUST NOT appear (value is at TF-15 transfer
    // position).
    let body = &func.blocks[0].body;
    // Pin 1: BurdenInc(var(1)) emitted before Set.
    let inc_value_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_value_pos.is_some(),
        "expected BurdenInc(value=var(1)) per TF-15 carve-out; body={body:?}",
    );
    let set_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Set { .. }))
        .unwrap_or_else(|| panic!("Set MUST appear in body"));
    assert!(
        inc_value_pos.unwrap_or_else(|| unreachable!("checked is_some above")) < set_pos,
        "BurdenInc(value) MUST appear BEFORE Set; body={body:?}",
    );
    // Pin 2: BurdenDec(var(1)) MUST NOT appear (RL-2 transfer skip).
    //  audit: predicate-stack owns physical RC for Set.value
    //  coexistence handshake; symmetric Dec emission here
    // would mark var(1) in burden_emitted and break class_covered.
    let dec_value_present = body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(1)));
    assert!(
        !dec_value_present,
        "Set value (var 1) MUST NOT receive BurdenDec at last-use (RL-2 transfer-point exception per coexistence handshake); body={body:?}",
    );
}

#[test]
fn set_emits_burden_dec_field_for_owned_field_before_burden_inc_value() {
    // positive pin per plan body line 1943 + navigator-verdict
    // (proceed verdict,): Set with heap-burden base MUST emit
    // BurdenDecField(base, field) BEFORE BurdenInc(value) BEFORE the Set
    // instruction. BurdenDecField releases the prior field value's burden;
    // symmetric with BurdenInc(value) which transfers ownership of the new
    // value INTO the field position. Both precede Set so codegen
    // can GEP+load the prior value BEFORE the store clobbers it. Per
    // `TF-15` + `§8 RL-2` ownership-transfer rules, plus
    // AIMS Invariant 5 unified-model preservation (extends
    // ArcInstr enum on the same dimension as BurdenDecPartial).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
        ],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![entry_block(
            vec![set_first(ArcVarId::new(0), ArcVarId::new(1))],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;

    // Pin 1: BurdenDecField(base=var(0), field=0) appears in body.
    let dec_field_pos = body.iter().position(
        |i| matches!(i, ArcInstr::BurdenDecField { base, field } if *base == ArcVarId::new(0) && *field == 0),
    );
    assert!(
        dec_field_pos.is_some(),
        "expected BurdenDecField(base=var(0), field=0) ; body={body:?}",
    );

    // Pin 2: BurdenInc(value=var(1)) appears in body.
    let inc_value_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_value_pos.is_some(),
        "expected BurdenInc(value=var(1)) per TF-15 carve-out; body={body:?}",
    );

    // Pin 3: Set appears in body.
    let set_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Set { .. }))
        .unwrap_or_else(|| panic!("Set MUST appear in body"));

    // Pin 4: Ordering — BurdenDecField BEFORE BurdenInc(value) BEFORE Set.
    // Codegen reads the prior field value via GEP+load BEFORE
    // the store clobbers it; this ordering is the load-bearing invariant.
    let dec_field = dec_field_pos.unwrap_or_else(|| unreachable!("checked is_some above"));
    let inc_value = inc_value_pos.unwrap_or_else(|| unreachable!("checked is_some above"));
    assert!(
        dec_field < inc_value,
        "BurdenDecField MUST precede BurdenInc(value); body={body:?}",
    );
    assert!(
        inc_value < set_pos,
        "BurdenInc(value) MUST precede Set; body={body:?}",
    );
}

#[test]
fn settag_emits_burden_dec_variant_before_settag() {
    // positive pin per `TF-15a` + `§8 RL-10`:
    // SetTag with heap-burden base MUST emit BurdenDecVariant(var=base) BEFORE
    // the SetTag instruction. BurdenDecVariant is the whole-var sibling to
    // BurdenDecField — SetTag invalidates ALL payload fields of the
    // OLD variant (RL-10), so codegen walks the entire variant
    // before the tag store clobbers the discriminant. AIMS Invariant 5
    // case (b) — extends ArcInstr enum on the same dimension as
    // BurdenDecPartial / BurdenDec; no parallel emission, no shadow tracker.
    // SetTag's TF-15a backward demand is `(base, Once)` only — no value
    // operand — so unlike Set, no symmetric BurdenInc(value).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR],
        blocks: vec![entry_block(
            vec![ArcInstr::SetTag {
                base: ArcVarId::new(0),
                tag: 1,
            }],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;

    // Pin 1: BurdenDecVariant(var=var(0)) appears in body.
    let dec_variant_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenDecVariant { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_variant_pos.is_some(),
        "expected BurdenDecVariant(var=var(0)) ; body={body:?}",
    );

    // Pin 2: SetTag appears in body.
    let settag_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::SetTag { .. }))
        .unwrap_or_else(|| panic!("SetTag MUST appear in body"));

    // Pin 3: Ordering — BurdenDecVariant BEFORE SetTag. Codegen
    // reads the current discriminant via GEP+load BEFORE the store clobbers
    // it; this ordering is the load-bearing invariant per `
    // §8 RL-10` (tag change invalidates ALL payload fields).
    let dec_variant = dec_variant_pos.unwrap_or_else(|| unreachable!("checked is_some above"));
    assert!(
        dec_variant < settag_pos,
        "BurdenDecVariant MUST precede SetTag; body={body:?}",
    );

    // Pin 4: round-trip through SSOT walk helpers.
    // Use `BurdenDecVariant` reflectively via the canonical helpers so the
    // four arms (defined_var/used_vars/uses_var/substitute_var) are
    // mechanically exercised and any future enum-variant grouping drift is
    // surfaced at this pin rather than at downstream pipeline consumption.
    let bdv = ArcInstr::BurdenDecVariant {
        var: ArcVarId::new(0),
    };
    assert!(
        bdv.defined_var().is_none(),
        "BurdenDecVariant defines no dst"
    );
    assert_eq!(
        bdv.used_vars().to_vec(),
        vec![ArcVarId::new(0)],
        "BurdenDecVariant used_vars = [var]",
    );
    assert!(
        bdv.uses_var(ArcVarId::new(0)),
        "BurdenDecVariant uses_var(var) holds",
    );
    assert!(
        !bdv.uses_var(ArcVarId::new(99)),
        "BurdenDecVariant uses_var(other) does not hold",
    );
    let mut bdv_sub = bdv.clone();
    bdv_sub.substitute_var(ArcVarId::new(0), ArcVarId::new(7));
    assert!(
        matches!(bdv_sub, ArcInstr::BurdenDecVariant { var } if var == ArcVarId::new(7)),
        "BurdenDecVariant substitute_var rewrites var",
    );
}

#[test]
fn settag_scalar_base_emits_no_burden_dec_variant() {
    // negative pin (clamps positive pin from below per
    //): SetTag on a base var whose burden is
    // EMPTY (scalar / no owned fields — fails `burden_carries_rc` filter at
    // `compute_owned_vars_needing_rc`) MUST NOT emit BurdenDecVariant.
    // Mirrors BurdenDecField's gate via
    // `owned_vars_needing_rc.contains(base)` — same gate, same filter.
    //  (no RC ops on scalars):
    // BurdenDecVariant on a scalar would be a structural violation. Idx::INT
    // is the canonical scalar negative-pin type per this
    // `set_scalar_value_emits_no_burden_inc_via_tf_15_carve_out_filter`.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::INT],
        blocks: vec![entry_block(
            vec![ArcInstr::SetTag {
                base: ArcVarId::new(0),
                tag: 1,
            }],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;

    // Pin (negative, VF-1 RcOnScalar mirror): zero BurdenDecVariant emitted.
    let dec_variant_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecVariant { .. }))
        .count();
    assert_eq!(
        dec_variant_count, 0,
        "expected zero BurdenDecVariant when base type is scalar (Idx::INT); body={body:?}",
    );
}

#[test]
fn burden_dec_emitted_after_non_transfer_last_use() {
    // : BurdenDec(v) emits immediately following last-use UNLESS
    // last-use is ownership-transferring per RL-2. ships filtered
    // BurdenDec emission. Positive pin: var(0):str (heap-burden type) used
    // ONLY at IsShared (non-transfer per is_owned_position `_ => false`)
    // ⇒ BurdenDec(var(0)) MUST emit after the IsShared instr.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::BOOL],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::IsShared {
                dst: ArcVarId::new(1),
                var: ArcVarId::new(0),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Expected post-emission: [IsShared, BurdenDec(0)] — IsShared is NOT
    // owned-position so no BurdenInc; var(0) last use is here, non-
    // transfer, so BurdenDec emits after.
    assert_eq!(
        func.blocks[0].body.len(),
        2,
        "expected [IsShared, BurdenDec(0)] post-emission, got {:?}",
        func.blocks[0].body,
    );
    assert!(
        matches!(&func.blocks[0].body[0], ArcInstr::IsShared { .. }),
        "first instr MUST be IsShared",
    );
    match &func.blocks[0].body[1] {
        ArcInstr::BurdenDec { var } => assert_eq!(*var, ArcVarId::new(0)),
        other => panic!("expected BurdenDec(0) after IsShared, got {other:?}"),
    }
}

#[test]
fn partial_apply_emits_burden_inc_for_captured_var() {
    //  +: PartialApply captures emit BurdenInc when
    // the captured var has a follow-up use (Let-Var alias keeps it alive past
    // the PartialApply). Last-use at this instr would suppress
    // ITEM-3 to preserve VF-1 balance.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_pos.is_some(),
        "expected BurdenInc(captured=var(0)) before PartialApply; body={body:?}",
    );
    let pa_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::PartialApply { .. }))
        .unwrap_or_else(|| panic!("PartialApply MUST appear in body"));
    assert!(
        inc_pos.unwrap_or_else(|| unreachable!("checked is_some above")) < pa_pos,
        "BurdenInc(captured) MUST appear BEFORE PartialApply; body={body:?}",
    );
}

#[test]
fn closure_capture_then_drop_emits_net_balanced_burden() {
    // verify-first pin (higher_order over-elim-closure cells
    // test_hof_closure_capture_in_loop / test_hof_make_predicate, proof
    // 04B.2-over-elim-closure): the predicate stack over-eliminated the
    // capture-side inc while retaining the paired scope-exit dec -> double-free.
    // The burden baseline MUST be NET-BALANCED (total BurdenInc == total
    // BurdenDec) so DP-2/DP-3 elimination over the balanced baseline cannot
    // orphan a dec. Shape: owned str captured into a closure, closure invoked,
    // closure env dropped at block exit (not returned).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                // var(0) = owned heap str (FRESH).
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                },
                // var(1) = closure capturing var(0) (ownership transfers into env).
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                },
                // closure invoked; var(2) = result.
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    closure: ArcVarId::new(1),
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                },
            ],
            // closure (var(1)) + result (var(2)) die here (not returned).
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { .. }))
        .count();
    let dec_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { .. }))
        .count();
    assert_eq!(
        inc_count, dec_count,
        "burden baseline MUST be net-balanced for closure capture+drop \
         (over-elim-closure double-free guard); inc={inc_count} dec={dec_count} body={body:?}",
    );
}

#[test]
fn closure_capture_in_loop_body_block_emits_net_balanced_burden() {
    // closures-inside-loops with conditional capture (the
    // test_hof_closure_capture_in_loop shape). A loop-carried owned value is
    // handed to a non-entry (loop-body) block via a block-param Jump, captured
    // into a closure there, the closure invoked + dropped. The burden baseline
    // MUST stay net-balanced when the capture happens in a non-entry block
    // (the per-block walk emits the capture inc + last-use dec in the body
    // block) — burden-emitted regardless of block position.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                }],
                // loop-carried owned value handed to the body block.
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![ArcVarId::new(0)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(ArcVarId::new(1), Idx::STR)],
                body: vec![
                    // closure captures the loop-carried value (var(1)).
                    ArcInstr::PartialApply {
                        dst: ArcVarId::new(2),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![ArcVarId::new(1)],
                    },
                    // closure invoked; var(2)+var(3) die at block exit.
                    ArcInstr::ApplyIndirect {
                        dst: ArcVarId::new(3),
                        ty: Idx::STR,
                        closure: ArcVarId::new(2),
                        args: Vec::new(),
                        arg_ownership: Vec::new(),
                    },
                ],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let inc_count: usize = func
        .blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter(|i| matches!(i, ArcInstr::BurdenInc { .. }))
        .count();
    let dec_count: usize = func
        .blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter(|i| matches!(i, ArcInstr::BurdenDec { .. }))
        .count();
    assert_eq!(
        inc_count, dec_count,
        "burden baseline MUST be net-balanced for a closure capture in a \
         non-entry (loop-body) block; inc={inc_count} dec={dec_count}",
    );
}

#[test]
fn captures_of_captures_emits_net_balanced_burden() {
    // captures-of-captures recursion. Closure A captures an owned
    // str; closure B captures closure A. Each capture transfers ownership into
    // the enclosing env; the burden baseline MUST stay net-balanced across the
    // nesting (no orphan inc/dec per level) — the N-level generalization of the
    // over-elim-closure guard.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                // var(0) = owned heap str.
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                },
                // var(1) = closure A capturing var(0).
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                },
                // var(2) = closure B capturing closure A (var(1)).
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: Name::from_raw(98),
                    args: vec![ArcVarId::new(1)],
                },
                // closure B invoked; var(3) = result. var(2)+var(3) die at exit.
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    closure: ArcVarId::new(2),
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { .. }))
        .count();
    let dec_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { .. }))
        .count();
    assert_eq!(
        inc_count, dec_count,
        "burden baseline MUST be net-balanced for captures-of-captures \
         (N-level over-elim-closure guard); inc={inc_count} dec={dec_count} body={body:?}",
    );
}

#[test]
fn value_heap_mixed_variant_emits_dec_only_for_heap_field() {
    // Value/HeapType-mixed-variant pin. A struct mixing a `Value` field
    // (`tag: int`, inline, no RC) with a `HeapType` field (`payload: str`,
    // owned) burden-emits exactly one whole-var BurdenDec covering the str
    // field via drop-glue; the Value field drives NO burden op (no per-field
    // inc, no BurdenDecField for field 0). The faithful Phase-5 emission keeps
    // the burden ledger balanced (VF-1 net-zero); this pins the mixed-variant
    // COMPOSITION — over-emission guard for the Value field + under-emission
    // guard for the HeapType field. owned_fields=[str@1] only: burden RC
    // tracking is true via the heap field, the int Value field omitted.
    use crate::lower::test_utils::registered_struct_value_heap_mixed;

    let mut registry = TypeRegistry::new();
    let struct_idx = Idx::from_raw(64); // first dynamic slot per TY-5
    registered_struct_value_heap_mixed(&mut registry, "Mixed", struct_idx);

    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: struct_idx,
            ownership: Ownership::Owned,
        }],
        // var(0)=Mixed param, var(1)=str (a borrow-view of field 1).
        var_types: vec![struct_idx, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            // Pure borrow of the heap field (Project = Borrowed per TF-4, NOT a
            // move — moved_fields.rs only sets the moved-out bit when the
            // project dst is transferred). var(1) is unused; this is the
            // last use of var(0), so its whole-var dec fires here. var(0) is
            // NOT moved out (nothing transfers field 1 onward).
            body: vec![ArcInstr::Project {
                dst: ArcVarId::new(1),
                ty: Idx::STR,
                value: ArcVarId::new(0),
                field: 1,
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;

    // Exactly one whole-var BurdenDec for var(0): the scope-exit drop-glue
    // covers the str field. owned_fields=[str@1] means burden_carries_rc fires;
    // an owned non-moved aggregate dies via a whole-var dec.
    let whole_decs: Vec<&ArcInstr> = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        whole_decs.len(),
        1,
        "Value/HeapType-mixed: MUST emit exactly one whole-var BurdenDec for var(0) \
         (drop-glue covers the str field at scope exit); got {whole_decs:?}; body={body:?}",
    );

    // The Value field (field 0, int) drives NO per-field burden op — no
    // over-emission. A fully-dropped mixed var emits a whole-var dec, never a
    // BurdenDecField / BurdenDecPartial naming the Value field.
    let field_ops: Vec<&ArcInstr> = body
        .iter()
        .filter(|i| {
            matches!(i, ArcInstr::BurdenDecField { base, .. } if *base == ArcVarId::new(0))
                || matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0))
        })
        .collect();
    assert!(
        field_ops.is_empty(),
        "Value/HeapType-mixed: a fully-dropped mixed var emits a whole-var dec, \
         NOT per-field ops; the Value field (int) must drive no burden op; got {field_ops:?}",
    );
}

#[test]
fn collection_reuse_emits_burden_inc_for_owned_arg() {
    //  +: CollectionReuse with Owned arg emits
    // BurdenInc when arg has a follow-up use (Let-Var keeps it alive past
    // the CollectionReuse). Last-use at this instr would suppress per
    //  to preserve VF-1 balance.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::CollectionReuse {
                    old_var: ArcVarId::new(0),
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    ctor: CtorKind::ListLiteral,
                    args: vec![ArcVarId::new(1)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_pos_arg = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_pos_arg.is_some(),
        "expected BurdenInc(arg=var(1)) before CollectionReuse; body={body:?}",
    );
    let cr_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::CollectionReuse { .. }))
        .unwrap_or_else(|| panic!("CollectionReuse MUST appear in body"));
    assert!(
        inc_pos_arg.unwrap_or_else(|| unreachable!("checked is_some above")) < cr_pos,
        "BurdenInc(arg) MUST appear BEFORE CollectionReuse; body={body:?}",
    );
    // Negative pin: var(0) is old_var (used_vars pos 0 — NOT owned position
    // per is_owned_position rule); MUST NOT receive BurdenInc.
    let inc_pos_old = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_pos_old.is_none(),
        "var(0) is old_var at used_vars pos 0 (NOT owned); BurdenInc(var(0)) MUST NOT emit; body={body:?}",
    );
}

#[test]
fn apply_indirect_emits_burden_inc_for_owned_arg_not_closure() {
    //  +: ApplyIndirect emits BurdenInc for Owned
    // arg when arg has follow-up use (Let-Var keeps it alive past the
    // ApplyIndirect). Closure at pos 0 always Borrowed, no Inc.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    closure: ArcVarId::new(0),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Pin (positive): BurdenInc(arg=var(1)) emitted BEFORE ApplyIndirect.
    let body = &func.blocks[0].body;
    let inc_pos_arg = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_pos_arg.is_some(),
        "expected BurdenInc(arg=var(1)) before ApplyIndirect; body={body:?}",
    );
    let ai_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::ApplyIndirect { .. }))
        .unwrap_or_else(|| panic!("ApplyIndirect MUST appear in body"));
    assert!(
        inc_pos_arg.unwrap_or_else(|| unreachable!("checked is_some above")) < ai_pos,
        "BurdenInc(arg) MUST appear BEFORE ApplyIndirect; body={body:?}",
    );
    // Pin (negative): closure (var(0)) at used_vars pos 0 is borrowed per
    // `is_owned_position`'s `pos == 0 → false` rule. MUST NOT receive
    // BurdenInc regardless of var_types (closures-as-Idx::STR is a test
    // simplification; the ownership rule is positional, not type-based).
    let inc_pos_closure = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_pos_closure.is_none(),
        "var(0) is closure at used_vars pos 0 (always borrowed per is_owned_position); BurdenInc(var(0)) MUST NOT emit; body={body:?}",
    );
}

#[test]
fn apply_mixed_owned_borrowed_args_emits_burden_inc_per_position() {
    //  +: per-position arg_ownership filter +
    // last-use check. Owned arg(0) with follow-up Let-Var alias keeps it
    // alive past Apply, so BurdenInc(arg=0) IS emitted. Borrowed arg(1)
    // never receives Inc regardless of last-use.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // Pin (positive): BurdenInc(args[0]=var(0)) emitted BEFORE Apply.
    let inc_pos_owned = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_pos_owned.is_some(),
        "expected BurdenInc(args[0]=var(0)) before Apply (arg_ownership[0]=Owned); body={body:?}",
    );
    let apply_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Apply { .. }))
        .unwrap_or_else(|| panic!("Apply MUST appear in body"));
    assert!(
        inc_pos_owned.unwrap_or_else(|| unreachable!("checked is_some above")) < apply_pos,
        "BurdenInc(args[0]) MUST appear BEFORE Apply; body={body:?}",
    );
    // Pin (negative): args[1] is Borrowed → NO BurdenInc emitted regardless
    // of var_type (the ownership rule is per-position via arg_ownership,
    // not type-based).
    let inc_pos_borrowed = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_pos_borrowed.is_none(),
        "var(1) is Borrowed at arg_ownership[1]; BurdenInc(var(1)) MUST NOT emit; body={body:?}",
    );
}

#[test]
fn apply_indirect_empty_arg_ownership_emits_no_burden_inc() {
    // success_criterion 1: ApplyIndirect's empty arg_ownership defaults
    // to all-Borrowed per `is_some_and(Owned)` (instr.rs:367-380) — CONSERVATIVE
    // for unknown callees; caller retains cleanup. This is the load-bearing
    // safety distinction from Apply (instr.rs:381-390) whose empty default is
    // all-Owned via `is_none_or(Owned)`. Without this pin, a future refactor
    // unifying the two predicates (copy-paste from Apply arm) would silently
    // break ApplyIndirect's conservative semantics — unannotated callsites
    // would receive spurious BurdenInc, doubling refcount and leaking.
    // Per `RL-2` ownership-transferring exception.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    closure: ArcVarId::new(0),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: Vec::new(), // empty → all-Borrowed default
                },
                // TWO follow-up aliases keep result var(2) GENUINELY live
                // (use-count >= 2 = duplication, not a single-use move): the
                // FRESH-site BurdenInc fires. A single-use move-alias is an RL-2
                // ownership transfer whose source inc is correctly suppressed.
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // : post-emission body is [BurdenInc(dst=2) [FRESH-site —
    // ApplyIndirect lowers to TF-5a CONSERVATIVE MaybeShared return],
    // ApplyIndirect]. The PER-ARG BurdenInc loop emits zero entries because
    // empty arg_ownership defaults all positions to Borrowed (instr.rs:367-
    // 380 is_some_and). The FRESH-site Inc is the symmetric pair-opener
    // for the last-use Dec on dst per RL-2.
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert_eq!(
        inc_vars,
        vec![ArcVarId::new(2)],
        "ApplyIndirect with empty arg_ownership MUST emit only the FRESH-site BurdenInc(dst=2) and ZERO per-arg BurdenInc (conservative all-Borrowed default per is_some_and); body={body:?}",
    );
    // Verify the instruction body still contains the ApplyIndirect.
    let ai_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::ApplyIndirect { .. }));
    assert!(
        ai_pos.is_some(),
        "ApplyIndirect MUST appear in body; body={body:?}",
    );
}

#[test]
fn multi_block_last_use_pinned_per_block_pending_cross_block() {
    // INTENTIONAL intra-block scope per `burden_lower.rs:128` comment.
    // cross-block CFG-aware last-use will collapse this to ONE entry
    // — that change IS the desired cell flip, not a regression.
    //
    // walker (burden_lower.rs:132-141) does per-block backward walks:
    // `seen: FxHashSet` declared INSIDE the block loop, so a variable used
    // in BOTH blocks produces TWO `last_use_points` entries — one per block.
    // Cross-block liveness via block-param handoffs lands.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::IsShared {
                    dst: ArcVarId::new(0), // dummy; reuses var(0) as Set arg
                    var: ArcVarId::new(0),
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::IsShared {
                    dst: ArcVarId::new(0),
                    var: ArcVarId::new(0),
                }],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Pin: TWO last_use_points entries for var(0) — one per block. Per-block
    // walk identifies last use in EACH block separately (intra-block scope).
    let var0_entries: Vec<_> = ctx
        .last_use_points()
        .iter()
        .filter(|(v, _, _)| *v == ArcVarId::new(0))
        .collect();
    assert_eq!(
        var0_entries.len(),
        2,
        "per-block walk MUST identify var(0) last-use in EACH block separately (intra-block scope); cross-block last-use will collapse to 1. last_use_points={:?}",
        ctx.last_use_points(),
    );
    // Verify one entry per block.
    let block_indices: std::collections::HashSet<usize> =
        var0_entries.iter().map(|(_, b, _)| *b).collect();
    assert_eq!(
        block_indices.len(),
        2,
        "var(0) MUST have one last-use entry in block 0 AND one in block 1 (intra-block scope); block_indices={block_indices:?}",
    );
}

#[test]
fn construct_multi_arg_emits_burden_inc_per_arg_in_iteration_order() {
    //  +: emission loop iterates ALL owned positions
    // in declaration order. Args with follow-up Let-Var aliases keep them
    // alive past the Construct, so per-arg BurdenInc IS emitted.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    ctor: CtorKind::Tuple,
                    args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(5),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(1)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(6),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
                // TWO follow-up aliases keep result var(3) GENUINELY live
                // (use-count >= 2 = duplication, not a single-use move): the
                // FRESH-site BurdenInc fires. A single-use move-alias is an RL-2
                // ownership transfer whose source inc is correctly suppressed.
                ArcInstr::Let {
                    dst: ArcVarId::new(7),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(3)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(8),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(3)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // Default path (predicate_stack_rc_disabled=false): per-arg owned-position
    // BurdenInc(0..=2) emit first (`emit_owned_position_incs`), THEN the
    // FRESH-site BurdenInc(dst=3); ALL Incs precede the Construct (burden ops
    // are codegen no-op markers here, never lowered to real RC at this site).
    // The probe path reorders the FRESH-site Inc to AFTER the instruction (so
    // the lowered RcInc sees a defined dst); covered by the predicate_stack_probe
    // AOT suite. Order: 0, 1, 2, then 3 — all before the Construct.
    let expected = [
        ArcVarId::new(0),
        ArcVarId::new(1),
        ArcVarId::new(2),
        ArcVarId::new(3),
    ];
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert_eq!(
        inc_vars,
        expected,
        "Construct with 3 Owned args MUST emit per-arg BurdenInc(0..=2) THEN FRESH-site BurdenInc(dst=3), all before the Construct, on the default path; got {inc_vars:?}; body={body:?}",
    );
    // Verify all BurdenInc emissions precede the Construct on the default path.
    let construct_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Construct { .. }))
        .unwrap_or_else(|| panic!("Construct MUST appear in body"));
    let last_inc_pos = body
        .iter()
        .rposition(|i| matches!(i, ArcInstr::BurdenInc { .. }))
        .unwrap_or_else(|| panic!("BurdenInc emissions MUST appear in body"));
    assert!(
        last_inc_pos < construct_pos,
        "ALL BurdenInc emissions MUST precede Construct on the default path; last_inc_pos={last_inc_pos}, construct_pos={construct_pos}; body={body:?}",
    );
}

#[test]
fn scalar_int_var_emits_no_burden_dec_at_last_use() {
    //  + `DP-1` (is_rc_needed:... ∧ ¬is_scalar)
    // + `§9 VF-1 RcOnScalar`. A var typed `Idx::INT` (scalar) MUST NOT
    // receive BurdenDec emission even at non-transfer last-use.
    //
    // This test surfaces the filter fix: `lookup_burden(Idx::INT)`
    // returns `Some(BurdenRef)` carrying `BuiltinBurdenSpec::EMPTY` (per
    // `BURDEN_TABLE` at `ori_registry/src/burden/table.rs:184-193`), NOT
    // None. A naive filter `burden.as_ref.map(|_| *var)` admits EMPTY
    // and emits BurdenDec on scalars (RcOnScalar violation). The
    // fix at `burden_lower.rs:154-178` checks BurdenRef contents via the
    // `Burden` trait: `self_heap_alloc || element_burden.is_some ||
    // variant_burdens.next.is_some || owned_fields.next.is_some`.
    //
    // Test fixture: var(0) = Idx::INT (scalar), used at IsShared { var } —
    // a non-transfer last-use (IsShared has no owned positions per
    // `is_owned_position`'s `_ => false`). Naive filter would emit
    // BurdenDec(var(0)); fixed filter must NOT.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::IsShared {
                dst: ArcVarId::new(1),
                var: ArcVarId::new(0),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let any_burden_dec = body.iter().any(|i| matches!(i, ArcInstr::BurdenDec { .. }));
    assert!(
        !any_burden_dec,
        "scalar Idx::INT var MUST NOT receive BurdenDec at last-use (VF-1 RcOnScalar); body={body:?}",
    );
}

#[test]
fn heap_burden_borrowed_param_skipped_at_ownership_filter() {
    // Burden-walk contract: ownership filter MUST skip Borrowed params BEFORE
    // `lookup_burden` is consulted. This is the load-bearing early-skip
    // for heap-burden Borrowed params: per `burden_lower.rs:111-113`,
    // `matches!(param_ownership.get(&var), Some(Ownership::Borrowed))
    // → continue` short-circuits the param loop before push to ctx.collected.
    //
    // Distinct from borrowed_params_skipped_owned_params_collected
    // which uses Idx::INT (scalar — burden=EMPTY, fails burden_carries_rc
    // anyway). This cell tests the realistic heap-burden case: Idx::STR
    // carries self_heap_alloc=true per BURDEN_TABLE, so without the early-
    // skip, var(1)=STR/Borrowed would flow into ctx.collected, pass
    // burden_carries_rc (self_heap_alloc=true), enter owned_vars_needing_rc,
    // and emit spurious BurdenInc/BurdenDec violating the burden-walk contract.
    // A future refactor removing the ownership filter would still pass the
    // scalar Idx::INT test AND the scalar VF-1 test —
    // this heap-burden Idx::STR test would FAIL, surfacing the regression.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Borrowed,
            },
        ],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Pin: var(1) STR/Borrowed MUST be absent from collected_burdens.
    let var1_collected = ctx
        .collected_burdens()
        .iter()
        .any(|(v, _)| *v == ArcVarId::new(1));
    assert!(
        !var1_collected,
        "Idx::STR + Borrowed param MUST be skipped at ownership filter (BEFORE lookup_burden); collected_burdens={:?}",
        ctx.collected_burdens(),
    );
    // Verify var(0) STR/Owned IS collected (early-skip is ownership-specific,
    // not type-specific).
    let var0_collected = ctx
        .collected_burdens()
        .iter()
        .any(|(v, _)| *v == ArcVarId::new(0));
    assert!(
        var0_collected,
        "Idx::STR + Owned param MUST be collected (filter skips Borrowed only); collected_burdens={:?}",
        ctx.collected_burdens(),
    );
    // Block body is empty → zero BurdenInc/BurdenDec emitted regardless.
    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "empty block body MUST emit no instructions; body={body:?}",
    );
}

#[test]
fn apply_three_args_with_non_adjacent_owned_positions_emits_burden_inc_per_owned() {
    //  +: per-position arg_ownership filter MUST
    // continue past Borrowed positions. Args with follow-up Let-Var aliases
    // keep them alive past Apply, so per-arg BurdenInc IS emitted.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
                    arg_ownership: vec![
                        ArgOwnership::Owned,
                        ArgOwnership::Borrowed,
                        ArgOwnership::Owned,
                    ],
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(5),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
                // Keep the result var(3) live so its FRESH-site BurdenInc emits.
                ArcInstr::Let {
                    dst: ArcVarId::new(6),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(3)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(7),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(3)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    // Default path: per-arg owned-position BurdenInc(0), BurdenInc(2) emit
    // first (`emit_owned_position_incs`; [Owned, Borrowed, Owned] skips
    // BurdenInc(1)), THEN the FRESH-site BurdenInc(dst=3) [Apply no contract →
    // MaybeShared return per TF-5]; all Incs precede the Apply. The probe path
    // reorders the FRESH-site Inc to AFTER the Apply (covered by the
    // predicate_stack_probe AOT suite). Order: 0, 2, then 3.
    let expected = [ArcVarId::new(0), ArcVarId::new(2), ArcVarId::new(3)];
    assert_eq!(
        inc_vars,
        expected,
        "Apply [Owned, Borrowed, Owned] MUST emit BurdenInc(0), BurdenInc(2) (skip 1) THEN FRESH-site BurdenInc(dst=3), all before the Apply, on the default path; got {inc_vars:?}; body={body:?}",
    );
    // Verify all BurdenInc emissions precede the Apply on the default path.
    let apply_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Apply { .. }))
        .unwrap_or_else(|| panic!("Apply MUST appear in body"));
    let last_inc_pos = body
        .iter()
        .rposition(|i| matches!(i, ArcInstr::BurdenInc { .. }))
        .unwrap_or_else(|| panic!("BurdenInc emissions MUST appear in body"));
    assert!(
        last_inc_pos < apply_pos,
        "ALL BurdenInc emissions MUST precede Apply on the default path; last_inc_pos={last_inc_pos}, apply_pos={apply_pos}",
    );
}

#[test]
fn partial_apply_mixed_str_int_emits_burden_inc_only_for_heap_burden() {
    //  + `VF-1` RcOnScalar` mirror to BurdenInc
    // emission. PartialApply args=[STR, INT]: STR carries heap-burden
    // (passes burden_carries_rc); INT carries EMPTY burden (per BURDEN_TABLE
    // at `ori_registry/src/burden/table.rs:184-193`) — filter MUST
    // admit STR and reject INT.
    //
    // This is the BurdenInc symmetric pin to this BurdenDec scalar
    // exclusion. Without the filter, `lookup_burden(Idx::INT)`
    // returns `Some(EMPTY_SPEC)` and the unfiltered BurdenInc loop would
    // emit BurdenInc on the scalar arg — RcOnScalar violation per IR
    // variant doc ("BurdenInc parallels RcInc; tracks burden lattice").
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_str_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_str_pos.is_some(),
        "expected BurdenInc(args[0]=STR=var(0)) before PartialApply; body={body:?}",
    );
    let inc_int_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_int_pos.is_none(),
        "var(1) is Idx::INT (scalar, EMPTY burden); BurdenInc(var(1)) MUST NOT emit per VF-1 RcOnScalar mirror; body={body:?}",
    );
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::PartialApply { .. })),
        "PartialApply MUST appear in body; body={body:?}",
    );
}

#[test]
fn apply_indirect_scalar_owned_arg_emits_no_burden_inc() {
    //  + VF-1 RcOnScalar mirror — cross-instr coverage.
    // ApplyIndirect with arg_ownership=[Owned] + arg=Idx::INT (scalar) MUST
    // emit ZERO BurdenInc. Per instr.rs:367-380 ApplyIndirect arm: closure
    // at pos 0 always borrowed; arg at pos 1 owned iff arg_ownership[0]=Owned.
    // Per filter (burden_lower.rs:171-175): owned_vars_needing_rc
    // rejects Idx::INT (EMPTY burden) so no BurdenInc emits.
    //
    // this partial_apply_mixed_str_int test covered PartialApply;
    // extends VF-1 BurdenInc-side coverage to ApplyIndirect via
    // the SAME single generic emission loop. A regression that re-introduces
    // a per-variant unfiltered emission path for ApplyIndirect specifically
    // (e.g., bypassing owned_vars_needing_rc) would FAIL this pin while
    // potentially passing the PartialApply pin.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    closure: ArcVarId::new(0),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                // Keep the result var(2) live so its FRESH-site BurdenInc emits.
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // : ApplyIndirect dst=2 is STR (heap) → FRESH-site
    // BurdenInc(dst=2) emits per TF-5a CONSERVATIVE MaybeShared return.
    // The per-arg loop still emits ZERO Incs for var(1)=Idx::INT (
    // VF-1 filter excludes EMPTY-spec scalars from owned_vars_needing_rc).
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert_eq!(
        inc_vars,
        vec![ArcVarId::new(2)],
        "ApplyIndirect with Idx::INT scalar Owned arg MUST emit only the FRESH-site BurdenInc(dst=2) and ZERO per-arg BurdenInc (cycle-24 VF-1 mirror); body={body:?}",
    );
    // Verify the ApplyIndirect itself is preserved (filter affects BurdenInc
    // emission, not the underlying instruction).
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::ApplyIndirect { .. })),
        "ApplyIndirect MUST appear in body; body={body:?}",
    );
}

#[test]
fn set_scalar_value_emits_no_burden_inc_via_tf_15_carve_out_filter() {
    //  + `TF-15` + `§9 VF-1 RcOnScalar`. Set's
    // `value` is owned via IA-5 alias-transfer step (1) per TF-15 carve-out;
    // NOT covered by `is_owned_position`'s `_ => false` catch-all. The
    // BurdenInc emission for Set's value happens in a SEPARATE if-let block
    // (burden_lower.rs:217-225) distinct from the main owned-position loop.
    //
    // added `owned_vars_needing_rc.contains(value)` to BOTH the
    // main loop AND the Set carve-out. This test closes the LAST unclamped
    // path of this filter: a regression reverting only the Set-path
    // filter (e.g., copy-paste from a different file or pre-logic
    // restored) would pass all current Apply/PartialApply/CollectionReuse/
    // ApplyIndirect scalar pins (cycles 21+24+25) but FAIL this Set pin.
    //
    // already covers the positive case (Idx::STR value emits
    // BurdenInc); this is the symmetric Idx::INT scalar negative pin.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Set {
                base: ArcVarId::new(0),
                field: 0,
                value: ArcVarId::new(1),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // Pin (negative, VF-1 mirror on Set carve-out): zero BurdenInc emitted.
    let any_burden_inc = body.iter().any(|i| matches!(i, ArcInstr::BurdenInc { .. }));
    assert!(
        !any_burden_inc,
        "Set with Idx::INT scalar value MUST emit ZERO BurdenInc via TF-15 carve-out filter (cycle-24 VF-1 mirror); body={body:?}",
    );
    // Verify Set itself is preserved.
    assert!(
        body.iter().any(|i| matches!(i, ArcInstr::Set { .. })),
        "Set MUST appear in body; body={body:?}",
    );
}

#[test]
fn construct_multi_arg_mixed_types_emits_burden_inc_for_heap_burden_args_only() {
    // : cross-dimension matrix cell — multi-arg Construct with
    // scalar in non-edge (middle) position. Combines multi-arg
    // ordering coverage with scalar-filter coverage; distinct from
    // both: uses all-STR (no filter exercise per-position), cycle
    // 24 uses 2-arg edge-only [STR, INT] on PartialApply.
    //
    // Defends per-position filter correctness against a regression that
    // would blanket-apply burden_carries_rc across all args (passes cycle
    // 20 + but fails).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![
            Idx::STR,
            Idx::INT,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    ctor: CtorKind::Tuple,
                    args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(5),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
                // Keep the result var(3) live so its FRESH-site BurdenInc emits.
                ArcInstr::Let {
                    dst: ArcVarId::new(6),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(3)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(7),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(3)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    //  + ITEM-3: FRESH-site BurdenInc(dst=3) precedes per-arg
    // Incs. var(0) and var(2) (STR, follow-up Let-Var aliases) emit owned-
    // pos BurdenInc; var(1) (INT) excluded per VF-1 RcOnScalar.
    assert!(
        inc_vars.contains(&ArcVarId::new(3)),
        "expected FRESH-site BurdenInc(dst=3); got {inc_vars:?}; body={body:?}",
    );
    assert!(
        inc_vars.contains(&ArcVarId::new(0)),
        "expected BurdenInc(arg=0=STR) before Construct; got {inc_vars:?}; body={body:?}",
    );
    assert!(
        inc_vars.contains(&ArcVarId::new(2)),
        "expected BurdenInc(arg=2=STR) before Construct; got {inc_vars:?}; body={body:?}",
    );
    assert!(
        !inc_vars.contains(&ArcVarId::new(1)),
        "scalar var(1) MUST NOT emit BurdenInc per VF-1 RcOnScalar; got {inc_vars:?}; body={body:?}",
    );
}

#[test]
fn apply_all_borrowed_args_emits_zero_burden_inc() {
    // : all-Borrowed corner cell. Matrix has all-Owned (
    // updated), [Owned,Borrowed] mixed, [Owned,Borrowed,Owned]
    // non-adjacent; all-Borrowed is the missing corner per
    //  Clamping clamp-from-all-sides.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                // Keep the result var(2) live so its FRESH-site BurdenInc emits
                // (a dead result is correctly suppressed — predicate-stack-
                // managed per the proven CH-comp case-3 coexistence handshake).
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // : per-arg loop still emits ZERO Incs (both args Borrowed).
    // FRESH-site BurdenInc(dst=2) emits because dst=STR is heap and Apply
    // with no contract defaults to MaybeShared return (TF-5 CONSERVATIVE).
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert_eq!(
        inc_vars,
        vec![ArcVarId::new(2)],
        "Apply with arg_ownership=[Borrowed, Borrowed] MUST emit only the FRESH-site BurdenInc(dst=2) and ZERO per-arg BurdenInc; body={body:?}",
    );
    assert!(
        body.iter().any(|i| matches!(i, ArcInstr::Apply { .. })),
        "Apply MUST appear in body; body={body:?}",
    );
}

#[test]
fn partial_apply_empty_args_emits_zero_burden_inc() {
    // : empty-args boundary cell. PartialApply with args=[]:
    // is_owned_position(pos) = pos < 0 = false for all pos; used_vars
    // returns empty SmallVec; emission loop body never executes. Pins
    // off-by-one (loop ends args.len-1) + unconditional-emit + hardcoded
    // pos==0 shortcut regressions per step 3 (edge cases:
    // empty, single-element, boundary).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: Vec::new(),
                },
                // Follow-up use keeps the result var(0) live so its FRESH-site
                // BurdenInc emits (a DEAD result is correctly suppressed —
                // predicate-stack-managed per the proven CH-comp case-3
                // coexistence handshake).
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // : per-arg loop emits zero Incs (args=[] → no positions).
    // FRESH-site BurdenInc(dst=0) emits because PartialApply dst is heap
    // per TF-7 FRESH(NonReusable).
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert_eq!(
        inc_vars,
        vec![ArcVarId::new(0)],
        "PartialApply with args=[] MUST emit only the FRESH-site BurdenInc(dst=0) and ZERO per-arg BurdenInc (args=[] → off-by-one / unconditional-emit / hardcoded-pos pins held); body={body:?}",
    );
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::PartialApply { .. })),
        "PartialApply MUST appear in body; body={body:?}",
    );
}

#[test]
fn construct_empty_args_emits_zero_burden_inc() {
    // : empty-args boundary mirror to this PartialApply
    // empty-args pin. Shared is_owned_position branch at instr.rs:352:
    // `Construct { args,.. } | PartialApply { args,.. } => pos < args.len`.
    // args=[] → predicate false for all pos → zero BurdenInc.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    ctor: CtorKind::Tuple,
                    args: Vec::new(),
                },
                // Keep the result var(0) live so its FRESH-site BurdenInc emits.
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // : per-arg loop emits zero Incs (args=[] → no positions).
    // FRESH-site BurdenInc(dst=0) emits because Construct dst is heap per
    // TF-3 FRESH.
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    assert_eq!(
        inc_vars,
        vec![ArcVarId::new(0)],
        "Construct with args=[] MUST emit only the FRESH-site BurdenInc(dst=0) and ZERO per-arg BurdenInc; body={body:?}",
    );
    assert!(
        body.iter().any(|i| matches!(i, ArcInstr::Construct { .. })),
        "Construct MUST appear in body; body={body:?}",
    );
}

#[test]
fn last_use_detected_at_single_block_use_position() {
    // success_criterion 2: "BurdenDec(v) emits immediately following
    // EVERY last-use of v along EVERY reachable CFG path." ships
    // per-block backward-walk scaffold. Semantic pin: var(0) is used once
    // (as Apply arg at block 0, instr 0); last_use_points MUST contain
    // exactly one entry pinning that position.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::INT, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: Name::from_raw(99),
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert_eq!(
        ctx.last_use_points(),
        &[(ArcVarId::new(0), 0, 0)],
        "var(0)'s last use is at block 0 instr 0 (Apply arg); per-block backward walk MUST identify it",
    );
}

#[test]
fn iteration_produces_one_entry_per_var_type() {
    // Semantic pin: would FAIL if iteration body is reverted to no-op or
    // todo! — collected_burdens length must match var_types length.
    let registry = TypeRegistry::new();
    let mut func = func_with_n_vars(3);
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert_eq!(
        ctx.collected_burdens().len(),
        3,
        "iteration scaffold MUST visit every var_types entry",
    );
    let vars: Vec<ArcVarId> = ctx.collected_burdens().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        vars,
        vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
        "iteration order MUST match var_types declaration order",
    );
}

#[test]
fn return_str_owned_value_used_in_prior_instr_suppresses_burden_dec_per_rl2() {
    // first rule positive pin: Return transfers ownership per
    // `RL-2` — Return's `value` is a terminator-transfer
    // point. When `value` is also used at an earlier instruction (here as
    // IsShared's `var` operand at non-owned position), the terminator-position
    // last-use registration takes precedence over the prior-instruction-position
    // entry (terminator scans first in backward walk; first-seen-wins). At
    // emission time the terminator-position entry hits terminator_transfer_vars
    // and is filtered out — no BurdenDec emits anywhere for `value`. Without
    // the terminator-walking last-use scan + Return-transfer-var filter,
    // the prior-instruction last-use would emit BurdenDec(0) after IsShared,
    // double-releasing the value Return transfers to the caller.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::BOOL],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::IsShared {
                dst: ArcVarId::new(1),
                var: ArcVarId::new(0),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    // Pin 1: NO BurdenInc on var(0) — IsShared is not owned-position; Return
    // is a transfer (not a BurdenInc site first rule).
    let inc_present = body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        !inc_present,
        "Return.value MUST NOT receive BurdenInc; body={body:?}",
    );
    // Pin 2: NO BurdenDec on var(0) — Return transfers ownership per RL-2;
    // prior-instruction last-use suppressed by terminator-position priority.
    let dec_present = body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)));
    assert!(
        !dec_present,
        "Return.value MUST NOT receive BurdenDec at prior-instruction last-use \
         (RL-2 transfer-point exception; double-release if emitted); body={body:?}",
    );
    // Pin 3: Body shape is [IsShared] only — the only instruction emitted.
    assert_eq!(
        body.len(),
        1,
        "expected [IsShared] only (no BurdenInc/Dec around Return-transferred value), got {body:?}",
    );
    assert!(matches!(&body[0], ArcInstr::IsShared { .. }));
}

#[test]
fn moved_out_fields_is_empty_when_function_has_no_project() {
    // negative pin: a function with NO Project instructions MUST yield
    // an empty `moved_out_fields` map after this Pass 1/Pass 2 population.
    // Pass 1 finds zero Project tuples → project_origins empty → Pass 2's
    // transfer-var lookups all miss → map stays empty. Clamps the population
    // logic from below: a reversion that erroneously populates on every
    // transferred var (regardless of project_origins membership) would fire
    // here. Per -TDD pseudo-tested-method ban —
    // assert the SPECIFIC expected state (empty map) rather than mere data-
    // structure-existence. Preserves skeleton intent post-population.
    let registry = TypeRegistry::new();
    let mut func = func_with_n_vars(2);
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        ctx.moved_out_fields().is_empty(),
        "moved_out_fields MUST remain empty when function has zero Project instructions (Pass 1 yields empty project_origins); got {:?}",
        ctx.moved_out_fields(),
    );
}

#[test]
fn project_then_construct_arg_sets_moved_out_fields_bit() {
    // positive pin (two-stage rule): `%1 = Project %0.0` followed
    // by `Construct(args=[%1])` MUST set bit `0` on `%0` in `moved_out_fields`.
    // Pass 1 collects (%1 → (%0, 0)); Pass 2 sees Construct's owned-position arg
    // %1, looks up project_origins[%1] = (%0, 0), inserts 0 into
    // moved_out_fields[%0]. Per `TF-3` Construct args at
    // owned positions (per `instr.rs:352-354 is_owned_position` returns true
    // for `pos < args.len`). Construct is the canonical transfer-point
    // consumer that fires the two-stage rule.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR, // %0: owned aggregate (use str for non-scalar burden)
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![entry_block(
            vec![
                project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0)),
                ArcInstr::Construct {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Tuple,
                    args: vec![ArcVarId::new(1)],
                },
            ],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let fields = ctx
        .moved_out_fields()
        .get(&ArcVarId::new(0))
        .unwrap_or_else(|| {
            panic!(
                "moved_out_fields MUST contain entry for %0 after Project → Construct consumption"
            )
        });
    assert!(
        fields.contains(&0u32),
        "moved_out_fields[%0] MUST contain field 0 (two-stage rule: Project → transfer-point consumer); got {fields:?}",
    );
}

#[test]
fn project_then_set_value_sets_moved_out_fields_bit_via_tf15_carve_out() {
    // positive pin (Set-value TF-15 carve-out): `%1 = Project %0.0`
    // followed by `Set { base: %2, field: 0, value: %1 }` MUST set bit `0` on `%0`
    // in `moved_out_fields`. Pass 1 collects (%1 → (%0, 0)); Pass 2's
    // `instr_transfer_vars` honors the Set-value carve-out per
    // `TF-15` + IA-5 step (1) — `value` is Owned via alias
    // transfer despite `is_owned_position`'s `_ => false`. Clamps the
    // Set-value carve-out symmetry with the existing transfer_points /
    // emit_instr_burdens carve-outs at `burden_lower.rs:231-236,463-466,490-491`.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(2),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
        ],
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![entry_block(
            vec![
                project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0)),
                set_first(ArcVarId::new(2), ArcVarId::new(1)),
            ],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let fields = ctx
        .moved_out_fields()
        .get(&ArcVarId::new(0))
        .unwrap_or_else(|| {
            panic!("moved_out_fields MUST contain entry for %0 after Project → Set.value carve-out per TF-15")
        });
    assert!(
        fields.contains(&0u32),
        "moved_out_fields[%0] MUST contain field 0 (TF-15 Set-value carve-out fires the two-stage rule despite is_owned_position _ => false); got {fields:?}",
    );
}

#[test]
fn project_with_no_transfer_consumer_leaves_moved_out_fields_unset() {
    // negative pin (two-stage rule clamp from below): `%1 = Project %0.0`
    // with NO downstream transfer-point consumer MUST leave `moved_out_fields[%0]`
    // unset. Per `TF-4`, Project produces Borrowed; per
    // `instr.rs:391 _ => false`, Project is NOT an owned position itself.
    // The two-stage rule fires only when a Project dst is THEN consumed at
    // a transfer point. This pin clamps the unsound-aggressive
    // failure mode (`populate on every Project`) — a reversion of Pass 1/Pass 2
    // to single-pass-on-every-Project would set the bit here and FAIL.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![entry_block(
            vec![project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0))],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        ctx.moved_out_fields().is_empty(),
        "moved_out_fields MUST remain empty when Project has no transfer-point consumer (TF-4 Borrowed; two-stage rule's stage-2 not fired); got {:?}",
        ctx.moved_out_fields(),
    );
}

#[test]
fn project_consumed_at_is_shared_leaves_moved_out_fields_unset() {
    // negative pin (borrowed-position clamp): `%1 = Project %0.0`
    // followed by `IsShared(%1)` MUST leave `moved_out_fields[%0]` unset. Per
    // `instr.rs:391 _ => false`, IsShared falls through `is_owned_position`'s
    // catch-all → NOT an owned position → `instr_transfer_vars` does NOT
    // include %1. The two-stage rule's stage-2 is NOT triggered by IsShared.
    // Clamps the Pass 2 logic from below: a reversion that erroneously
    // treats every `used_vars` member as a transfer (ignoring
    // `is_owned_position`) would set the bit here and FAIL. Per
    // `TF-10`, IsShared produces SCALAR (boolean) — no
    // ownership transfer happens.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR, Idx::BOOL],
        blocks: vec![entry_block(
            vec![
                project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0)),
                ArcInstr::IsShared {
                    dst: ArcVarId::new(2),
                    var: ArcVarId::new(1),
                },
            ],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        ctx.moved_out_fields().is_empty(),
        "moved_out_fields MUST remain empty when Project dst is consumed at borrowed position (IsShared; TF-10 SCALAR result; is_owned_position _ => false); got {:?}",
        ctx.moved_out_fields(),
    );
}

#[test]
fn jump_arg_to_borrowed_target_block_param_emits_burden_dec_at_terminator_per_rl2_negative() {
    //  negative pin: clamps this
    // `if matches!(ownership, DerivedOwnership::Owned)` guard at
    // `burden_lower.rs:273` from below. When target block param's
    // `DerivedOwnership` is `BorrowedFrom(...)` (NOT Owned), Jump.args[i]
    // MUST NOT enter terminator_transfer_vars — the prior-instruction /
    // terminator-position last-use of arg DOES receive BurdenDec because
    // Jump-to-Borrowed-param is a borrow (not an ownership transfer) per
    // `RL-2` ownership-transferring exception list.
    // Production borrow inference at `borrow/derived.rs:60` currently marks
    // all block params Owned, so this case is structurally unreachable in
    // shipped code — BUT the guard itself is load-bearing (a reversion that
    // always treats Jump.args as transfer would silently miscompile when
    // block-param borrow inference distinguishes Borrowed). Test constructs
    // explicit `&[DerivedOwnership::BorrowedFrom(...)]` to exercise the
    // negative path completeness rule.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![ArcVarId::new(0)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(ArcVarId::new(1), Idx::STR)],
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    // Block 1's param var(1) is BorrowedFrom var(0) — clamps this
    // DerivedOwnership::Owned guard; transfer set MUST exclude var(0).
    let derived = vec![
        DerivedOwnership::Owned,
        DerivedOwnership::BorrowedFrom(ArcVarId::new(0)),
    ];
    let _ctx = emit_burden_ops(
        &mut func,
        &registry,
        &derived,
        &[],
        &FxHashMap::default(),
        false,
    );
    let body_0 = &func.blocks[0].body;
    // Pin: BurdenDec(0) IS emitted at terminator-position — Jump-to-
    // Borrowed-block-param is NOT a transfer per RL-2, so var(0)'s
    // terminator-position last-use must receive a non-suppressed BurdenDec.
    let dec_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_present,
        "Jump.args[0] (var(0)) to Borrowed-target-block-param MUST receive BurdenDec at terminator-position (NOT a transfer per RL-2; cycle-37 guard load-bearing); block 0 body={body_0:?}",
    );
    //  emission-side negative pin:
    // BurdenInc MUST NOT fire when target-block-param is Borrowed (clamps the
    // emission-side ownership guard from below — a reversion that always
    // emitted BurdenInc at Jump.args would silently over-emit).
    let inc_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        !inc_present,
        "Jump.args[0] (var(0)) to Borrowed-target-block-param MUST NOT receive BurdenInc at terminator (rule 3 emission-side guard); block 0 body={body_0:?}",
    );
}

#[test]
fn invoke_scalar_int_arg_at_owned_position_emits_no_burden_ops_per_vf1_rconscalar() {
    //  negative pin (, VF-1 RcOnScalar mirror per
    //): scalar-typed Invoke arg at owned position MUST
    // NOT receive BurdenInc/BurdenDec even though terminator_transfer_per_block
    // marks it as transfer. The `owned_vars_needing_rc` filter at
    // `burden_lower.rs:225-234` rejects scalars (Idx::INT carries
    // `BuiltinBurdenSpec::EMPTY` per `BURDEN_TABLE` at
    // `ori_registry/src/burden/table.rs:184-193`); `burden_carries_rc`
    // returns false → var excluded from owned_vars_needing_rc → no
    // emission. Clamps this Invoke transfer logic from below.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::INT, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(1),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body_0 = &func.blocks[0].body;
    assert!(
        body_0.is_empty(),
        "scalar Int Invoke.args[0] MUST trigger zero burden ops (VF-1 RcOnScalar mirror clamps cycle-38 Invoke transfer); body={body_0:?}",
    );
}

#[test]
fn invoke_indirect_owned_args_at_pos_one_emits_symmetric_burden_dec_for_vf1_balance() {
    //  InvokeIndirect positive pin + terminator-
    // level VF-1 symmetry: canonical `ArcTerminator::is_owned_position(pos)`
    // at `terminator.rs:117-126` encodes closure-pos-0-always-Borrowed
    // semantics. used_vars = [closure,...args]; closure at pos 0 →
    // is_owned_position(0) == false; args at pos 1+ checked against
    // arg_ownership[pos-1]. Test: closure var(0) + args [var(1)] with
    // arg_ownership=[Owned] → var(1) gets BurdenInc at terminator AND
    // symmetric BurdenDec at terminator to balance VF-1 intraprocedural
    // net per `VF-1`. The terminator-level symmetric Dec
    // is safe (does NOT cause class_covered to fire for body-internal vars,
    // since the var's last-use IS the terminator); the instruction-level
    // case keeps transfer-suppression per `emit_instr_burdens`. BurdenDec
    // is a TF-N/A metadata annotation per `aims/realize/walk.rs:75-93`;
    // codegen does NOT emit a real RcDec.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            },
        ],
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::InvokeIndirect {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    closure: ArcVarId::new(0),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(1),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body_0 = &func.blocks[0].body;
    //  — Pin: BurdenDec on var(1) MUST appear, paired with the
    // BurdenInc to preserve VF-1 intraprocedural balance per `
    // §9 VF-1`. Codegen does NOT emit a real RcDec — BurdenDec is a TF-N/A
    // metadata annotation per `aims/realize/walk.rs:75-93`, so the
    // ownership-transfer semantic at the runtime layer is unaffected.
    let dec_arg_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(1)));
    assert!(
        dec_arg_present,
        "InvokeIndirect.args[0] at owned position 1 MUST receive symmetric BurdenDec at terminator for VF-1 balance; body={body_0:?}",
    );
    //  emission-side positive pin: BurdenInc(var(1)) fires at
    // owned position 1 per `RL-1`. Conservative Phase 5
    // emission mirroring instruction-level pattern.
    let inc_arg_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_arg_present,
        "InvokeIndirect.args[0] at owned position 1 MUST receive BurdenInc at terminator (rule 5 emission-side per RL-1); body={body_0:?}",
    );
    //  emission-side negative pin:
    // closure at position 0 is ALWAYS Borrowed per is_owned_position(0)
    // returning false. Clamps the SSOT helper's closure-Borrowed semantic;
    // a reversion that emitted BurdenInc on the closure would over-emit.
    let inc_closure_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        !inc_closure_present,
        "InvokeIndirect.closure (var(0)) at position 0 MUST NOT receive BurdenInc (closure always Borrowed per is_owned_position(0)); body={body_0:?}",
    );
}

#[test]
fn invoke_arg_at_owned_position_emits_symmetric_burden_dec_at_terminator_for_vf1() {
    //  (Tail-call) positive pin + terminator-
    // level VF-1 symmetry: `ArcTerminator::Invoke` args at owned positions
    // transfer ownership per `RL-2` AND receive symmetric
    // BurdenInc + BurdenDec pair at the terminator per `
    // VF-1` intraprocedural balance. extended
    // `terminator_transfer_per_block` with `Invoke + InvokeIndirect`
    // match-arms using canonical SSOT helper `is_owned_position(pos)`.
    // With empty `arg_ownership`, is_owned_position defaults to all-Owned
    // (per `terminator.rs:100-129`). BurdenDec is a TF-N/A metadata
    // annotation per `aims/realize/walk.rs:75-93`; codegen does NOT emit
    // a real RcDec, preserving the runtime transfer semantic. The
    // terminator-level symmetric Dec is safe (does NOT trigger
    // class_covered suppression of predicate-stack RC for body-internal
    // vars coexistence handshake).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(1),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body_0 = &func.blocks[0].body;
    //  — Pin: BurdenDec on var(0) MUST appear, paired with the
    // terminator-position BurdenInc to preserve VF-1 intraprocedural net-zero.
    let dec_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_present,
        "Invoke.args[0] at owned position MUST receive symmetric BurdenDec at terminator for VF-1 balance; body={body_0:?}",
    );
    //  emission-side: Invoke.args at owned positions receive
    // BurdenInc per `RL-1` — conservative Phase 5 emission
    // mirroring `emit_instr_burdens` instruction-level pattern; lattice
    // rewrite eliminates redundant Incs.
    let incs: Vec<&ArcInstr> = body_0
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        incs.len(),
        1,
        "Invoke.args[0] at owned position MUST receive exactly one BurdenInc at terminator (rule 5 emission-side per RL-1); got body={body_0:?}",
    );
    assert_eq!(
        body_0.len(),
        2,
        "block 0 body MUST contain the terminator-position BurdenInc + symmetric BurdenDec pair (no body instructions); got {body_0:?}",
    );
}

#[test]
fn jump_arg_to_owned_target_block_param_emits_symmetric_burden_dec_at_terminator_for_vf1() {
    //  positive pin + terminator-level VF-1
    // symmetry: Jump.args at positions whose target-block params have
    // `DerivedOwnership::Owned` transfer ownership to the target block
    // param per `RL-2` AND receive symmetric BurdenInc +
    // BurdenDec pair per `VF-1` intraprocedural balance.
    // terminator-transfer pre-computation marks Jump.args[i] as
    // transfer when target_block.params[i].0 looked up in derived_ownership
    // returns Owned. BurdenDec is a TF-N/A metadata annotation per
    // `aims/realize/walk.rs:75-93`; codegen does NOT emit a real RcDec,
    // preserving Jump's runtime transfer semantic to the target block
    // param. The terminator-level symmetric Dec is safe (does NOT cause
    // class_covered suppression of predicate-stack RC for body-internal
    // vars coexistence handshake).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![ArcVarId::new(0)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(ArcVarId::new(1), Idx::STR)],
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let derived = vec![DerivedOwnership::Owned, DerivedOwnership::Owned];
    let _ctx = emit_burden_ops(
        &mut func,
        &registry,
        &derived,
        &[],
        &FxHashMap::default(),
        false,
    );
    let body_0 = &func.blocks[0].body;
    //  — Pin 1: BurdenDec on var(0) MUST appear at terminator,
    // paired with the BurdenInc to preserve VF-1 intraprocedural balance.
    // BurdenDec is a TF-N/A metadata annotation per
    // `aims/realize/walk.rs:75-93`; codegen does NOT emit a real RcDec,
    // preserving Jump's runtime ownership-transfer semantic.
    let dec_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_present,
        "Jump.args[0] (var(0)) to Owned-target-block-param MUST receive symmetric BurdenDec at terminator for VF-1 balance; block 0 body={body_0:?}",
    );
    // Pin 2: Block 0 body contains EXACTLY ONE BurdenInc(var(0)) —
    //  emission-side per `RL-1` (RC inc emitted
    // at every ownership-transfer point on owned non-scalar SSA values).
    // Conservative Phase 5 emission per ` goal:` ban on lattice
    // consultation (RC traffic overcounted but balanced); lattice
    // rewrite eliminates redundant Incs. Mirrors instruction-level pattern
    // at `emit_instr_burdens` line ~966.
    let incs: Vec<&ArcInstr> = body_0
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        incs.len(),
        1,
        "Jump.args[0] (var(0)) to Owned-target-block-param MUST receive exactly one BurdenInc at terminator (rule 3 emission-side per RL-1); got body={body_0:?}",
    );
    assert_eq!(
        body_0.len(),
        2,
        "block 0 body MUST contain the terminator-position BurdenInc + symmetric BurdenDec pair; got {body_0:?}",
    );
}

#[test]
fn return_scalar_int_value_emits_zero_burden_ops_per_vf1_rconscalar() {
    // first rule negative pin (VF-1 RcOnScalar mirror per
    //): scalar-typed Return value MUST NOT receive
    // BurdenInc or BurdenDec, regardless of terminator-position registration.
    // `lookup_burden(Idx::INT)` returns `Some(BurdenRef)` carrying
    // `BuiltinBurdenSpec::EMPTY` (per `BURDEN_TABLE` at
    // `ori_registry/src/burden/table.rs:184-193`); `burden_carries_rc` filter
    // rejects EMPTY specs so var(0) is excluded from owned_vars_needing_rc.
    // Without this filter the terminator-position emission would attempt to
    // process the scalar var (which it now filters out via the same
    // owned_vars_needing_rc gate inherited via last_uses_at population at
    // burden_lower.rs:211-217).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "scalar Int Return.value MUST trigger zero burden ops (VF-1 RcOnScalar mirror); body={body:?}",
    );
}

#[test]
fn partial_move_at_last_use_emits_burden_dec_partial() {
    // positive pin — partial-move emission. Construct a 2-field
    // user-defined struct `{ data: str, name: str }` with UserBurdenSpec naming
    // BOTH fields as owned. Function body projects ONLY field 0 (data) and
    // transfers it via Construct (records moved_out_fields[parent] = {0}); a
    // later Project of field 1 (name) is the parent's terminal last-use site
    // (Project is `_ => false` for is_owned_position so the terminal Project
    // is NOT a transfer point). At parent's last-use:
    //   - transfer_vars empty (Project is not transfer)
    //   - full_move_vars excludes parent ({0} does not cover {0, 1})
    //   - partial_move_vars contains parent with skip_fields = [0]
    //   - → BurdenDecPartial { var: parent, skip_fields: vec![0] } emitted
    //
    // Negative pin clamping the full-move case lives in the existing
    // test (full-move asserts zero BurdenDec / zero BurdenDecPartial);
    // inherits that suppression branch unchanged. Per `
    // §8 RL-2` partial-transfer semantics (partial-move = partial-transfer;
    // non-moved fields still need drop; skip_fields names transferred subset).
    //
    // AIMS Invariant 5 case (b) preserved: BurdenDecPartial extends ArcInstr
    // on the SAME var dimension; no parallel emission, no shadow tracker.
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let mut registry = TypeRegistry::new();
    let struct_idx = Idx::from_raw(64); // first dynamic slot per TY-5
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: struct_idx,
            ownership: Ownership::Owned,
        }],
        var_types: vec![struct_idx, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![entry_block(
            vec![
                project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0)),
                ArcInstr::Construct {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Tuple,
                    args: vec![ArcVarId::new(1)],
                },
                ArcInstr::Project {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcVarId::new(0),
                    field: 1,
                },
            ],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;

    let partial_decs: Vec<&ArcInstr> = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        partial_decs.len(),
        1,
        "MUST emit exactly one BurdenDecPartial for parent var(0) at its last-use (Project field 1); got {partial_decs:?}; body={body:?}",
    );
    let ArcInstr::BurdenDecPartial { skip_fields, .. } = partial_decs[0] else {
        panic!("filter guaranteed BurdenDecPartial");
    };
    assert_eq!(
        skip_fields,
        &vec![0u32],
        "BurdenDecPartial.skip_fields MUST contain the moved-out top-level field index 0 (field 'data' projected at instr 0 and transferred to Construct at instr 1); got {skip_fields:?}",
    );

    let parent_full_decs: Vec<&ArcInstr> = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
        .collect();
    assert!(
        parent_full_decs.is_empty(),
        "MUST NOT emit BurdenDec for parent var(0) when partial-move applies (partial-drop replaces, not augments); got {parent_full_decs:?}; body={body:?}",
    );
}

/// Match-destructuring positive pin for the partial-move emission path.
///
/// Exercises `populate_moved_out_fields` walker on `Project` instructions
/// living in a NON-block-0 arm body, plus `compute_partial_move_vars` deriving
/// the correct `skip_fields` set for a scrutinee var whose field-projections
/// straddle a `Switch` terminator. Companion to the direct-field-projection
/// pin above: that pin exercises Project + Construct co-located in block 0;
/// this pin exercises the same chain split across a Switch dispatch.
///
/// IR shape (mimics what `ori_canon::patterns` lowers for a struct destructure
/// arm body that lives in a separate block from the dispatch entry):
///
/// - block 0 (entry, fn param var(0) = `Pair { data: str, name: str }`):
///   * `let var(1) = Literal(Int 0)` (synthetic scalar discriminant)
///   * `Switch { scrutinee: var(1), cases: [(0, block1)], default: block1 }`
/// - block 1 (arm body):
///   * `let var(2) = Project { value: var(0), field: 0 }` (a: str)
///   * `Construct { dst: var(3), Tuple, args: [var(2)] }` (transfers field 0)
///   * `let var(4) = Project { value: var(0), field: 1 }` (b: str — LAST USE
///     of var(0); Project is `_ => false` for `is_owned_position` so it is NOT
///     a transfer point)
///   * `Unreachable`
///
/// Expected outcome — identical SHAPE to the direct-projection pin, but
/// derived through multi-block walking:
///   - `populate_moved_out_fields` Pass 1 walks blocks 0 + 1; records
///     `var(2) → (var(0), 0)` and `var(4) → (var(0), 1)` from block 1's
///     two Project instructions
///   - Pass 2 walks blocks 0 + 1; finds `var(2)` consumed by Construct in
///     block 1 (TF-3 Construct positions are owned per
///     `ArcInstr::is_owned_position`) → `moved_out_fields[var(0)].insert(0)`
///   - `compute_full_move_vars`: `{0}` does not cover `{0, 1}` →
///     var(0) NOT in full-move set
///   - `compute_partial_move_vars`: var(0) → `skip_fields = vec![0]`
///   - At var(0)'s last-use (Project var(0).1 inside block 1, the terminal
///     site for the parent), emit `BurdenDecPartial { var: var(0),
///     skip_fields: vec![0] }`; suppress conservative `BurdenDec`
///
/// Spec: Annex E §AIMS RL-2 partial-transfer semantics. AIMS Invariant 5
/// case (b): extends `ArcInstr` on the SAME var dimension as the direct-
/// projection pin; no parallel emission path for the Switch case.
#[test]
fn match_destructuring_partial_move_at_last_use_emits_burden_dec_partial() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let mut registry = TypeRegistry::new();
    let struct_idx = Idx::from_raw(64); // first dynamic slot per TY-5
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: struct_idx,
            ownership: Ownership::Owned,
        }],
        // var(0)=Pair, var(1)=int discriminant, var(2)=str (a),
        // var(3)=str (tuple result), var(4)=str (b)
        var_types: vec![struct_idx, Idx::INT, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![
            // Block 0: entry — synthetic discriminant + Switch into arm body.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Switch {
                    scrutinee: ArcVarId::new(1),
                    cases: vec![(0, ArcBlockId::new(1))],
                    default: ArcBlockId::new(1),
                },
            },
            // Block 1: arm body — Project field 0, transfer via Construct,
            // then Project field 1 (last use of var(0)).
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(2), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(3),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(2)],
                    },
                    ArcInstr::Project {
                        dst: ArcVarId::new(4),
                        ty: Idx::STR,
                        value: ArcVarId::new(0),
                        field: 1,
                    },
                ],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    // BurdenDecPartial emission MUST land inside block 1 (the arm body that
    // contains var(0)'s last use); inspect that block's body specifically so
    // the multi-block dispatch shape is part of the asserted contract.
    let arm_body = &func.blocks[1].body;
    let partial_decs: Vec<&ArcInstr> = arm_body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        partial_decs.len(),
        1,
        "MUST emit exactly one BurdenDecPartial for parent var(0) at its last-use Project inside the arm block (block 1); got {partial_decs:?}; arm_body={arm_body:?}",
    );
    let ArcInstr::BurdenDecPartial { skip_fields, .. } = partial_decs[0] else {
        panic!("filter guaranteed BurdenDecPartial");
    };
    assert_eq!(
        skip_fields,
        &vec![0u32],
        "BurdenDecPartial.skip_fields MUST contain the moved-out top-level field index 0 (field 'data' projected + transferred via Construct in the arm block, even though the Project lives outside block 0); got {skip_fields:?}",
    );

    // Cross-block soundness: NO conservative full BurdenDec for var(0) in
    // either block — partial-drop replaces, not augments. invariant
    // preserved across the Switch-dispatched shape.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let parent_full_decs: Vec<&ArcInstr> = block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
            .collect();
        assert!(
            parent_full_decs.is_empty(),
            "MUST NOT emit BurdenDec for parent var(0) in block {block_idx} when partial-move applies; got {parent_full_decs:?}; body={:?}",
            block.body,
        );
    }

    // Entry block (block 0) MUST contain no BurdenDecPartial for var(0) — the
    // last use lives in the arm body, not at the Switch terminator. Pins the
    // walker against an over-eager emission at the dispatch site.
    let entry_partial: Vec<&ArcInstr> = func.blocks[0]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0)))
        .collect();
    assert!(
        entry_partial.is_empty(),
        "MUST NOT emit BurdenDecPartial for var(0) in entry block 0 — var(0)'s last use lives in the arm block (block 1), not at the Switch terminator; got {entry_partial:?}",
    );
}

/// CFG-diamond positive pin for the INTERSECT-merge path.
///
/// Exercises forward dataflow `entry(B) = INTERSECT over P: exit(P)` at a
/// merge block whose two predecessors symmetrically move the SAME top-level
/// field. Companion to the match-destructuring pin: that pin's two-block
/// dispatch is structurally a single-predecessor merge (the arm body has
/// only the entry block as predecessor); this pin pins true multi-
/// predecessor INTERSECT at a diamond join.
///
/// IR shape (4 blocks):
///
/// - block 0 (entry, fn param var(0) = `Pair { data: str, name: str }`):
///   * `let var(1) = Literal(Int 0)` (synthetic scrutinee)
///   * `Switch { scrutinee: var(1), cases: [(0, block1)], default: block2 }`
/// - block 1 (case 0):
///   * `let var(2) = Project { value: var(0), field: 0 }`
///   * `Construct { dst: var(3), Tuple, args: [var(2)] }` (transfers field 0)
///   * `Jump block 3`
/// - block 2 (case 1):
///   * `let var(4) = Project { value: var(0), field: 0 }` (SYMMETRIC)
///   * `Construct { dst: var(5), Tuple, args: [var(4)] }` (transfers field 0)
///   * `Jump block 3`
/// - block 3 (merge):
///   * `let var(6) = Project { value: var(0), field: 1 }` (last use of var(0))
///   * `Unreachable`
///
/// Expected outcome:
///   - `block_local[1] = block_local[2] = {var(0): {0}}` (symmetric moves)
///   - Pass 3 INTERSECT at block 3 entry:
///     `entry(3) = INTERSECT(exit(1), exit(2)) = INTERSECT({var(0):{0}}, {var(0):{0}}) = {var(0):{0}}`
///   - Union over exits = `{var(0):{0}}`
///   - `compute_partial_move_vars`: var(0) → `skip_fields = vec![0]`
///   - At var(0)'s last-use (`Project var(0).1` in block 3), emit
///     `BurdenDecPartial { var: var(0), skip_fields: vec![0] }`; no
///     conservative `BurdenDec` for var(0) anywhere.
///
/// Spec: Annex E §AIMS RL-2 partial-transfer semantics; INTERSECT is the
/// architecturally-correct merge — sound for both pre-rejection and post-
/// E2043-typeck-rejection states (post-rejection: predecessor sets are
/// guaranteed equal so INTERSECT degenerates to pick-any, but implementing
/// real INTERSECT defends against future typeck-rejection regressions).
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "diamond IR fixture requires four basic-block literals; splitting hides matrix-clamped pin shape"
)]
fn match_branches_with_symmetric_partial_move_intersect_emits_burden_dec_partial() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let mut registry = TypeRegistry::new();
    let struct_idx = Idx::from_raw(64); // first dynamic slot per TY-5
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: struct_idx,
            ownership: Ownership::Owned,
        }],
        // var(0)=Pair, var(1)=int scrutinee, var(2)/var(4)=projected field 0,
        // var(3)/var(5)=tuple ctor result, var(6)=projected field 1
        var_types: vec![
            struct_idx,
            Idx::INT,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::STR,
        ],
        blocks: vec![
            // Block 0: synthetic scrutinee + Switch dispatch.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Switch {
                    scrutinee: ArcVarId::new(1),
                    cases: vec![(0, ArcBlockId::new(1))],
                    default: ArcBlockId::new(2),
                },
            },
            // Block 1: case 0 — project field 0, transfer via Construct, jump merge.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(2), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(3),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(2)],
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: Vec::new(),
                },
            },
            // Block 2: case 1 — symmetric move of field 0, jump merge.
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(4), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(5),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(4)],
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: Vec::new(),
                },
            },
            // Block 3: merge — project field 1 (terminal last use of var(0)).
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: vec![ArcInstr::Project {
                    dst: ArcVarId::new(6),
                    ty: Idx::STR,
                    value: ArcVarId::new(0),
                    field: 1,
                }],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    // BurdenDecPartial fires at var(0)'s last use, which lives in block 3
    // (the merge block). Pin the count + skip_fields against the INTERSECT
    // result.
    let merge_body = &func.blocks[3].body;
    let partial_decs: Vec<&ArcInstr> = merge_body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        partial_decs.len(),
        1,
        "MUST emit exactly one BurdenDecPartial for var(0) at its last-use Project in the merge block (block 3) — INTERSECT of symmetric exits MUST yield {{var(0): {{0}}}}; got {partial_decs:?}; merge_body={merge_body:?}",
    );
    let ArcInstr::BurdenDecPartial { skip_fields, .. } = partial_decs[0] else {
        panic!("filter guaranteed BurdenDecPartial");
    };
    assert_eq!(
        skip_fields,
        &vec![0u32],
        "BurdenDecPartial.skip_fields MUST contain the moved-out top-level field index 0 (field 'data' projected and transferred symmetrically on BOTH diamond arms); got {skip_fields:?}",
    );

    // No conservative full BurdenDec for var(0) in any block — partial-drop
    // replaces, not augments. Diamond-shape invariant preserved.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let parent_full_decs: Vec<&ArcInstr> = block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
            .collect();
        assert!(
            parent_full_decs.is_empty(),
            "MUST NOT emit BurdenDec for var(0) in block {block_idx} when partial-move applies through the diamond; got {parent_full_decs:?}; body={:?}",
            block.body,
        );
    }
}

/// Loop-entry positive pin for the INTERSECT-merge fixpoint path.
///
/// Exercises bounded fixpoint iteration over a CFG with a back edge. Pre-
/// loop block unconditionally moves a field; loop header has two
/// predecessors (entry block + self-loop). Post-loop block sees the move
/// via the loop header. Pins optimistic-⊤ initialization + fixpoint
/// convergence — without ⊤ seeding, the back edge's empty initial exit
/// would falsely intersect away the entry-block contribution at the loop
/// header, yielding a strictly-weaker (incorrect) state at loop exit.
///
/// IR shape (3 blocks with self-loop on header):
///
/// - block 0 (entry, pre-loop):
///   * `let var(2) = Project { value: var(0), field: 0 }`
///   * `Construct { dst: var(3), Tuple, args: [var(2)] }` (transfers field 0)
///   * `let var(4) = Literal(Bool false)` (loop continuation flag)
///   * `Jump block 1`
/// - block 1 (loop header):
///   * `Branch { cond: var(4), then: block 1 (back), else: block 2 (exit) }`
/// - block 2 (post-loop):
///   * `let var(5) = Project { value: var(0), field: 1 }` (last use of var(0))
///   * `Unreachable`
///
/// Expected outcome (optimistic-⊤ seeding + worklist fixpoint):
///   - `block_local[0] = {var(0): {0}}`; `block_local[1..2]` empty.
///   - Initial `exit(0)=empty`, `exit(1)=exit(2)=⊤={var(0):{0}}`.
///   - First worklist pass: `exit(0)={var(0):{0}}`;
///     `entry(1) = INTERSECT(exit(0), exit(1)) =
///     INTERSECT({var(0):{0}}, ⊤) = {var(0):{0}}`;
///     `exit(1)` unchanged from ⊤.
///     `entry(2) = exit(1) = {var(0):{0}}`; `exit(2) = {var(0):{0}}`.
///   - Second worklist pass: fixpoint.
///   - Union over exits = `{var(0): {0}}`.
///   - `BurdenDecPartial { var: var(0), skip_fields: vec![0] }` emitted at
///     `Project var(0).1` in block 2.
///
/// Spec: Annex E §AIMS RL-2 partial-transfer + IC-7 convergence bound.
/// Optimistic-⊤ initialization per Kildall (1973); Aho/Lam/Sethi/Ullman
/// chapter 9.3 — standard MUST-analysis fixpoint shape.
#[test]
fn loop_back_edge_partial_move_intersect_with_entry_emits_burden_dec_partial() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let mut registry = TypeRegistry::new();
    let struct_idx = Idx::from_raw(64);
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: struct_idx,
            ownership: Ownership::Owned,
        }],
        // var(0)=Pair, var(2)=projected field 0, var(3)=tuple ctor result,
        // var(4)=bool loop cond, var(5)=projected field 1
        var_types: vec![
            struct_idx,
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::BOOL,
            Idx::STR,
        ],
        blocks: vec![
            // Block 0: pre-loop — move field 0, prepare loop cond, jump header.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(2), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(3),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(2)],
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(4),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(false)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            // Block 1: loop header — back-edge to self OR exit to block 2.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(4),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            // Block 2: post-loop — project field 1 (terminal last use of var(0)).
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![ArcInstr::Project {
                    dst: ArcVarId::new(5),
                    ty: Idx::STR,
                    value: ArcVarId::new(0),
                    field: 1,
                }],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    // BurdenDecPartial fires at var(0)'s last use in block 2. Fixpoint
    // soundness: the move from block 0 propagates through the loop header
    // (block 1) to the exit block (block 2).
    let exit_body = &func.blocks[2].body;
    let partial_decs: Vec<&ArcInstr> = exit_body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        partial_decs.len(),
        1,
        "MUST emit exactly one BurdenDecPartial for var(0) at its last-use Project in the post-loop block (block 2) — optimistic-⊤ fixpoint MUST propagate {{var(0): {{0}}}} through the loop header; got {partial_decs:?}; exit_body={exit_body:?}",
    );
    let ArcInstr::BurdenDecPartial { skip_fields, .. } = partial_decs[0] else {
        panic!("filter guaranteed BurdenDecPartial");
    };
    assert_eq!(
        skip_fields,
        &vec![0u32],
        "BurdenDecPartial.skip_fields MUST contain field 0 (moved in pre-loop block 0; propagated through the loop header's back-edge fixpoint); got {skip_fields:?}",
    );

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let parent_full_decs: Vec<&ArcInstr> = block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
            .collect();
        assert!(
            parent_full_decs.is_empty(),
            "MUST NOT emit BurdenDec for var(0) in block {block_idx} when partial-move propagates through the loop; got {parent_full_decs:?}; body={:?}",
            block.body,
        );
    }
}

/// Nested-join positive pin for the INTERSECT-merge composition path.
///
/// Exercises INTERSECT at a 3-predecessor join whose predecessors include
/// a 2-predecessor inner merge. Pins compositionality: INTERSECT must
/// distribute correctly through nested CFG joins — the inner merge's
/// INTERSECT-of-symmetric-moves must yield the right value at the outer
/// merge's INTERSECT input.
///
/// IR shape (6 blocks, nested diamond):
///
/// - block 0 (outer scrutinee): Switch → block 1 (case 0), block 4 (default)
/// - block 1 (outer case 0, inner scrutinee): Switch → block 2 (case 0), block 3 (default)
/// - block 2 (inner case 0): Project var(0).0 → Construct → Jump block 5
/// - block 3 (inner case 1): Project var(0).0 → Construct → Jump block 5 (symmetric)
/// - block 4 (outer case 1): Project var(0).0 → Construct → Jump block 5 (symmetric)
/// - block 5 (outer merge): Project var(0).1 → Unreachable (last use of var(0))
///
/// Expected outcome:
///   - `block_local[2] = block_local[3] = block_local[4] = {var(0): {0}}`.
///   - Pass 3: exit(2) = exit(3) = exit(4) = {var(0):{0}}.
///   - entry(5) = INTERSECT(exit(2), exit(3), exit(4)) = {var(0):{0}}.
///   - Union = `{var(0): {0}}`; `BurdenDecPartial` fires at block 5.
///
/// Spec: Annex E §AIMS RL-2 — INTERSECT semantics compose correctly across
/// nested CFG joins because INTERSECT is associative (lattice meet law).
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "nested-diamond IR fixture requires six basic-block literals; splitting hides matrix-clamped pin shape"
)]
fn nested_match_with_inner_diamond_partial_move_emits_burden_dec_partial() {
    use crate::lower::test_utils::registered_struct_with_two_owned_str_fields;

    let mut registry = TypeRegistry::new();
    let struct_idx = Idx::from_raw(64);
    registered_struct_with_two_owned_str_fields(&mut registry, "Pair", struct_idx);

    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: struct_idx,
            ownership: Ownership::Owned,
        }],
        // var(0)=Pair scrutinee, var(1)=outer-Switch scrutinee, var(2)=inner-
        // Switch scrutinee, var(3..=5)=inner-arm field 0 projections + ctor
        // results, var(6)=inner-case-1 ctor result, var(7)=outer-case-1
        // projected field 0, var(8)=outer-case-1 ctor result, var(9)=projected
        // field 1 at merge.
        var_types: vec![
            struct_idx,
            Idx::INT, // var(1)
            Idx::INT, // var(2)
            Idx::STR, // var(3) inner-case-0 field 0
            Idx::STR, // var(4) inner-case-0 ctor result
            Idx::STR, // var(5) inner-case-1 field 0
            Idx::STR, // var(6) inner-case-1 ctor result
            Idx::STR, // var(7) outer-case-1 field 0
            Idx::STR, // var(8) outer-case-1 ctor result
            Idx::STR, // var(9) merge field 1
        ],
        blocks: vec![
            // Block 0: outer scrutinee + Switch to block 1 (case 0) or block 4.
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Switch {
                    scrutinee: ArcVarId::new(1),
                    cases: vec![(0, ArcBlockId::new(1))],
                    default: ArcBlockId::new(4),
                },
            },
            // Block 1: outer case 0 — inner scrutinee + Switch to block 2 or block 3.
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Switch {
                    scrutinee: ArcVarId::new(2),
                    cases: vec![(0, ArcBlockId::new(2))],
                    default: ArcBlockId::new(3),
                },
            },
            // Block 2: inner case 0 — project field 0, transfer, jump merge.
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(3), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(4),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(3)],
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(5),
                    args: Vec::new(),
                },
            },
            // Block 3: inner case 1 — symmetric move of field 0.
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(5), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(6),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(5)],
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(5),
                    args: Vec::new(),
                },
            },
            // Block 4: outer case 1 — symmetric move of field 0 (parallel to
            // the inner diamond's joined output).
            ArcBlock {
                id: ArcBlockId::new(4),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(7), Idx::STR, ArcVarId::new(0)),
                    ArcInstr::Construct {
                        dst: ArcVarId::new(8),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: vec![ArcVarId::new(7)],
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(5),
                    args: Vec::new(),
                },
            },
            // Block 5: outer merge — project field 1 (terminal last use).
            ArcBlock {
                id: ArcBlockId::new(5),
                params: Vec::new(),
                body: vec![ArcInstr::Project {
                    dst: ArcVarId::new(9),
                    ty: Idx::STR,
                    value: ArcVarId::new(0),
                    field: 1,
                }],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let merge_body = &func.blocks[5].body;
    let partial_decs: Vec<&ArcInstr> = merge_body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDecPartial { var, .. } if *var == ArcVarId::new(0)))
        .collect();
    assert_eq!(
        partial_decs.len(),
        1,
        "MUST emit exactly one BurdenDecPartial for var(0) at its last-use Project in the outer-merge block (block 5) — INTERSECT composes correctly across the inner diamond + outer fork; got {partial_decs:?}; merge_body={merge_body:?}",
    );
    let ArcInstr::BurdenDecPartial { skip_fields, .. } = partial_decs[0] else {
        panic!("filter guaranteed BurdenDecPartial");
    };
    assert_eq!(
        skip_fields,
        &vec![0u32],
        "BurdenDecPartial.skip_fields MUST contain field 0 (moved symmetrically on all 3 paths reaching the outer merge); got {skip_fields:?}",
    );

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let parent_full_decs: Vec<&ArcInstr> = block
            .body
            .iter()
            .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
            .collect();
        assert!(
            parent_full_decs.is_empty(),
            "MUST NOT emit BurdenDec for var(0) in block {block_idx} when nested INTERSECT yields partial-move; got {parent_full_decs:?}; body={:?}",
            block.body,
        );
    }
}

// Closure capture composition — burden_lower emission pins
//
// Tests below pin the closure-capture-composition story at the
// burden_lower layer: registered closure `UserBurdenSpec` (composed via
// `ori_types::burden_compose::closure::compose_closure_burden_spec`)
// flows correctly through the existing trivial-emission walker. PartialApply
// IS the capture-time transfer point Rule 5 (already shipped);
// adds the spec-population side so the walker sees a non-empty burden for
// closure types and emits BurdenInc at PartialApply args.
//
// Burden-spec-registration discipline at the test layer mirrors fixtures
// (`registered_struct_with_two_owned_str_fields`) — the closure's Idx is
// registered as a struct shell with a closure-shaped burden via
// `compose_closure_burden_spec`. Production wiring lives at the lambda
// type-check site (`infer_lambda` at `compiler_repo/compiler/ori_types/src/
// infer/expr/blocks.rs:223`); the deliverable shipped here pins the
// spec composer + wires the burden walker to consume registered closure
// burdens without changes to the walker itself.

#[test]
fn closure_capture_by_value_of_owned_str_emits_burden_inc_at_partial_apply() {
    // success_criterion (positive — capture by value of Owned binding):
    // `let s = "hello"; let c = (-> s.length)` — capture site IS
    // PartialApply IS transfer point Rule 5. With the closure's
    // UserBurdenSpec composed via compose_closure_burden_spec (self_heap_alloc=
    // true, owned_fields=[STR]), the existing trivial-emission walker emits
    // BurdenInc on the captured arg before the PartialApply instruction.
    //
    // The existing `partial_apply_emits_burden_inc_for_captured_var`
    // pin uses Idx::STR for the closure result type — exercising the SAME code
    // path as (PartialApply args owned-position emission + the
    // VF-1 RcOnScalar filter). This pin is the closure-burden-aware variant:
    // it composes + registers the closure's spec at the closure's Idx so the
    // walker sees the closure type as carrying heap burden.
    use ori_types::burden::{UserBurdenSpec, UserOwnedField};
    use ori_types::burden_compose::closure::{compose_closure_burden_spec, ClosureCapture};

    let mut registry = TypeRegistry::new();
    let closure_idx = Idx::from_raw(64);
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Closure_capture_str",
        closure_idx,
        Some(compose_closure_burden_spec(
            closure_idx,
            &[ClosureCapture {
                field_index: 0,
                field_type: Idx::STR,
            }],
            &[],
        )),
    );

    // Sanity: registered spec carries the expected closure shape.
    let spec = registry
        .burden(closure_idx)
        .unwrap_or_else(|| panic!("closure burden MUST be registered"));
    assert!(spec.self_heap_alloc);
    assert_eq!(spec.owned_fields.len(), 1);
    assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
    assert!(spec.compiled_drop.is_some());

    let mut func = ArcFunction {
        var_types: vec![Idx::STR, closure_idx, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: closure_idx,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let body = &func.blocks[0].body;
    let inc_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_pos.is_some(),
        "expected BurdenInc(captured=var(0)=STR) before PartialApply on closure with composed Owned-str burden; body={body:?}",
    );
    let pa_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::PartialApply { .. }))
        .unwrap_or_else(|| panic!("PartialApply MUST appear in body"));
    assert!(
        inc_pos.unwrap_or_else(|| unreachable!("checked is_some above")) < pa_pos,
        "BurdenInc(captured) MUST appear BEFORE PartialApply; body={body:?}",
    );

    let _ = (
        UserBurdenSpec::default,
        UserOwnedField {
            field_path: Vec::new(),
            field_type: Idx::INT,
        },
    );
}

#[test]
fn closure_capture_by_reference_emits_no_burden_inc() {
    // success_criterion (negative — capture by reference): borrow stored
    // in borrowed_fields[i]; no drop on env field (borrows do not own). The
    // burden walker MUST NOT emit BurdenInc for a borrowed capture at the
    // PartialApply site.
    //
    // The Tag::Borrowed target type is target-only in the shipped pool; per
    // + the design the borrowed-capture's CAPTURED VARIABLE is
    // typed as a borrow target (modeled here as Idx::STR with the closure
    // taking a borrowed view). The existing PartialApply branch in
    // `is_owned_position` (`instr.rs:350-393`) treats args as owned-position
    // by structural shape — the negative-emission discipline here relies on
    // the captured variable itself NOT being in `owned_vars_needing_rc`
    // because borrow-captures don't surface in the closure's `owned_fields`
    // (they live in `borrowed_fields`). Until borrow-mode propagation at the
    // lambda type-check site flips the arg's classification (target-only with
    // Tag::Borrowed), the conservative pin here is the closure's
    // borrowed_fields-only burden shape — verified at the registry-side
    // (compose) level rather than the emission-side.
    //
    // Negative-pin discipline: the assertion
    // tier matches the success_criterion exactly — the burden spec
    // populates borrowed_fields, NOT owned_fields, and the registered spec's
    // owned_fields.is_empty guarantees no BurdenInc fires from the
    // closure's burden walk at codegen.
    use ori_types::burden_compose::closure::{compose_closure_burden_spec, ClosureCapture};

    let mut registry = TypeRegistry::new();
    let closure_idx = Idx::from_raw(64);
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Closure_borrow_str",
        closure_idx,
        Some(compose_closure_burden_spec(
            closure_idx,
            &[],
            &[ClosureCapture {
                field_index: 0,
                field_type: Idx::STR,
            }],
        )),
    );

    // Negative-pin (compose-level): registered spec carries borrowed_fields
    // populated + owned_fields empty. The trivial-emission walker reads
    // owned_fields from the closure's burden at codegen time; an empty
    // owned_fields list means no BurdenInc is emitted from the closure-burden
    // walk (the design intent for borrow captures).
    let spec = registry
        .burden(closure_idx)
        .unwrap_or_else(|| panic!("closure burden MUST be registered"));
    assert!(
        spec.owned_fields.is_empty(),
        "borrow-capture closure MUST have empty owned_fields (no drop on env field) — spec={spec:?}",
    );
    assert_eq!(
        spec.borrowed_fields.len(),
        1,
        "borrow-capture closure MUST have one borrowed_fields entry — spec={spec:?}",
    );
    assert_eq!(spec.borrowed_fields[0].field_type, Idx::STR);
}

#[test]
fn nested_closure_emits_recursive_burden_inc_through_outer_env() {
    // success_criterion (positive — captures-of-captures): outer env
    // field IS Closure<...> with its OWN UserBurdenSpec.compiled_drop.
    // Recursion is handled identically to recursive types — outer
    // closure's drop body recursively invokes inner closure's compiled_drop
    // via the inner field's UserBurdenSpec lookup at codegen.
    //
    // Composition records the inner closure's Idx in
    // outer.owned_fields[0].field_type; outer + inner each carry their own
    // distinct compiled_drop FnSyms per the per-Idx mangling shared with.
    use ori_types::burden_compose::closure::{compose_closure_burden_spec, ClosureCapture};

    let mut registry = TypeRegistry::new();
    let outer_idx = Idx::from_raw(64);
    let inner_idx = Idx::from_raw(65);

    // Inner closure: captures one STR by value.
    let inner_spec = compose_closure_burden_spec(
        inner_idx,
        &[ClosureCapture {
            field_index: 0,
            field_type: Idx::STR,
        }],
        &[],
    );
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Inner_closure",
        inner_idx,
        Some(inner_spec.clone()),
    );

    // Outer closure: captures the INNER closure by value (captures-of-captures).
    let outer_spec = compose_closure_burden_spec(
        outer_idx,
        &[ClosureCapture {
            field_index: 0,
            field_type: inner_idx,
        }],
        &[],
    );
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Outer_closure",
        outer_idx,
        Some(outer_spec.clone()),
    );

    // Sanity: outer's owned_field carries the inner closure's Idx.
    let registered_outer = registry
        .burden(outer_idx)
        .unwrap_or_else(|| panic!("outer closure burden MUST be registered"));
    assert_eq!(
        registered_outer.owned_fields[0].field_type, inner_idx,
        "outer closure MUST carry inner closure's Idx in owned_fields[0]",
    );

    // Distinct compiled_drop FnSyms per per-Idx mangling.
    let registered_inner = registry
        .burden(inner_idx)
        .unwrap_or_else(|| panic!("inner closure burden MUST be registered"));
    assert_ne!(
        registered_outer.compiled_drop, registered_inner.compiled_drop,
        "outer and inner closures MUST get distinct compiled_drop FnSyms",
    );

    // Positive emission: PartialApply capturing the inner closure into the
    // outer's env emits BurdenInc(inner) before PartialApply. Follow-up Let-
    // Var keeps var(0) alive past the PartialApply.
    let mut func = ArcFunction {
        var_types: vec![inner_idx, outer_idx, inner_idx],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: outer_idx,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: inner_idx,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let body = &func.blocks[0].body;
    let inc_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)));
    assert!(
        inc_pos.is_some(),
        "expected BurdenInc(captured=inner_closure=var(0)) before outer PartialApply; body={body:?}",
    );
    let pa_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::PartialApply { .. }))
        .unwrap_or_else(|| panic!("PartialApply MUST appear in body"));
    assert!(
        inc_pos.unwrap_or_else(|| unreachable!("checked is_some above")) < pa_pos,
        "BurdenInc(inner) MUST appear BEFORE outer PartialApply; body={body:?}",
    );
}

#[test]
fn closure_capture_of_projection_emits_borrowed_field_with_parent_lifetime() {
    // success_criterion (positive — capture of projection): treated as
    // borrowed_fields entry with parent variable's lifetime tied — the
    // projection itself does not own; parent owns.
    //
    // Spec-level pin: composing a closure that captures-by-reference a
    // projection records the projected field's type (NOT the parent's) in
    // borrowed_fields[i]. Lifetime tie-back to the parent variable is
    // enforced downstream by the borrow-inference machinery at
    // `ori_arc/src/borrow/mod.rs`; the burden composer's contract is to
    // populate the field-type slot correctly so codegen + borrow inference
    // can compose without ambiguity.
    use ori_types::burden_compose::closure::{compose_closure_burden_spec, ClosureCapture};

    let mut registry = TypeRegistry::new();
    let closure_idx = Idx::from_raw(64);
    // Projected field's resolved type (e.g., `p.a` where p has field `a: int`).
    let projected_field_type = Idx::INT;
    let spec = compose_closure_burden_spec(
        closure_idx,
        &[],
        &[ClosureCapture {
            field_index: 0,
            field_type: projected_field_type,
        }],
    );
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Closure_projection_capture",
        closure_idx,
        Some(spec),
    );

    let registered = registry
        .burden(closure_idx)
        .unwrap_or_else(|| panic!("closure burden MUST be registered"));
    assert!(
        registered.owned_fields.is_empty(),
        "capture-of-projection MUST NOT populate owned_fields — projection does not own",
    );
    assert_eq!(
        registered.borrowed_fields.len(),
        1,
        "capture-of-projection MUST populate borrowed_fields with one entry",
    );
    assert_eq!(
        registered.borrowed_fields[0].field_type, projected_field_type,
        "borrowed_fields entry MUST carry the projected field's resolved Idx (NOT parent's)",
    );
}

#[test]
fn partial_apply_owned_capture_passed_to_owned_callee_emits_two_transfer_point_burden_inc() {
    // specific PartialApply matrix pin per success_criterion 5: binding
    // consumed by PartialApply AND passed to Owned callee in same expr →
    // transfer-count 2 → one BurdenInc lands (zero-net Rule 5).
    //
    // The shipped Rule 5 invariant is: each captured arg gets ONE
    // transfer-point per consumption site; PartialApply + Owned callee = 2
    // transfer points = 2 BurdenInc emissions on the captured variable. The
    // closure-burden composition does NOT change this — the closure's
    // own burden walk emits BurdenInc on the CLOSURE's env-field side (NOT
    // the captured variable side); the captured-side BurdenInc count comes
    // from `is_owned_position` at the PartialApply + Apply sites +
    //.
    //
    // This pin verifies the Rule 5 invariant holds UNCHANGED under
    // closure-burden registration: registering a closure burden on the
    // closure type's Idx does NOT alter the captured-variable transfer-count.
    use ori_types::burden_compose::closure::{compose_closure_burden_spec, ClosureCapture};

    let mut registry = TypeRegistry::new();
    let closure_idx = Idx::from_raw(64);
    crate::lower::test_utils::registered_struct_with_burden(
        &mut registry,
        "Closure_passes_through",
        closure_idx,
        Some(compose_closure_burden_spec(
            closure_idx,
            &[ClosureCapture {
                field_index: 0,
                field_type: Idx::STR,
            }],
            &[],
        )),
    );

    // : Build with follow-up Let-Var keeping var(0) alive past
    // BOTH owned-position consumption sites — var(0)'s last-use is the Let,
    // not PartialApply or Apply, so both owned-pos Incs are emitted.
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, closure_idx, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(1),
                    ty: closure_idx,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: Name::from_raw(100),
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let body = &func.blocks[0].body;
    let inc_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)))
        .count();
    assert_eq!(
        inc_count, 2,
        "RL-2 Rule 5: captured arg consumed by PartialApply AND Owned callee MUST get 2 transfer-point BurdenInc emissions on var(0); got {inc_count} in body={body:?}",
    );

    // Both BurdenInc emissions precede their respective consumption sites.
    let pa_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::PartialApply { .. }))
        .unwrap_or_else(|| panic!("PartialApply MUST appear in body"));
    let apply_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Apply { .. }))
        .unwrap_or_else(|| panic!("Apply MUST appear in body"));
    assert!(pa_pos < apply_pos, "PartialApply MUST precede Apply");

    // Ordering: each BurdenInc precedes its corresponding consumption site.
    let inc_positions: Vec<usize> = body
        .iter()
        .enumerate()
        .filter_map(|(idx, i)| {
            matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)).then_some(idx)
        })
        .collect();
    assert_eq!(inc_positions.len(), 2, "two BurdenInc positions expected");
    assert!(
        inc_positions[0] < pa_pos,
        "first BurdenInc MUST precede PartialApply"
    );
    assert!(
        inc_positions[1] < apply_pos && inc_positions[1] > pa_pos,
        "second BurdenInc MUST appear between PartialApply and Apply; positions={inc_positions:?}, pa_pos={pa_pos}, apply_pos={apply_pos}",
    );
}

/// RL-1 duplication-alias emission: a Let-Var dup-alias whose terminator-position
/// last-use is NOT an ownership transfer (Jump arg to a Borrowed
/// target-block-param) is a genuine duplication of its still-live source. The
/// burden path emits the alias's OWN paired RC — a FRESH-site `BurdenInc` at the
/// alias site balanced by a `BurdenDec` at the terminator-position last-use —
/// net 0, WITHOUT deferring to the predicate stack. This pins §07A.1's
/// move-vs-duplication classifier: `dup_alias_dsts` are the DUPLICATION case
/// (own inc + matching dec), not the MOVE case (inc-suppressed, no source dec).
/// Positive pin: exactly one inc + one dec for the alias (net 0). Negative pin:
/// re-suppressing the dup-alias dec re-introduces the regressing gap where
/// the alias inc orphans (net +1) once the predicate stack is deleted.
#[test]
fn dup_alias_at_terminator_nontransfer_emits_paired_burden_inc_dec() {
    let registry = TypeRegistry::new();
    // %0: heap STR produced by an Apply (no contract -> FRESH-site BurdenInc).
    // %1: Let-Var alias of %0; %0 stays live (used again by the second Apply),
    //     so %1 is a dup_alias_dst (DUPLICATION, not MOVE).
    // %2: second Apply consuming %0 — keeps %0's use count >= 2.
    // Terminator: Jump block1, args=[%1] — block1's param (%3) is BorrowedFrom,
    //     so the Jump arg is NON-transfer; %1's last use lands in the
    //     non-transfer loop of emit_terminator_burden_decs.
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Apply {
                        dst: ArcVarId::new(0),
                        ty: Idx::STR,
                        func: Name::from_raw(100),
                        args: Vec::new(),
                        arg_ownership: Vec::new(),
                        mono_instance_id: None,
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::STR,
                        value: ArcValue::Var(ArcVarId::new(0)),
                    },
                    ArcInstr::Apply {
                        dst: ArcVarId::new(2),
                        ty: Idx::STR,
                        func: Name::from_raw(101),
                        args: vec![ArcVarId::new(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![ArcVarId::new(1)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(ArcVarId::new(3), Idx::STR)],
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };

    // Mark block1's param (%3) as a borrow so the Jump arg is non-transfer.
    let derived_ownership = vec![
        DerivedOwnership::Owned,                          // %0
        DerivedOwnership::Owned,                          // %1
        DerivedOwnership::Owned,                          // %2
        DerivedOwnership::BorrowedFrom(ArcVarId::new(1)), // %3 (block1 param)
    ];

    let _ctx = emit_burden_ops(
        &mut func,
        &registry,
        &derived_ownership,
        &[],
        &FxHashMap::default(),
        false,
    );

    let body = &func.blocks[0].body;
    // Positive pin: the alias %1 receives its own FRESH-site BurdenInc (RL-1
    // duplication) AND a matching last-use BurdenDec at the non-transfer Jump
    // arg position — net 0 for the alias, owned wholly by the burden path.
    let alias_incs = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)))
        .count();
    assert_eq!(
        alias_incs, 1,
        "RL-1: dup-alias var(1) MUST receive exactly one alias-site BurdenInc; body={body:?}",
    );
    let alias_decs = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(1)))
        .count();
    assert_eq!(
        alias_decs, 1,
        "RL-1: dup-alias var(1) MUST receive exactly one matching last-use BurdenDec (net 0 with its alias-site inc); body={body:?}",
    );

    // VF-1 semantic pin: faithful Phase-5 emission nets the burden ledger to 0
    // for the duplication alias — neither orphaned inc (net +1, the gap)
    // nor orphaned dec (net -1).
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "RL-1: dup-alias inc/dec pair must net VF-1 to 0; imbalances={imbalances:?}",
    );
}

// RL-4 live-out suppression matrix (`Spec: Annex E §AIMS RL-4`): a per-block
// last-use `BurdenDec` is a genuine release only when the var is dead at block
// exit. A var live-out of a block (used in a reachable successor) must NOT
// receive an in-block last-use dec — the value lives on; the release belongs on
// the dying CFG edge (predicate-stack edge cleanup) or at the dead-out block.
// Reverting the suppression re-emits the spurious in-block dec → VF-1 net=-1/-2.

/// Total `BurdenDec` / `BurdenDecPartial` / `BurdenDecVariant` ops targeting
/// `var` across every block body.
fn count_burden_decs(func: &ArcFunction, var: ArcVarId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter(|i| {
            matches!(
                i,
                ArcInstr::BurdenDec { var: v }
                | ArcInstr::BurdenDecPartial { var: v, .. }
                | ArcInstr::BurdenDecVariant { var: v }
                if *v == var
            )
        })
        .count()
}

#[test]
fn owned_param_live_out_of_block_gets_no_in_block_last_use_dec() {
    // Conditional-transfer shape (the `comparable::find_max`/`find_min` v0/v1
    // residual): owned param %0: str is aliased in bb0 (`%2 = %0`, live-out to
    // bb1 on the then-edge) and again in bb1 (`%3 = %0`). The bb0 last-use is
    // SPURIOUS — %0 lives to bb1 — so RL-4 suppresses it; the genuine release is
    // bb1's dead-out last-use dec (plus the predicate-stack dead-edge dec on the
    // else-edge at realize time, not modeled here). Negative pin: reverting the
    // live-out suppression re-adds the bb0 dec → 2 decs on the then-path → net
    // -2 against the owned param's single incoming reference (VF-1 imbalance).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Semantic pin: %0 (owned param, no FRESH inc) receives exactly ONE
    // in-block last-use dec (bb1, where it is dead-out); the bb0 last-use is
    // suppressed because %0 is live-out of bb0. The bb2 else-edge dead release
    // is the realize-walk's job (not the burden walk's).
    let bb0_decs = func.blocks[0]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
        .count();
    assert_eq!(
        bb0_decs, 0,
        "RL-4: %0 is live-out of bb0 (used in bb1) — bb0's last-use BurdenDec MUST be suppressed; bb0={:?}",
        func.blocks[0].body,
    );
    let bb1_decs = func.blocks[1]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
        .count();
    assert_eq!(
        bb1_decs, 1,
        "RL-4: %0 is dead-out of bb1 — its genuine last-use BurdenDec is kept there; bb1={:?}",
        func.blocks[1].body,
    );
}

#[test]
fn fresh_value_live_across_blocks_nets_burden_balance_zero() {
    // Multi-block live-out (the `cow::shared_substring` / `debug::escape`
    // residual): a FRESH str %0 is created in bb0 (FRESH-site BurdenInc),
    // aliased in bb0 (`%1 = %0`, live-out to bb1) and consumed at bb1's Return
    // via `%2 = %0`. RL-4 suppresses the bb0 last-use dec (%0 live-out); the
    // single FRESH inc is balanced by the single kept bb1 dec → VF-1 net 0.
    // Semantic pin: `verify_burden_balance` reports zero imbalances. Negative
    // pin: reverting the live-out suppression re-adds the bb0 dec → inc(bb0) -
    // dec(bb0) - dec(bb1) = -1 → a `BurdenBalanceError` for %0.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(0),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::STR,
                        value: ArcValue::Var(ArcVarId::new(0)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Semantic pin: faithful Phase-5 emission nets the burden ledger to 0 on
    // every path (one FRESH inc, one kept last-use dec).
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "RL-4: FRESH %0 live across bb0->bb1 must net VF-1 to 0; imbalances={imbalances:?}",
    );
    // %0 receives exactly one FRESH inc and exactly one (bb1) dec.
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        1,
        "RL-4: %0's only BurdenDec is bb1's dead-out last-use; bb0={:?} bb1={:?}",
        func.blocks[0].body,
        func.blocks[1].body,
    );
    let bb0_decs = func.blocks[0]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)))
        .count();
    assert_eq!(
        bb0_decs, 0,
        "RL-4: %0 is live-out of bb0 — bb0's last-use BurdenDec MUST be suppressed; bb0={:?}",
        func.blocks[0].body,
    );
}

#[test]
fn dead_out_value_keeps_its_single_block_last_use_dec() {
    // RL-4 boundary / negative-space pin: a FRESH str %0 read by two
    // duplication aliases within bb0 (use_counts=2 → NOT a move-alias transfer)
    // and dead at bb0 exit (the Return value is an unrelated scalar) is NOT
    // live-out — so its in-block last-use dec is KEPT (the suppression fires
    // ONLY for live-out vars). Pairs with the two suppression pins above:
    // confirms the fix narrows to live-out and does not over-suppress the
    // genuine single-block release. %1/%2 are dup-alias dsts (their own decs
    // suppressed); %0 carries the FRESH inc + the kept last-use dec → net 0.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(3),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        1,
        "RL-4 narrowness: a dead-out (not live-out) %0 KEEPS its single-block last-use BurdenDec; body={:?}",
        func.blocks[0].body,
    );
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "dead-out single-block release nets VF-1 to 0; imbalances={imbalances:?}",
    );
}

// Apply-aliases coverage — `burden_emitted ⊇ apply-alias-class members`
//
// The Phase-5 burden walk is VAR-INDEXED (`collect_owned_burdens` over
// `func.var_types`, mod.rs), so every apply-alias-class member that is an
// owned non-scalar SSA var is structurally visited and marked in
// `func.burden_emitted` when it receives a Burden op. These pins assert the
// observable coverage property directly over `func.burden_emitted`,
// NOT the `class_covered` handshake (structurally empty during the
// coexistence phase: `populate_class_covered` runs Step 4, short-circuits on
// empty `burden_emitted`, filled only at Step 4b).
//
// One cell per `ApplyAliasSource` shape modeled at the caller-side SSA level:
//   Direct      `@id<T>(x: T) -> T = x`             — dst aliases arg
//   Project     `@unwrap<T>(b: Box<T>) -> T = b.inner` — dst is a borrow-view
//   Conditional multi-param path-conditional alias  — every candidate owned
//   Wrapped     `@wrap_ok(m: T) -> Result<T, E> = Ok(m)` — dst is a separate
//                                                          wrapper allocation
//
// The RC-carrying member (the consumed Owned arg) MUST be in `burden_emitted`
// (positive pin). The RC-SUPPRESSED dst of Project (a borrow-view carrying no
// independent RC slot — its RC is parent-drop-covered / predicate-stack-owned
// during coexistence) MUST be EXCLUDED (negative pin): the exclusion is
// intentional emission fidelity, not a coverage gap.
//
// ADDITIVE only — touches NO production code. `collect_all_borrowed_defs` and
// the borrow classification stay untouched (3 reclassification attempts each
// regressed AOT 29 -> 74).

/// Read-only helper: is `var`'s `burden_emitted` bit set after the walk?
fn burden_emitted_for(func: &ArcFunction, var: ArcVarId) -> bool {
    func.burden_emitted
        .get(var.index())
        .copied()
        .unwrap_or(false)
}

/// Build a caller-side `ArcFunction` modeling one apply-alias shape: a single
/// `Apply` whose consumed arg(s) each have a follow-up `Let { Var }` alias so
/// the arg's owned-position `BurdenInc` reliably emits (last-use is the Let,
/// not the Apply — matching the existing
/// `apply_emits_burden_inc_immediately_before_consuming_apply` shape). The
/// result `dst` is kept live by a follow-up `Let { Var }` so its FRESH-site
/// `BurdenInc` emits. Every var is `Idx::STR` (heap-burden, carries RC) unless
/// noted.
fn apply_alias_caller_func(
    apply: ArcInstr,
    consumed_args: &[ArcVarId],
    dst: ArcVarId,
    var_count: u32,
) -> ArcFunction {
    let mut body = vec![apply];
    // Follow-up alias per consumed arg keeps it live past the Apply.
    let mut next_var = var_count;
    for &arg in consumed_args {
        body.push(ArcInstr::Let {
            dst: ArcVarId::new(next_var),
            ty: Idx::STR,
            value: ArcValue::Var(arg),
        });
        next_var += 1;
    }
    // TWO follow-up aliases keep the result GENUINELY live (use-count >= 2 =
    // duplication, not a single-use move): the FRESH-site BurdenInc fires. A
    // single-use move-alias is an RL-2 ownership transfer whose source inc is
    // correctly suppressed (the lineage's single inc+dec lands at the dst).
    body.push(ArcInstr::Let {
        dst: ArcVarId::new(next_var),
        ty: Idx::STR,
        value: ArcValue::Var(dst),
    });
    next_var += 1;
    body.push(ArcInstr::Let {
        dst: ArcVarId::new(next_var),
        ty: Idx::STR,
        value: ArcValue::Var(dst),
    });
    let total = next_var + 1;
    ArcFunction {
        var_types: (0..total).map(|_| Idx::STR).collect(),
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

/// `ArcInstr::Apply` with the apply-alias test defaults: `str` result, callee
/// `Name::from_raw(99)`, every arg Owned, no monomorphization id. Keeps the
/// `mono_instance_id` default at one site (avoids parameter sprawl).
fn apply_str(dst: ArcVarId, args: Vec<ArcVarId>) -> ArcInstr {
    let arg_ownership = vec![ArgOwnership::Owned; args.len()];
    ArcInstr::Apply {
        dst,
        ty: Idx::STR,
        func: Name::from_raw(99),
        args,
        arg_ownership,
        mono_instance_id: None,
    }
}

#[test]
fn apply_alias_direct_shape_marks_consumed_arg_in_burden_emitted() {
    // ApplyAliasSource::Direct (`@id<T>(x: T) -> T = x`).
    // Caller `dst = id(arg)`: dst and arg are the SAME RC slot (union-find
    // unites them). The consumed Owned arg carries the independent RC slot,
    // so `burden_emitted[arg]` MUST be set (positive pin); the var-indexed
    // walk visits arg as an owned non-scalar SSA var.
    let registry = TypeRegistry::new();
    let arg = ArcVarId::new(0);
    let dst = ArcVarId::new(1);
    let mut func = apply_alias_caller_func(apply_str(dst, vec![arg]), &[arg], dst, 2);
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        burden_emitted_for(&func, arg),
        "Direct: consumed Owned arg (RC-carrying alias-class member) MUST be in burden_emitted; burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
    assert!(
        burden_emitted_for(&func, dst),
        "Direct: Apply result dst (FRESH-site Inc) MUST be in burden_emitted; burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
}

#[test]
fn apply_alias_project_shape_marks_arg_excludes_borrow_view_dst() {
    // ApplyAliasSource::Project
    // (`@unwrap<T>(b: Box<T>) -> T = b.inner`). The consumed Owned arg `b`
    // carries the independent RC slot → MUST be in burden_emitted (positive
    // pin). The dst is a borrow-view projection of arg's field — it carries
    // NO independent RC slot during coexistence (its RC is parent-drop-
    // covered; predicate-stack-owned until the predicate-stack retirement
    // phase). Modeling the dst as a
    // `Project` of the arg makes it a borrow per TF-4; the walk emits no
    // BurdenInc/Dec for a borrow-view, so `burden_emitted[dst]` MUST be unset
    // (negative pin — the exclusion is intentional emission fidelity, NOT a
    // coverage gap).
    let registry = TypeRegistry::new();
    let arg = ArcVarId::new(0); // the consumed Box<T> — RC-carrying
    let dst = ArcVarId::new(1); // b.inner borrow-view — RC-suppressed
    let arg_alias = ArcVarId::new(2);
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                // dst = Project arg.0 — borrow-view (callee returns b.inner).
                project_first(dst, Idx::STR, arg),
                // Follow-up alias keeps arg live past the projection.
                ArcInstr::Let {
                    dst: arg_alias,
                    ty: Idx::STR,
                    value: ArcValue::Var(arg),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        burden_emitted_for(&func, arg),
        "Project: consumed Owned arg (the Box<T>, RC-carrying) MUST be in burden_emitted; burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
    // Negative pin: a borrow-view dst (Project of arg) carries no independent
    // RC slot — its exclusion is intentional (parent-drop-covers per the
    // borrow model), NOT a missed apply-alias-class member.
    assert!(
        !burden_emitted_for(&func, dst),
        "Project: borrow-view dst (b.inner, RC-suppressed) MUST be EXCLUDED from burden_emitted (parent-drop-covered during coexistence); burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
}

#[test]
fn apply_alias_conditional_shape_marks_every_owned_candidate() {
    // ApplyAliasSource::Conditional (2+ Owned params alias
    // the return path-conditionally, e.g. callee `match x { A -> a, B -> b }`).
    // Caller `dst = select(a, b)`: BOTH candidates `a` and `b` are owned
    // non-scalar args carrying independent RC slots → BOTH MUST be in
    // burden_emitted (positive pin across every candidate). Self-verifying
    // member count proves no candidate cell was skipped.
    let registry = TypeRegistry::new();
    let a = ArcVarId::new(0);
    let b = ArcVarId::new(1);
    let dst = ArcVarId::new(2);
    let mut func = apply_alias_caller_func(apply_str(dst, vec![a, b]), &[a, b], dst, 3);
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let candidates = [a, b];
    let covered = candidates
        .iter()
        .filter(|&&c| burden_emitted_for(&func, c))
        .count();
    assert_eq!(
        covered,
        candidates.len(),
        "Conditional: EVERY Owned candidate ({candidates:?}) MUST be in burden_emitted (no skipped cell); burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
}

#[test]
fn apply_alias_wrapped_shape_marks_consumed_arg_and_wrapper() {
    // ApplyAliasSource::Wrapped
    // (`@wrap_ok(m: T) -> Result<T, E> = Ok(m)`). The dst is a SEPARATE
    // allocation (the constructed wrapper) and the consumed arg `m`'s
    // ownership transfers INTO dst's payload. The Apply consumes `m` at an
    // Owned position, so the owned-position BurdenInc marks
    // `burden_emitted[m]` (positive pin — m is the RC-carrying member). The
    // wrapper dst gets a FRESH-site Inc (positive pin). NOTE: Wrapped does NOT
    // union dst with m in the union-find (different RC slots) — both
    // nonetheless each carry a burden op, so both bits set.
    let registry = TypeRegistry::new();
    let m = ArcVarId::new(0);
    let dst = ArcVarId::new(1);
    let mut func = apply_alias_caller_func(apply_str(dst, vec![m]), &[m], dst, 2);
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert!(
        burden_emitted_for(&func, m),
        "Wrapped: consumed Owned arg m (ownership transferred into wrapper payload) MUST be in burden_emitted; burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
    assert!(
        burden_emitted_for(&func, dst),
        "Wrapped: wrapper dst (separate allocation, FRESH-site Inc) MUST be in burden_emitted; burden_emitted={:?}; body={:?}",
        func.burden_emitted,
        func.blocks[0].body,
    );
}

#[test]
fn apply_alias_coverage_self_verifying_member_count_across_all_shapes() {
    // self-verifying matrix completeness. One cell
    // per ApplyAliasSource shape; the RC-carrying consumed arg of each shape
    // MUST be in burden_emitted. The count assertion proves every shape cell
    // was visited — a silently-skipped shape would drop the count below 4.
    let registry = TypeRegistry::new();
    let mut shapes_covered = 0usize;

    // Direct.
    {
        let arg = ArcVarId::new(0);
        let dst = ArcVarId::new(1);
        let mut func = apply_alias_caller_func(apply_str(dst, vec![arg]), &[arg], dst, 2);
        emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
        assert!(burden_emitted_for(&func, arg), "Direct arg uncovered");
        shapes_covered += 1;
    }
    // Project (RC-carrying arg covered; borrow-view dst excluded).
    {
        let arg = ArcVarId::new(0);
        let dst = ArcVarId::new(1);
        let arg_alias = ArcVarId::new(2);
        let mut func = ArcFunction {
            var_types: vec![Idx::STR, Idx::STR, Idx::STR],
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    project_first(dst, Idx::STR, arg),
                    ArcInstr::Let {
                        dst: arg_alias,
                        ty: Idx::STR,
                        value: ArcValue::Var(arg),
                    },
                ],
                terminator: ArcTerminator::Unreachable,
            }],
            entry: ArcBlockId::new(0),
            name: Name::from_raw(0),
            ..ArcFunction::default()
        };
        emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
        assert!(burden_emitted_for(&func, arg), "Project arg uncovered");
        assert!(
            !burden_emitted_for(&func, dst),
            "Project borrow-view dst MUST be excluded"
        );
        shapes_covered += 1;
    }
    // Conditional.
    {
        let a = ArcVarId::new(0);
        let b = ArcVarId::new(1);
        let dst = ArcVarId::new(2);
        let mut func = apply_alias_caller_func(apply_str(dst, vec![a, b]), &[a, b], dst, 3);
        emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
        assert!(
            burden_emitted_for(&func, a) && burden_emitted_for(&func, b),
            "Conditional candidate uncovered"
        );
        shapes_covered += 1;
    }
    // Wrapped.
    {
        let m = ArcVarId::new(0);
        let dst = ArcVarId::new(1);
        let mut func = apply_alias_caller_func(apply_str(dst, vec![m]), &[m], dst, 2);
        emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
        assert!(burden_emitted_for(&func, m), "Wrapped arg uncovered");
        shapes_covered += 1;
    }

    assert_eq!(
        shapes_covered, 4,
        "every ApplyAliasSource shape (Direct / Project / Conditional / Wrapped) MUST be visited; no cell skipped",
    );
}

// §07A.1 move-vs-duplication classifier matrix (Let { Var } aliases).
//
// Per the classifier table, every `Let { Var(src) }` alias `%d = %s`
// partitions into exactly one case (WBS 100% — no overlap, no gap):
//   - MOVE (terminator-transfer):  `%s` used once, lineage transfers out at a
//     terminator → NO inc on aliases, NO source last-use dec (discharged at the
//     transfer point per RL-2 ownership-transfer exception).
//   - MOVE (same/cross-block, dies non-transfer): `%s` used once, lineage's
//     true survivor last-use is non-transfer → inc-suppressed on aliases, single
//     freeing dec at the survivor last-use (RL-1 inc only on genuine dup).
//   - DUPLICATION (post-alias-source-use): `%s` used >= 2 times (stays live) →
//     burden path emits the alias's own paired BurdenInc + matching BurdenDec.
//
// Each pin asserts the burden-op EMISSION SHAPE and the VF-1 net per var.

/// Total `BurdenInc` ops targeting `var` across every block body.
fn count_burden_incs(func: &ArcFunction, var: ArcVarId) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var: v } if *v == var))
        .count()
}

/// §07A.1 case (a) — MOVE-alias chain `%0 -> %2 -> %4` whose terminal use
/// TRANSFERS out (Return %4). Each hop's source is used exactly once, so the
/// whole lineage is a MOVE: RL-1 emits NO `BurdenInc` on any alias, and RL-2's
/// ownership-transfer exception suppresses every last-use dec — the release is
/// discharged at the Return transfer point (the caller inherits it), not at any
/// move-alias site. Negative pin: a stray inc OR dec on any chain member would
/// fail VF-1 per-var (orphan), regressing the burden-as-sole-emitter path.
#[test]
fn move_alias_chain_to_return_emits_no_burden_ops() {
    let registry = TypeRegistry::new();
    // %0: FRESH str literal. %2 = %0 (move). %4 = %2 (move). Return %4.
    // %1, %3 are scalar fillers keeping the var indices aligned with the
    // %0->%2->%4 chain naming from the mission (skipped odd indices).
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::STR, Idx::INT, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(4),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    for chain_var in [ArcVarId::new(0), ArcVarId::new(2), ArcVarId::new(4)] {
        assert_eq!(
            count_burden_incs(&func, chain_var),
            0,
            "MOVE-alias chain-to-Return: {chain_var:?} MUST receive zero BurdenInc (RL-1: inc only on genuine duplication); body={:?}",
            func.blocks[0].body,
        );
        assert_eq!(
            count_burden_decs(&func, chain_var),
            0,
            "MOVE-alias chain-to-Return: {chain_var:?} MUST receive zero BurdenDec (RL-2 ownership-transfer exception; discharged at Return); body={:?}",
            func.blocks[0].body,
        );
    }
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "MOVE-alias chain-to-Return must net VF-1 to 0 per var (no orphan inc/dec); imbalances={imbalances:?}",
    );
}

/// §07A.1 case (b) — DUPLICATION alias whose LIVE source is consumed again at a
/// later body instruction, and whose own last-use is a non-transfer body instr.
/// `%0` FRESH; `%1 = %0` (dup, %0 stays live); `%0` used again at a borrowed
/// Apply; `%1` used at a borrowed Apply (its non-transfer last use). The burden
/// path emits `%1`'s OWN paired `BurdenInc` (alias site) + `BurdenDec` (last use) —
/// net 0 — WITHOUT deferring to the predicate stack. Negative pin: dropping the
/// alias inc OR the matching dec breaks VF-1 per-var for %1.
#[test]
fn duplication_alias_with_live_source_emits_paired_inc_dec_at_body_last_use() {
    let registry = TypeRegistry::new();
    // %0: FRESH str (Apply, no contract). %1 = %0 (dup; %0 stays live).
    // %2: Apply borrowing %0 (keeps %0 use-count >= 2; borrowed -> non-transfer).
    // %3: Apply borrowing %1 (%1's non-transfer last use).
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Apply {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    func: Name::from_raw(100),
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: Name::from_raw(101),
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    func: Name::from_raw(102),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(1)),
        1,
        "DUPLICATION alias var(1): burden path MUST emit exactly one alias-site BurdenInc (RL-1); body={body:?}",
    );
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(1)),
        1,
        "DUPLICATION alias var(1): burden path MUST emit exactly one matching last-use BurdenDec (RL-2); body={body:?}",
    );
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "DUPLICATION alias paired inc/dec must net VF-1 to 0 per var; imbalances={imbalances:?}",
    );
}

/// §07A.1 case (c) — terminator-transfer MOVE-alias single hop `%1 = %0`
/// (the `@id<T>(x: T) -> T = x` minimal witness over an owned PARAM). `%0` is
/// an owned str param (NO FRESH inc — params carry their inbound reference);
/// `%1 = %0` (move, %0 used once); `Return %1` transfers ownership to the
/// caller. NO inc anywhere (param has none; alias is a move); NO source last-use
/// dec (RL-2 ownership-transfer exception — `%0`'s reference returns to the
/// caller through the move chain). Negative pin: a last-use dec on `%0` would
/// double-release the returned reference (VF-1 net=-1, the move-alias regression
/// the mission's `id<T>` witness names).
#[test]
fn terminator_transfer_move_alias_over_owned_param_emits_no_burden_ops() {
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::STR,
                value: ArcValue::Var(ArcVarId::new(0)),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    for v in [ArcVarId::new(0), ArcVarId::new(1)] {
        assert_eq!(
            count_burden_incs(&func, v),
            0,
            "terminator-transfer MOVE-alias: {v:?} MUST receive zero BurdenInc; body={body:?}",
        );
        assert_eq!(
            count_burden_decs(&func, v),
            0,
            "terminator-transfer MOVE-alias: {v:?} MUST receive zero BurdenDec (RL-2 ownership-transfer exception); body={body:?}",
        );
    }
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "terminator-transfer MOVE-alias over owned param must net VF-1 to 0; imbalances={imbalances:?}",
    );
}

#[test]
fn project_of_borrowed_param_dst_gets_no_burden_dec() {
    // §07A.2 task 3 — `Project { dst, value: borrowed_src }` (TF-4 borrow-view).
    // A heap-burden (str) projected from a borrowed param is itself Borrowed
    // (TF-4: dst.access := Borrowed, inheriting source uniqueness/locality) and
    // carries NO RC obligation (`Spec: Annex E §AIMS RL-2` — borrowed values do
    // not receive decs). The projected dst's genuine last-use here (consumed at
    // a borrowed Invoke arg, NOT transfer-exempt) would otherwise receive a
    // last-use BurdenDec — a double-free in the standalone ledger, since the
    // borrow-view does not own its allocation. The source-gated Project
    // propagation in `compute_borrowed_alias_vars` excludes the dst.
    //
    // Shape mirrors `@uses_field(p: Pair) -> int { let x = p.a; x.len() }`:
    //   %0: borrowed Pair param. %1 = %0 (Let-Var alias). %2 = Project %1.0 (str
    //   borrow-view). Invoke @len(%2 [borrow]) — %2 borrowed, not transferred.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Borrowed,
        }],
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::STR,
                        value: ArcValue::Var(ArcVarId::new(0)),
                    },
                    project_first(ArcVarId::new(2), Idx::STR, ArcVarId::new(1)),
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    // Pin: the Project dst (var 2) — a TF-4 borrow-view of the borrowed param
    // chain — receives NO BurdenDec (double-free guard) and NO BurdenInc.
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(2)),
        0,
        "Project dst of a borrowed-param chain MUST receive zero BurdenDec \
         (TF-4 Borrowed: no RC obligation; a dec is a double-free); body={:?}",
        func.blocks[0].body,
    );
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(2)),
        0,
        "Project dst of a borrowed-param chain MUST receive zero BurdenInc; body={:?}",
        func.blocks[0].body,
    );
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "Project-of-borrowed-param must net VF-1 to 0; imbalances={imbalances:?}",
    );
}

#[test]
fn nested_project_of_borrowed_param_dst_gets_no_burden_dec() {
    // §07A.2 task 3 matrix cell (d) — `Project` of a `Project` (nested borrow).
    // %0 borrowed Pair param; %1 = Project %0.0 (str borrow-view, TF-4); %2 =
    // Project %1.0 (nested borrow-view of the projection). Per TF-4 + RL-15a
    // README note, the source-gated propagation marks BOTH projection dsts
    // borrowed because each `value` source is itself in the borrowed set — so
    // neither receives a BurdenDec (double-free guard at every nesting level).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Borrowed,
        }],
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0)),
                    project_first(ArcVarId::new(2), Idx::STR, ArcVarId::new(1)),
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    for v in [ArcVarId::new(1), ArcVarId::new(2)] {
        assert_eq!(
            count_burden_decs(&func, v),
            0,
            "nested Project dst {v:?} of a borrowed source MUST receive zero BurdenDec \
             (TF-4 Borrowed at every nesting level); body={:?}",
            func.blocks[0].body,
        );
        assert_eq!(
            count_burden_incs(&func, v),
            0,
            "nested Project dst {v:?} of a borrowed source MUST receive zero BurdenInc; body={:?}",
            func.blocks[0].body,
        );
    }
    let imbalances = crate::aims::verify::burden_balance::verify_burden_balance(&func);
    assert!(
        imbalances.is_empty(),
        "nested Project-of-borrowed-source must net VF-1 to 0; imbalances={imbalances:?}",
    );
}

#[test]
fn project_of_owned_source_dst_is_not_borrow_excluded() {
    // §07A.2 task 3 source-gating boundary (negative pin): a `Project` whose
    // `value` source is OWNED (NOT borrowed) MUST NOT be added to the
    // borrowed-alias set. Source-gating is the safe form — a blanket Project-dst
    // borrow exclusion would be unsafe under RL-15a project-escape (a Project of
    // an owned source may carry an RC obligation per RL-33 projection promotion).
    // Here %0 is an OWNED str param; %1 = Project %0.0 is a borrow-VIEW per TF-4
    // (so %1 itself is Borrowed by TF-4 and emits no dec) — but the SOURCE %0 is
    // owned and stays in owned_vars_needing_rc. This pins that the source gate
    // keys on the SOURCE's borrowed-ness, not on "is a Project dst".
    //
    // The observable for this pin: the borrowed-alias fixpoint, seeded ONLY from
    // borrowed params, is EMPTY when there are no borrowed params — so no
    // Project-dst is excluded via the borrow path. %0 (owned) keeps its own
    // FRESH/last-use burden treatment (it is a param: no FRESH inc, last-use dec
    // unless transfer-exempt).
    let func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::STR, Idx::INT],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![project_first(ArcVarId::new(1), Idx::STR, ArcVarId::new(0))],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    // The borrowed-alias set is empty (no borrowed params), so the source gate
    // does not exclude the owned param %0 OR the Project dst %1. The contract of
    // the source-gated propagation is exactly this set-membership boundary:
    // exclusion keys on the SOURCE's borrowed-ness, never on "is a Project dst".
    // (VF-1 balance of owned-param-projection is governed by separate
    // transfer/live-out suppression, not by this borrow-exclusion path — so this
    // pin asserts only the set-membership contract this fix owns.)
    let borrowed = super::compute_borrowed_alias_vars(&func);
    assert!(
        !borrowed.contains(&ArcVarId::new(0)),
        "owned param %0 MUST NOT be in the borrowed-alias set",
    );
    assert!(
        !borrowed.contains(&ArcVarId::new(1)),
        "Project dst of an OWNED source MUST NOT be borrow-excluded (source-gate keys on source borrowed-ness)",
    );
}

/// `compute_borrowed_arg_let_aliases`: a `Let { Var(src) }` whose sole use is a
/// BORROWED Invoke arg AND whose `src` is used >= 2 (genuine dup, source stays
/// live to carry the release) is a borrow-view → excluded. `f(x, x)` over two
/// Borrowed params. The `src >= 2` gate is LOAD-BEARING: a
/// move source (src used once) makes the alias the sole RC carrier, so it MUST
/// NOT be excluded (else the release is dropped — leak). Per RL-1.
#[test]
fn borrowed_arg_let_aliases_excludes_dup_source_keeps_move_source() {
    fn alias_func(dup: bool) -> ArcFunction {
        // %0 owned param; %1=Var(%0); (dup) %2=Var(%0); Invoke f(borrowed args).
        let mut body = vec![ArcInstr::Let {
            dst: ArcVarId::new(1),
            ty: Idx::STR,
            value: ArcValue::Var(ArcVarId::new(0)),
        }];
        let (args, arg_own, var_types) = if dup {
            body.push(ArcInstr::Let {
                dst: ArcVarId::new(2),
                ty: Idx::STR,
                value: ArcValue::Var(ArcVarId::new(0)),
            });
            (
                vec![ArcVarId::new(1), ArcVarId::new(2)],
                vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
            )
        } else {
            (
                vec![ArcVarId::new(1)],
                vec![ArgOwnership::Borrowed],
                vec![Idx::STR, Idx::STR, Idx::INT],
            )
        };
        let dst = ArcVarId::new(u32::try_from(var_types.len() - 1).unwrap_or(u32::MAX));
        ArcFunction {
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::STR,
                ownership: Ownership::Owned,
            }],
            var_types,
            blocks: vec![
                ArcBlock {
                    id: ArcBlockId::new(0),
                    params: Vec::new(),
                    body,
                    terminator: ArcTerminator::Invoke {
                        dst,
                        ty: Idx::INT,
                        func: Name::from_raw(99),
                        args,
                        arg_ownership: arg_own,
                        normal: ArcBlockId::new(1),
                        unwind: ArcBlockId::new(2),
                        mono_instance_id: None,
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(1),
                    params: Vec::new(),
                    body: Vec::new(),
                    terminator: ArcTerminator::Return { value: dst },
                },
                ArcBlock {
                    id: ArcBlockId::new(2),
                    params: Vec::new(),
                    body: Vec::new(),
                    terminator: ArcTerminator::Resume,
                },
            ],
            entry: ArcBlockId::new(0),
            name: Name::from_raw(0),
            ..ArcFunction::default()
        }
    }
    // Dup source (src %0 used 2x): both borrow-view aliases excluded.
    let dup = super::compute_borrowed_arg_let_aliases(&alias_func(true));
    assert!(
        dup.contains(&ArcVarId::new(1)) && dup.contains(&ArcVarId::new(2)),
        "dup-source borrowed-arg aliases MUST be excluded (f(x,x)); got {dup:?}",
    );
    // Move source (src %0 used 1x): the alias is the sole carrier — NOT excluded.
    let mv = super::compute_borrowed_arg_let_aliases(&alias_func(false));
    assert!(
        mv.is_empty(),
        "move-source borrowed-arg alias MUST NOT be excluded (sole RC carrier); got {mv:?}",
    );
}

// Collection-buffer last-use freeing dec.
//
// A monomorphized collection instance (`[str]`, `{int:str}`, `Set<str>`) has
// NO `TypeEntry` — its burden lives in the `TypeRegistry` collection-burden
// side-table, registered by the monomorphization-composer flush. Once the
// burden resolves, `Construct List/Map/Set` enters `owned_vars_needing_rc` and
// the buffer receives a FRESH-site `BurdenInc` (TF-3) plus a last-use
// `BurdenDec` (RL-2). VF-1 net must stay 0 (every Inc paired with a Dec).
//
// These exercise `emit_burden_ops` with a registry whose side-table carries
// the composed collection burden — the data path this exercises.

/// Register a composed `[T]` collection burden against `idx` in the registry
/// side-table, mirroring the monomorphization-composer flush.
fn register_list_burden(registry: &mut TypeRegistry, idx: Idx, elem: Idx) {
    use ori_registry::burden::table::{BurdenRegistry, TYPE_ID_LIST};
    use ori_types::Pool;
    let pool = Pool::new();
    let Some(template) = BurdenRegistry::lookup_builtin(TYPE_ID_LIST) else {
        panic!("List template missing from BURDEN_TABLE");
    };
    let spec = ori_types::burden_compose::compose_user_burden(template, &[elem], &pool, registry);
    registry.register_user_burden(idx, spec);
}

fn register_map_burden(registry: &mut TypeRegistry, idx: Idx, key: Idx, val: Idx) {
    use ori_registry::burden::table::{BurdenRegistry, TYPE_ID_MAP};
    use ori_types::Pool;
    let pool = Pool::new();
    let Some(template) = BurdenRegistry::lookup_builtin(TYPE_ID_MAP) else {
        panic!("Map template missing from BURDEN_TABLE");
    };
    let spec =
        ori_types::burden_compose::compose_user_burden(template, &[key, val], &pool, registry);
    registry.register_user_burden(idx, spec);
}

fn register_set_burden(registry: &mut TypeRegistry, idx: Idx, elem: Idx) {
    use ori_registry::burden::table::{BurdenRegistry, TYPE_ID_SET};
    use ori_types::Pool;
    let pool = Pool::new();
    let Some(template) = BurdenRegistry::lookup_builtin(TYPE_ID_SET) else {
        panic!("Set template missing from BURDEN_TABLE");
    };
    let spec = ori_types::burden_compose::compose_user_burden(template, &[elem], &pool, registry);
    registry.register_user_burden(idx, spec);
}

/// Count `BurdenInc(var)` and every `BurdenDec*`-family op targeting `var`
/// across all blocks. VF-1 net = incs - decs MUST be 0 for a balanced buffer.
fn burden_net_for(func: &ArcFunction, var: ArcVarId) -> i64 {
    let mut net: i64 = 0;
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var: v } if *v == var => net += 1,
                ArcInstr::BurdenDec { var: v } if *v == var => net -= 1,
                ArcInstr::BurdenDecPartial { var: v, .. } if *v == var => net -= 1,
                ArcInstr::BurdenDecVariant { var: v } if *v == var => net -= 1,
                ArcInstr::BurdenDecField { base, .. } if *base == var => net -= 1,
                _ => {}
            }
        }
    }
    net
}

/// Build a single-block func mirroring `{ let $xs = [...]; xs.len() }`:
/// construct a collection buffer at `%0`, then borrow it via an Apply (the
/// `len` call), returning a scalar. The buffer's last use is the in-function
/// borrow, so it owns a FRESH-site `BurdenInc` (TF-3) + a last-use freeing
/// `BurdenDec` (RL-2) that net to 0 — the collection-buffer scenario.
fn collection_buffer_then_borrow_func(buf_idx: Idx, ctor: CtorKind) -> ArcFunction {
    ArcFunction {
        // var 0: the constructed buffer; var 1: the scalar `len` result.
        var_types: vec![buf_idx, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: buf_idx,
                    ctor,
                    args: Vec::new(),
                },
                // Borrowing use (mirrors `len(xs [borrow])`): the buffer's last
                // use is here, so it receives a freeing BurdenDec after.
                ArcInstr::Apply {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn list_buffer_emits_fresh_inc_and_last_use_freeing_dec_vf1_zero() {
    let mut registry = TypeRegistry::new();
    let list_idx = Idx::from_raw(300);
    register_list_burden(&mut registry, list_idx, Idx::STR);
    let mut func = collection_buffer_then_borrow_func(list_idx, CtorKind::ListLiteral);
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let buf = ArcVarId::new(0);
    let body = &func.blocks[0].body;
    let has_inc = body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == buf));
    assert!(
        has_inc,
        "[str] buffer must receive a FRESH-site BurdenInc (TF-3); body={body:?}",
    );
    // The buffer's last use is the in-function borrow, so its freeing
    // BurdenDec lands on the buffer var itself. VF-1 net on the buffer = 0.
    let has_dec = body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == buf));
    assert!(
        has_dec,
        "[str] buffer must receive a last-use freeing BurdenDec (RL-2); body={body:?}",
    );
    assert_eq!(
        burden_net_for(&func, buf),
        0,
        "VF-1: [str] buffer Inc/Dec must net to 0; body={body:?}",
    );
}

#[test]
fn map_buffer_emits_fresh_inc_and_freeing_dec_vf1_zero() {
    let mut registry = TypeRegistry::new();
    let map_idx = Idx::from_raw(301);
    register_map_burden(&mut registry, map_idx, Idx::INT, Idx::STR);
    let mut func = collection_buffer_then_borrow_func(map_idx, CtorKind::MapLiteral);
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let buf = ArcVarId::new(0);
    let body = &func.blocks[0].body;
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == buf)),
        "{{int:str}} buffer must receive a FRESH-site BurdenInc; body={body:?}",
    );
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == buf)),
        "{{int:str}} buffer must receive a last-use freeing BurdenDec; body={body:?}",
    );
    assert_eq!(
        burden_net_for(&func, buf),
        0,
        "VF-1: map buffer Inc/Dec must net to 0; body={body:?}",
    );
}

#[test]
fn set_buffer_emits_fresh_inc_and_freeing_dec_vf1_zero() {
    let mut registry = TypeRegistry::new();
    let set_idx = Idx::from_raw(302);
    register_set_burden(&mut registry, set_idx, Idx::STR);
    let mut func = collection_buffer_then_borrow_func(set_idx, CtorKind::SetLiteral);
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);

    let buf = ArcVarId::new(0);
    let body = &func.blocks[0].body;
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == buf)),
        "Set<str> buffer must receive a FRESH-site BurdenInc; body={body:?}",
    );
    assert!(
        body.iter()
            .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == buf)),
        "Set<str> buffer must receive a last-use freeing BurdenDec; body={body:?}",
    );
    assert_eq!(
        burden_net_for(&func, buf),
        0,
        "VF-1: set buffer Inc/Dec must net to 0; body={body:?}",
    );
}

#[test]
fn unregistered_collection_buffer_emits_no_burden_ops() {
    // Negative pin: WITHOUT a registered collection burden, the buffer
    // resolves no burden, fails `burden_carries_rc`, and receives ZERO burden
    // ops (the pre-fix no-emission path — VF-1=0 vacuously). Proves the Inc/Dec
    // emergence above is driven by the registered side-table burden, not by
    // unconditional Construct emission.
    let registry = TypeRegistry::new();
    let list_idx = Idx::from_raw(303);
    let mut func = collection_buffer_then_borrow_func(list_idx, CtorKind::ListLiteral);
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let buf = ArcVarId::new(0);
    let body = &func.blocks[0].body;
    let has_any_burden = body.iter().any(|i| {
        matches!(
            i,
            ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } if *var == buf
        )
    });
    assert!(
        !has_any_burden,
        "unregistered [str] buffer must receive NO burden ops (no resolved burden); body={body:?}",
    );
}

#[test]
fn scalar_literal_var_typed_as_heap_emits_no_burden_dec() {
    // Semantic pin: a var declared as a heap-burden type (`Idx::STR`) but
    // DEFINED by a scalar `Literal(Int(0))` is a scalar sentinel carrying NO
    // RC burden (`Spec: Annex E §AIMS L-9`; TF-1 `Let { Literal } -> SCALAR`).
    // The `__iter_next` element-type-marker scratch slot has exactly this
    // type/value-grain mismatch: typed as the iterator Item (heap aggregate),
    // valued `Int(0)` (the LLVM emitter reads the declared type to size the
    // out-buffer; the runtime value is an unused zero sentinel). The inc side
    // (`fresh_site_burden_inc_dst`) emits NO BurdenInc for a scalar literal, so
    // the dec side MUST emit no BurdenDec either — else the marker carries an
    // unbalanced dec that lowers to an `extract_value`-on-`i64 0` codegen crash
    // under `ORI_DISABLE_PREDICATE_STACK_RC=1`. Fail-before/pass-after: pre-fix
    // `collect_owned_burdens` keyed membership on the declared type and emitted
    // a `BurdenDec` on var(0).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::BOOL],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                // var(0): declared STR (heap-burden type) but DEFINED Int(0)
                // (scalar sentinel — the iterator element-type-marker shape).
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::Int(0)),
                },
                // Borrow-use of the marker (mirrors the `__iter_next` call
                // taking the marker [borrow]); IsShared is non-owned-position,
                // so var(0)'s last use is non-transferring.
                ArcInstr::IsShared {
                    dst: ArcVarId::new(1),
                    var: ArcVarId::new(0),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let marker = ArcVarId::new(0);
    let body = &func.blocks[0].body;
    let has_burden = body.iter().any(|i| {
        matches!(
            i,
            ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } if *var == marker
        )
    });
    assert!(
        !has_burden,
        "scalar-Literal(Int) var typed as heap MUST receive NO burden ops \
         (L-9 scalar sentinel; INC/DEC symmetry with fresh_site_burden_inc_dst); body={body:?}",
    );
}

#[test]
fn string_literal_var_still_emits_burden_dec() {
    // Negative pin: the scalar-`Literal` exclusion is `String`-EXEMPT — a heap
    // `Let { Literal(String) }` var DOES still receive its burden ops (heap str
    // bodies carry RC; the inc side emits a paired BurdenInc for `String`).
    // Guards against an over-broad exclusion that would skip ALL `Literal` vars
    // and silently drop real str RC. Same shape as the semantic pin but with a
    // `String` literal instead of `Int` — the only-passing-with-correct-semantics
    // companion that distinguishes the scalar-only exclusion from a blanket one.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::BOOL],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                },
                ArcInstr::IsShared {
                    dst: ArcVarId::new(1),
                    var: ArcVarId::new(0),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let strvar = ArcVarId::new(0);
    let body = &func.blocks[0].body;
    let dec_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == strvar))
        .count();
    assert_eq!(
        dec_count, 1,
        "heap str `Let {{ Literal(String) }}` var MUST still emit its BurdenDec \
         (scalar-Literal exclusion is String-exempt); body={body:?}",
    );
}

// iterator-from-collection + collection-element standalone ledger

/// Build the `for x in xs do x.len()` post-burden fixture: `%0` `[str]` coll,
/// `%1 = iter(%0)`, `%2 = __iter_next(%1, marker %3)`, `%4 = Project %2.1`
/// (str element view), `Invoke @len(%4 [borrow])`. Shared by the projection pin.
fn iter_next_projection_func(interner: &ori_ir::StringInterner) -> ArcFunction {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let iter_next = interner.intern(ProtocolBuiltin::IterNext.name());
    let iter_fn = interner.intern(ProtocolBuiltin::Iter.name());
    ArcFunction {
        var_types: vec![
            Idx::from_raw(50),
            Idx::INT,
            Idx::INT,
            Idx::STR,
            Idx::STR,
            Idx::INT,
        ],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: ArcVarId::new(0),
                        ty: Idx::from_raw(50),
                        ctor: CtorKind::ListLiteral,
                        args: Vec::new(),
                    },
                    ArcInstr::Apply {
                        dst: ArcVarId::new(1),
                        ty: Idx::INT,
                        func: iter_fn,
                        args: vec![ArcVarId::new(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                    ArcInstr::Apply {
                        dst: ArcVarId::new(2),
                        ty: Idx::INT,
                        func: iter_next,
                        args: vec![ArcVarId::new(1), ArcVarId::new(3)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                        mono_instance_id: None,
                    },
                    ArcInstr::Project {
                        dst: ArcVarId::new(4),
                        ty: Idx::STR,
                        value: ArcVarId::new(2),
                        field: 1,
                    },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(5),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(4)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

/// Inverse case — iterator-element borrow-views projected from an
/// `Apply @__iter_next` result MUST receive zero `BurdenDec`. The yielded
/// element (`Project { field: 1 }` of the `__iter_next` result) is a BORROWED
/// view into the collection buffer per `Spec: Annex E §AIMS Protocol Builtins`
/// (`IterNext` Owned iterator + Borrowed element-type marker); the element is
/// owned by the collection and freed by `elem_dec_fn` when the iterator handle
/// drops via `ori_iter_drop`. Emitting a last-use `BurdenDec` on the element
/// view is a double-free in the standalone ledger (the `fat_ptr_iter` failure
/// shape under `ORI_DISABLE_PREDICATE_STACK_RC=1`).
///
/// Shape mirrors the post-burden IR of `for x in xs do x.len()`:
///   %0: [str] collection. %1 = Apply @iter(%0). %2 = Apply `@__iter_next(%1, %m)`
///   (%m = element-type marker). %3 = Project %2.1 (str element view).
///   Invoke @len(%3 [borrow]) — %3 borrowed, not transferred.
/// Negative pin (revert): if `collect_iter_element_defs` exclusion is removed,
/// %3 (declared `str`, RcPtr-burden) receives a last-use `BurdenDec` → the
/// standalone-ledger double-free this section fixes.
#[test]
fn iter_next_element_projection_gets_no_burden_dec() {
    let interner = ori_ir::StringInterner::new();
    let registry = TypeRegistry::new();
    let mut func = iter_next_projection_func(&interner);
    emit_burden_ops_with_interner(
        &mut func,
        &registry,
        &[],
        &[],
        &FxHashMap::default(),
        true,
        &interner,
    );
    // Semantic pin: the element view (%4) projected from __iter_next.1 receives
    // ZERO BurdenDec — it is a borrowed collection-buffer view (elem_dec_fn owns
    // its release). Negative direction: reverting the exclusion emits a dec here
    // → standalone-ledger double-free.
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(4)),
        0,
        "iterator element view (Project __iter_next.1) MUST receive zero \
         BurdenDec under the standalone ledger (borrowed buffer view; \
         elem_dec_fn owns release — a dec double-frees); body={:?}",
        func.blocks[0].body,
    );
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(4)),
        0,
        "iterator element view MUST receive zero BurdenInc; body={:?}",
        func.blocks[0].body,
    );
}

/// Collection-element scope cell — a Let-Var alias of the iterator
/// element view also receives no `BurdenDec`. Mirrors `for (k, v) in m` and the
/// `x = elem` rebinding shapes the `elem_dec_scope` / `collections_ext` clusters
/// exercise: the element view flows through `Let { Var }` aliases (and block
/// params), each of which inherits the borrowed-buffer-view classification via
/// `collect_iter_element_defs`'s transitive closure. A dec on any alias of the
/// element view is the same double-free.
#[test]
fn iter_next_element_let_alias_gets_no_burden_dec() {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let registry = TypeRegistry::new();
    let interner = ori_ir::StringInterner::new();
    let iter_next = interner.intern(ProtocolBuiltin::IterNext.name());
    // %0 __iter_next result (scalar), %1 marker (str), %2 element view (str),
    // %3 Let-Var alias of %2 (str), %4 len result (int).
    let mut func = ArcFunction {
        var_types: vec![Idx::INT, Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Apply {
                        dst: ArcVarId::new(0),
                        ty: Idx::INT,
                        func: iter_next,
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                        mono_instance_id: None,
                    },
                    ArcInstr::Project {
                        dst: ArcVarId::new(2),
                        ty: Idx::STR,
                        value: ArcVarId::new(0),
                        field: 1,
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(3),
                        ty: Idx::STR,
                        value: ArcValue::Var(ArcVarId::new(2)),
                    },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(4),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(3)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops_with_interner(
        &mut func,
        &registry,
        &[],
        &[],
        &FxHashMap::default(),
        true,
        &interner,
    );
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(2)),
        0,
        "iterator element view MUST receive zero BurdenDec; body={:?}",
        func.blocks[0].body,
    );
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(3)),
        0,
        "Let-Var alias of an iterator element view MUST receive zero BurdenDec \
         (transitive borrowed-buffer-view classification); body={:?}",
        func.blocks[0].body,
    );
}

// concat dual-consuming operand: no spurious scope-exit BurdenDec

/// Semantic pin: a list-concat `Let { PrimOp Binary(Add) }` consumes BOTH
/// `RcPointer` operands (`ori_list_concat_cow` dec/frees them — list + list → COW
/// concat). The Phase-5 burden walk MUST NOT emit
/// a scope-exit `BurdenDec` on a concat operand whose last-use is the concat —
/// the helper's internal consume IS the release; a paired `BurdenDec` double-frees
/// the buffer (the `coll_list_cow_concat_shared` SIGSEGV under the probe). Per
/// AIMS RL-2 (concat operand = ownership-transfer position).
#[test]
fn list_concat_operand_at_last_use_gets_no_burden_dec() {
    let mut registry = TypeRegistry::new();
    let list_idx = Idx::from_raw(320);
    register_list_burden(&mut registry, list_idx, Idx::INT);
    // %0/%1 fresh lists, %2 = %0 + %1 (concat consumes both), %3 = %2 kept live;
    // %0/%1 last-use is the concat. var_reprs := RcPointer (the list discriminator
    // `list_concat_consumed_operands` reads; not populated by `emit_burden_ops`).
    let mut func = ArcFunction {
        var_types: vec![list_idx, list_idx, list_idx, list_idx],
        var_reprs: vec![ValueRepr::RcPointer; 4],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: list_idx,
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: list_idx,
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: list_idx,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: list_idx,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        0,
        "concat LHS operand (consumed by ori_list_concat_cow) MUST receive ZERO \
         scope-exit BurdenDec; body={body:?}",
    );
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(1)),
        0,
        "concat RHS operand (consumed by ori_list_concat_cow) MUST receive ZERO \
         scope-exit BurdenDec; body={body:?}",
    );
}

/// Negative pin: a `str` concat (`var_repr == FatValue`) is NOT consuming —
/// `ori_str_concat` takes `*const OriStr` (BORROWED) and the caller RC-decs after.
/// The list-concat consume suppression MUST NOT fire for a str operand: a heap
/// str operand whose last-use is the concat STILL receives its scope-exit
/// `BurdenDec` (distinguishes the list `RcPointer` consume from the str
/// `FatValue` borrow — guards against an over-broad `Binary(Add)` rule).
#[test]
fn str_concat_operand_still_gets_burden_dec() {
    let registry = TypeRegistry::new();
    // %0/%1 fresh str, %2 = %0 + %1 (str concat — borrowed), %3 = %2 kept live;
    // var_reprs := FatValue (str), the non-list discriminator.
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        var_reprs: vec![ValueRepr::FatValue; 4],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(2))),
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::STR,
                    value: ArcValue::Var(ArcVarId::new(2)),
                },
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        1,
        "str concat operand (BORROWED by ori_str_concat) MUST still receive its \
         scope-exit BurdenDec (list-consume suppression is FatValue-exempt); body={body:?}",
    );
}

// A' COW-inc on borrowed-param alias (step 1)

/// A' step-1 pin: a borrowed-param alias (`%1 = %0`, `%0: [int] [borrow]`)
/// consumed as the RECEIVER (arg 0) of a COW-MUTATOR (`push`) at an OWNED
/// `Invoke`-terminator position receives a `BurdenInc` under the probe
/// (`predicate_stack_rc_disabled = true`). Per AIMS RL-1: the COW-mutation
/// re-reads the receiver's refcount, so the borrowed-alias use is a DUPLICATING
/// use whose inc is NOT elidable — the inc raises rc ≥ 2 so the COW helper
/// COPIES vs corrupting the caller's still-live value
/// (`AimsProof.Realization::RL1_emit_iff_not_elidable`). Semantic pin: would
/// FAIL if `compute_cow_inc_borrowed_aliases` is reverted (the original `push`
/// fixture leaks/corrupts under the standalone ledger).
#[test]
fn cow_mutator_borrowed_alias_receiver_emits_inc_under_probe() {
    let registry = TypeRegistry::new();
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("push");
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::from_raw(100),
            ownership: Ownership::Borrowed,
        }],
        var_types: vec![
            Idx::from_raw(100),
            Idx::from_raw(100),
            Idx::INT,
            Idx::from_raw(100),
        ],
        var_reprs: vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::from_raw(100),
                        value: ArcValue::Var(ArcVarId::new(0)),
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(2),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(99)),
                    },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(3),
                    ty: Idx::from_raw(100),
                    func: push,
                    args: vec![ArcVarId::new(1), ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops_with_interner(
        &mut func,
        &registry,
        &[],
        &[],
        &FxHashMap::default(),
        true, // probe path
        &interner,
    );
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(1)),
        1,
        "borrowed-alias COW-mutator receiver MUST get exactly one step-1 BurdenInc under the probe; body={:?}",
        func.blocks[0].body,
    );
    // burden_emitted must be set so step-2 edge release (carries_burden gate) fires.
    assert!(
        func.burden_emitted
            .get(ArcVarId::new(1).index())
            .copied()
            .unwrap_or(false),
        "COW-inc'd var MUST be marked in burden_emitted so step-2 edge cleanup releases it",
    );
}

/// A' negative pin: the SAME borrowed-param alias consumed at a BORROWED
/// position (`@len` — read-only) receives NO step-1 `BurdenInc`. Read-only
/// borrows do not duplicate (no COW realloc), so the borrowed-alias exclusion
/// (checkbox-1) is preserved. Would FAIL if the discriminator dropped the
/// COW-method-name gate and fired on any owned-or-borrowed `RcPtr` position.
#[test]
fn borrowed_alias_at_read_only_position_emits_no_cow_inc() {
    let registry = TypeRegistry::new();
    let interner = ori_ir::StringInterner::new();
    let len = interner.intern("len");
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::from_raw(100),
            ownership: Ownership::Borrowed,
        }],
        var_types: vec![Idx::from_raw(100), Idx::from_raw(100), Idx::INT],
        var_reprs: vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::from_raw(100),
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    func: len,
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops_with_interner(
        &mut func,
        &registry,
        &[],
        &[],
        &FxHashMap::default(),
        true, // probe path
        &interner,
    );
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(1)),
        0,
        "borrowed-alias at a READ-ONLY (@len, borrowed) position MUST get zero COW-inc (preserves borrowed-alias exclusion); body={:?}",
        func.blocks[0].body,
    );
}

/// A' default-path pin: the COW-inc is PROBE-ONLY. On the default path
/// (`predicate_stack_rc_disabled = false`) the predicate stack emits the
/// equivalent `RcInc`, so the burden walk emits NO COW-inc — keeping default
/// AOT codegen byte-identical. Would FAIL if `compute_cow_inc_borrowed_aliases`
/// were not gated on the probe flag.
#[test]
fn cow_inc_is_probe_only_no_inc_on_default_path() {
    let registry = TypeRegistry::new();
    let interner = ori_ir::StringInterner::new();
    let push = interner.intern("push");
    let mut func = ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::from_raw(100),
            ownership: Ownership::Borrowed,
        }],
        var_types: vec![
            Idx::from_raw(100),
            Idx::from_raw(100),
            Idx::INT,
            Idx::from_raw(100),
        ],
        var_reprs: vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
            ValueRepr::RcPointer,
        ],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::from_raw(100),
                        value: ArcValue::Var(ArcVarId::new(0)),
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(2),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(99)),
                    },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(3),
                    ty: Idx::from_raw(100),
                    func: push,
                    args: vec![ArcVarId::new(1), ArcVarId::new(2)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    emit_burden_ops_with_interner(
        &mut func,
        &registry,
        &[],
        &[],
        &FxHashMap::default(),
        false, // default path
        &interner,
    );
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(1)),
        0,
        "COW-inc MUST be probe-only — zero BurdenInc on the default path (predicate stack emits the RcInc); body={:?}",
        func.blocks[0].body,
    );
}

/// Joint-fixpoint pin: a NESTED projection of an iter-element-view that
/// reaches the view ONLY through a `Let`-alias hop MUST receive zero `BurdenDec`.
///
/// Shape mirrors `for item in items yield match item { Some(s) -> s.length() }`:
///   `%0: [str?]` source. `%1 = @iter(%0)`. `%2 = @__iter_next(%1, %m)`.
///   `%4 = Project %2.1` (the `str?` compound element view — a borrow).
///   `%5 = Let %4` (alias of the compound view).
///   `%6 = Project %5.1` (the INTERIOR `str` payload, reached through the alias).
///   `Invoke @length(%6 [borrow])` — %6 borrowed, not transferred.
///
/// `collect_iter_element_defs` runs the Project-chain and Let-alias propagations
/// to a SINGLE fixpoint: %4 is a direct iter-element-view (`Project __iter_next.1`),
/// %5 is its Let-alias, and %6 is a Project of %5. A Project-chain pass that
/// completes BEFORE the Let-alias pass would miss %6 (its source %5 not yet in the
/// set), emitting a spurious `BurdenDec %6` -> a double-free of the interior str
/// (the source's `elem_dec_fn` already frees it). Negative direction (revert): if
/// the two propagations run sequentially instead of jointly, %6 receives a dec.
fn nested_let_aliased_iter_element_projection_func(
    interner: &ori_ir::StringInterner,
) -> ArcFunction {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let iter_next = interner.intern(ProtocolBuiltin::IterNext.name());
    let iter_fn = interner.intern(ProtocolBuiltin::Iter.name());
    // var_types: %0 [str?] list (Idx 50), %1 int handle, %2 int next-result,
    // %3 marker (str?), %4 str? element view, %5 str? alias, %6 str payload,
    // %7 int @length result.
    ArcFunction {
        var_types: vec![
            Idx::from_raw(50),
            Idx::INT,
            Idx::INT,
            Idx::from_raw(51),
            Idx::from_raw(51),
            Idx::from_raw(51),
            Idx::STR,
            Idx::INT,
        ],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: ArcVarId::new(0),
                        ty: Idx::from_raw(50),
                        ctor: CtorKind::ListLiteral,
                        args: Vec::new(),
                    },
                    ArcInstr::Apply {
                        dst: ArcVarId::new(1),
                        ty: Idx::INT,
                        func: iter_fn,
                        args: vec![ArcVarId::new(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                    ArcInstr::Apply {
                        dst: ArcVarId::new(2),
                        ty: Idx::INT,
                        func: iter_next,
                        args: vec![ArcVarId::new(1), ArcVarId::new(3)],
                        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Borrowed],
                        mono_instance_id: None,
                    },
                    // %4 = Project %2.1 — the compound `str?` element view.
                    ArcInstr::Project {
                        dst: ArcVarId::new(4),
                        ty: Idx::from_raw(51),
                        value: ArcVarId::new(2),
                        field: 1,
                    },
                    // %5 = Let %4 — Let-alias of the compound view (this hop is
                    // what a sequential Project-then-Let pass would not bridge).
                    ArcInstr::Let {
                        dst: ArcVarId::new(5),
                        ty: Idx::from_raw(51),
                        value: crate::ir::ArcValue::Var(ArcVarId::new(4)),
                    },
                    // %6 = Project %5.1 — the INTERIOR str payload through %5.
                    ArcInstr::Project {
                        dst: ArcVarId::new(6),
                        ty: Idx::STR,
                        value: ArcVarId::new(5),
                        field: 1,
                    },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: ArcVarId::new(7),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![ArcVarId::new(6)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                    mono_instance_id: None,
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(7),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn nested_projection_through_let_alias_of_iter_element_view_gets_no_burden_dec() {
    let interner = ori_ir::StringInterner::new();
    let registry = TypeRegistry::new();
    let mut func = nested_let_aliased_iter_element_projection_func(&interner);
    emit_burden_ops_with_interner(
        &mut func,
        &registry,
        &[],
        &[],
        &FxHashMap::default(),
        true,
        &interner,
    );
    // The interior str payload (%6), reached through the Let-aliased compound
    // element view, MUST receive zero BurdenDec — it is a borrow into the
    // collection buffer (elem_dec_fn owns its release).
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(6)),
        0,
        "interior projection of a Let-aliased iter-element-view MUST receive \
         zero BurdenDec (joint Project+Let fixpoint); a dec double-frees the \
         interior str; body={:?}",
        func.blocks[0].body,
    );
    // And the compound view (%4) + its alias (%5) likewise.
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(4)) + count_burden_decs(&func, ArcVarId::new(5)),
        0,
        "compound iter-element-view + its Let-alias MUST receive zero BurdenDec; \
         body={:?}",
        func.blocks[0].body,
    );
}

// RL-1 transfer-through-return result-inc elision matrix (M-a forwarder).
//
// A callee that returns its owned param unchanged (`@id<T>(x: T) -> T = x`)
// transfers the SAME allocation back to the caller — the result is NOT a fresh
// value, so its FRESH-site result-`BurdenInc` is ELIDED per AIMS RL-1
// (`RL1_emit_iff_not_elidable`). The caller already owns the reference it
// transferred IN at the owned arg position; emitting a result-inc double-counts
// the transferred-in allocation under sole-emitter Phase-7 lowering (net +1
// LEAK). SSOT: `compute_transfer_through_return_results`.
//
// Over-fire boundary: the repr gate admits only `RcPointer` / `FatValue`
// results (a single directly-RC-managed reference read via borrows). An
// `Aggregate` result (a forwarded struct/sum whose inner heap FIELDS are
// projected and independently dec'd) is EXCLUDED — its result-inc keeps the
// inner buffer alive across projection paths.

/// Build a single-param `MemoryContract` whose param has
/// `transfers_through_return == true` (`return_alias == Some(Direct)`), modeling
/// a forwarder `@id(x) = x`. The remaining dimensions are conservative.
fn forwarder_contract() -> crate::aims::contract::MemoryContract {
    use crate::aims::contract::{MemoryContract, ParamContract, ReturnAliasShape};
    let mut param = ParamContract::CONSERVATIVE;
    param.transfers_through_return = true;
    param.return_alias = Some(ReturnAliasShape::Direct);
    MemoryContract {
        params: vec![param],
        ..MemoryContract::conservative(1)
    }
}

/// Positive pin: an `Apply` result of an `RcPointer` type whose callee is a
/// transfer-through-return forwarder receives ZERO result-`BurdenInc` (RL-1
/// elision). Reverting the elision re-introduces the forwarder LEAK
/// (alloc(+1) + spurious result-inc − path decs = net +1).
#[test]
fn rcptr_forwarder_result_gets_no_result_burden_inc() {
    let registry = TypeRegistry::new();
    let callee = Name::from_raw(100);
    // %0: FRESH list (Construct). %1: Apply @forwarder(%0 [own]) -> %1 aliases %0.
    // %1 read once via a borrow (Project) then dropped.
    let mut func = ArcFunction {
        var_types: vec![Idx::from_raw(50), Idx::from_raw(50)],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::RcPointer],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: Idx::from_raw(50),
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(1),
                    ty: Idx::from_raw(50),
                    func: callee,
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let mut contracts = FxHashMap::default();
    contracts.insert(callee, forwarder_contract());
    emit_burden_ops(&mut func, &registry, &[], &[], &contracts, false);
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(1)),
        0,
        "RL-1: RcPointer forwarder result var(1) MUST receive zero result-BurdenInc \
         (transfer-through-return alias of the transferred-in arg, not fresh); \
         body={:?}",
        func.blocks[0].body,
    );
}

/// Negative pin (over-fire boundary): an `Aggregate` result (a forwarded
/// struct whose inner heap fields are projected) is NOT a `RcPointer`, so the
/// repr gate does NOT suppress its result handling — eliding the inc here would
/// double-free the inner field across projection paths. The result var is NOT
/// in `transfer_through_return_results`, so its normal fresh-site handling
/// applies (≥1 result-inc or owned-position handling). This pins the repr gate.
#[test]
fn aggregate_forwarder_result_inc_not_suppressed() {
    let registry = TypeRegistry::new();
    let callee = Name::from_raw(101);
    // %0: FRESH list. %1: Box struct wrapping %0. %2: Apply @forwarder(%1) -> Box.
    let box_ty = Idx::from_raw(60);
    let func = ArcFunction {
        var_types: vec![Idx::from_raw(50), box_ty, box_ty],
        var_reprs: vec![
            ValueRepr::RcPointer,
            ValueRepr::Aggregate,
            ValueRepr::Aggregate,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Construct {
                    dst: ArcVarId::new(0),
                    ty: Idx::from_raw(50),
                    ctor: CtorKind::ListLiteral,
                    args: Vec::new(),
                },
                ArcInstr::Construct {
                    dst: ArcVarId::new(1),
                    ty: box_ty,
                    ctor: CtorKind::Struct(Name::from_raw(5)),
                    args: vec![ArcVarId::new(0)],
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: box_ty,
                    func: callee,
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let mut contracts = FxHashMap::default();
    contracts.insert(callee, forwarder_contract());
    let mut ctx_func = func.clone();
    emit_burden_ops(&mut ctx_func, &registry, &[], &[], &contracts, false);
    // The Aggregate result var(2) is EXCLUDED from the RcPointer/FatValue
    // transfer-through-return gate — verify by checking the SSOT directly: the
    // result set does not contain it (so its normal handling is unaltered).
    let gate = super::ownership_scans::compute_transfer_through_return_results(&func, &contracts);
    assert!(
        !gate.contains(&ArcVarId::new(2)),
        "repr gate: Aggregate forwarder result var(2) MUST be EXCLUDED from \
         transfer-through-return result-inc elision (inner-field projections \
         need the result-inc); gate={gate:?}",
    );
    // And an RcPointer result of the SAME forwarder IS in the gate (positive
    // contrast — proves the gate keys on repr, not on the contract alone).
    assert!(
        compute_transfer_through_return_results_gate_includes_rcptr(&contracts, callee),
        "repr gate: an RcPointer forwarder result MUST be admitted to the gate",
    );
}

/// Helper: build a minimal `RcPointer` forwarder func and assert its result is in
/// the gate. Keeps `aggregate_forwarder_result_inc_not_suppressed`'s positive
/// contrast self-contained.
fn compute_transfer_through_return_results_gate_includes_rcptr(
    contracts: &FxHashMap<Name, crate::aims::contract::MemoryContract>,
    callee: Name,
) -> bool {
    let func = ArcFunction {
        var_types: vec![Idx::from_raw(50), Idx::from_raw(50)],
        var_reprs: vec![ValueRepr::RcPointer, ValueRepr::RcPointer],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::from_raw(50),
                func: callee,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    super::ownership_scans::compute_transfer_through_return_results(&func, contracts)
        .contains(&ArcVarId::new(1))
}

/// Negative pin: a fresh-allocating callee (NO transfer-through-return — e.g.
/// `@mk() -> [int]`) KEEPS its result-inc. The result is a genuinely fresh
/// allocation; eliding its inc would leak it across downstream borrow-reads.
/// The result `%0` is read at a BORROW position (`@rd(%0 [borrow])`) so it is
/// NOT transferred out — its fresh-site inc is the expected balanced pair with
/// its scope-exit dec.
#[test]
fn fresh_result_keeps_result_burden_inc() {
    // `@mk()` is a fresh-allocating callee with NO `transfers_through_return`
    // param. Its RcPointer result (%0) is EXCLUDED from the gate — so the result
    // -inc elision never fires and a genuine fresh allocation keeps its inc. SSOT
    // contrast against the positive pin
    // `rcptr_forwarder_result_gets_no_result_burden_inc`: same RcPointer repr,
    // only a forwarder-aliased result is admitted.
    let callee = Name::from_raw(102);
    let func = ArcFunction {
        var_types: vec![Idx::from_raw(50)],
        var_reprs: vec![ValueRepr::RcPointer],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(0),
                ty: Idx::from_raw(50),
                func: callee,
                args: Vec::new(),
                arg_ownership: Vec::new(),
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    // Conservative contract: NO param with transfers_through_return.
    let mut contracts = FxHashMap::default();
    contracts.insert(
        callee,
        crate::aims::contract::MemoryContract::conservative(0),
    );
    let gate = super::ownership_scans::compute_transfer_through_return_results(&func, &contracts);
    assert!(
        !gate.contains(&ArcVarId::new(0)),
        "fresh-allocating callee result var(0) MUST NOT be in the \
         transfer-through-return gate (no forwarder param); gate={gate:?}",
    );
}

// Forwarder-identity alias transparency
// (`compute_forwarder_identity_transparent_aliases`)
//
// A `Let { Var(src) }` alias of an Owned param that transfers through the
// return (own contract) with a read-only-or-move-out lineage is a
// same-allocation view of the moved-through value — transparent, NOT an RL-1
// duplication. All-or-nothing per src: one non-vetted use anywhere in the
// lineage keeps every alias classified.

/// Multi-use-then-return forwarder shape over an Owned ttr param:
/// bb0: %1 = %0; %2 = Project %1.0; Branch %2 ? bb1 : bb2
/// bb1: %3 = %0; Jump bb3(%3)
/// bb2: %4 = %0; Jump bb3(%4)
/// bb3(%5): Return %5
fn multi_use_forwarder_func() -> ArcFunction {
    ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::from_raw(50),
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::from_raw(50); 6],
        var_reprs: vec![ValueRepr::RcPointer; 6],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::from_raw(50),
                        value: ArcValue::Var(ArcVarId::new(0)),
                    },
                    project_first(ArcVarId::new(2), Idx::from_raw(50), ArcVarId::new(1)),
                ],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(2),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::from_raw(50),
                    value: ArcValue::Var(ArcVarId::new(0)),
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: vec![ArcVarId::new(3)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(4),
                    ty: Idx::from_raw(50),
                    value: ArcValue::Var(ArcVarId::new(0)),
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: vec![ArcVarId::new(4)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: vec![(ArcVarId::new(5), Idx::from_raw(50))],
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

/// Own-contract map: this function's single param transfers through return.
fn own_forwarder_contracts(
    func_name: Name,
) -> FxHashMap<Name, crate::aims::contract::MemoryContract> {
    let mut contracts = FxHashMap::default();
    contracts.insert(func_name, forwarder_contract());
    contracts
}

#[test]
fn ttr_param_branch_aliases_with_borrow_read_are_transparent() {
    // Semantic pin: would FAIL if the forwarder-identity transparency is
    // reverted — the branch aliases regain dup-alias pairs whose per-var
    // DP-2/DP-3 split over-releases the moved-through allocation.
    let func = multi_use_forwarder_func();
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    let mut got: Vec<u32> = set
        .iter()
        .map(|v| u32::try_from(v.index()).unwrap_or(u32::MAX))
        .collect();
    got.sort_unstable();
    assert_eq!(
        got,
        vec![1, 3, 4],
        "every read-only / move-out alias of the ttr param MUST be transparent",
    );
}

/// A ttr Owned param with `%1 = %0` iter-consumed by `@iter(%1 [own])` and the
/// param returned at `%0`. `%1` is a genuine RL-1 duplication needing its inc
/// kept (the iterator frees the dup; the param's original transfers via Return).
fn ttr_iter_consume_func(interner: &ori_ir::StringInterner) -> ArcFunction {
    use ori_ir::builtin_constants::protocol::ProtocolBuiltin;
    let iter_fn = interner.intern(ProtocolBuiltin::Iter.name());
    ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::from_raw(50),
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::from_raw(50), Idx::from_raw(50), Idx::INT],
        var_reprs: vec![
            ValueRepr::RcPointer,
            ValueRepr::RcPointer,
            ValueRepr::Scalar,
        ],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::from_raw(50),
                    value: ArcValue::Var(ArcVarId::new(0)),
                },
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    func: iter_fn,
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(77),
        ..ArcFunction::default()
    }
}

#[test]
fn ttr_iter_consume_alias_admits_dup_inc() {
    // The `%1 = %0` alias iter-consumed while `%0` is returned MUST be admitted
    // — its kept inc is the duplicate the iterator frees (RL-1 duplication).
    let interner = ori_ir::StringInterner::new();
    let func = ttr_iter_consume_func(&interner);
    let set = super::ownership_scans::compute_ttr_iter_consume_dup_aliases(
        &func,
        &own_forwarder_contracts(func.name),
        &interner,
    );
    assert!(
        set.contains(&ArcVarId::new(1)),
        "iter-consumed alias of a ttr param MUST keep its dup inc; got={set:?}",
    );
}

#[test]
fn iter_consume_alias_of_non_ttr_param_declines() {
    // Over-fire guard: the SAME shape but the param does NOT transfer through
    // the return (no ttr contract) — the param dies at the iter-consume (single
    // transfer), so the dup inc MUST NOT fire (an extra inc leaks).
    let interner = ori_ir::StringInterner::new();
    let func = ttr_iter_consume_func(&interner);
    // Conservative (non-ttr) contract for the function's own name.
    let mut contracts = FxHashMap::default();
    contracts.insert(
        func.name,
        crate::aims::contract::MemoryContract::conservative(1),
    );
    let set =
        super::ownership_scans::compute_ttr_iter_consume_dup_aliases(&func, &contracts, &interner);
    assert!(
        set.is_empty(),
        "iter-consumed alias of a NON-ttr param MUST decline (single transfer); got={set:?}",
    );
}

#[test]
fn alias_consumed_at_owned_position_keeps_whole_lineage_classified() {
    // Negative over-fire pin: a GENUINE duplication (alias consumed at an
    // owned Construct position while the lineage stays live) NEEDS its paired
    // inc/dec — the gate must decline ALL aliases of the src (all-or-nothing).
    let mut func = multi_use_forwarder_func();
    func.blocks[0].body.push(ArcInstr::Construct {
        dst: ArcVarId::new(2),
        ty: Idx::from_raw(50),
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(1)],
    });
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    assert!(
        set.is_empty(),
        "an owned-position consume of ANY alias MUST decline the whole lineage; got={set:?}",
    );
}

#[test]
fn alias_jump_to_dead_block_param_keeps_lineage_classified() {
    // The RL-5 dead-param release machinery owns the dead-successor shape; the
    // transparency gate declines so the status quo is preserved there.
    let mut func = multi_use_forwarder_func();
    func.blocks[3].terminator = ArcTerminator::Unreachable;
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    assert!(
        set.is_empty(),
        "a Jump into a DEAD successor param MUST decline the lineage; got={set:?}",
    );
}

#[test]
fn conservative_own_contract_yields_no_transparent_aliases() {
    // Without the proven transfers_through_return fact the aliases stay
    // classified (conservative status quo).
    let func = multi_use_forwarder_func();
    let mut contracts = FxHashMap::default();
    contracts.insert(
        func.name,
        crate::aims::contract::MemoryContract::conservative(1),
    );
    let set =
        super::ownership_scans::compute_forwarder_identity_transparent_aliases(&func, &contracts);
    assert!(
        set.is_empty(),
        "no own-ttr contract fact MUST mean no transparency; got={set:?}",
    );
}

#[test]
fn borrowed_ttr_param_yields_no_transparent_aliases() {
    // A Borrowed param carries no RC obligation — its aliases are excluded by
    // the borrowed-alias retain, never by this gate.
    let mut func = multi_use_forwarder_func();
    func.params[0].ownership = Ownership::Borrowed;
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    assert!(
        set.is_empty(),
        "a Borrowed param MUST yield no forwarder-identity transparency; got={set:?}",
    );
}

#[test]
fn multi_hop_read_only_re_alias_stays_transparent() {
    // Multi-hop: a read-only nested re-alias of an alias (`%6 = %3` where
    // `%3 = %0` aliases the ttr param) joins the SAME-allocation lineage and is
    // transparent — the whole chain `%0 -> %3 -> %6` is one allocation moving
    // through the Return. `collect_ttr_param_aliases` folds the transitive chain
    // so the deepest alias carries no spurious burden ops (the loop-body-alias
    // double-free cure).
    let mut func = multi_use_forwarder_func();
    func.var_types.push(Idx::from_raw(50));
    func.var_reprs.push(ValueRepr::RcPointer);
    func.blocks[1].body.push(ArcInstr::Let {
        dst: ArcVarId::new(6),
        ty: Idx::from_raw(50),
        value: ArcValue::Var(ArcVarId::new(3)),
    });
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    assert!(
        set.contains(&ArcVarId::new(6)),
        "a read-only nested re-alias MUST be admitted (multi-hop transparent); got={set:?}",
    );
    // The whole lineage stays admitted: the original single-hop aliases too.
    for raw in [1u32, 3, 4] {
        assert!(
            set.contains(&ArcVarId::new(raw)),
            "lineage alias %{raw} MUST stay transparent with the nested hop; got={set:?}",
        );
    }
}

#[test]
fn nested_re_alias_owned_consume_declines_whole_lineage() {
    // Over-fire guard: a nested re-alias that ESCAPES to an owned-position
    // consume (`Construct(%6)`) declines the WHOLE multi-hop lineage — the
    // escape is a genuine duplication needing its own paired RC, not a
    // transparent read-only view.
    let mut func = multi_use_forwarder_func();
    func.var_types.push(Idx::from_raw(50));
    func.var_reprs.push(ValueRepr::RcPointer);
    func.blocks[1].body.push(ArcInstr::Let {
        dst: ArcVarId::new(6),
        ty: Idx::from_raw(50),
        value: ArcValue::Var(ArcVarId::new(3)),
    });
    func.blocks[1].body.push(ArcInstr::Construct {
        dst: ArcVarId::new(7),
        ty: Idx::from_raw(50),
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(6)],
    });
    func.var_types.push(Idx::from_raw(50));
    func.var_reprs.push(ValueRepr::RcPointer);
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    assert!(
        set.is_empty(),
        "a nested re-alias escaping to an owned consume MUST decline the whole \
         lineage; got={set:?}",
    );
}

#[test]
fn set_base_through_alias_keeps_lineage_classified() {
    // Mutation through the view invalidates the read-only premise.
    let mut func = multi_use_forwarder_func();
    func.blocks[0].body.push(ArcInstr::Set {
        base: ArcVarId::new(1),
        field: 0,
        value: ArcVarId::new(2),
    });
    let set = super::ownership_scans::compute_forwarder_identity_transparent_aliases(
        &func,
        &own_forwarder_contracts(func.name),
    );
    assert!(
        set.is_empty(),
        "a Set base through the alias MUST decline the lineage; got={set:?}",
    );
}

#[test]
fn transparent_alias_carries_no_burden_ops_end_to_end() {
    // Integration pin through emit_burden_ops: the transparent alias gets
    // NEITHER the dup-alias FRESH-site inc NOR any last-use dec — zero burden
    // ops on the alias; the lineage release stays with the caller of the
    // bound result per RL-34.
    let registry = TypeRegistry::new();
    let mut func = multi_use_forwarder_func();
    let contracts = own_forwarder_contracts(func.name);
    emit_burden_ops(&mut func, &registry, &[], &[], &contracts, false);
    for raw in [1u32, 3, 4] {
        assert_eq!(
            count_burden_incs(&func, ArcVarId::new(raw)),
            0,
            "transparent alias %{raw} MUST carry no BurdenInc",
        );
        assert_eq!(
            count_burden_decs(&func, ArcVarId::new(raw)),
            0,
            "transparent alias %{raw} MUST carry no BurdenDec",
        );
    }
}

// Genuine-duplication store-out alias scan
// (`compute_genuine_dup_move_aliases`)

/// `Let` alias of `%0` into `dst`, typed STR.
fn alias_of(dst: u32, src: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        value: ArcValue::Var(ArcVarId::new(src)),
    }
}

/// Aggregate store consuming `arg` into `dst` (a struct Construct).
fn store_of(dst: u32, arg: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(arg)],
    }
}

/// Two-block func: `bb0: %1 = %0; %3 = Construct(%1); Jump bb1` then `bb1`
/// uses `%0` — the source is forward-reachable past the store-out alias, so
/// the alias is a genuine RL-1 duplication.
fn dup_alias_src_used_in_successor_func() -> ArcFunction {
    ArcFunction {
        var_types: (0..5).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![alias_of(1, 0), store_of(3, 1)],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![alias_of(2, 0), store_of(4, 2)],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn genuine_dup_scan_fires_when_src_used_in_reachable_successor() {
    let func = dup_alias_src_used_in_successor_func();
    let dup_alias_dsts: FxHashSet<ArcVarId> =
        [ArcVarId::new(1), ArcVarId::new(2)].into_iter().collect();
    let set = super::ownership_scans::compute_genuine_dup_move_aliases(
        &func,
        &dup_alias_dsts,
        &FxHashSet::default(),
    );
    assert!(
        set.contains(&ArcVarId::new(1)),
        "bb0 store-out alias %1 of %0 with a reachable bb1 use of %0 is a genuine duplication"
    );
    assert!(
        !set.contains(&ArcVarId::new(2)),
        "bb1 alias %2 is %0's last reachable use — a terminal move, not a duplication"
    );
}

#[test]
fn genuine_dup_scan_fires_on_later_same_block_use() {
    // `%1 = %0; Construct(%1); %2 = %0; Construct(%2)` in ONE block: the
    // first alias has a later same-block use of the source (genuine); the
    // second is terminal.
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        store_of(3, 1),
        alias_of(2, 0),
        store_of(4, 2),
    ];
    let dup_alias_dsts: FxHashSet<ArcVarId> =
        [ArcVarId::new(1), ArcVarId::new(2)].into_iter().collect();
    let set = super::ownership_scans::compute_genuine_dup_move_aliases(
        &func,
        &dup_alias_dsts,
        &FxHashSet::default(),
    );
    assert!(
        set.contains(&ArcVarId::new(1)),
        "first store-out alias has a later same-block source use — genuine duplication"
    );
    assert!(
        !set.contains(&ArcVarId::new(2)),
        "second alias is the source's terminal use — a move"
    );
}

#[test]
fn genuine_dup_scan_excludes_branch_exclusive_aliases() {
    // `bb0: Branch -> bb1 | bb2`; each branch store-out aliases `%0` then
    // returns. The source has TWO alias uses but neither reaches the other —
    // each path's alias is the terminal move of the one reference (per-path
    // RL-2 transfer, no duplication).
    let func = ArcFunction {
        var_types: (0..6).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![alias_of(2, 0), store_of(4, 2)],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![alias_of(3, 0), store_of(5, 3)],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let dup_alias_dsts: FxHashSet<ArcVarId> =
        [ArcVarId::new(2), ArcVarId::new(3)].into_iter().collect();
    let set = super::ownership_scans::compute_genuine_dup_move_aliases(
        &func,
        &dup_alias_dsts,
        &FxHashSet::default(),
    );
    assert!(
        set.is_empty(),
        "branch-exclusive aliases are per-path terminal moves, not duplications: {set:?}"
    );
}

#[test]
fn genuine_dup_scan_fires_on_loop_back_edge_reuse() {
    // `bb0: %3 = %0 (earlier use); %2 = %0; Construct(%2); Branch -> bb0|bb1`
    // — the earlier-in-block use of the source is re-reachable through the
    // back edge, so the store-out alias IS a genuine duplication (the next
    // iteration reads the source again).
    let func = ArcFunction {
        var_types: (0..6).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![alias_of(3, 0), alias_of(2, 0), store_of(4, 2)],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(0),
                    else_block: ArcBlockId::new(1),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let dup_alias_dsts: FxHashSet<ArcVarId> = [ArcVarId::new(2)].into_iter().collect();
    let set = super::ownership_scans::compute_genuine_dup_move_aliases(
        &func,
        &dup_alias_dsts,
        &FxHashSet::default(),
    );
    assert!(
        set.contains(&ArcVarId::new(2)),
        "an earlier-in-block source use re-reached via the back edge is a use after the alias"
    );
}

#[test]
fn genuine_dup_scan_excludes_call_arg_consumers() {
    // `%1 = %0; Apply @f(%1); %2 = %0; Construct(%2)` — the FIRST alias moves
    // into a CALL arg (iter-protocol / interprocedural ownership accounting
    // owns it), so it is OUT of the store-out family even though the source
    // is live after; only the aggregate-store alias classifies.
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        ArcInstr::Apply {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            func: Name::from_raw(7),
            args: vec![ArcVarId::new(1)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
        alias_of(2, 0),
        store_of(4, 2),
    ];
    let dup_alias_dsts: FxHashSet<ArcVarId> =
        [ArcVarId::new(1), ArcVarId::new(2)].into_iter().collect();
    let set = super::ownership_scans::compute_genuine_dup_move_aliases(
        &func,
        &dup_alias_dsts,
        &FxHashSet::default(),
    );
    assert!(
        !set.contains(&ArcVarId::new(1)),
        "a call-arg consumer is outside the duplication-by-storage family"
    );
    assert!(
        !set.contains(&ArcVarId::new(2)),
        "the store alias is the source's terminal use here — a move"
    );
}

#[test]
fn genuine_dup_scan_respects_dup_and_full_move_gates() {
    // Same shape as the same-block genuine case, but (a) the dst is not a
    // dup-alias, or (b) the dst is a full-move var — neither classifies.
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        store_of(3, 1),
        alias_of(2, 0),
        store_of(4, 2),
    ];
    let empty: FxHashSet<ArcVarId> = FxHashSet::default();
    let not_dup = super::ownership_scans::compute_genuine_dup_move_aliases(&func, &empty, &empty);
    assert!(
        not_dup.is_empty(),
        "a dst outside dup_alias_dsts never classifies"
    );
    let dup_alias_dsts: FxHashSet<ArcVarId> = [ArcVarId::new(1)].into_iter().collect();
    let full_move: FxHashSet<ArcVarId> = [ArcVarId::new(1)].into_iter().collect();
    let full_moved = super::ownership_scans::compute_genuine_dup_move_aliases(
        &func,
        &dup_alias_dsts,
        &full_move,
    );
    assert!(
        full_moved.is_empty(),
        "a full-move dst is owned by the field-projection suppression, not the alias pair"
    );
}

// Borrowed-store duplication scan (`compute_borrowed_store_dup_args`)

/// One-block func with one param of `ownership`, typed `param_ty`, and the
/// given body. Var types: `%0..%n` all `param_ty`.
fn borrowed_store_func(
    ownership: Ownership,
    param_ty: Idx,
    n: u32,
    body: Vec<ArcInstr>,
) -> ArcFunction {
    ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: param_ty,
            ownership,
        }],
        var_types: (0..n).map(|_| param_ty).collect(),
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn borrowed_store_scan_fires_on_borrowed_param_and_alias_stores() {
    // `%1 = %0(borrowed); Construct(%2, [%1]); Construct(%3, [%0])` — both the
    // alias and the param itself are borrowed-rooted store args: each store
    // duplicates the caller's retained reference (RL-1).
    let registry = TypeRegistry::new();
    let func = borrowed_store_func(
        Ownership::Borrowed,
        Idx::STR,
        4,
        vec![alias_of(1, 0), store_of(2, 1), store_of(3, 0)],
    );
    let set = super::ownership_scans::compute_borrowed_store_dup_args(&func, &registry);
    assert!(
        set.contains(&ArcVarId::new(1)) && set.contains(&ArcVarId::new(0)),
        "borrowed param + its alias consumed at aggregate stores both classify; set = {set:?}"
    );
}

#[test]
fn borrowed_store_scan_excludes_owned_param_stores() {
    // An OWNED param's store is the existing terminal-move / genuine-dup
    // territory — over-adding an inc on an owned source leaks.
    let registry = TypeRegistry::new();
    let func = borrowed_store_func(
        Ownership::Owned,
        Idx::STR,
        3,
        vec![alias_of(1, 0), store_of(2, 1)],
    );
    let set = super::ownership_scans::compute_borrowed_store_dup_args(&func, &registry);
    assert!(
        set.is_empty(),
        "owned-param-rooted stores never classify; set = {set:?}"
    );
}

#[test]
fn borrowed_store_scan_excludes_call_arg_consumers() {
    // A borrowed value passed at an `Apply` arg is a CALL consume — its
    // ownership accounting is interprocedural/protocol, never a store dup.
    let registry = TypeRegistry::new();
    let func = borrowed_store_func(
        Ownership::Borrowed,
        Idx::STR,
        3,
        vec![
            alias_of(1, 0),
            ArcInstr::Apply {
                dst: ArcVarId::new(2),
                ty: Idx::STR,
                func: Name::from_raw(99),
                args: vec![ArcVarId::new(1)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            },
        ],
    );
    let set = super::ownership_scans::compute_borrowed_store_dup_args(&func, &registry);
    assert!(
        set.is_empty(),
        "call-arg consumers are out of the aggregate-store family; set = {set:?}"
    );
}

#[test]
fn borrowed_store_scan_excludes_rc_free_types() {
    // A borrowed scalar-typed param stored into an aggregate carries no RC
    // burden — no inc to emit.
    let registry = TypeRegistry::new();
    let func = borrowed_store_func(
        Ownership::Borrowed,
        Idx::INT,
        3,
        vec![alias_of(1, 0), store_of(2, 1)],
    );
    let set = super::ownership_scans::compute_borrowed_store_dup_args(&func, &registry);
    assert!(
        set.is_empty(),
        "an RC-free stored value needs no duplication inc; set = {set:?}"
    );
}

#[test]
fn borrowed_store_scan_includes_set_value() {
    // `Set { base, value: %1(borrowed-rooted) }` stores the borrowed value
    // into an existing aggregate field — same RL-1 duplication as Construct.
    let registry = TypeRegistry::new();
    let func = borrowed_store_func(
        Ownership::Borrowed,
        Idx::STR,
        3,
        vec![
            alias_of(1, 0),
            ArcInstr::Set {
                base: ArcVarId::new(2),
                field: 0,
                value: ArcVarId::new(1),
            },
        ],
    );
    let set = super::ownership_scans::compute_borrowed_store_dup_args(&func, &registry);
    assert!(
        set.contains(&ArcVarId::new(1)),
        "Set.value of a borrowed-rooted var classifies; set = {set:?}"
    );
}

#[test]
fn borrowed_store_inc_emitted_before_aggregate_store() {
    // End-to-end Phase-5 pin: the emitted body carries `BurdenInc %1`
    // immediately BEFORE the `Construct` consuming the borrowed-rooted alias
    // (the aggregate takes a real second reference; the caller keeps its own).
    let registry = TypeRegistry::new();
    let mut func = borrowed_store_func(
        Ownership::Borrowed,
        Idx::STR,
        3,
        vec![alias_of(1, 0), store_of(2, 1)],
    );
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    let body = &func.blocks[0].body;
    let store_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::Construct { .. }))
        .unwrap_or_else(|| panic!("store missing from emitted body"));
    assert!(
        store_pos > 0
            && matches!(
                body[store_pos - 1],
                ArcInstr::BurdenInc { var } if var == ArcVarId::new(1)
            ),
        "BurdenInc %1 must immediately precede the consuming store; body = {body:?}"
    );
}

// Repr-aware admission gate (`is_provably_scalar_repr`)
//
// The type-level `burden_carries_rc` filter admits a TYPE whose burden carries
// RC dimensions (variant entries / owned fields / self_heap_alloc) even when
// the concrete var's MONOMORPHIZED repr is `Scalar` (a niche-packed
// all-scalar-payload sum instantiation — `Option<int>` / `Result<int, int>`).
// A whole-var burden op on a Scalar-repr var can never lower to RC (Phase-7
// `RcStrategy::from_repr` rejects `Scalar`) and survives as VF-1 ledger
// residue (net=-1 per exit path). The admission consults the SAME `var_reprs`
// classification Phase-7 consults and skips ONLY the provable case.
// Spec: Annex E §AIMS L-9 + RE-2.

/// Owned param whose TYPE-level burden carries RC (`Idx::STR` —
/// `self_heap_alloc=true` per `BURDEN_TABLE`) read once via a borrow
/// projection. Pre-gate, the param receives a last-use `BurdenDec` (the
/// family shape: lone unlowerable dec). `reprs` selects the monomorphized
/// classification under pin.
fn type_admitted_param_func(reprs: Vec<ValueRepr>) -> ArcFunction {
    ArcFunction {
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        var_types: vec![Idx::STR, Idx::INT],
        var_reprs: reprs,
        blocks: vec![entry_block(
            vec![project_first(ArcVarId::new(1), Idx::INT, ArcVarId::new(0))],
            ArcTerminator::Unreachable,
        )],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn whole_var_burden_ops_skipped_when_repr_provably_scalar() {
    // Positive pin for the repr-aware admission gate: a var whose
    // monomorphized repr is provably `Scalar` receives ZERO whole-var burden
    // ops even though its TYPE-level burden carries RC. Reverting the gate
    // re-admits the var and re-introduces the unlowerable lone `BurdenDec`
    // (VF-1 net=-1 residue at every exit through the def).
    let registry = TypeRegistry::new();
    let mut func = type_admitted_param_func(vec![ValueRepr::Scalar, ValueRepr::Scalar]);
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        0,
        "provably-Scalar-repr var(0) MUST receive zero BurdenDec (L-9: scalars \
         carry no RC; the op could never lower); body={:?}",
        func.blocks[0].body,
    );
    assert_eq!(
        count_burden_incs(&func, ArcVarId::new(0)),
        0,
        "provably-Scalar-repr var(0) MUST receive zero BurdenInc; body={:?}",
        func.blocks[0].body,
    );
}

#[test]
fn whole_var_burden_ops_preserved_when_repr_heap() {
    // Over-fire negative (clamps the gate from below): the SAME fixture with
    // the TRUE heap repr (`FatValue` for STR — stands in for any heap-backed
    // instantiation, e.g. `Option<str>`'s `Aggregate`) keeps its admission and
    // its last-use release. Widening the skip beyond provably-Scalar would
    // drop this dec — a missing release (leak).
    let registry = TypeRegistry::new();
    let mut func = type_admitted_param_func(vec![ValueRepr::FatValue, ValueRepr::Scalar]);
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        1,
        "heap-repr var(0) MUST keep its last-use BurdenDec (the release of the \
         owned param); body={:?}",
        func.blocks[0].body,
    );
}

#[test]
fn whole_var_burden_ops_preserved_when_var_reprs_unpopulated() {
    // Conservative-fallback pin: with `var_reprs` UNPOPULATED (pre-pipeline —
    // `func.var_repr` returns `None`), the repr is NOT provably Scalar and the
    // admission is unchanged. Widening the skip to `None` reprs would silently
    // strip releases wherever the classification is unavailable.
    let registry = TypeRegistry::new();
    let mut func = type_admitted_param_func(Vec::new());
    emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default(), false);
    assert_eq!(
        count_burden_decs(&func, ArcVarId::new(0)),
        1,
        "unpopulated var_reprs MUST keep the admission (conservative: only the \
         provable Scalar case skips); body={:?}",
        func.blocks[0].body,
    );
}

#[test]
fn borrowed_store_scan_excludes_provably_scalar_repr_member() {
    // Repr-gate pin on the independent borrowed-store admission: the SAME
    // shape that fires in `borrowed_store_scan_fires_on_borrowed_param_and_alias_stores`
    // (the vacuity guard) admits NOTHING when every member's repr is provably
    // Scalar — a niche-packed stored value is a copy; no second reference
    // exists and a store-site inc on it can never lower.
    let registry = TypeRegistry::new();
    let mut func = borrowed_store_func(
        Ownership::Borrowed,
        Idx::STR,
        4,
        vec![alias_of(1, 0), store_of(2, 1), store_of(3, 0)],
    );
    func.var_reprs = vec![ValueRepr::Scalar; 4];
    let set = super::ownership_scans::compute_borrowed_store_dup_args(&func, &registry);
    assert!(
        set.is_empty(),
        "provably-Scalar-repr members MUST NOT classify as borrowed-store dups; set = {set:?}"
    );
}

/// Two-block forwarder-fed dead-param fixture: `bb0: %0 = Construct;
/// %1 = Apply @fwd(%0 [own]); Jump bb1(%1)` then `bb1(%2 DEAD): Return %3`.
/// The dead param's feeding rep is the forwarder lineage `{%0, %1}`.
fn dead_forwarder_param_func(reprs: Vec<ValueRepr>, callee: Name) -> ArcFunction {
    ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::INT],
        var_reprs: reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Construct {
                        dst: ArcVarId::new(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Tuple,
                        args: Vec::new(),
                    },
                    ArcInstr::Apply {
                        dst: ArcVarId::new(1),
                        ty: Idx::STR,
                        func: callee,
                        args: vec![ArcVarId::new(0)],
                        arg_ownership: vec![ArgOwnership::Owned],
                        mono_instance_id: None,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![ArcVarId::new(1)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(ArcVarId::new(2), Idx::STR)],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(3),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

#[test]
fn dead_forwarder_param_release_fires_on_heap_repr_param() {
    // Vacuity guard for the scalar-repr pin below: with heap reprs the RL-5
    // dead-param release DOES fire on this fixture (the dead param holds the
    // forwarded allocation's sole reference).
    let callee = Name::from_raw(200);
    let func = dead_forwarder_param_func(
        vec![
            ValueRepr::FatValue,
            ValueRepr::FatValue,
            ValueRepr::FatValue,
            ValueRepr::Scalar,
        ],
        callee,
    );
    let mut contracts = FxHashMap::default();
    contracts.insert(callee, forwarder_contract());
    let releases =
        super::ownership_scans::compute_dead_forwarder_block_param_releases(&func, &contracts);
    assert_eq!(
        releases.get(&1).map(Vec::as_slice),
        Some([ArcVarId::new(2)].as_slice()),
        "heap-repr dead forwarder param var(2) MUST receive the RL-5 release; \
         releases = {releases:?}"
    );
}

#[test]
fn dead_forwarder_param_release_skipped_for_scalar_repr_param() {
    // Repr-gate pin on the independent RL-5 dead-param admission: the SAME
    // fixture with provably-Scalar reprs admits NOTHING — a Scalar-repr dead
    // param carries no allocation to release, and the dec could never lower.
    let callee = Name::from_raw(201);
    let func = dead_forwarder_param_func(vec![ValueRepr::Scalar; 4], callee);
    let mut contracts = FxHashMap::default();
    contracts.insert(callee, forwarder_contract());
    let releases =
        super::ownership_scans::compute_dead_forwarder_block_param_releases(&func, &contracts);
    assert!(
        releases.is_empty(),
        "provably-Scalar-repr dead param MUST NOT receive an RL-5 release; \
         releases = {releases:?}"
    );
}

// Cross-block final-use cancellation gate (`compute_transfer_via_move_alias`)
//
// Verdict pins on the dup'd-source successor-reachability final-use proof per
// AIMS RL-2 (`RL2_transfer_kinds_no_dec`): a dup'd cross-block move source is
// cancelled iff its `Let { Var }` alias is the proven global final use AND the
// alias genuinely transfers out at an owned position. Decline pins cover the
// later-in-block use, the forward-successor use, the loop back-edge re-use,
// and the non-transfer terminal alias (fixpoint-only: no owned-RC-dst seed).

/// Drive the move-alias transfer scan with the same inputs the Phase-5 driver
/// assembles (per-block last-use detection + function-wide use counts +
/// terminator transfer seeds). `owned` lists vars whose burden carries RC.
fn run_move_alias_scan(func: &ArcFunction, owned: &[u32]) -> FxHashSet<ArcVarId> {
    let mut ctx = BurdenLowerCtx::new(func);
    super::ownership_scans::detect_last_uses(&mut ctx, func);
    let (use_counts, _) = super::ownership_scans::compute_use_counts_and_dup_aliases(
        func,
        &mut FxHashSet::default(),
        &FxHashSet::default(),
    );
    let terminator_transfer = super::terminator::compute_terminator_transfer_per_block(func, &[]);
    let owned: FxHashSet<ArcVarId> = owned.iter().map(|&v| ArcVarId::new(v)).collect();
    let alias_table = crate::aims::intraprocedural::project_aliases::compute_project_alias_table(
        func,
        &FxHashMap::default(),
    );
    let registry = TypeRegistry::new();
    let empty_aliases = FxHashMap::default();
    super::ownership_scans::compute_transfer_via_move_alias(
        func,
        &terminator_transfer,
        &use_counts,
        ctx.last_use_points(),
        &owned,
        &[],
        &super::ownership_scans::SameAllocIdentity {
            genuine_same_alloc_reps: &alias_table.genuine_same_alloc_reps,
            apply_result_aliases: &empty_aliases,
            type_registry: &registry,
        },
    )
}

/// bb0: `%0` fresh str + a non-terminal use, Jump bb1. bb1 body per
/// `bb1_body`; bb1 terminator per `bb1_term`. Optional bb2 for successor-use
/// shapes. `%0` is the dup'd source (2+ uses).
///
/// The `Owned` non-terminal use routes through a `Let { Var }` alias
/// (`%5 = %0; %1 = Apply f(%5 [own])`) — the lowered COW-receiver hand-off
/// shape (`%6 = %4; Invoke @insert(%6 [own], ..)`). A DIRECT owned-position
/// consume of `%0` would put `%0` itself into the instruction-transfer seed
/// (`instr_transfer_vars`) and mask the relaxed gate's verdict. The `Borrowed`
/// variant reads `%0` directly (a borrow is not a seed position).
fn cross_block_dup_source_func_with(
    bb0_use_ownership: ArgOwnership,
    bb1_body: Vec<ArcInstr>,
    bb1_term: ArcTerminator,
    bb2: Option<(Vec<ArcInstr>, ArcTerminator)>,
) -> ArcFunction {
    let mut bb0_body = vec![ArcInstr::Let {
        dst: ArcVarId::new(0),
        ty: Idx::STR,
        value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
    }];
    let consumed_arg = if matches!(bb0_use_ownership, ArgOwnership::Owned) {
        bb0_body.push(ArcInstr::Let {
            dst: ArcVarId::new(5),
            ty: Idx::STR,
            value: ArcValue::Var(ArcVarId::new(0)),
        });
        ArcVarId::new(5)
    } else {
        ArcVarId::new(0)
    };
    bb0_body.push(ArcInstr::Apply {
        dst: ArcVarId::new(1),
        ty: Idx::INT,
        func: Name::from_raw(100),
        args: vec![consumed_arg],
        arg_ownership: vec![bb0_use_ownership],
        mono_instance_id: None,
    });
    let mut blocks = vec![
        ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: bb0_body,
            terminator: ArcTerminator::Jump {
                target: ArcBlockId::new(1),
                args: Vec::new(),
            },
        },
        ArcBlock {
            id: ArcBlockId::new(1),
            params: Vec::new(),
            body: bb1_body,
            terminator: bb1_term,
        },
    ];
    if let Some((body, terminator)) = bb2 {
        blocks.push(ArcBlock {
            id: ArcBlockId::new(2),
            params: Vec::new(),
            body,
            terminator,
        });
    }
    ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::STR, Idx::STR, Idx::INT, Idx::STR],
        blocks,
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

/// Owned-consuming bb0 use (the mk2 `insert` receiver shape).
fn cross_block_dup_source_func(
    bb1_body: Vec<ArcInstr>,
    bb1_term: ArcTerminator,
    bb2: Option<(Vec<ArcInstr>, ArcTerminator)>,
) -> ArcFunction {
    cross_block_dup_source_func_with(ArgOwnership::Owned, bb1_body, bb1_term, bb2)
}

fn alias_let() -> ArcInstr {
    ArcInstr::Let {
        dst: ArcVarId::new(2),
        ty: Idx::STR,
        value: ArcValue::Var(ArcVarId::new(0)),
    }
}

fn tuple_construct_of_alias() -> ArcInstr {
    ArcInstr::Construct {
        dst: ArcVarId::new(3),
        ty: Idx::STR,
        ctor: CtorKind::Tuple,
        args: vec![ArcVarId::new(2)],
    }
}

#[test]
fn move_alias_cross_block_final_use_into_construct_cancels_source_release() {
    // bb1: `%2 = %0` (the global final use of `%0`); `%3 = Construct
    // Tuple(%2)` (owned-position transfer); Return %3. The fixpoint reaches
    // `%0` through the hand-off edge: its pending release is cancelled
    // (RL-2 ConstructArg transfer — `RL2_transfer_kinds_no_dec`).
    let func = cross_block_dup_source_func(
        vec![alias_let(), tuple_construct_of_alias()],
        ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
        None,
    );
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        transferred.contains(&ArcVarId::new(0)),
        "dup'd cross-block source whose final-use alias is Construct-consumed \
         MUST be transfer-cancelled; transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_alias_with_later_in_block_use_keeps_source_release() {
    // bb1: `%2 = %0`; `%4 = Apply len(%0 [borrow])` AFTER the alias — the
    // alias is NOT the in-block last use of `%0`, so the proof's in-block
    // finality clause declines and the source keeps its release.
    let later_borrow = ArcInstr::Apply {
        dst: ArcVarId::new(4),
        ty: Idx::INT,
        func: Name::from_raw(100),
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    };
    let func = cross_block_dup_source_func(
        vec![alias_let(), later_borrow, tuple_construct_of_alias()],
        ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
        None,
    );
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "source read AFTER the alias in the same block MUST keep its release; \
         transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_alias_with_successor_use_keeps_source_release() {
    // bb1: `%2 = %0`; Construct(%2); Jump bb2. bb2 reads `%0` — a use in a
    // forward-successor block: the cross-block finality clause declines.
    let bb2_borrow = ArcInstr::Apply {
        dst: ArcVarId::new(4),
        ty: Idx::INT,
        func: Name::from_raw(100),
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    };
    let func = cross_block_dup_source_func(
        vec![alias_let(), tuple_construct_of_alias()],
        ArcTerminator::Jump {
            target: ArcBlockId::new(2),
            args: Vec::new(),
        },
        Some((
            vec![bb2_borrow],
            ArcTerminator::Return {
                value: ArcVarId::new(4),
            },
        )),
    );
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "source used in a successor block MUST keep its release; \
         transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_alias_in_loop_back_edge_keeps_source_release() {
    // bb1 jumps back to ITSELF: the reachability walk re-reaches bb1 through
    // the back edge, so the alias's own block carries a (next-iteration) use
    // of `%0` — a back-edge re-use is a later use of the same lineage and the
    // cancellation MUST decline (the next iteration still consumes the
    // reference).
    let func = cross_block_dup_source_func(
        vec![alias_let(), tuple_construct_of_alias()],
        ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
        None,
    );
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "loop back-edge re-use MUST decline the cancellation; \
         transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_final_use_without_transfer_keeps_source_release() {
    // bb1: `%2 = %0` IS the global final use, but `%2`'s own last use is a
    // borrow-read (no owned-position transfer anywhere downstream). The
    // hand-off edge is FIXPOINT-ONLY — without a genuine transfer of the
    // alias, the source keeps its release even though `%2` is owned-RC (the
    // owned-RC-dst seed does NOT apply to relaxed cross-block edges).
    let alias_borrow = ArcInstr::Apply {
        dst: ArcVarId::new(4),
        ty: Idx::INT,
        func: Name::from_raw(100),
        args: vec![ArcVarId::new(2)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    };
    let func = cross_block_dup_source_func(
        vec![alias_let(), alias_borrow],
        ArcTerminator::Return {
            value: ArcVarId::new(4),
        },
        None,
    );
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "non-transfer terminal alias MUST NOT cancel the source's release; \
         transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_param_source_keeps_release_marker() {
    // A function PARAM source is excluded from the relaxed cross-block gate:
    // its last-use dec marker is load-bearing on the default coexistence path
    // (it drives `populate_class_covered` so the predicate stack's own real
    // dec stays suppressed); the param-transfers-through-return case is owned
    // by the contract-driven `transfer_through_return_param_vars` strip.
    let mut func = cross_block_dup_source_func(
        vec![alias_let(), tuple_construct_of_alias()],
        ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
        None,
    );
    // Rebind %0 as an Owned param: drop its defining Let, declare the param.
    func.blocks[0].body.remove(0);
    func.params = vec![ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::STR,
        ownership: Ownership::Owned,
    }];
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "param source MUST NOT join the relaxed cross-block cancellation; \
         transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_with_borrowed_non_terminal_use_keeps_source_release() {
    // The bb0 non-terminal use is a BORROW (`%1 = Apply f(%0 [borrow])`) — it
    // consumes no reference, so the dup'd extra reference's only release IS
    // the terminal dec. Cancellation MUST decline even though the terminal
    // alias transfers (the slice-then-push shape: `let s = list.slice(0, 2);
    // let list = list.push(4); (s, list)`).
    let func = cross_block_dup_source_func_with(
        ArgOwnership::Borrowed,
        vec![alias_let(), tuple_construct_of_alias()],
        ArcTerminator::Return {
            value: ArcVarId::new(3),
        },
        None,
    );
    let transferred = run_move_alias_scan(&func, &[0, 2, 3]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "a non-consuming non-terminal use MUST keep the source's terminal \
         release; transferred = {transferred:?}"
    );
}

#[test]
fn move_alias_cross_block_alternative_arm_aliases_keep_source_release() {
    // bb0: `%0` fresh, Branch -> bb1 | bb2. EACH arm has its own
    // `Let { Var(%0) }` alias consumed at a Construct (the recursive
    // sum-rebuild shape: `if .. then Left(v, next: tail) else
    // Right(v, next: tail)`). Per arm, the alias IS the final use (the sibling
    // arm is unreachable from it), but the arms are ALTERNATIVES: the sibling
    // arm's consume never runs on this path, so it does NOT discharge this
    // path's duplicate reference — the kept terminal dec is that duplicate's
    // only release. Cancellation MUST decline on BOTH arms (a non-terminal
    // use discharges only when its block DOMINATES the terminal block).
    let arm = |alias: u32, dst: u32| {
        vec![
            ArcInstr::Let {
                dst: ArcVarId::new(alias),
                ty: Idx::STR,
                value: ArcValue::Var(ArcVarId::new(0)),
            },
            ArcInstr::Construct {
                dst: ArcVarId::new(dst),
                ty: Idx::STR,
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(alias)],
            },
        ]
    };
    let func = ArcFunction {
        var_types: vec![Idx::STR, Idx::BOOL, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(0),
                        ty: Idx::STR,
                        value: ArcValue::Literal(LitValue::String(Name::from_raw(1))),
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(1),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: arm(2, 3),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(3),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: arm(4, 5),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let transferred = run_move_alias_scan(&func, &[0, 2, 3, 4, 5]);
    assert!(
        !transferred.contains(&ArcVarId::new(0)),
        "alternative-arm aliases MUST keep the source's per-arm release; \
         transferred = {transferred:?}"
    );
}

// Genuine-duplication OWNED-CALL-ARG alias scan
// (`compute_genuine_dup_call_arg_aliases`)

/// Owned-position user-fn `Apply` consuming `arg` (structural `[own]`).
fn owned_apply_of(dst: u32, callee: Name, arg: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        func: callee,
        args: vec![ArcVarId::new(arg)],
        arg_ownership: vec![ArgOwnership::Owned],
        mono_instance_id: None,
    }
}

/// Borrowed-annotated user-fn `Apply` consuming `arg` (the pre-`realize`
/// borrowed default at Phase-5 call sites).
fn borrowed_apply_of(dst: u32, callee: Name, arg: u32) -> ArcInstr {
    ArcInstr::Apply {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        func: callee,
        args: vec![ArcVarId::new(arg)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    }
}

/// One-block func: `%1 = %0; %3 = callee(%1 [own]); %2 = %0; %4 = callee(%2)`.
/// The first call-arg alias has a later source use (genuine duplication); the
/// second is the source's terminal move.
fn call_arg_dup_func(callee: Name) -> ArcFunction {
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        owned_apply_of(3, callee, 1),
        alias_of(2, 0),
        owned_apply_of(4, callee, 2),
    ];
    func
}

#[test]
fn call_arg_dup_scan_fires_on_owned_user_call_with_later_source_use() {
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("user_fork");
    let func = call_arg_dup_func(callee);
    let set = super::ownership_scans::compute_genuine_dup_call_arg_aliases(
        &func,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        set.contains(&ArcVarId::new(1)),
        "first owned-call-arg alias with a later source use is a genuine duplication"
    );
    assert!(
        !set.contains(&ArcVarId::new(2)),
        "second alias is the source's terminal use — a move, never admitted"
    );
}

#[test]
fn call_arg_dup_scan_excludes_iter_protocol_consumes() {
    let interner = ori_ir::StringInterner::new();
    let iter = interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let func = call_arg_dup_func(iter);
    let set = super::ownership_scans::compute_genuine_dup_call_arg_aliases(
        &func,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        set.is_empty(),
        "`iter` owned args are iter-consume transfers with their own RL-2 accounting — never admitted"
    );
}

#[test]
fn call_arg_dup_scan_excludes_dunder_protocol_builtins() {
    let interner = ori_ir::StringInterner::new();
    let next =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
    let func = call_arg_dup_func(next);
    let set = super::ownership_scans::compute_genuine_dup_call_arg_aliases(
        &func,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        set.is_empty(),
        "`__`-prefixed protocol builtins carry their own ownership protocol — never admitted"
    );
}

#[test]
fn call_arg_dup_scan_admits_contract_owned_borrowed_annotation() {
    use crate::aims::contract::{MemoryContract, ParamContract};
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("user_consumer");
    let mut func = func_with_n_vars(5);
    // Borrowed call-site annotations (the Phase-5 default for user calls) with
    // a contract proving the param Owned: the alias is still a duplication.
    func.blocks[0].body = vec![
        alias_of(1, 0),
        borrowed_apply_of(3, callee, 1),
        alias_of(2, 0),
        borrowed_apply_of(4, callee, 2),
    ];
    let mut param = ParamContract::CONSERVATIVE;
    param.access = crate::aims::lattice::AccessClass::Owned;
    let contract = MemoryContract {
        params: vec![param],
        ..MemoryContract::conservative(1)
    };
    let contracts: FxHashMap<Name, MemoryContract> = [(callee, contract)].into_iter().collect();
    let set =
        super::ownership_scans::compute_genuine_dup_call_arg_aliases(&func, &contracts, &interner);
    assert!(
        set.contains(&ArcVarId::new(1)),
        "contract-Owned position admits despite the stale borrowed call-site annotation"
    );
}

#[test]
fn call_arg_dup_scan_admits_borrowed_cow_consumed_contract() {
    use crate::aims::contract::{MemoryContract, ParamContract};
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("cow_pusher");
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        borrowed_apply_of(3, callee, 1),
        alias_of(2, 0),
        borrowed_apply_of(4, callee, 2),
    ];
    let mut param = ParamContract::CONSERVATIVE;
    param.access = crate::aims::lattice::AccessClass::Borrowed;
    param.borrowed_cow_consumed = true;
    let contract = MemoryContract {
        params: vec![param],
        ..MemoryContract::conservative(1)
    };
    let contracts: FxHashMap<Name, MemoryContract> = [(callee, contract)].into_iter().collect();
    let set =
        super::ownership_scans::compute_genuine_dup_call_arg_aliases(&func, &contracts, &interner);
    assert!(
        set.contains(&ArcVarId::new(1)),
        "a borrowed-COW-consumed-at-death callee param obligates caller funding — admitted"
    );
}

#[test]
fn call_arg_dup_scan_excludes_borrowed_read_only_contract() {
    use crate::aims::contract::{MemoryContract, ParamContract};
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("pure_reader");
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        borrowed_apply_of(3, callee, 1),
        alias_of(2, 0),
        borrowed_apply_of(4, callee, 2),
    ];
    let mut param = ParamContract::CONSERVATIVE;
    param.access = crate::aims::lattice::AccessClass::Borrowed;
    param.borrowed_read_only = true;
    let contract = MemoryContract {
        params: vec![param],
        ..MemoryContract::conservative(1)
    };
    let contracts: FxHashMap<Name, MemoryContract> = [(callee, contract)].into_iter().collect();
    let set =
        super::ownership_scans::compute_genuine_dup_call_arg_aliases(&func, &contracts, &interner);
    assert!(
        set.is_empty(),
        "a pure borrow-read callee nets 0 on the caller's lineage — never admitted"
    );
}

#[test]
fn call_arg_dup_scan_excludes_ttr_pass_through_positions() {
    use crate::aims::contract::{MemoryContract, ParamContract};
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("forwarder_id");
    let func = call_arg_dup_func(callee);
    let mut param = ParamContract::CONSERVATIVE;
    param.access = crate::aims::lattice::AccessClass::Owned;
    param.transfers_through_return = true;
    let contract = MemoryContract {
        params: vec![param],
        ..MemoryContract::conservative(1)
    };
    let contracts: FxHashMap<Name, MemoryContract> = [(callee, contract)].into_iter().collect();
    let set =
        super::ownership_scans::compute_genuine_dup_call_arg_aliases(&func, &contracts, &interner);
    assert!(
        set.is_empty(),
        "a transfers-through-return position is a pass-through (RL-34), not a consume"
    );
}

#[test]
fn call_arg_dup_scan_declines_loop_variant_reassignment_source() {
    // `bb0 -> bb1(header, param %0) -> bb2(body): %1 = %0; %2 = push(%1 [own]);
    // Jump bb1(%2)` — the back-edge threads the CONSUME RESULT (`xs =
    // xs.push(i)`): re-reaching the header re-DEFINES the binding, so the
    // alias is the old binding's terminal move, never a duplication.
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("user_push");
    let func = ArcFunction {
        var_types: (0..3).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(ArcVarId::new(0), Idx::STR)],
                body: Vec::new(),
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(2),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![alias_of(1, 0), owned_apply_of(2, callee, 1)],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![ArcVarId::new(2)],
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let set = super::ownership_scans::compute_genuine_dup_call_arg_aliases(
        &func,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        set.is_empty(),
        "loop-variant reassignment: the source's only re-reach crosses its defining block — declined"
    );
}

#[test]
fn call_arg_dup_scan_admits_loop_invariant_source_with_post_loop_use() {
    // `bb0: %0 born; Jump bb1` then `bb1(loop): %1 = %0; %2 = fork(%1 [own]);
    // Branch -> bb1 | bb2` then `bb2` stores `%0` — the source is defined
    // OUTSIDE the cycle and read after the loop, so the in-loop fork alias is
    // a genuine duplication (the source's defining block is never re-reached
    // from the alias's successors — the cut does not fire).
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("user_fork");
    let func = ArcFunction {
        var_types: (0..5).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![store_of(0, 3)],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![alias_of(1, 0), owned_apply_of(2, callee, 1)],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(2),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![store_of(4, 0)],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let set = super::ownership_scans::compute_genuine_dup_call_arg_aliases(
        &func,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        set.contains(&ArcVarId::new(1)),
        "loop-invariant source with a post-loop use — the per-iteration fork is a duplication"
    );
}

// Funded store-family duplication set (`compute_funded_store_dup_aliases`)

#[test]
fn funded_store_dup_admits_alias_with_src_used_in_successor() {
    let func = dup_alias_src_used_in_successor_func();
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        funded.contains(&ArcVarId::new(1)),
        "bb0 store alias of a source read in a reachable successor is funded; funded = {funded:?}"
    );
    assert!(
        !funded.contains(&ArcVarId::new(2)),
        "the source's terminal store alias is a move, never funded"
    );
}

#[test]
fn funded_store_dup_admits_alias_with_later_same_block_source_use() {
    // The direct two-store shape: `%1 = %0; Construct(%1); %2 = %0;
    // Construct(%2)` — the first alias has a later same-block source use.
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        store_of(3, 1),
        alias_of(2, 0),
        store_of(4, 2),
    ];
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        funded.contains(&ArcVarId::new(1)),
        "first store alias is funded (source live past it); funded = {funded:?}"
    );
    assert!(
        !funded.contains(&ArcVarId::new(2)),
        "second store alias is the terminal move"
    );
}

#[test]
fn funded_store_dup_admits_set_value_endpoint() {
    // `%1 = %0; Set base.f = %1; %2 = %0; Construct(%2)` — the Set.value
    // consume is an aggregate-store endpoint per the Phase-5 SSOT.
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        ArcInstr::Set {
            base: ArcVarId::new(3),
            field: 0,
            value: ArcVarId::new(1),
        },
        alias_of(2, 0),
        store_of(4, 2),
    ];
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        funded.contains(&ArcVarId::new(1)),
        "a Set.value consume of a still-live source is a funded store duplication"
    );
}

#[test]
fn funded_store_dup_declines_branch_exclusive_aliases() {
    // Each branch store-aliases the source then returns — per-path terminal
    // moves, no duplication (the BUG-04-176 boundary: NO kept inc by design).
    let func = ArcFunction {
        var_types: (0..6).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![alias_of(2, 0), store_of(4, 2)],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: vec![alias_of(3, 0), store_of(5, 3)],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        funded.is_empty(),
        "branch-exclusive store aliases are per-path terminal moves: {funded:?}"
    );
}

#[test]
fn funded_store_dup_declines_call_arg_chain() {
    // The first alias moves into an OWNED call arg — the call-arg family has
    // its own funded machinery; the store SSOT never admits Apply/Invoke
    // consumers (over-admitting re-creates the +1-per-loop over-fire).
    let mut func = func_with_n_vars(5);
    func.blocks[0].body = vec![
        alias_of(1, 0),
        ArcInstr::Apply {
            dst: ArcVarId::new(3),
            ty: Idx::INT,
            func: Name::from_raw(7),
            args: vec![ArcVarId::new(1)],
            arg_ownership: vec![ArgOwnership::Owned],
            mono_instance_id: None,
        },
        alias_of(2, 0),
        store_of(4, 2),
    ];
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        !funded.contains(&ArcVarId::new(1)),
        "a call-arg consumer chain never enters the store family"
    );
}

#[test]
fn funded_store_dup_declines_terminal_single_store() {
    // Sole alias, sole store, no other source use — the store consumes the
    // source's original reference (RL-2 transfer, no duplication).
    let mut func = func_with_n_vars(4);
    func.blocks[0].body = vec![alias_of(1, 0), store_of(3, 1)];
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        funded.is_empty(),
        "a terminal single store is a move, never funded: {funded:?}"
    );
}

#[test]
fn funded_store_dup_declines_per_iteration_terminal_store_via_def_cut() {
    // The source is DEFINED in the loop block (per-iteration fresh construct)
    // with an earlier-in-block read; the back-edge re-reach belongs to the
    // NEXT binding instance, so the definition cut declines (per-iteration
    // TERMINAL store — funding it would leak +1 per iteration). Contrast the
    // loop-invariant admission below.
    let func = ArcFunction {
        var_types: (0..6).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    store_of(0, 5),
                    alias_of(3, 0),
                    alias_of(2, 0),
                    store_of(4, 2),
                ],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(0),
                    else_block: ArcBlockId::new(1),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        !funded.contains(&ArcVarId::new(2)),
        "a per-iteration-defined source's back-edge re-reach is the NEXT binding — declined"
    );
}

#[test]
fn funded_store_dup_admits_loop_invariant_source_stored_per_iteration() {
    // The loop-outside/store-inside subshape: source defined BEFORE the loop,
    // store-aliased INSIDE it each iteration — every iteration's store is a
    // genuine duplication (the source survives into the next iteration).
    let func = ArcFunction {
        var_types: (0..6).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![store_of(0, 5)],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![alias_of(2, 0), store_of(4, 2), alias_of(3, 0)],
                terminator: ArcTerminator::Branch {
                    cond: ArcVarId::new(1),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(4),
                },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let funded =
        super::ownership_scans::compute_funded_store_dup_aliases(&func, &FxHashMap::default());
    assert!(
        funded.contains(&ArcVarId::new(2)),
        "a loop-invariant source stored per iteration funds each store; funded = {funded:?}"
    );
}

// Funded owned-call-arg duplication set (`compute_funded_call_arg_dup_aliases`)

#[test]
fn funded_call_arg_dup_set_excludes_forwarder_transparent_raw_member() {
    use crate::aims::contract::{MemoryContract, ParamContract};
    let interner = ori_ir::StringInterner::new();
    let self_name = interner.intern("self_forwarder");
    let callee = interner.intern("user_consumer");
    // `@self_forwarder(%0 owned-ttr) = { %1 = %0; callee(%1 [borrow]); Return %0 }`
    // — the RAW scan admits %1 via the contract-Owned position (the borrowed
    // call-site annotation keeps the forwarder vetting's structural
    // owned-position decline from firing), yet %1 is forwarder-identity
    // TRANSPARENT: Phase 5 skips its dup classification and emits NO
    // alias-site inc. The FUNDED set MUST exclude it, else the Phase-6/7
    // accounting consumers debit a reference no inc supplied.
    let mut func = func_with_n_vars(4);
    func.name = self_name;
    func.params = vec![ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::STR,
        ownership: Ownership::Owned,
    }];
    func.blocks[0].body = vec![alias_of(1, 0), borrowed_apply_of(3, callee, 1)];
    func.blocks[0].terminator = ArcTerminator::Return {
        value: ArcVarId::new(0),
    };
    let mut callee_param = ParamContract::CONSERVATIVE;
    callee_param.access = crate::aims::lattice::AccessClass::Owned;
    let callee_contract = MemoryContract {
        params: vec![callee_param],
        ..MemoryContract::conservative(1)
    };
    let mut own_param = ParamContract::CONSERVATIVE;
    own_param.transfers_through_return = true;
    let own_contract = MemoryContract {
        params: vec![own_param],
        ..MemoryContract::conservative(1)
    };
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> =
        [(callee, callee_contract), (self_name, own_contract)]
            .into_iter()
            .collect();
    let raw =
        super::ownership_scans::compute_genuine_dup_call_arg_aliases(&func, &contracts, &interner);
    assert!(
        raw.contains(&ArcVarId::new(1)),
        "RAW scan admits the contract-Owned call-arg alias of the ttr param; raw = {raw:?}"
    );
    let funded =
        super::ownership_scans::compute_funded_call_arg_dup_aliases(&func, &contracts, &interner);
    assert!(
        !funded.contains(&ArcVarId::new(1)),
        "FUNDED set excludes the forwarder-transparent alias — Phase 5 kept no \
         alias-site inc for it; funded = {funded:?}"
    );
}

#[test]
fn funded_call_arg_dup_set_keeps_plain_genuine_duplication() {
    let interner = ori_ir::StringInterner::new();
    let callee = interner.intern("user_fork");
    let func = call_arg_dup_func(callee);
    let funded = super::ownership_scans::compute_funded_call_arg_dup_aliases(
        &func,
        &FxHashMap::default(),
        &interner,
    );
    assert!(
        funded.contains(&ArcVarId::new(1)),
        "an unfiltered genuine duplication stays funded — the gates only strip \
         members Phase 5 never funded"
    );
    assert!(
        !funded.contains(&ArcVarId::new(2)),
        "the source's terminal move stays out (raw exclusion carries through)"
    );
}

// Call-result-aggregate element FINAL-READ release designation
// (`compute_call_result_element_final_read_releases`)

/// Contract for a 0-param callee whose result is a fresh caller-owned
/// acquisition (`return_info.uniqueness == Unique`, no ttr params).
fn unique_return_contract() -> crate::aims::contract::MemoryContract {
    let mut c = crate::aims::contract::MemoryContract::conservative(0);
    c.return_info.uniqueness = crate::aims::lattice::Uniqueness::Unique;
    c
}

/// Canonical multi-read shape: `%0 = make()` (Aggregate result),
/// `%1 = Project %0.0` (`RcPtr` element view), two `Let { Var }` read aliases
/// (`%2`, `%4`) each borrow-read by `reader` — `let (a, b) = make();
/// a.len(); a[0]`.
fn result_elem_multi_read_func(make: Name, reader: Name) -> ArcFunction {
    let mut func = func_with_n_vars(6);
    func.var_reprs = vec![
        ValueRepr::Aggregate, // %0 call-result tuple
        ValueRepr::RcPointer, // %1 element projection
        ValueRepr::RcPointer, // %2 read alias 1
        ValueRepr::Scalar,    // %3 reader result
        ValueRepr::RcPointer, // %4 read alias 2 (execution-final)
        ValueRepr::Scalar,    // %5 reader result
    ];
    func.blocks[0].body = vec![
        ArcInstr::Apply {
            dst: ArcVarId::new(0),
            ty: Idx::STR,
            func: make,
            args: Vec::new(),
            arg_ownership: Vec::new(),
            mono_instance_id: None,
        },
        ArcInstr::Project {
            dst: ArcVarId::new(1),
            ty: Idx::STR,
            value: ArcVarId::new(0),
            field: 0,
        },
        alias_of(2, 1),
        borrowed_apply_of(3, reader, 2),
        alias_of(4, 1),
        borrowed_apply_of(5, reader, 4),
    ];
    func
}

#[test]
fn final_read_release_designates_execution_final_alias_of_multi_read_element() {
    let interner = ori_ir::StringInterner::new();
    let make = interner.intern("make_pair");
    let reader = interner.intern("reader");
    let func = result_elem_multi_read_func(make, reader);
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> =
        [(make, unique_return_contract())].into_iter().collect();
    let releases = super::ownership_scans::compute_call_result_element_final_read_releases(
        &func,
        &contracts,
        &FxHashSet::default(),
    );
    assert!(
        releases.contains(&ArcVarId::new(4)),
        "the execution-final read alias carries the element's single release; \
         releases = {releases:?}"
    );
    assert_eq!(
        releases.len(),
        1,
        "exactly ONE release per multi-read lineage (RL-2 release-exactly-once); \
         releases = {releases:?}"
    );
}

#[test]
fn final_read_release_skips_single_read_element_unchanged() {
    // Single-read lineage: one alias, one borrow-read — the alias already
    // carries its lone last-use dec on today's arrangement; no designation.
    let interner = ori_ir::StringInterner::new();
    let make = interner.intern("make_pair");
    let reader = interner.intern("reader");
    let mut func = result_elem_multi_read_func(make, reader);
    func.blocks[0].body.truncate(4); // drop the second alias + its read
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> =
        [(make, unique_return_contract())].into_iter().collect();
    let releases = super::ownership_scans::compute_call_result_element_final_read_releases(
        &func,
        &contracts,
        &FxHashSet::default(),
    );
    assert!(
        releases.is_empty(),
        "a single-read element keeps its lone last-use dec — never designated; \
         releases = {releases:?}"
    );
}

#[test]
fn final_read_release_declines_element_consumed_at_owned_position() {
    // The second alias is CONSUMED at an owned call-arg position — the element
    // escapes; its release belongs to the consumer, not a designated read.
    let interner = ori_ir::StringInterner::new();
    let make = interner.intern("make_pair");
    let reader = interner.intern("reader");
    let consumer = interner.intern("consumer");
    let mut func = result_elem_multi_read_func(make, reader);
    func.blocks[0].body[5] = owned_apply_of(5, consumer, 4);
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> =
        [(make, unique_return_contract())].into_iter().collect();
    let releases = super::ownership_scans::compute_call_result_element_final_read_releases(
        &func,
        &contracts,
        &FxHashSet::default(),
    );
    assert!(
        releases.is_empty(),
        "an owned-position consume declines the lineage (the consumer owns the \
         release); releases = {releases:?}"
    );
}

#[test]
fn final_read_release_declines_non_unique_call_result() {
    // A conservative (MaybeShared-return) contract never proves the caller owns
    // the only reference — the lineage stays on today's arrangement.
    let interner = ori_ir::StringInterner::new();
    let make = interner.intern("make_pair");
    let reader = interner.intern("reader");
    let func = result_elem_multi_read_func(make, reader);
    let contracts: FxHashMap<Name, crate::aims::contract::MemoryContract> =
        [(make, crate::aims::contract::MemoryContract::conservative(0))]
            .into_iter()
            .collect();
    let releases = super::ownership_scans::compute_call_result_element_final_read_releases(
        &func,
        &contracts,
        &FxHashSet::default(),
    );
    assert!(
        releases.is_empty(),
        "a non-Unique call result is not a proven fresh acquisition — declined; \
         releases = {releases:?}"
    );
}

// RL-4 branch-exclusive terminal-move edge release
// (`ownership_scans::compute_branch_exclusive_edge_releases`)

use super::ownership_scans::ForwarderReleasePos;

/// FRESH local `Construct` root (empty args — the lineage birth site).
fn fresh_root(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: ArcVarId::new(dst),
        ty: Idx::STR,
        ctor: CtorKind::Tuple,
        args: Vec::new(),
    }
}

fn branch_block(cond: u32, then_b: u32, else_b: u32) -> ArcTerminator {
    ArcTerminator::Branch {
        cond: ArcVarId::new(cond),
        then_block: ArcBlockId::new(then_b),
        else_block: ArcBlockId::new(else_b),
    }
}

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn ret(value: u32) -> ArcTerminator {
    ArcTerminator::Return {
        value: ArcVarId::new(value),
    }
}

fn func_with_blocks(n_vars: u32, blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        var_types: (0..n_vars).map(|i| Idx::from_raw(i + 1)).collect(),
        blocks,
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    }
}

/// Run the scan with the default gate inputs: root %0 owned, nothing
/// suppressed / full-moved / funded, empty contracts.
fn branch_exclusive_releases_for(
    func: &ArcFunction,
    inc_suppressed: &[u32],
) -> FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> {
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let suppressed: FxHashSet<ArcVarId> =
        inc_suppressed.iter().map(|&v| ArcVarId::new(v)).collect();
    super::ownership_scans::compute_branch_exclusive_edge_releases(
        func,
        &owned,
        &suppressed,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
    )
    .releases
}

/// The pinned `@store_one` shape: root %0 constructed pre-branch, stored
/// (terminal move) on the then-arm only, borrow-aliased on the else-arm.
fn branch_exclusive_store_func() -> ArcFunction {
    func_with_blocks(
        7,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            // then: alias + terminal store — the consuming path.
            block(1, vec![alias_of(2, 0), store_of(4, 2)], ret(5)),
            // else: borrow alias only — the non-consuming path.
            block(2, vec![alias_of(3, 0)], ret(6)),
        ],
    )
}

#[test]
fn branch_exclusive_store_releases_root_after_final_borrow_read() {
    let func = branch_exclusive_store_func();
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert_eq!(
        releases.len(),
        1,
        "exactly the non-consuming else edge is admitted; releases = {releases:?}"
    );
    assert_eq!(
        releases.get(&(2, ForwarderReleasePos::AfterInstr(0))),
        Some(&vec![ArcVarId::new(0)]),
        "the root's release lands AFTER the else-arm's final lineage read \
         (the alias at instr 0); releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_store_no_use_arm_releases_at_block_entry() {
    let func = func_with_blocks(
        6,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            block(1, vec![alias_of(2, 0), store_of(4, 2)], ret(5)),
            // else: no lineage use at all — release at block entry.
            block(2, Vec::new(), ret(5)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert_eq!(
        releases.get(&(2, ForwarderReleasePos::BlockEntry)),
        Some(&vec![ArcVarId::new(0)]),
        "a no-use arm releases the funded duplicate at its entry; \
         releases = {releases:?}"
    );
    assert_eq!(releases.len(), 1, "the consuming edge emits nothing extra");
}

#[test]
fn branch_exclusive_no_use_arm_owes_dead_edge_birth_release() {
    // A fully-dead no-use arm carries TWO outstanding references (birth +
    // kept funding inc) and no RL-2 last-use anchor — the edge owes the
    // dead-edge birth dec BESIDE the funded-duplicate release.
    let func = func_with_blocks(
        6,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            block(1, vec![alias_of(2, 0), store_of(4, 2)], ret(5)),
            block(2, Vec::new(), ret(5)),
        ],
    );
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = super::ownership_scans::compute_branch_exclusive_edge_releases(
        &func,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
    );
    assert_eq!(
        out.dead_edge_birth_releases.get(&2),
        Some(&vec![ArcVarId::new(0)]),
        "the no-use edge owes the RL-4 birth dec: {:?}",
        out.dead_edge_birth_releases
    );
}

#[test]
fn branch_exclusive_borrow_arm_owes_no_dead_edge_birth_release() {
    // A non-consuming arm WITH a borrow read carries its own RL-2 last-use
    // dec for the birth reference — only the funded-duplicate release is owed.
    let func = branch_exclusive_store_func();
    let owned: FxHashSet<ArcVarId> = [ArcVarId::new(0)].into_iter().collect();
    let out = super::ownership_scans::compute_branch_exclusive_edge_releases(
        &func,
        &owned,
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashSet::default(),
        &FxHashMap::default(),
    );
    assert!(
        out.dead_edge_birth_releases.is_empty(),
        "a borrow-read arm never gains the birth dec (its last-use dec owns \
         the birth reference): {:?}",
        out.dead_edge_birth_releases
    );
}

#[test]
fn branch_exclusive_declines_both_paths_consume() {
    // Both arms terminally store — per-path ledgers balance; the
    // both-paths-consume green clamp (`burden_dup_inc.rs` sibling).
    let func = func_with_blocks(
        7,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            block(1, vec![alias_of(2, 0), store_of(4, 2)], ret(5)),
            block(2, vec![alias_of(3, 0), store_of(6, 3)], ret(5)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert!(
        releases.is_empty(),
        "both paths consume the funding — no edge release owed; \
         releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_declines_multi_pred_target() {
    // The store arm falls through INTO the borrow arm: the candidate has two
    // predecessors, so block-entry placement is not the edge release.
    let func = func_with_blocks(
        7,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            block(
                1,
                vec![alias_of(2, 0), store_of(4, 2)],
                ArcTerminator::Jump {
                    target: ArcBlockId::new(2),
                    args: Vec::new(),
                },
            ),
            block(2, vec![alias_of(3, 0)], ret(5)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert!(
        releases.is_empty(),
        "a multi-pred target declines (a block-entry dec would fire on the \
         consuming path too); releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_declines_loop_funded_per_iteration_store() {
    // Loop-inside-branch: the then-region stores the loop-INVARIANT root per
    // iteration (the source survives into the next iteration via the
    // back-edge, so the store alias is FUNDED — `store_dup` admits it), and
    // the else-arm borrows. No UNFUNDED consume exists, so the scan declines
    // globally (the residual is the dead-block-param threading root, not the
    // per-edge partition).
    let func = func_with_blocks(
        9,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 4)),
            // loop header
            block(1, Vec::new(), branch_block(2, 2, 3)),
            // loop body: per-iteration funded store, back-edge to header.
            block(
                2,
                vec![alias_of(3, 0), store_of(5, 3)],
                ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            ),
            // loop exit
            block(3, Vec::new(), ret(6)),
            // else arm: borrow alias.
            block(4, vec![alias_of(7, 0)], ret(8)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert!(
        releases.is_empty(),
        "funded per-iteration stores leave no unfunded consume — the scan \
         declines every edge; releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_declines_edge_reachable_from_consume() {
    // Terminal store inside a loop body (single static use of the root — NOT
    // funded): the loop-EXIT edge is reachable FROM the consume block, so it
    // shares its runtime path with the consume — mutual exclusion declines it.
    // The sibling else-arm (never on the consuming path) is admitted.
    let func = func_with_blocks(
        9,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 4)),
            // loop header
            block(1, Vec::new(), branch_block(2, 2, 3)),
            // loop body: TERMINAL store (root used nowhere else on this path).
            block(
                2,
                vec![alias_of(3, 0), store_of(5, 3)],
                ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: Vec::new(),
                },
            ),
            // loop exit — reachable from the consume block: DECLINED.
            block(3, Vec::new(), ret(6)),
            // else arm: no lineage use — admitted at entry.
            block(4, Vec::new(), ret(8)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert_eq!(
        releases.get(&(4, ForwarderReleasePos::BlockEntry)),
        Some(&vec![ArcVarId::new(0)]),
        "the branch-exclusive else edge is admitted; releases = {releases:?}"
    );
    assert!(
        !releases.contains_key(&(3, ForwarderReleasePos::BlockEntry)),
        "the loop-exit edge shares its runtime path with the in-loop consume \
         — a release there double-frees; releases = {releases:?}"
    );
    assert_eq!(releases.len(), 1, "exactly one admitted edge");
}

#[test]
fn branch_exclusive_declines_borrowed_only_lineage() {
    // No consume anywhere — the funding has no pending job on any path; the
    // surplus is Phase-6/7 elision territory, not a per-edge release.
    let func = func_with_blocks(
        6,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            block(1, vec![alias_of(2, 0)], ret(4)),
            block(2, vec![alias_of(3, 0)], ret(5)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert!(
        releases.is_empty(),
        "a borrow-only lineage owes no edge release; releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_declines_when_fresh_inc_suppressed() {
    // The funded duplicate does not exist when the FRESH-site inc was
    // suppressed — releasing would free the birth reference.
    let func = branch_exclusive_store_func();
    let releases = branch_exclusive_releases_for(&func, &[0]);
    assert!(
        releases.is_empty(),
        "no kept funding inc → no duplicate to release; releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_declines_post_merge_read() {
    // Post-merge borrow-read: the lineage is read in the SHARED merge block,
    // flipping the store to a genuine funded duplication (the GREEN clamp) —
    // the confinement gate declines both edges.
    let func = func_with_blocks(
        8,
        vec![
            block(0, vec![fresh_root(0)], branch_block(1, 1, 2)),
            block(
                1,
                vec![alias_of(2, 0), store_of(4, 2)],
                ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: Vec::new(),
                },
            ),
            block(
                2,
                vec![alias_of(3, 0)],
                ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: Vec::new(),
                },
            ),
            // merge: post-merge borrow-read of the root.
            block(3, vec![alias_of(6, 0)], ret(7)),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert!(
        releases.is_empty(),
        "a post-merge read keeps the birth reference live past the branch — \
         the edge release must not fire; releases = {releases:?}"
    );
}

#[test]
fn branch_exclusive_switch_unreachable_default_arm_gets_no_release() {
    // Switch with an impossible default arm: the borrow arm is admitted; the
    // `Unreachable` default arm never executes a release.
    let func = func_with_blocks(
        8,
        vec![
            block(
                0,
                vec![fresh_root(0)],
                ArcTerminator::Switch {
                    scrutinee: ArcVarId::new(1),
                    cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
                    default: ArcBlockId::new(3),
                },
            ),
            block(1, vec![alias_of(2, 0), store_of(4, 2)], ret(5)),
            block(2, vec![alias_of(6, 0)], ret(7)),
            block(3, Vec::new(), ArcTerminator::Unreachable),
        ],
    );
    let releases = branch_exclusive_releases_for(&func, &[]);
    assert_eq!(
        releases.get(&(2, ForwarderReleasePos::AfterInstr(0))),
        Some(&vec![ArcVarId::new(0)]),
        "the borrow arm is admitted per-arm; releases = {releases:?}"
    );
    assert!(
        !releases.keys().any(|&(b, _)| b == 3),
        "the Unreachable default arm never executes — no release placed; \
         releases = {releases:?}"
    );
    assert_eq!(releases.len(), 1, "exactly one admitted arm");
}
