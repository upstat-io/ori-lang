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

// ─── Alias Tracking (05.1.1) ───

/// Maps every `ArcVarId` that is an alias of a function parameter to its
/// parameter index. Handles transitive aliasing through `Let { value: Var(_) }`
/// chains AND block-parameter propagation through `Jump` terminators.
fn build_param_alias_map(func: &ArcFunction) -> FxHashMap<ArcVarId, usize> {
    let mut alias_map: FxHashMap<ArcVarId, usize> = FxHashMap::default();

    // Seed: direct parameter variables.
    for (i, param) in func.params.iter().enumerate() {
        alias_map.insert(param.var, i);
    }

    // Forward pass: propagate through Let { value: Var(_) } bindings.
    // Within a single block, ARC IR is SSA so one pass suffices.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if let Some(&param_idx) = alias_map.get(src) {
                    alias_map.insert(*dst, param_idx);
                }
            }
        }
    }

    // Worklist: propagate through Jump → block-param edges.
    // A Jump { target, args: [v1, v2] } targeting a block with params
    // [(bp0, _), (bp1, _)] means bp0 aliases v1 and bp1 aliases v2.
    // Loop back-edges require iterating until no new aliases are found.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_block = func.blocks.iter().find(|b| b.id == *target);
                if let Some(target_block) = target_block {
                    for (bp, arg) in target_block.params.iter().zip(args.iter()) {
                        if let Some(&param_idx) = alias_map.get(arg) {
                            if alias_map.insert(bp.0, param_idx).is_none() {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
    }

    alias_map
}

// ─── Per-Parameter Observation (05.1.2) ───

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
                    // For all other instructions: use used_vars() + is_owned_position()
                    // to classify each use as owned transfer or non-RC use.
                    let used = instr.used_vars();
                    for (pos, used_var) in used.iter().enumerate() {
                        if let Some(&idx) = alias_map.get(used_var) {
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

        // Terminator uses.
        let term_used = block.terminator.used_vars();
        match &block.terminator {
            // Return transfers ownership of the returned value.
            // is_owned_position() returns false for Return, so handle explicitly.
            ArcTerminator::Return { value } => {
                if let Some(&idx) = alias_map.get(value) {
                    obs[idx].has_owned_transfer = true;
                }
            }
            // Invoke/InvokeIndirect have arg_ownership — use is_owned_position().
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
                for (pos, used_var) in term_used.iter().enumerate() {
                    if let Some(&idx) = alias_map.get(used_var) {
                        if block.terminator.is_owned_position(pos) {
                            obs[idx].has_owned_transfer = true;
                        } else {
                            obs[idx].non_rc_uses += 1;
                        }
                    }
                }
            }
            // Jump args propagate aliases (handled in alias map, not here).
            // Count as non-RC uses for the parameter observation.
            _ => {
                for used_var in &term_used {
                    if let Some(&idx) = alias_map.get(used_var) {
                        obs[idx].non_rc_uses += 1;
                    }
                }
            }
        }
    }

    obs
}

// ─── Realized Contract Derivation (05.1.3) ───

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

// ─── Effect Derivation (05.2) ───

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
    let may_allocate = func.blocks.iter().any(|b| {
        b.body
            .iter()
            .any(|i| matches!(i, ArcInstr::Construct { .. }))
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

// ─── Coherence Comparison (05.3) ───

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

    // Effects: check all three dimensions via derive_effects().
    let effects = derive_effects(func, missed_reuses);

    if !inferred.effects.may_deallocate && effects.may_deallocate {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field: "may_deallocate",
            inferred: false,
            realized: true,
        });
    } else if inferred.effects.may_deallocate && !effects.may_deallocate {
        tracing::info!("conservative may_deallocate inference — no missed reuses detected");
    }

    if !inferred.effects.may_allocate && effects.may_allocate {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field: "may_allocate",
            inferred: false,
            realized: true,
        });
    } else if inferred.effects.may_allocate && !effects.may_allocate {
        tracing::info!("conservative may_allocate inference — no Construct instructions found");
    }

    if !inferred.effects.may_share && effects.may_share {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field: "may_share",
            inferred: false,
            realized: true,
        });
    } else if inferred.effects.may_share && !effects.may_share {
        tracing::info!("conservative may_share inference — no RcInc instructions found");
    }

    mismatches
}

#[cfg(test)]
mod tests;
