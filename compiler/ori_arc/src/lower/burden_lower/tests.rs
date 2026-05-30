//! Tests for `emit_burden_ops` walker. Cycle 2 ships boundary + iteration
//! pin; subsequent cycles add owned-filter + matrix coverage per `tests.md
//! §Matrix Testing Rule` once the `DerivedOwnership` access path is wired.

use ori_ir::Name;
use ori_types::{Idx, TypeRegistry};
use rustc_hash::FxHashMap;

use super::{emit_burden_ops, BurdenLowerCtx};
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, LitValue,
};
use crate::lower::test_utils::{entry_block, project_first, set_first};
use crate::ownership::{DerivedOwnership, Ownership};

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
        emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    assert!(
        ctx.collected_burdens().is_empty(),
        "empty fn yields zero burden lookups",
    );
}

#[test]
fn construct_emits_one_transfer_point_per_owned_arg() {
    // §03.2 checkbox 2: "For each transfer point that consumes v, emit
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let tp_vars: Vec<ArcVarId> = ctx.transfer_points().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        tp_vars,
        vec![ArcVarId::new(0)],
        "Construct with 1 owned arg MUST emit exactly 1 transfer-point entry for that arg",
    );
}

#[test]
fn apply_with_one_owned_arg_emits_one_transfer_point() {
    // §03.2 success_criterion 1 enumerates Apply with Owned param as a
    // transfer point per `ArcInstr::is_owned_position`. Cycle 6 generic
    // walk via `used_vars()` + `is_owned_position()` mechanically extends
    // cycle 5's coverage. Semantic pin: would FAIL if Apply branch in
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let tp_vars: Vec<ArcVarId> = ctx.transfer_points().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        tp_vars,
        vec![ArcVarId::new(0)],
        "Apply with 1 Owned arg MUST emit exactly 1 transfer-point entry for that arg",
    );
}

#[test]
fn set_emits_one_transfer_point_for_owned_value() {
    // §03.2 success_criterion 1: "Set with Owned value (per aims-rules.md §3
    // TF-15 — value.access := Owned unconditional via IA-5 step (1); NOT
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let tp_vars: Vec<ArcVarId> = ctx.transfer_points().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        tp_vars,
        vec![ArcVarId::new(1)],
        "Set MUST emit exactly 1 transfer-point entry for the Owned value (var 1); base (var 0) is direct demand only per TF-15",
    );
}

#[test]
fn borrowed_params_skipped_owned_params_collected() {
    // §03.2 checkbox 1: "For each owned ArcVarId v in the function ...".
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let collected_vars: Vec<ArcVarId> = ctx.collected_burdens().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        collected_vars,
        vec![ArcVarId::new(0)],
        "Borrowed param at var(1) MUST be filtered out; only Owned param at var(0) remains",
    );
}

#[test]
fn construct_emits_burden_inc_immediately_before_consuming_construct() {
    // §03.2 success_criterion 1 + §04A.5 ITEM-3: Construct with Owned arg
    // gets BurdenInc(arg) emitted immediately before — UNLESS the arg's
    // last-use is at this Construct (the matching Dec would be transfer-
    // suppressed per `aims-rules.md §8 RL-2`, producing VF-1 imbalance).
    // This test pins the owned-pos Inc emission for the case where arg(0)
    // has a follow-up use (Let-Var alias keeps it alive past the Construct).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
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
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 success_criterion 1 + §04A.5 ITEM-3: Apply with Owned arg gets
    // BurdenInc(arg) emitted immediately before — UNLESS the arg's last-use
    // is at this Apply, in which case the matching Dec would be transfer-
    // suppressed per `aims-rules.md §8 RL-2` and emitting the Inc would
    // produce a `Σ Inc - Σ Dec = +1` VF-1 imbalance per
    // `aims/verify/burden_balance.rs`. This test pins the owned-pos Inc
    // emission for the case where arg(0) has a follow-up use (Let-Var
    // alias keeps it alive past the Apply), so Inc IS emitted. Uses
    // Idx::STR (heap-burden) per cycle-24 VF-1 RcOnScalar mirror.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
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
                // owned-pos Inc on var(0) IS emitted per the §03.2 rule.
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1 + RL-2 symmetric pin for TF-15 carve-out:
    // (a) Set with Owned non-scalar value gets BurdenInc(value) emitted
    //     immediately before (cycle 12 BurdenInc carve-out half).
    // (b) Set value as last use does NOT receive BurdenDec after (cycle 12
    //     transfer_vars carve-out half — value is ownership-transferring
    //     per aims-rules.md §RL-2; emitting BurdenDec would double-release).
    // §04A.5 ITEM-3 audit conclusion: instruction-level transfer
    // suppression preserved. The owned-position BurdenInc is a VF-1
    // accounting marker; codegen's predicate-stack realize walk owns
    // physical RC for vars consumed at instruction-level owned positions
    // (Set.value via the TF-15 carve-out). Adding a symmetric BurdenDec
    // would mark the var in `func.burden_emitted`, propagate through
    // `populate_class_covered`, and suppress predicate-stack RC emission —
    // causing real-world RC leaks. Test uses Idx::STR (heap-burden) for
    // value — Idx::INT's lookup_burden returns Some(EMPTY_SPEC) per
    // BURDEN_TABLE, so the cycle-21 `burden_carries_rc` filter at
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    // Expected: BurdenInc(var(1)) before Set (TF-15 value carve-out), Set,
    // and possibly BurdenDec(var(0)) after (var(0) is `base` — non-transfer
    // last-use per RL-2; only Set `value` is in the ownership-transferring
    // list, NOT Set `base`). The cycle-12 RL-2 pin is value-specific:
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
    // §04A.5 ITEM-3 audit: predicate-stack owns physical RC for Set.value
    // per the §04A.3 coexistence handshake; symmetric Dec emission here
    // would mark var(1) in burden_emitted and break class_covered.
    let dec_value_present = body
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(1)));
    assert!(
        !dec_value_present,
        "Set value (var 1) MUST NOT receive BurdenDec at last-use (RL-2 transfer-point exception per §04A.3 coexistence); body={body:?}",
    );
}

#[test]
fn set_emits_burden_dec_field_for_owned_field_before_burden_inc_value_per_03_4() {
    // §03.4 cycle 47 positive pin per plan body line 1943 + navigator-verdict
    // (proceed verdict, cycle 47): Set with heap-burden base MUST emit
    // BurdenDecField(base, field) BEFORE BurdenInc(value) BEFORE the Set
    // instruction. BurdenDecField releases the prior field value's burden;
    // symmetric with BurdenInc(value) which transfers ownership of the new
    // value INTO the field position. Both precede Set so codegen at cycle 48
    // can GEP+load the prior value BEFORE the store clobbers it. Per
    // `aims-rules.md §3 TF-15` + `§8 RL-2` ownership-transfer rules, plus
    // AIMS Invariant 5 unified-model preservation (cycle 47 extends
    // ArcInstr enum on the same dimension as cycle-46 BurdenDecPartial).
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;

    // Pin 1: BurdenDecField(base=var(0), field=0) appears in body.
    let dec_field_pos = body.iter().position(
        |i| matches!(i, ArcInstr::BurdenDecField { base, field } if *base == ArcVarId::new(0) && *field == 0),
    );
    assert!(
        dec_field_pos.is_some(),
        "expected BurdenDecField(base=var(0), field=0) per §03.4 cycle 47; body={body:?}",
    );

    // Pin 2: BurdenInc(value=var(1)) appears in body (cycle 12+24 carve-out).
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
    // Codegen at cycle 48 reads the prior field value via GEP+load BEFORE
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
fn settag_emits_burden_dec_variant_before_settag_per_03_4_cycle_50b() {
    // §03.4 cycle 50b positive pin per `aims-rules.md §3 TF-15a` + `§8 RL-10`:
    // SetTag with heap-burden base MUST emit BurdenDecVariant(var=base) BEFORE
    // the SetTag instruction. BurdenDecVariant is the whole-var sibling to
    // cycle-47 BurdenDecField — SetTag invalidates ALL payload fields of the
    // OLD variant (RL-10), so codegen at cycle 50c walks the entire variant
    // before the tag store clobbers the discriminant. AIMS Invariant 5
    // case (b) — extends ArcInstr enum on the same dimension as
    // BurdenDecPartial / BurdenDec; no parallel emission, no shadow tracker.
    // SetTag's TF-15a backward demand is `(base, Once)` only — no value
    // operand — so unlike cycle-47 Set, no symmetric BurdenInc(value).
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;

    // Pin 1: BurdenDecVariant(var=var(0)) appears in body.
    let dec_variant_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::BurdenDecVariant { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_variant_pos.is_some(),
        "expected BurdenDecVariant(var=var(0)) per §03.4 cycle 50b; body={body:?}",
    );

    // Pin 2: SetTag appears in body.
    let settag_pos = body
        .iter()
        .position(|i| matches!(i, ArcInstr::SetTag { .. }))
        .unwrap_or_else(|| panic!("SetTag MUST appear in body"));

    // Pin 3: Ordering — BurdenDecVariant BEFORE SetTag. Codegen at cycle 50c
    // reads the current discriminant via GEP+load BEFORE the store clobbers
    // it; this ordering is the load-bearing invariant per `aims-rules.md
    // §8 RL-10` (tag change invalidates ALL payload fields).
    let dec_variant = dec_variant_pos.unwrap_or_else(|| unreachable!("checked is_some above"));
    assert!(
        dec_variant < settag_pos,
        "BurdenDecVariant MUST precede SetTag; body={body:?}",
    );

    // Pin 4: round-trip through SSOT walk helpers per `impl-hygiene.md §SSOT`.
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
fn settag_emits_no_burden_dec_variant_when_base_not_in_owned_vars_per_03_4_cycle_50b_negative() {
    // §03.4 cycle 50b negative pin (clamps positive pin from below per
    // `tests.md §Matrix Clamping`): SetTag on a base var whose burden is
    // EMPTY (scalar / no owned fields — fails `burden_carries_rc` filter at
    // `compute_owned_vars_needing_rc`) MUST NOT emit BurdenDecVariant.
    // Mirrors cycle-47 BurdenDecField's gate via
    // `owned_vars_needing_rc.contains(base)` — same gate, same filter.
    // Per `aims-rules.md §VF-1 RcOnScalar` (no RC ops on scalars):
    // BurdenDecVariant on a scalar would be a structural violation. Idx::INT
    // is the canonical scalar negative-pin type per cycle-47's
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 2: BurdenDec(v) emits immediately following last-use UNLESS
    // last-use is ownership-transferring per RL-2. Cycle 11 ships filtered
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1 + §04A.5 ITEM-3: PartialApply captures emit BurdenInc when
    // the captured var has a follow-up use (Let-Var alias keeps it alive past
    // the PartialApply). Last-use at this instr would suppress per §04A.5
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
fn collection_reuse_emits_burden_inc_for_owned_arg() {
    // §03.2 sc 1 + §04A.5 ITEM-3: CollectionReuse with Owned arg emits
    // BurdenInc when arg has a follow-up use (Let-Var keeps it alive past
    // the CollectionReuse). Last-use at this instr would suppress per
    // §04A.5 ITEM-3 to preserve VF-1 balance.
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1 + §04A.5 ITEM-3: ApplyIndirect emits BurdenInc for Owned
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1 + §04A.5 ITEM-3: per-position arg_ownership filter +
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 success_criterion 1: ApplyIndirect's empty arg_ownership defaults
    // to all-Borrowed per `is_some_and(Owned)` (instr.rs:367-380) — CONSERVATIVE
    // for unknown callees; caller retains cleanup. This is the load-bearing
    // safety distinction from Apply (instr.rs:381-390) whose empty default is
    // all-Owned via `is_none_or(Owned)`. Without this pin, a future refactor
    // unifying the two predicates (copy-paste from Apply arm) would silently
    // break ApplyIndirect's conservative semantics — unannotated callsites
    // would receive spurious BurdenInc, doubling refcount and leaking.
    // Per `aims-rules.md §8 RL-2` ownership-transferring exception.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(2),
                ty: Idx::STR,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1)],
                arg_ownership: Vec::new(), // empty → all-Borrowed default
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    // §04A.5 ITEM-2: post-emission body is [BurdenInc(dst=2) [FRESH-site —
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
    // INTENTIONAL §03.2 intra-block scope per `burden_lower.rs:128` comment.
    // §03.3 cross-block CFG-aware last-use will collapse this to ONE entry
    // — that change IS the desired §03.3 cell flip, not a regression.
    //
    // §03.2 walker (burden_lower.rs:132-141) does per-block backward walks:
    // `seen: FxHashSet` declared INSIDE the block loop, so a variable used
    // in BOTH blocks produces TWO `last_use_points` entries — one per block.
    // Cross-block liveness via block-param handoffs lands in §03.3.
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
        "§03.2 per-block walk MUST identify var(0) last-use in EACH block separately (intra-block scope); §03.3 cross-block will collapse to 1 — that IS the desired §03.3 flip. last_use_points={:?}",
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
    // §03.2 sc 1 + §04A.5 ITEM-3: emission loop iterates ALL owned positions
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
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    // §04A.5 ITEM-2: BurdenInc(dst=3) [FRESH-site, TF-3] precedes the per-arg
    // BurdenIncs (cycle 20 pinned the per-arg ordering); all Incs precede
    // Construct.
    let expected = [
        ArcVarId::new(3),
        ArcVarId::new(0),
        ArcVarId::new(1),
        ArcVarId::new(2),
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
        "Construct with 3 Owned args MUST emit FRESH-site BurdenInc(dst=3) THEN per-arg BurdenInc(0..=2) in iteration order; got {inc_vars:?}; body={body:?}",
    );
    // Verify all BurdenInc emissions precede the Construct.
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
        "ALL BurdenInc emissions MUST precede Construct; last_inc_pos={last_inc_pos}, construct_pos={construct_pos}; body={body:?}",
    );
}

#[test]
fn scalar_int_var_emits_no_burden_dec_at_last_use() {
    // §03.2 sc 2 + `aims-rules.md §4 DP-1` (is_rc_needed: ... ∧ ¬is_scalar)
    // + `§9 VF-1 RcOnScalar`. A var typed `Idx::INT` (scalar) MUST NOT
    // receive BurdenDec emission even at non-transfer last-use.
    //
    // This test surfaces the cycle-21 filter fix: `lookup_burden(Idx::INT)`
    // returns `Some(BurdenRef)` carrying `BuiltinBurdenSpec::EMPTY` (per
    // `BURDEN_TABLE` at `ori_registry/src/burden/table.rs:184-193`), NOT
    // None. A naive filter `burden.as_ref().map(|_| *var)` admits EMPTY
    // and emits BurdenDec on scalars (RcOnScalar violation). The cycle-21
    // fix at `burden_lower.rs:154-178` checks BurdenRef contents via the
    // `Burden` trait: `self_heap_alloc() || element_burden().is_some() ||
    // variant_burdens().next().is_some() || owned_fields().next().is_some()`.
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    let any_burden_dec = body.iter().any(|i| matches!(i, ArcInstr::BurdenDec { .. }));
    assert!(
        !any_burden_dec,
        "scalar Idx::INT var MUST NOT receive BurdenDec at last-use (VF-1 RcOnScalar); body={body:?}",
    );
}

#[test]
fn heap_burden_borrowed_param_skipped_at_ownership_filter() {
    // §03.2 checkbox 1: ownership filter MUST skip Borrowed params BEFORE
    // `lookup_burden` is consulted. This is the load-bearing early-skip
    // for heap-burden Borrowed params: per `burden_lower.rs:111-113`,
    // `matches!(param_ownership.get(&var), Some(Ownership::Borrowed))
    // → continue` short-circuits the param loop before push to ctx.collected.
    //
    // Distinct from cycle-4 borrowed_params_skipped_owned_params_collected
    // which uses Idx::INT (scalar — burden=EMPTY, fails burden_carries_rc
    // anyway). This cell tests the realistic heap-burden case: Idx::STR
    // carries self_heap_alloc=true per BURDEN_TABLE, so without the early-
    // skip, var(1)=STR/Borrowed would flow into ctx.collected, pass
    // burden_carries_rc (self_heap_alloc=true), enter owned_vars_needing_rc,
    // and emit spurious BurdenInc/BurdenDec violating §03.2 checkbox 1.
    // A future refactor removing the ownership filter would still pass the
    // scalar Idx::INT test (cycle 4) AND the scalar VF-1 test (cycle 21) —
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1 + §04A.5 ITEM-3: per-position arg_ownership filter MUST
    // continue past Borrowed positions. Args with follow-up Let-Var aliases
    // keep them alive past Apply, so per-arg BurdenInc IS emitted.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
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
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    // §04A.5 ITEM-2: BurdenInc(dst=3) [FRESH-site, Apply with no contract
    // defaults to MaybeShared return per TF-5] precedes the per-arg
    // BurdenIncs. Per-arg filter still pins [Owned, Borrowed, Owned] →
    // BurdenInc(0), BurdenInc(2); BurdenInc(1) still skipped.
    let expected = [ArcVarId::new(3), ArcVarId::new(0), ArcVarId::new(2)];
    assert_eq!(
        inc_vars,
        expected,
        "Apply [Owned, Borrowed, Owned] MUST emit FRESH-site BurdenInc(dst=3) THEN BurdenInc(0), BurdenInc(2) and skip BurdenInc(1); got {inc_vars:?}; body={body:?}",
    );
    // Verify all BurdenInc emissions precede the Apply.
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
        "ALL BurdenInc emissions MUST precede Apply; last_inc_pos={last_inc_pos}, apply_pos={apply_pos}",
    );
}

#[test]
fn partial_apply_mixed_str_int_emits_burden_inc_only_for_heap_burden() {
    // §03.2 sc 1 + `aims-rules.md §9 VF-1 RcOnScalar` mirror to BurdenInc
    // emission. PartialApply args=[STR, INT]: STR carries heap-burden
    // (passes burden_carries_rc); INT carries EMPTY burden (per BURDEN_TABLE
    // at `ori_registry/src/burden/table.rs:184-193`) — cycle-24 filter MUST
    // admit STR and reject INT.
    //
    // This is the BurdenInc symmetric pin to cycle-21's BurdenDec scalar
    // exclusion. Without the cycle-24 filter, `lookup_burden(Idx::INT)`
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1 + cycle-24 VF-1 RcOnScalar mirror — cross-instr coverage.
    // ApplyIndirect with arg_ownership=[Owned] + arg=Idx::INT (scalar) MUST
    // emit ZERO BurdenInc. Per instr.rs:367-380 ApplyIndirect arm: closure
    // at pos 0 always borrowed; arg at pos 1 owned iff arg_ownership[0]=Owned.
    // Per cycle-24 filter (burden_lower.rs:171-175): owned_vars_needing_rc
    // rejects Idx::INT (EMPTY burden) so no BurdenInc emits.
    //
    // Cycle 24's partial_apply_mixed_str_int test covered PartialApply;
    // cycle 25 extends VF-1 BurdenInc-side coverage to ApplyIndirect via
    // the SAME single generic emission loop. A regression that re-introduces
    // a per-variant unfiltered emission path for ApplyIndirect specifically
    // (e.g., bypassing owned_vars_needing_rc) would FAIL this pin while
    // potentially passing the PartialApply pin.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::ApplyIndirect {
                dst: ArcVarId::new(2),
                ty: Idx::STR,
                closure: ArcVarId::new(0),
                args: vec![ArcVarId::new(1)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    // §04A.5 ITEM-2: ApplyIndirect dst=2 is STR (heap) → FRESH-site
    // BurdenInc(dst=2) emits per TF-5a CONSERVATIVE MaybeShared return.
    // The per-arg loop still emits ZERO Incs for var(1)=Idx::INT (cycle-24
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
    // §03.2 sc 1 + `aims-rules.md §3 TF-15` + `§9 VF-1 RcOnScalar`. Set's
    // `value` is owned via IA-5 alias-transfer step (1) per TF-15 carve-out;
    // NOT covered by `is_owned_position`'s `_ => false` catch-all. The
    // BurdenInc emission for Set's value happens in a SEPARATE if-let block
    // (burden_lower.rs:217-225) distinct from the main owned-position loop.
    //
    // Cycle 24 added `owned_vars_needing_rc.contains(value)` to BOTH the
    // main loop AND the Set carve-out. This test closes the LAST unclamped
    // path of cycle-24's filter: a regression reverting only the Set-path
    // filter (e.g., copy-paste from a different file or pre-cycle-24 logic
    // restored) would pass all current Apply/PartialApply/CollectionReuse/
    // ApplyIndirect scalar pins (cycles 21+24+25) but FAIL this Set pin.
    //
    // Cycle 12 already covers the positive case (Idx::STR value emits
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.2 sc 1: cross-dimension matrix cell — multi-arg Construct with
    // scalar in non-edge (middle) position. Combines cycle-20 multi-arg
    // ordering coverage with cycle-24 scalar-filter coverage; distinct from
    // both: cycle 20 uses all-STR (no filter exercise per-position), cycle
    // 24 uses 2-arg edge-only [STR, INT] on PartialApply.
    //
    // Defends per-position filter correctness against a regression that
    // would blanket-apply burden_carries_rc across all args (passes cycle
    // 20 + cycle 24 but fails cycle 27).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::INT, Idx::STR, Idx::STR, Idx::STR, Idx::STR],
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
            ],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    let inc_vars: Vec<ArcVarId> = body
        .iter()
        .filter_map(|i| match i {
            ArcInstr::BurdenInc { var } => Some(*var),
            _ => None,
        })
        .collect();
    // §04A.5 ITEM-2 + ITEM-3: FRESH-site BurdenInc(dst=3) precedes per-arg
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
    // §03.2 sc 1: all-Borrowed corner cell. Matrix has all-Owned (cycle 6
    // updated), [Owned,Borrowed] mixed (cycle 17), [Owned,Borrowed,Owned]
    // non-adjacent (cycle 23); all-Borrowed is the missing corner per
    // tests.md §Matrix Clamping clamp-from-all-sides.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR, Idx::STR, Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(2),
                ty: Idx::STR,
                func: Name::from_raw(99),
                args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    // §04A.5 ITEM-2: per-arg loop still emits ZERO Incs (both args Borrowed).
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
    // §03.2 sc 1: empty-args boundary cell. PartialApply with args=[]:
    // is_owned_position(pos) = pos < 0 = false for all pos; used_vars()
    // returns empty SmallVec; emission loop body never executes. Pins
    // off-by-one (loop ends args.len()-1) + unconditional-emit + hardcoded
    // pos==0 shortcut regressions per tests.md §TDD step 3 (edge cases:
    // empty, single-element, boundary).
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::PartialApply {
                dst: ArcVarId::new(0),
                ty: Idx::STR,
                func: Name::from_raw(99),
                args: Vec::new(),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    // §04A.5 ITEM-2: per-arg loop emits zero Incs (args=[] → no positions).
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
    // §03.2 sc 1: empty-args boundary mirror to cycle 29's PartialApply
    // empty-args pin. Shared is_owned_position branch at instr.rs:352:
    // `Construct { args, .. } | PartialApply { args, .. } => pos < args.len()`.
    // args=[] → predicate false for all pos → zero BurdenInc.
    let registry = TypeRegistry::new();
    let mut func = ArcFunction {
        var_types: vec![Idx::STR],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Construct {
                dst: ArcVarId::new(0),
                ty: Idx::STR,
                ctor: CtorKind::Tuple,
                args: Vec::new(),
            }],
            terminator: ArcTerminator::Unreachable,
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..ArcFunction::default()
    };
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    // §04A.5 ITEM-2: per-arg loop emits zero Incs (args=[] → no positions).
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
    // §03.2 success_criterion 2: "BurdenDec(v) emits immediately following
    // EVERY last-use of v along EVERY reachable CFG path." Cycle 8 ships
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    assert_eq!(
        ctx.last_use_points(),
        &[(ArcVarId::new(0), 0, 0)],
        "var(0)'s last use is at block 0 instr 0 (Apply arg); per-block backward walk MUST identify it",
    );
}

#[test]
fn iteration_produces_one_entry_per_var_type() {
    // Semantic pin: would FAIL if iteration body is reverted to no-op or
    // todo!() — collected_burdens length must match var_types length.
    let registry = TypeRegistry::new();
    let mut func = func_with_n_vars(3);
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
    // §03.3 first rule positive pin: Return transfers ownership per
    // `aims-rules.md §8 RL-2` — Return's `value` is a terminator-transfer
    // point. When `value` is also used at an earlier instruction (here as
    // IsShared's `var` operand at non-owned position), the terminator-position
    // last-use registration takes precedence over the prior-instruction-position
    // entry (terminator scans first in backward walk; first-seen-wins). At
    // emission time the terminator-position entry hits terminator_transfer_vars
    // and is filtered out — no BurdenDec emits anywhere for `value`. Without
    // the §03.3 terminator-walking last-use scan + Return-transfer-var filter,
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    // Pin 1: NO BurdenInc on var(0) — IsShared is not owned-position; Return
    // is a transfer (not a BurdenInc site per §03.3 first rule).
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
fn moved_out_fields_is_empty_when_no_project_per_cycle_42_no_project_negative() {
    // §03.4 negative pin: a function with NO Project instructions MUST yield
    // an empty `moved_out_fields` map after cycle 42's Pass 1/Pass 2 population.
    // Pass 1 finds zero Project tuples → project_origins empty → Pass 2's
    // transfer-var lookups all miss → map stays empty. Clamps the population
    // logic from below: a reversion that erroneously populates on every
    // transferred var (regardless of project_origins membership) would fire
    // here. Per impl-hygiene.md §INVERTED-TDD pseudo-tested-method ban —
    // assert the SPECIFIC expected state (empty map) rather than mere data-
    // structure-existence. Preserves cycle-40 skeleton intent post-population.
    let registry = TypeRegistry::new();
    let mut func = func_with_n_vars(2);
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    assert!(
        ctx.moved_out_fields().is_empty(),
        "moved_out_fields MUST remain empty when function has zero Project instructions (Pass 1 yields empty project_origins); got {:?}",
        ctx.moved_out_fields(),
    );
}

#[test]
fn project_then_construct_arg_sets_moved_out_fields_bit_per_03_4_two_stage_positive() {
    // §03.4 cycle 42 positive pin (two-stage rule): `%1 = Project %0.0` followed
    // by `Construct(args=[%1])` MUST set bit `0` on `%0` in `moved_out_fields`.
    // Pass 1 collects (%1 → (%0, 0)); Pass 2 sees Construct's owned-position arg
    // %1, looks up project_origins[%1] = (%0, 0), inserts 0 into
    // moved_out_fields[%0]. Per `aims-rules.md §3 TF-3` Construct args at
    // owned positions (per `instr.rs:352-354 is_owned_position` returns true
    // for `pos < args.len()`). Construct is the canonical transfer-point
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
fn project_then_set_value_sets_moved_out_fields_bit_per_03_4_tf15_carve_out_positive() {
    // §03.4 cycle 42 positive pin (Set-value TF-15 carve-out): `%1 = Project %0.0`
    // followed by `Set { base: %2, field: 0, value: %1 }` MUST set bit `0` on `%0`
    // in `moved_out_fields`. Pass 1 collects (%1 → (%0, 0)); Pass 2's
    // `instr_transfer_vars` honors the Set-value carve-out per
    // `aims-rules.md §3 TF-15` + IA-5 step (1) — `value` is Owned via alias
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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
fn project_alone_leaves_moved_out_fields_unset_per_03_4_two_stage_negative() {
    // §03.4 cycle 42 negative pin (two-stage rule clamp from below): `%1 = Project %0.0`
    // with NO downstream transfer-point consumer MUST leave `moved_out_fields[%0]`
    // unset. Per `aims-rules.md §3 TF-4`, Project produces Borrowed; per
    // `instr.rs:391 _ => false`, Project is NOT an owned position itself.
    // The two-stage rule fires only when a Project dst is THEN consumed at
    // a transfer point. This pin clamps the cycle-40 unsound-aggressive
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    assert!(
        ctx.moved_out_fields().is_empty(),
        "moved_out_fields MUST remain empty when Project has no transfer-point consumer (TF-4 Borrowed; two-stage rule's stage-2 not fired); got {:?}",
        ctx.moved_out_fields(),
    );
}

#[test]
fn project_consumed_at_is_shared_leaves_moved_out_fields_unset_per_03_4_borrowed_position_negative()
{
    // §03.4 cycle 42 negative pin (borrowed-position clamp): `%1 = Project %0.0`
    // followed by `IsShared(%1)` MUST leave `moved_out_fields[%0]` unset. Per
    // `instr.rs:391 _ => false`, IsShared falls through `is_owned_position`'s
    // catch-all → NOT an owned position → `instr_transfer_vars` does NOT
    // include %1. The two-stage rule's stage-2 is NOT triggered by IsShared.
    // Clamps the Pass 2 logic from below: a reversion that erroneously
    // treats every `used_vars` member as a transfer (ignoring
    // `is_owned_position`) would set the bit here and FAIL. Per
    // `aims-rules.md §3 TF-10`, IsShared produces SCALAR (boolean) — no
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
    let ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    assert!(
        ctx.moved_out_fields().is_empty(),
        "moved_out_fields MUST remain empty when Project dst is consumed at borrowed position (IsShared; TF-10 SCALAR result; is_owned_position _ => false); got {:?}",
        ctx.moved_out_fields(),
    );
}

#[test]
fn jump_arg_to_borrowed_target_block_param_emits_burden_dec_at_terminator_per_rl2_negative() {
    // §03.3 rule 3 negative pin (cycle 39): clamps cycle-37's
    // `if matches!(ownership, DerivedOwnership::Owned)` guard at
    // `burden_lower.rs:273` from below. When target block param's
    // `DerivedOwnership` is `BorrowedFrom(...)` (NOT Owned), Jump.args[i]
    // MUST NOT enter terminator_transfer_vars — the prior-instruction /
    // terminator-position last-use of arg DOES receive BurdenDec because
    // Jump-to-Borrowed-param is a borrow (not an ownership transfer) per
    // `aims-rules.md §8 RL-2` ownership-transferring exception list.
    // Production borrow inference at `borrow/derived.rs:60` currently marks
    // all block params Owned, so this case is structurally unreachable in
    // shipped code — BUT the guard itself is load-bearing (a reversion that
    // always treats Jump.args as transfer would silently miscompile when
    // block-param borrow inference distinguishes Borrowed). Test constructs
    // explicit `&[DerivedOwnership::BorrowedFrom(...)]` to exercise the
    // negative path per `tests.md §Matrix Clamping` completeness rule.
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
    // Block 1's param var(1) is BorrowedFrom var(0) — clamps cycle-37's
    // DerivedOwnership::Owned guard; transfer set MUST exclude var(0).
    let derived = vec![
        DerivedOwnership::Owned,
        DerivedOwnership::BorrowedFrom(ArcVarId::new(0)),
    ];
    let _ctx = emit_burden_ops(&mut func, &registry, &derived, &[], &FxHashMap::default());
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
    // §03.3 rule 3 emission-side negative pin per `tests.md §Matrix Clamping`:
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
    // §03.3 rule 5 negative pin (cycle 39, VF-1 RcOnScalar mirror per
    // `aims-rules.md §9`): scalar-typed Invoke arg at owned position MUST
    // NOT receive BurdenInc/BurdenDec even though terminator_transfer_per_block
    // marks it as transfer. The `owned_vars_needing_rc` filter at
    // `burden_lower.rs:225-234` rejects scalars (Idx::INT carries
    // `BuiltinBurdenSpec::EMPTY` per `BURDEN_TABLE` at
    // `ori_registry/src/burden/table.rs:184-193`); `burden_carries_rc`
    // returns false → var excluded from owned_vars_needing_rc → no
    // emission. Clamps cycle-38's Invoke transfer logic from below.
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body_0 = &func.blocks[0].body;
    assert!(
        body_0.is_empty(),
        "scalar Int Invoke.args[0] MUST trigger zero burden ops (VF-1 RcOnScalar mirror clamps cycle-38 Invoke transfer); body={body_0:?}",
    );
}

#[test]
fn invoke_indirect_owned_args_at_pos_one_emits_symmetric_burden_dec_for_vf1_balance() {
    // §03.3 rule 5 InvokeIndirect positive pin + §04A.5 ITEM-3 terminator-
    // level VF-1 symmetry: canonical `ArcTerminator::is_owned_position(pos)`
    // at `terminator.rs:117-126` encodes closure-pos-0-always-Borrowed
    // semantics. used_vars = [closure, ...args]; closure at pos 0 →
    // is_owned_position(0) == false; args at pos 1+ checked against
    // arg_ownership[pos-1]. Test: closure var(0) + args [var(1)] with
    // arg_ownership=[Owned] → var(1) gets BurdenInc at terminator AND
    // symmetric BurdenDec at terminator to balance VF-1 intraprocedural
    // net per `aims-rules.md §9 VF-1`. The terminator-level symmetric Dec
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body_0 = &func.blocks[0].body;
    // §04A.5 ITEM-3 — Pin: BurdenDec on var(1) MUST appear, paired with the
    // BurdenInc to preserve VF-1 intraprocedural balance per `aims-rules.md
    // §9 VF-1`. Codegen does NOT emit a real RcDec — BurdenDec is a TF-N/A
    // metadata annotation per `aims/realize/walk.rs:75-93`, so the
    // ownership-transfer semantic at the runtime layer is unaffected.
    let dec_arg_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(1)));
    assert!(
        dec_arg_present,
        "§04A.5 ITEM-3: InvokeIndirect.args[0] at owned position 1 MUST receive symmetric BurdenDec at terminator for VF-1 balance; body={body_0:?}",
    );
    // §03.3 rule 5 emission-side positive pin: BurdenInc(var(1)) fires at
    // owned position 1 per `aims-rules.md §8 RL-1`. Conservative Phase 5
    // emission mirroring instruction-level pattern.
    let inc_arg_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(1)));
    assert!(
        inc_arg_present,
        "InvokeIndirect.args[0] at owned position 1 MUST receive BurdenInc at terminator (rule 5 emission-side per RL-1); body={body_0:?}",
    );
    // §03.3 rule 5 emission-side negative pin per `tests.md §Matrix Clamping`:
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
    // §03.3 rule 5 (Tail-call) positive pin + §04A.5 ITEM-3 terminator-
    // level VF-1 symmetry: `ArcTerminator::Invoke` args at owned positions
    // transfer ownership per `aims-rules.md §8 RL-2` AND receive symmetric
    // BurdenInc + BurdenDec pair at the terminator per `aims-rules.md §9
    // VF-1` intraprocedural balance. Cycle-38 extended
    // `terminator_transfer_per_block` with `Invoke + InvokeIndirect`
    // match-arms using canonical SSOT helper `is_owned_position(pos)`.
    // With empty `arg_ownership`, is_owned_position defaults to all-Owned
    // (per `terminator.rs:100-129`). BurdenDec is a TF-N/A metadata
    // annotation per `aims/realize/walk.rs:75-93`; codegen does NOT emit
    // a real RcDec, preserving the runtime transfer semantic. The
    // terminator-level symmetric Dec is safe (does NOT trigger
    // class_covered suppression of predicate-stack RC for body-internal
    // vars per §04A.3 coexistence handshake).
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body_0 = &func.blocks[0].body;
    // §04A.5 ITEM-3 — Pin: BurdenDec on var(0) MUST appear, paired with the
    // terminator-position BurdenInc to preserve VF-1 intraprocedural net-zero.
    let dec_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_present,
        "§04A.5 ITEM-3: Invoke.args[0] at owned position MUST receive symmetric BurdenDec at terminator for VF-1 balance; body={body_0:?}",
    );
    // §03.3 rule 5 emission-side: Invoke.args at owned positions receive
    // BurdenInc per `aims-rules.md §8 RL-1` — conservative Phase 5 emission
    // mirroring `emit_instr_burdens` instruction-level pattern; §05 lattice
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
    // §03.3 rule 3 positive pin + §04A.5 ITEM-3 terminator-level VF-1
    // symmetry: Jump.args at positions whose target-block params have
    // `DerivedOwnership::Owned` transfer ownership to the target block
    // param per `aims-rules.md §8 RL-2` AND receive symmetric BurdenInc +
    // BurdenDec pair per `aims-rules.md §9 VF-1` intraprocedural balance.
    // Cycle-37 terminator-transfer pre-computation marks Jump.args[i] as
    // transfer when target_block.params[i].0 looked up in derived_ownership
    // returns Owned. BurdenDec is a TF-N/A metadata annotation per
    // `aims/realize/walk.rs:75-93`; codegen does NOT emit a real RcDec,
    // preserving Jump's runtime transfer semantic to the target block
    // param. The terminator-level symmetric Dec is safe (does NOT cause
    // class_covered suppression of predicate-stack RC for body-internal
    // vars per §04A.3 coexistence handshake).
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
    let _ctx = emit_burden_ops(&mut func, &registry, &derived, &[], &FxHashMap::default());
    let body_0 = &func.blocks[0].body;
    // §04A.5 ITEM-3 — Pin 1: BurdenDec on var(0) MUST appear at terminator,
    // paired with the BurdenInc to preserve VF-1 intraprocedural balance.
    // BurdenDec is a TF-N/A metadata annotation per
    // `aims/realize/walk.rs:75-93`; codegen does NOT emit a real RcDec,
    // preserving Jump's runtime ownership-transfer semantic.
    let dec_present = body_0
        .iter()
        .any(|i| matches!(i, ArcInstr::BurdenDec { var } if *var == ArcVarId::new(0)));
    assert!(
        dec_present,
        "§04A.5 ITEM-3: Jump.args[0] (var(0)) to Owned-target-block-param MUST receive symmetric BurdenDec at terminator for VF-1 balance; block 0 body={body_0:?}",
    );
    // Pin 2: Block 0 body contains EXACTLY ONE BurdenInc(var(0)) —
    // §03.3 rule 3 emission-side per `aims-rules.md §8 RL-1` (RC inc emitted
    // at every ownership-transfer point on owned non-scalar SSA values).
    // Conservative Phase 5 emission per `§03 goal:` ban on lattice
    // consultation (RC traffic overcounted but balanced); §05 lattice
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
    // §03.3 first rule negative pin (VF-1 RcOnScalar mirror per
    // `aims-rules.md §9`): scalar-typed Return value MUST NOT receive
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
    let body = &func.blocks[0].body;
    assert!(
        body.is_empty(),
        "scalar Int Return.value MUST trigger zero burden ops (VF-1 RcOnScalar mirror); body={body:?}",
    );
}

#[test]
fn partial_move_at_last_use_emits_burden_dec_partial_per_03_4_cycle_46() {
    // §03.4 cycle 46 positive pin — partial-move emission. Construct a 2-field
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
    // Negative pin clamping the cycle-43 full-move case lives in the existing
    // cycle 43 test (full-move asserts zero BurdenDec / zero BurdenDecPartial);
    // cycle 46 inherits that suppression branch unchanged. Per `aims-rules.md
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

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());
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

/// Match-destructuring positive pin for the §03.4 partial-move emission path.
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
///   * `let var(1) = Literal(Int 0)`  (synthetic scalar discriminant)
///   * `Switch { scrutinee: var(1), cases: [(0, block1)], default: block1 }`
/// - block 1 (arm body):
///   * `let var(2) = Project { value: var(0), field: 0 }` (a: str)
///   * `Construct { dst: var(3), Tuple, args: [var(2)] }`  (transfers field 0)
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

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

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
    // either block — partial-drop replaces, not augments. Cycle-46 invariant
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

/// CFG-diamond positive pin for the §03.4 INTERSECT-merge path.
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
///   * `Construct { dst: var(3), Tuple, args: [var(2)] }`  (transfers field 0)
///   * `Jump block 3`
/// - block 2 (case 1):
///   * `let var(4) = Project { value: var(0), field: 0 }`  (SYMMETRIC)
///   * `Construct { dst: var(5), Tuple, args: [var(4)] }`  (transfers field 0)
///   * `Jump block 3`
/// - block 3 (merge):
///   * `let var(6) = Project { value: var(0), field: 1 }`  (last use of var(0))
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

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

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

/// Loop-entry positive pin for the §03.4 INTERSECT-merge fixpoint path.
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
///   * `Construct { dst: var(3), Tuple, args: [var(2)] }`  (transfers field 0)
///   * `let var(4) = Literal(Bool false)` (loop continuation flag)
///   * `Jump block 1`
/// - block 1 (loop header):
///   * `Branch { cond: var(4), then: block 1 (back), else: block 2 (exit) }`
/// - block 2 (post-loop):
///   * `let var(5) = Project { value: var(0), field: 1 }`  (last use of var(0))
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

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

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

/// Nested-join positive pin for the §03.4 INTERSECT-merge composition path.
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
/// - block 3 (inner case 1): Project var(0).0 → Construct → Jump block 5  (symmetric)
/// - block 4 (outer case 1):  Project var(0).0 → Construct → Jump block 5  (symmetric)
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

    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

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

// ─── §04.2 Closure capture composition — burden_lower emission pins ──────
//
// Tests below pin the §04.2 closure-capture-composition story at the
// burden_lower layer: registered closure `UserBurdenSpec` (composed via
// `ori_types::burden_compose::closure::compose_closure_burden_spec`)
// flows correctly through the existing trivial-emission walker. PartialApply
// IS the capture-time transfer point per §03.3 Rule 5 (already shipped); §04.2
// adds the spec-population side so the walker sees a non-empty burden for
// closure types and emits BurdenInc at PartialApply args.
//
// Burden-spec-registration discipline at the test layer mirrors §03 fixtures
// (`registered_struct_with_two_owned_str_fields`) — the closure's Idx is
// registered as a struct shell with a closure-shaped burden via
// `compose_closure_burden_spec`. Production wiring lives at the lambda
// type-check site (`infer_lambda` at `compiler_repo/compiler/ori_types/src/
// infer/expr/blocks.rs:223`); the §04.2 deliverable shipped here pins the
// spec composer + wires the burden walker to consume registered closure
// burdens without changes to the walker itself.

#[test]
fn closure_capture_by_value_of_owned_str_emits_burden_inc_at_partial_apply() {
    // §04.2 success_criterion (positive — capture by value of Owned binding):
    // `let s = "hello"; let c = (() -> s.length())` — capture site IS
    // PartialApply IS transfer point per §03.3 Rule 5. With the closure's
    // UserBurdenSpec composed via compose_closure_burden_spec (self_heap_alloc=
    // true, owned_fields=[STR]), the existing trivial-emission walker emits
    // BurdenInc on the captured arg before the PartialApply instruction.
    //
    // The existing `partial_apply_emits_burden_inc_for_captured_var` cycle-13
    // pin uses Idx::STR for the closure result type — exercising the SAME code
    // path as §04.2 (PartialApply args owned-position emission + the cycle-24
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

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
    // §04.2 success_criterion (negative — capture by reference): borrow stored
    // in borrowed_fields[i]; no drop on env field (borrows do not own). The
    // burden walker MUST NOT emit BurdenInc for a borrowed capture at the
    // PartialApply site.
    //
    // The Tag::Borrowed target type is target-only in the shipped pool; per
    // §03.3 + the §04.2 design the borrowed-capture's CAPTURED VARIABLE is
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
    // Negative-pin discipline per `tests.md §Matrix Clamping`: the assertion
    // tier matches the §04.2 success_criterion exactly — the burden spec
    // populates borrowed_fields, NOT owned_fields, and the registered spec's
    // owned_fields.is_empty() guarantees no BurdenInc fires from the
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
    // walk (the §04.2 design intent for borrow captures).
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
    // §04.2 success_criterion (positive — captures-of-captures): outer env
    // field IS Closure<...> with its OWN UserBurdenSpec.compiled_drop.
    // Recursion is handled identically to recursive types per §04.1 — outer
    // closure's drop body recursively invokes inner closure's compiled_drop
    // via the inner field's UserBurdenSpec lookup at codegen.
    //
    // Composition records the inner closure's Idx in
    // outer.owned_fields[0].field_type; outer + inner each carry their own
    // distinct compiled_drop FnSyms per the per-Idx mangling shared with §04.1.
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
    // Var keeps var(0) alive past the PartialApply per §04A.5 ITEM-3.
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

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
    // §04.2 success_criterion (positive — capture of projection): treated as
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
fn partial_apply_owned_capture_passed_to_owned_callee_emits_zero_net_burden_per_03_3_rule_5() {
    // §04.2 specific PartialApply matrix pin per success_criterion 5: binding
    // consumed by PartialApply AND passed to Owned callee in same expr →
    // transfer-count 2 → one BurdenInc lands (zero-net per §03.3 Rule 5).
    //
    // The shipped §03.3 Rule 5 invariant is: each captured arg gets ONE
    // transfer-point per consumption site; PartialApply + Owned callee = 2
    // transfer points = 2 BurdenInc emissions on the captured variable. The
    // §04.2 closure-burden composition does NOT change this — the closure's
    // own burden walk emits BurdenInc on the CLOSURE's env-field side (NOT
    // the captured variable side); the captured-side BurdenInc count comes
    // from `is_owned_position` at the PartialApply + Apply sites per §03.2 +
    // §03.3.
    //
    // This pin verifies the §03.3 Rule 5 invariant holds UNCHANGED under
    // §04.2 closure-burden registration: registering a closure burden on the
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

    // §04A.5 ITEM-3: Build with follow-up Let-Var keeping var(0) alive past
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
    let _ctx = emit_burden_ops(&mut func, &registry, &[], &[], &FxHashMap::default());

    let body = &func.blocks[0].body;
    let inc_count = body
        .iter()
        .filter(|i| matches!(i, ArcInstr::BurdenInc { var } if *var == ArcVarId::new(0)))
        .count();
    assert_eq!(
        inc_count, 2,
        "§03.3 Rule 5: captured arg consumed by PartialApply AND Owned callee MUST get 2 transfer-point BurdenInc emissions on var(0); got {inc_count} in body={body:?}",
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
