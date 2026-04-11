//! Contract coherence oracle.
//!
//! Walks realized ARC IR and re-derives a contract from the actual RC
//! operations emitted. Compares against the inferred [`MemoryContract`]
//! to verify coherence — catching bugs where analysis infers a correct
//! contract but realization emits inconsistent RC instructions, or where
//! the analysis infers an incorrect contract that happens to produce
//! working code by accident.

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::{AccessClass, Cardinality, Consumption};
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

// Re-derived contracts

/// A contract re-derived from walking realized ARC IR.
///
/// Each field is derived from OBSERVING what the pipeline actually emitted,
/// not from the analysis's inferred state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedParamContract {
    /// Derived access: `Owned` if the param has any `RcInc`/`RcDec`, `Borrowed` otherwise.
    pub access: AccessClass,
    /// Derived consumption based on RC operation pattern.
    pub consumption: Consumption,
    /// Derived cardinality based on forward use count.
    pub cardinality: Cardinality,
}

/// Derive per-parameter contracts from the realized (post-pipeline) ARC IR
/// by observing actual RC operations emitted for each parameter variable.
pub fn derive_param_contracts(func: &ArcFunction) -> Vec<RealizedParamContract> {
    let num_params = func.params.len();
    let mut rc_incs: Vec<u32> = vec![0; num_params];
    let mut rc_decs: Vec<u32> = vec![0; num_params];
    let mut use_counts: Vec<u32> = vec![0; num_params];

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => {
                    if let Some(idx) = param_index(func, *var) {
                        rc_incs[idx] += 1;
                    }
                }
                ArcInstr::RcDec { var, .. } => {
                    if let Some(idx) = param_index(func, *var) {
                        rc_decs[idx] += 1;
                    }
                }
                _ => {
                    for used_var in instr.used_vars() {
                        if let Some(idx) = param_index(func, used_var) {
                            use_counts[idx] += 1;
                        }
                    }
                }
            }
        }
        // Also count uses in the terminator
        for used_var in block.terminator.used_vars() {
            if let Some(idx) = param_index(func, used_var) {
                use_counts[idx] += 1;
            }
        }
    }

    (0..num_params)
        .map(|i| derive_single_param(rc_incs[i], rc_decs[i], use_counts[i]))
        .collect()
}

/// Map an `ArcVarId` to a parameter index, if the var is a parameter.
fn param_index(func: &ArcFunction, var: ArcVarId) -> Option<usize> {
    func.params.iter().position(|p| p.var == var)
}

/// Infer a `RealizedParamContract` from observed RC and use counts.
fn derive_single_param(rc_incs: u32, rc_decs: u32, use_count: u32) -> RealizedParamContract {
    // Access: Owned if any RC operations exist, Borrowed otherwise
    let has_rc = rc_incs > 0 || rc_decs > 0;
    let access = if has_rc {
        AccessClass::Owned
    } else {
        AccessClass::Borrowed
    };

    // Consumption: derived from RC pattern
    let consumption = if rc_incs > 0 && rc_decs > 0 {
        // Both inc and dec → Unrestricted (value is copied and dropped)
        Consumption::Unrestricted
    } else if rc_incs > 0 {
        // Inc only → value is shared (copied for another consumer)
        Consumption::Unrestricted
    } else if rc_decs > 0 {
        // Dec only → Affine (may be dropped without further use)
        Consumption::Affine
    } else if use_count > 0 {
        // Used but no RC → Linear (consumed once without RC)
        Consumption::Linear
    } else {
        // Not used at all → Dead
        Consumption::Dead
    };

    // Cardinality: derived from use count (excluding RC ops)
    let cardinality = if use_count == 0 && !has_rc {
        Cardinality::Absent
    } else if use_count <= 1 && rc_incs == 0 {
        Cardinality::Once
    } else {
        Cardinality::Many
    };

    RealizedParamContract {
        access,
        consumption,
        cardinality,
    }
}

// Realized effects

/// Effect summary re-derived from realized IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedEffects {
    /// Whether the function body contains `Construct` instructions
    /// that allocate heap memory.
    pub may_allocate: bool,
    /// Whether missed reuses were detected (from second pass).
    pub may_deallocate: bool,
}

// Coherence comparison

/// A coherence mismatch between inferred and realized contracts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoherenceMismatch {
    /// Parameter access class differs (unsafe direction).
    ParamAccess {
        param_index: usize,
        inferred: AccessClass,
        realized: AccessClass,
    },
    /// Parameter consumption mode differs (unsafe direction).
    ParamConsumption {
        param_index: usize,
        inferred: Consumption,
        realized: Consumption,
    },
    /// Effect summary disagrees.
    EffectMismatch {
        field: &'static str,
        inferred: bool,
        realized: bool,
    },
}

impl CoherenceMismatch {
    /// Whether this mismatch is in the unsafe direction (analysis too optimistic).
    pub fn is_unsafe(&self) -> bool {
        // All mismatches reported by verify_coherence are already filtered
        // to the unsafe direction (inferred more optimistic than realized).
        true
    }
}

/// Compare the oracle's re-derived contract against the inferred contract.
///
/// Only reports **unsafe mismatches** — where the analysis was more optimistic
/// than what the realization actually needed. Conservative mismatches (analysis
/// was more pessimistic than necessary) are acceptable and not reported.
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
        // Access: unsafe if inferred Borrowed but realized needs Owned
        if inferred_p.access == AccessClass::Borrowed && realized_p.access == AccessClass::Owned {
            mismatches.push(CoherenceMismatch::ParamAccess {
                param_index: i,
                inferred: inferred_p.access,
                realized: realized_p.access,
            });
        }

        // Consumption: unsafe if inferred is more optimistic than realized.
        // Lattice order: Dead < Linear < Affine < Unrestricted.
        // Unsafe: inferred < realized (analysis says simpler, realization needs more).
        if inferred_p.consumption < realized_p.consumption {
            mismatches.push(CoherenceMismatch::ParamConsumption {
                param_index: i,
                inferred: inferred_p.consumption,
                realized: realized_p.consumption,
            });
        }
    }

    // Effects: may_deallocate
    let realized_may_deallocate = missed_reuses > 0;
    if !inferred.effects.may_deallocate && realized_may_deallocate {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field: "may_deallocate",
            inferred: inferred.effects.may_deallocate,
            realized: realized_may_deallocate,
        });
    }

    mismatches
}

#[cfg(test)]
mod tests;
