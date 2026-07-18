//! Phase 7 mechanical lowering: surviving whole-var and field-grain burden
//! ops to their realized RC instructions.
//!
//! This module owns the final burden-to-RC spelling boundary.

use std::sync::LazyLock;

use ori_types::{Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, RcAtomicity, RcStrategy};
use crate::lower::type_has_user_drop;

/// Phase 7 (probe): mechanically lower surviving whole-var burden ops to the
/// shipped transitional RC carrier.
///
/// `BurdenInc { var }` → `RcInc { var, count: 1, strategy, atomicity }` and
/// whole-var `BurdenDec { var }` → `RcDec { var, strategy, atomicity }`, with
/// the canonical `RcStrategy::from_repr` (same strategy the predicate-stack
/// emitter embeds) and compatibility `atomicity = Atomic`. The latter preserves
/// the shipped compiled runtime; AIMS freezes thread reachability rather than
/// selecting this physical mechanism.
///
/// Field-grain `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` are
/// rewritten to their REALIZED spellings (`RcDecPartial` / `RcDecField` /
/// `RcDecVariant`) with the same logical partial, variant, and replacement
/// cleanup obligations, never a whole-var `RcDec` (would double-drop). The re-spelling takes the lowered op
/// OUT of the Step-11 burden census: the VF-1 whole-var ledger counts SURVIVING
/// burden ops, and a mechanically-lowered op must leave the burden stream with
/// its pair partner (the whole-var acquire inc lowers to `RcInc` in the same
/// pass) — a half-pair surviving in burden spelling nets `-1` at every exit
/// through its path and aborts gated runs (Spec: Annex E §AIMS RL-comp
/// net-preservation). `ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1` restores the
/// legacy burden-spelled survival for bisection.
///
/// `Scalar` reprs cannot reach here from class-ledger emission: state-map
/// exclusion and class admission consult this same `var_reprs` source (Spec:
/// Annex E §AIMS RE-2 / DP-1 / L-9). A `Scalar` or out-of-range `var_repr`
/// leaves the burden op in place rather than synthesizing an unsound `RcDec`.
///
/// `elidable_fresh_incs` (per `compute_elidable_fresh_self_alloc_incs`): FRESH
/// self-allocation `BurdenInc` def-sites whose paired fresh inc is REDUNDANT
/// under lowering — the allocation already supplies the lineage's `+1`. The
/// FIRST `BurdenInc` encountered for such a var is REMOVED (an elided op is
/// gone from the op stream, keeping the VF-1 whole-var ledger net-0; a
/// surviving no-op marker would count `+1` at function exit and abort gated
/// runs). Subsequent `BurdenInc`s for the same var (genuine dup-alias
/// acquires) still lower — only the ONE redundant fresh-site inc per var is
/// elided. `ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL=1` restores the legacy
/// no-op-marker form (codegen no-ops it) for bisection.
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
                _ => unreachable!("filtered to whole-var burden ops above"),
            };
            func.blocks[block_idx].body[instr_idx] = lowered;
        }
    }
    if !elided_fresh_inc_removal_disabled() {
        remove_elided_fresh_inc_sites(func, &elided_sites);
    }
}

/// `ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL=1` keeps each elided fresh-site
/// `BurdenInc` as a codegen-no-op marker instead of removing it. Bisection
/// surface: isolates a behavior change to the marker removal vs the elision
/// verdict. Default (unset): elided incs are removed.
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

/// `ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1` keeps `BurdenDecPartial` /
/// `BurdenDecField` / `BurdenDecVariant` in burden spelling through Phase 7
/// (the legacy half-pair shape the Step-11 VF-1 ledger nets `-1`). Bisection
/// surface: isolates a gated-verification change to the field-grain
/// re-spelling vs the rest of the lowering. Default (unset): field-grain decs
/// lower to `RcDecPartial` / `RcDecField` / `RcDecVariant`.
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
        ArcInstr::BurdenDecPartial { var, skip_fields }
            if field_grain_repr_lowerable(func, *var) =>
        {
            ArcInstr::RcDecPartial {
                var: *var,
                skip_fields: skip_fields.clone(),
            }
        }
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
    #[test]
    fn elided_fresh_inc_removal_toggle_reports_effect() {
        crate::test_helpers::assert_ablation_env_event(
            concat!(
                module_path!(),
                "::elided_fresh_inc_removal_toggle_reports_effect"
            ),
            "ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL",
            "retain elided fresh-site BurdenInc markers",
            super::elided_fresh_inc_removal_disabled,
        );
    }

    #[test]
    fn field_grain_dec_lowering_toggle_reports_effect() {
        crate::test_helpers::assert_ablation_env_event(
            concat!(
                module_path!(),
                "::field_grain_dec_lowering_toggle_reports_effect"
            ),
            "ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING",
            "retain field-grain decrements in burden spelling",
            super::field_grain_dec_lowering_disabled,
        );
    }
}
