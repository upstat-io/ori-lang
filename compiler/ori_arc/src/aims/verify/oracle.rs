//! Contract coherence oracle.
//!
//! Walks realized ARC IR and re-derives a contract from the actual RC
//! operations emitted. Compares against the inferred [`MemoryContract`]
//! to verify coherence — catching bugs where analysis infers a correct
//! contract but realization emits inconsistent RC instructions, or where
//! the analysis infers an incorrect contract that happens to produce
//! working code by accident.
//!
//! Layer 3 of the AIMS verification stack (see `.claude/rules/arc.md`
//! §Verification Surface). Checks access, consumption, `may_share`, and
//! effect dimensions — directionally tolerant (conservative inference OK,
//! unsafe optimistic inference is a blocking error).

use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::{AccessClass, Consumption};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

// Alias tracking

/// Maps every `ArcVarId` that is an alias of a function parameter to its
/// parameter index. Handles transitive aliasing through `Let { value: Var(_) }`
/// chains AND block-parameter propagation through `Jump` terminators.
fn build_param_alias_map(func: &ArcFunction) -> FxHashMap<ArcVarId, usize> {
    let mut alias_map: FxHashMap<ArcVarId, usize> = FxHashMap::default();

    // Seed: direct parameter variables.
    for (i, param) in func.params.iter().enumerate() {
        alias_map.insert(param.var, i);
    }

    // Fixpoint loop: propagate aliases through BOTH Let { value: Var(_) }
    // bindings AND Jump → block-param edges. Both must be inside the loop
    // because a Jump may introduce a block-param alias that a subsequent
    // Let re-aliases — the Let pass must see those block-param aliases.
    // Matches the canonical pattern in interprocedural/extract.rs:244-277.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            changed |= propagate_let_aliases(block, &mut alias_map);
            changed |= propagate_jump_aliases(block, func, &mut alias_map);
        }
    }

    alias_map
}

/// Propagate aliases through `Let { value: Var(_) }` bindings in a block.
fn propagate_let_aliases(
    block: &crate::ir::ArcBlock,
    alias_map: &mut FxHashMap<ArcVarId, usize>,
) -> bool {
    let mut changed = false;
    for instr in &block.body {
        let ArcInstr::Let {
            dst,
            value: ArcValue::Var(src),
            ..
        } = instr
        else {
            continue;
        };
        let Some(&param_idx) = alias_map.get(src) else {
            continue;
        };
        if !alias_map.contains_key(dst) {
            alias_map.insert(*dst, param_idx);
            changed = true;
        }
    }
    changed
}

/// Propagate aliases through Jump → block-param edges.
fn propagate_jump_aliases(
    block: &crate::ir::ArcBlock,
    func: &ArcFunction,
    alias_map: &mut FxHashMap<ArcVarId, usize>,
) -> bool {
    let ArcTerminator::Jump { target, args } = &block.terminator else {
        return false;
    };
    let Some(target_block) = func.blocks.iter().find(|b| b.id == *target) else {
        return false;
    };
    let mut changed = false;
    for (bp, arg) in target_block.params.iter().zip(args.iter()) {
        let Some(&param_idx) = alias_map.get(arg) else {
            continue;
        };
        if alias_map.insert(bp.0, param_idx).is_none() {
            changed = true;
        }
    }
    changed
}

// Per-parameter observation

/// Per-parameter observations from walking realized IR.
#[derive(Clone, Debug, Default)]
struct ParamObservation {
    /// Total RC increments (accounting for `RcInc.count` batching).
    rc_incs: u32,
    /// Total RC decrements.
    rc_decs: u32,
    /// Number of non-RC uses (appearances at non-owned positions).
    non_rc_uses: u32,
    /// Whether the param was passed to an owned position in any instruction.
    has_owned_transfer: bool,
}

/// Derive per-parameter observations from the realized (post-pipeline) ARC IR
/// using the alias map. Handles batched `RcInc.count`, ownership transfers via
/// `is_owned_position()`, and explicit `Return` handling.
fn derive_param_observations(
    func: &ArcFunction,
    alias_map: &FxHashMap<ArcVarId, usize>,
) -> Vec<ParamObservation> {
    let num_params = func.params.len();
    let mut obs = vec![ParamObservation::default(); num_params];

    for block in &func.blocks {
        observe_block_body(block, alias_map, &mut obs);
        observe_block_terminator(block, alias_map, &mut obs);
    }

    obs
}

/// Record observations from instructions in a block body.
fn observe_block_body(
    block: &crate::ir::ArcBlock,
    alias_map: &FxHashMap<ArcVarId, usize>,
    obs: &mut [ParamObservation],
) {
    for instr in &block.body {
        match instr {
            ArcInstr::RcInc { var, count, .. } => {
                if let Some(&idx) = alias_map.get(var) {
                    obs[idx].rc_incs += count;
                }
            }
            ArcInstr::RcDec { var, .. } => {
                if let Some(&idx) = alias_map.get(var) {
                    obs[idx].rc_decs += 1;
                }
            }
            _ => {
                // All other instructions: classify each use via is_owned_position().
                for (pos, used_var) in instr.used_vars().iter().enumerate() {
                    let Some(&idx) = alias_map.get(used_var) else {
                        continue;
                    };
                    if instr.is_owned_position(pos) {
                        obs[idx].has_owned_transfer = true;
                    } else {
                        obs[idx].non_rc_uses += 1;
                    }
                }
            }
        }
    }
}

/// Record observations from a block's terminator.
fn observe_block_terminator(
    block: &crate::ir::ArcBlock,
    alias_map: &FxHashMap<ArcVarId, usize>,
    obs: &mut [ParamObservation],
) {
    let term_used = block.terminator.used_vars();
    match &block.terminator {
        // Return transfers ownership of the returned value.
        ArcTerminator::Return { value } => {
            if let Some(&idx) = alias_map.get(value) {
                obs[idx].has_owned_transfer = true;
            }
        }
        // Invoke/InvokeIndirect have arg_ownership — use is_owned_position().
        ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
            for (pos, used_var) in term_used.iter().enumerate() {
                let Some(&idx) = alias_map.get(used_var) else {
                    continue;
                };
                if block.terminator.is_owned_position(pos) {
                    obs[idx].has_owned_transfer = true;
                } else {
                    obs[idx].non_rc_uses += 1;
                }
            }
        }
        // Jump args propagate aliases (handled in alias map, not here).
        // Count as non-RC uses for the parameter observation.
        _ => {
            for used_var in &term_used {
                let Some(&idx) = alias_map.get(used_var) else {
                    continue;
                };
                obs[idx].non_rc_uses += 1;
            }
        }
    }
}

// Realized contract derivation

/// A contract re-derived from walking realized ARC IR.
///
/// Each field is derived from OBSERVING what the pipeline actually emitted,
/// not from the analysis's inferred state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedParamContract {
    /// Derived access: `Owned` if the param has any `RcInc`/`RcDec` or owned transfers.
    pub access: AccessClass,
    /// Derived consumption based on RC operation pattern (aliasing-aware).
    pub consumption: Consumption,
    /// Whether the callee may have incremented the parameter's RC.
    /// Derived from `rc_incs > 0` (per `aims-rules` IC-3).
    pub may_share: bool,
}

/// Derive `RealizedParamContract` from observed RC and use counts.
fn derive_single_param(obs: &ParamObservation) -> RealizedParamContract {
    // Access: Owned if any RC operations or ownership transfers exist.
    let access = if obs.rc_incs > 0 || obs.rc_decs > 0 || obs.has_owned_transfer {
        AccessClass::Owned
    } else {
        AccessClass::Borrowed
    };

    // Consumption: derived from aggregate counts (no intra-block ordering needed).
    let consumption = if obs.rc_incs > 0 {
        // Any RcInc → value was duplicated/shared (Unrestricted).
        Consumption::Unrestricted
    } else if obs.rc_decs > 0 && obs.non_rc_uses > 0 {
        // Used AND then dropped → Linear (consumed then cleaned up).
        Consumption::Linear
    } else if obs.non_rc_uses > 0 || obs.has_owned_transfer {
        // Used or ownership-transferred without RC ops → Linear.
        Consumption::Linear
    } else if obs.rc_decs > 0 {
        // Only dropped, no non-RC uses, no ownership transfers → Affine.
        Consumption::Affine
    } else {
        // Nothing at all → Dead.
        Consumption::Dead
    };

    // may_share: true if any RC increments exist.
    let may_share = obs.rc_incs > 0;

    RealizedParamContract {
        access,
        consumption,
        may_share,
    }
}

/// Public entry point: derive per-parameter contracts from post-pipeline ARC IR.
pub fn derive_param_contracts(func: &ArcFunction) -> Vec<RealizedParamContract> {
    let alias_map = build_param_alias_map(func);
    let observations = derive_param_observations(func, &alias_map);
    observations.iter().map(derive_single_param).collect()
}

// Effect derivation

/// Effects re-derived from walking realized ARC IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedEffects {
    /// Whether the function body contains `Construct` instructions.
    /// Conservative: any Construct → true (oracle cannot classify types).
    pub may_allocate: bool,
    /// Whether missed reuses were detected (from second-pass data).
    /// NOT derived from the IR walk — comes from the pipeline's tracking.
    pub may_deallocate: bool,
    /// Whether the function body contains ANY `RcInc` instructions
    /// (on parameters OR local variables). Per aims-rules IC-5.
    pub may_share: bool,
}

/// Derive function-level effects from realized ARC IR.
///
/// `missed_reuses` comes from the batch pipeline's second pass.
pub fn derive_effects(func: &ArcFunction, missed_reuses: u32) -> RealizedEffects {
    // may_allocate: Construct (heap allocation) OR PartialApply (closure env).
    // Matches the canonical effect sources in intraprocedural/effects.rs.
    let may_allocate = func.blocks.iter().any(|b| {
        b.body.iter().any(|i| {
            matches!(
                i,
                ArcInstr::Construct { .. } | ArcInstr::PartialApply { .. }
            )
        })
    });

    let may_deallocate = missed_reuses > 0;

    let may_share = func
        .blocks
        .iter()
        .any(|b| b.body.iter().any(|i| matches!(i, ArcInstr::RcInc { .. })));

    RealizedEffects {
        may_allocate,
        may_deallocate,
        may_share,
    }
}

// Coherence comparison

/// A coherence mismatch between inferred and realized contracts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoherenceMismatch {
    /// Parameter access class differs (unsafe direction).
    ParamAccess {
        param_index: usize,
        param_var: ArcVarId,
        inferred: AccessClass,
        realized: AccessClass,
    },
    /// Parameter consumption mode differs (unsafe direction).
    ParamConsumption {
        param_index: usize,
        param_var: ArcVarId,
        inferred: Consumption,
        realized: Consumption,
    },
    /// Parameter `may_share` disagrees (unsafe direction).
    ParamMayShare {
        param_index: usize,
        param_var: ArcVarId,
        inferred: bool,
        realized: bool,
    },
    /// Effect summary disagrees (unsafe direction).
    EffectMismatch {
        field: &'static str,
        inferred: bool,
        realized: bool,
    },
}

impl CoherenceMismatch {
    /// Whether this mismatch is in the unsafe direction (analysis too optimistic).
    /// All mismatches reported by `verify_coherence` are already filtered to the
    /// unsafe direction, so this always returns `true`.
    pub fn is_unsafe(&self) -> bool {
        true
    }
}

impl std::fmt::Display for CoherenceMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParamAccess {
                param_index,
                param_var,
                inferred,
                realized,
            } => write!(
                f,
                "param {param_index} (var {param_var:?}): access inferred={inferred:?}, realized={realized:?}"
            ),
            Self::ParamConsumption {
                param_index,
                param_var,
                inferred,
                realized,
            } => write!(
                f,
                "param {param_index} (var {param_var:?}): consumption inferred={inferred:?}, realized={realized:?}"
            ),
            Self::ParamMayShare {
                param_index,
                param_var,
                inferred,
                realized,
            } => write!(
                f,
                "param {param_index} (var {param_var:?}): may_share inferred={inferred}, realized={realized}"
            ),
            Self::EffectMismatch {
                field,
                inferred,
                realized,
            } => write!(
                f,
                "effect {field}: inferred={inferred}, realized={realized}"
            ),
        }
    }
}

/// Compare the oracle's re-derived contract against the inferred contract.
///
/// Only reports **unsafe mismatches** — where the analysis was more optimistic
/// than what the realization actually needed. Conservative mismatches (analysis
/// was more pessimistic than necessary) are logged at `tracing::info!` level
/// per `aims-rules` RL-3/VF-6 for optimization diagnostics — they indicate
/// the analysis is leaving performance on the table but is not unsound.
///
/// `missed_reuses` comes from the second pass in `batch.rs`.
pub fn verify_coherence(
    func: &ArcFunction,
    inferred: &MemoryContract,
    missed_reuses: u32,
) -> Vec<CoherenceMismatch> {
    let realized_params = derive_param_contracts(func);
    let mut mismatches = Vec::new();

    for (i, (inferred_p, realized_p)) in inferred
        .params
        .iter()
        .zip(realized_params.iter())
        .enumerate()
    {
        let param_var = func.params[i].var;

        // Access: unsafe if inferred Borrowed but realized needs Owned.
        if inferred_p.access == AccessClass::Borrowed && realized_p.access == AccessClass::Owned {
            mismatches.push(CoherenceMismatch::ParamAccess {
                param_index: i,
                param_var,
                inferred: inferred_p.access,
                realized: realized_p.access,
            });
        } else if inferred_p.access != realized_p.access {
            // Conservative: inferred Owned but realized only needs Borrowed.
            tracing::info!(
                param_index = i,
                ?param_var,
                ?inferred_p.access,
                ?realized_p.access,
                "conservative access inference — analysis is leaving performance on the table"
            );
        }

        // Consumption: unsafe if inferred is more optimistic than realized.
        // Lattice order: Dead < Linear < Affine < Unrestricted.
        if inferred_p.consumption < realized_p.consumption {
            mismatches.push(CoherenceMismatch::ParamConsumption {
                param_index: i,
                param_var,
                inferred: inferred_p.consumption,
                realized: realized_p.consumption,
            });
        } else if inferred_p.consumption > realized_p.consumption {
            // Conservative: analysis inferred heavier consumption than needed.
            tracing::info!(
                param_index = i,
                ?param_var,
                ?inferred_p.consumption,
                ?realized_p.consumption,
                "conservative consumption inference — analysis is leaving performance on the table"
            );
        }

        // may_share: unsafe if inferred false but realized true.
        if !inferred_p.may_share && realized_p.may_share {
            mismatches.push(CoherenceMismatch::ParamMayShare {
                param_index: i,
                param_var,
                inferred: false,
                realized: true,
            });
        } else if inferred_p.may_share && !realized_p.may_share {
            // Conservative: analysis claims sharing but realized has no RcInc.
            tracing::info!(
                param_index = i,
                ?param_var,
                "conservative may_share inference — parameter was not shared"
            );
        }
    }

    // Effects: check unsafe mismatches, log conservative ones.
    check_effect_coherence(func, inferred, missed_reuses, &mut mismatches);

    mismatches
}

/// Check effect dimensions for unsafe mismatches.
///
/// `may_deallocate` and `may_share` are checked directionally (unsafe = error).
/// `may_allocate` is treated as info-only because the oracle overestimates
/// (cannot distinguish scalar from heap Construct).
fn check_effect_coherence(
    func: &ArcFunction,
    inferred: &MemoryContract,
    missed_reuses: u32,
    mismatches: &mut Vec<CoherenceMismatch>,
) {
    let effects = derive_effects(func, missed_reuses);

    // may_deallocate: unsafe if analysis said no deallocation but missed reuses detected.
    if !inferred.effects.may_deallocate && effects.may_deallocate {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field: "may_deallocate",
            inferred: false,
            realized: true,
        });
    } else if inferred.effects.may_deallocate && !effects.may_deallocate {
        tracing::info!("conservative may_deallocate inference — no missed reuses detected");
    }

    // may_allocate: oracle overestimates (can't classify types), so treat as info.
    if !inferred.effects.may_allocate && effects.may_allocate {
        tracing::info!(
            "oracle may_allocate overestimate — Construct/PartialApply seen \
             but analysis says no allocation (likely scalar constructor)"
        );
    } else if inferred.effects.may_allocate && !effects.may_allocate {
        tracing::info!("conservative may_allocate inference — no Construct/PartialApply found");
    }

    // may_share: unsafe if analysis said no sharing but RcInc instructions exist.
    if !inferred.effects.may_share && effects.may_share {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field: "may_share",
            inferred: false,
            realized: true,
        });
    } else if inferred.effects.may_share && !effects.may_share {
        tracing::info!("conservative may_share inference — no RcInc instructions found");
    }
}

#[cfg(test)]
mod tests;
