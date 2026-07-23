//! Mechanical lowering from whole-var and field-grain burden operations to
//! realized RC instructions.
//!
//! This module owns the final burden-to-RC spelling boundary.

use std::sync::LazyLock;

use ori_types::{Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, RcAtomicity, RcStrategy};
use crate::lower::type_has_user_drop;

/// Lowers whole-variable and field-grain burden operations to RC instructions.
///
/// Whole-variable operations use the representation's canonical strategy and
/// atomic RC. Field-grain decrements retain their partial cleanup obligations;
/// lowering them to a whole-variable decrement would double-drop fields.
/// Scalar or out-of-range representations remain burden operations. Each entry
/// in `elidable_fresh_incs` omits one redundant increment because allocation
/// supplies the lineage's initial `+1`.
///
/// Spec: Annex E §AIMS RL-comp (lowered `BurdenInc`/`BurdenDec` net-preservation).
pub(super) fn lower_burden_ops_to_rc(
    func: &mut ArcFunction,
    pool: &Pool,
    type_registry: &TypeRegistry,
    elidable_fresh_incs: &FxHashSet<ArcVarId>,
) {
    let mut fresh_inc_elided: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut elided_sites: Vec<(usize, usize)> = Vec::new();
    let lower_field_grain = !field_grain_dec_lowering_disabled();
    for block_idx in 0..func.blocks.len() {
        let body_len = func.blocks[block_idx].body.len();
        for instr_idx in 0..body_len {
            if lower_field_grain && respell_field_grain_dec(func, block_idx, instr_idx) {
                continue;
            }
            let (ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var }) =
                func.blocks[block_idx].body[instr_idx]
            else {
                continue;
            };
            // Allocation supplies the first fresh-owner credit; later burden
            // increments are genuine duplicate-alias acquisitions.
            if matches!(
                func.blocks[block_idx].body[instr_idx],
                ArcInstr::BurdenInc { .. }
            ) && elidable_fresh_incs.contains(&var)
                && fresh_inc_elided.insert(var)
            {
                elided_sites.push((block_idx, instr_idx));
                continue;
            }
            // RE-2 backstop: class-ledger emission never emits whole-var burden
            // ops on a repr-less var. An absent repr here is a contract violation;
            // leave the burden op in place rather than emit unsound RC.
            let Some(repr) = func.var_repr(var) else {
                continue;
            };
            let ty = func.var_type(var);
            let has_user_drop = type_has_user_drop(ty, type_registry);
            // Scalars have no count obligation unless RL-DROP requires their
            // user drop strategy.
            if matches!(repr, crate::ir::ValueRepr::Scalar) && !has_user_drop {
                continue;
            }
            // Why: scalar+`@drop` has no RC fields → `UserDrop` (the `@drop` call
            // alone, balance-neutral); heap-field+`@drop` → `AggregateFields` (run
            // `@drop` THEN walk RC fields). Spec: Annex E §AIMS RL-DROP.
            let strategy = if has_user_drop && matches!(repr, crate::ir::ValueRepr::Scalar) {
                RcStrategy::UserDrop
            } else if has_user_drop {
                RcStrategy::AggregateFields
            } else {
                RcStrategy::from_repr(repr, pool, ty)
            };
            let atomicity = RcAtomicity::default_atomic();
            let lowered = match func.blocks[block_idx].body[instr_idx] {
                ArcInstr::BurdenInc { var } => ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                    atomicity,
                },
                ArcInstr::BurdenDec { var } => ArcInstr::RcDec {
                    var,
                    strategy,
                    atomicity,
                },
                ArcInstr::Let { .. }
                | ArcInstr::Apply { .. }
                | ArcInstr::ApplyIndirect { .. }
                | ArcInstr::PartialApply { .. }
                | ArcInstr::Project { .. }
                | ArcInstr::Construct { .. }
                | ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::RcDecPartial { .. }
                | ArcInstr::RcDecField { .. }
                | ArcInstr::RcDecVariant { .. }
                | ArcInstr::BurdenDecPartial { .. }
                | ArcInstr::BurdenDecField { .. }
                | ArcInstr::BurdenDecVariant { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Reuse { .. }
                | ArcInstr::CollectionReuse { .. }
                | ArcInstr::Select { .. } => {
                    unreachable!("filtered to whole-var burden ops above")
                }
            };
            func.blocks[block_idx].body[instr_idx] = lowered;
        }
    }
    if !elided_fresh_inc_removal_disabled() {
        remove_elided_fresh_inc_sites(func, &elided_sites);
    }
}

// Env: ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL - retains elided fresh-site markers, debug-only.
static ELIDED_FRESH_INC_REMOVAL_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    report_elided_fresh_inc_removal_toggle(
        std::env::var("ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL").as_deref() == Ok("1"),
    )
});

fn report_elided_fresh_inc_removal_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL",
            effect = "retain elided fresh-site BurdenInc markers",
            "ablation toggle fired"
        );
    }
    disabled
}

fn elided_fresh_inc_removal_disabled() -> bool {
    *ELIDED_FRESH_INC_REMOVAL_DISABLED
}

// Env: ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING - retains burden-spelled field decrements, debug-only.
static FIELD_GRAIN_DEC_LOWERING_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    report_field_grain_dec_lowering_toggle(
        std::env::var("ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING").as_deref() == Ok("1"),
    )
});

fn report_field_grain_dec_lowering_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING",
            effect = "retain field-grain decrements in burden spelling",
            "ablation toggle fired"
        );
    }
    disabled
}

fn field_grain_dec_lowering_disabled() -> bool {
    *FIELD_GRAIN_DEC_LOWERING_DISABLED
}

/// RE-2 backstop for the field-grain re-spelling: a field-grain dec on a
/// provably-Scalar (or repr-unpopulated) subject is an upstream admission
/// contract violation — leave it burden-spelled so the Step-11 census surfaces
/// it instead of emitting drop glue against a header-less value.
fn field_grain_repr_lowerable(func: &ArcFunction, var: ArcVarId) -> bool {
    match func.var_repr(var) {
        Some(crate::ir::ValueRepr::Scalar) | None => false,
        Some(_) => true,
    }
}

/// Phase-7 field-grain dec re-spelling at one instruction slot:
/// `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` become their
/// realized `RcDecPartial` / `RcDecField` / `RcDecVariant` forms (same
/// drop-glue codegen arm; out of the Step-11 burden census). Returns `true`
/// when the slot held a field-grain dec (re-spelled, or left in place by the
/// [`field_grain_repr_lowerable`] RE-2 backstop so the census surfaces it).
fn respell_field_grain_dec(func: &mut ArcFunction, block_idx: usize, instr_idx: usize) -> bool {
    let realized = match &func.blocks[block_idx].body[instr_idx] {
        ArcInstr::BurdenDecPartial {
            var,
            skip_fields: _,
        } if field_grain_repr_lowerable(func, *var) => ArcInstr::RcDecPartial {
            var: *var,
            skip_fields: Vec::new(),
        },
        ArcInstr::BurdenDecField { base, field } if field_grain_repr_lowerable(func, *base) => {
            ArcInstr::RcDecField {
                base: *base,
                field: *field,
            }
        }
        ArcInstr::BurdenDecVariant { var } if field_grain_repr_lowerable(func, *var) => {
            ArcInstr::RcDecVariant { var: *var }
        }
        ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. } => return true,
        _ => return false,
    };
    func.blocks[block_idx].body[instr_idx] = realized;
    true
}

/// Remove the elided fresh-site `BurdenInc` instructions at `sites`
/// (`(block_idx, instr_idx)` pairs recorded by [`lower_burden_ops_to_rc`]).
/// An elided op is GONE from the op stream — the VF-1 whole-var ledger
/// (`verify_burden_balance`) counts surviving burden ops, so a retained no-op
/// marker would net `+1` at every function exit through its definition.
fn remove_elided_fresh_inc_sites(func: &mut ArcFunction, sites: &[(usize, usize)]) {
    if sites.is_empty() {
        return;
    }
    let mut by_block: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    for &(b, i) in sites {
        by_block.entry(b).or_default().insert(i);
    }
    for (block_idx, remove) in by_block {
        let body = &mut func.blocks[block_idx].body;
        let mut idx = 0usize;
        body.retain(|_| {
            let keep = !remove.contains(&idx);
            idx += 1;
            keep
        });
    }
}

#[cfg(test)]
mod toggle_tests {
    crate::test_helpers::ablation_env_event_test!(
        elided_fresh_inc_removal_reproduces_marker_retention,
        "ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL",
        "retain elided fresh-site BurdenInc markers",
        || {
            let var = crate::test_helpers::v(0);
            let mut func = crate::test_helpers::make_func(
                Vec::new(),
                ori_types::Idx::UNIT,
                vec![crate::test_helpers::make_block(
                    crate::test_helpers::b(0),
                    vec![crate::ArcInstr::BurdenInc { var }],
                    crate::ArcTerminator::Unreachable,
                )],
                vec![ori_types::Idx::STR],
            );
            let elidable = std::iter::once(var).collect();

            super::lower_burden_ops_to_rc(
                &mut func,
                &ori_types::Pool::default(),
                &ori_types::TypeRegistry::default(),
                &elidable,
            );

            assert!(matches!(
                func.blocks[0].body.as_slice(),
                [crate::ArcInstr::BurdenInc { var: retained }] if *retained == var
            ));
            super::elided_fresh_inc_removal_disabled()
        },
    );

    crate::test_helpers::ablation_env_event_test!(
        field_grain_dec_lowering_reproduces_burden_spelling,
        "ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING",
        "retain field-grain decrements in burden spelling",
        || {
            let var = crate::test_helpers::v(0);
            let mut func = crate::test_helpers::make_func(
                Vec::new(),
                ori_types::Idx::UNIT,
                vec![crate::test_helpers::make_block(
                    crate::test_helpers::b(0),
                    vec![crate::ArcInstr::BurdenDecPartial {
                        var,
                        skip_fields: vec![0],
                    }],
                    crate::ArcTerminator::Unreachable,
                )],
                vec![ori_types::Idx::STR],
            );
            func.var_reprs = vec![crate::ValueRepr::RcPointer];

            super::lower_burden_ops_to_rc(
                &mut func,
                &ori_types::Pool::default(),
                &ori_types::TypeRegistry::default(),
                &rustc_hash::FxHashSet::default(),
            );

            assert!(matches!(
                func.blocks[0].body.as_slice(),
                [crate::ArcInstr::BurdenDecPartial { var: retained, skip_fields }]
                    if *retained == var && skip_fields == &[0]
            ));
            super::field_grain_dec_lowering_disabled()
        },
    );
}
