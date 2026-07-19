//! Contract coherence oracle.
//!
//! Re-derives parameter and effect facts from realized ARC IR, then compares
//! them with the inferred contract. The subject contract is unavailable to the
//! evidence adapter, so recursive summaries cannot manufacture their own proof.

mod demand;
mod evidence;
mod local_funding;

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::{AccessClass, Consumption};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};

/// A parameter contract re-derived from realized events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedParamContract {
    /// `Owned` when any reachable path contains an unfunded discharge.
    pub access: AccessClass,
    /// TF-11 sequential demand joined across IC-3 alternatives.
    pub consumption: Consumption,
    /// Explicit positive credit or a Borrowed call to a sharing callee.
    pub may_share: bool,
    /// Independently observed iter-consuming terminal transfer.
    pub iter_transfers: bool,
}

/// Derive parameter evidence without interprocedural context.
///
/// Production verification uses [`derive_param_contracts_with_context`]. This
/// entry point remains useful for isolated IR tests that contain no direct
/// iterator or summary-mediated calls.
pub fn derive_param_contracts(func: &ArcFunction) -> Vec<RealizedParamContract> {
    let contracts = FxHashMap::default();
    let interner = StringInterner::default();
    evidence::derive_param_contracts(func, &contracts, &interner)
}

fn derive_param_contracts_with_context(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
) -> Vec<RealizedParamContract> {
    evidence::derive_param_contracts(func, contracts, interner)
}

/// Effects re-derived from realized IR and other callees' summaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedEffects {
    pub may_allocate: bool,
    pub may_deallocate: bool,
    pub may_share: bool,
}

/// Derive effects without interprocedural context.
pub fn derive_effects(func: &ArcFunction, missed_reuses: u32) -> RealizedEffects {
    derive_effects_with_context(func, &FxHashMap::default(), missed_reuses)
}

fn derive_effects_with_context(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    missed_reuses: u32,
) -> RealizedEffects {
    let mut effects = RealizedEffects {
        may_allocate: false,
        may_deallocate: missed_reuses > 0,
        may_share: false,
    };

    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, .. } => {
                    let scalar = func
                        .var_reprs
                        .get(dst.index())
                        .is_some_and(|repr| *repr == ValueRepr::Scalar);
                    effects.may_allocate |= !scalar;
                }
                ArcInstr::PartialApply { .. } => effects.may_allocate = true,
                ArcInstr::RcInc { count, .. } => effects.may_share |= *count > 0,
                ArcInstr::BurdenInc { .. } => effects.may_share = true,
                ArcInstr::Apply { func: callee, .. } => {
                    absorb_callee_effects(func.name, *callee, contracts, &mut effects);
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
            absorb_callee_effects(func.name, *callee, contracts, &mut effects);
        }
    }
    effects
}

fn absorb_callee_effects(
    subject: Name,
    callee: Name,
    contracts: &FxHashMap<Name, MemoryContract>,
    realized: &mut RealizedEffects,
) {
    if callee == subject {
        return;
    }
    let Some(contract) = contracts.get(&callee) else {
        return;
    };
    realized.may_allocate |= contract.effects.may_allocate;
    realized.may_deallocate |= contract.effects.may_deallocate;
    realized.may_share |= contract.effects.may_share;
}

/// An unsafe disagreement between inferred and independently realized facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoherenceMismatch {
    ParamArity {
        function_params: usize,
        inferred_params: usize,
    },
    ParamAccess {
        param_index: usize,
        param_var: ArcVarId,
        inferred: AccessClass,
        realized: AccessClass,
    },
    ParamConsumption {
        param_index: usize,
        param_var: ArcVarId,
        inferred: Consumption,
        realized: Consumption,
    },
    ParamMayShare {
        param_index: usize,
        param_var: ArcVarId,
        inferred: bool,
        realized: bool,
    },
    ParamIterTransfer {
        param_index: usize,
        param_var: ArcVarId,
        inferred: bool,
        realized: bool,
    },
    EffectMismatch {
        field: &'static str,
        inferred: bool,
        realized: bool,
    },
}

impl CoherenceMismatch {
    pub fn is_unsafe(&self) -> bool {
        true
    }
}

impl std::fmt::Display for CoherenceMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParamArity {
                function_params,
                inferred_params,
            } => write!(
                f,
                "parameter arity: function={function_params}, inferred={inferred_params}"
            ),
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
            Self::ParamIterTransfer {
                param_index,
                param_var,
                inferred,
                realized,
            } => write!(
                f,
                "param {param_index} (var {param_var:?}): iter transfer inferred={inferred}, realized={realized}"
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

/// Compare subject-independent realized evidence with the inferred contract.
pub(crate) fn verify_coherence(
    func: &ArcFunction,
    inferred: &MemoryContract,
    all_contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
    missed_reuses: u32,
) -> Vec<CoherenceMismatch> {
    let realized_params = derive_param_contracts_with_context(func, all_contracts, interner);
    let mut mismatches = Vec::new();

    if func.params.len() != inferred.params.len() {
        mismatches.push(CoherenceMismatch::ParamArity {
            function_params: func.params.len(),
            inferred_params: inferred.params.len(),
        });
    }

    let shared_params = func
        .params
        .len()
        .min(inferred.params.len())
        .min(realized_params.len());
    for (index, realized_param) in realized_params.iter().enumerate().take(shared_params) {
        compare_param(
            index,
            func.params[index].var,
            &inferred.params[index],
            realized_param,
            &mut mismatches,
        );
    }

    compare_effects(
        inferred,
        &derive_effects_with_context(func, all_contracts, missed_reuses),
        &mut mismatches,
    );
    mismatches
}

fn compare_param(
    index: usize,
    param_var: ArcVarId,
    inferred: &crate::aims::contract::ParamContract,
    realized: &RealizedParamContract,
    mismatches: &mut Vec<CoherenceMismatch>,
) {
    if inferred.access < realized.access {
        mismatches.push(CoherenceMismatch::ParamAccess {
            param_index: index,
            param_var,
            inferred: inferred.access,
            realized: realized.access,
        });
    } else if inferred.access > realized.access {
        tracing::info!(
            param_index = index,
            ?param_var,
            ?inferred.access,
            ?realized.access,
            "conservative access inference"
        );
    }

    if inferred.consumption < realized.consumption {
        mismatches.push(CoherenceMismatch::ParamConsumption {
            param_index: index,
            param_var,
            inferred: inferred.consumption,
            realized: realized.consumption,
        });
    } else if inferred.consumption > realized.consumption {
        tracing::info!(
            param_index = index,
            ?param_var,
            ?inferred.consumption,
            ?realized.consumption,
            "conservative consumption inference"
        );
    }

    if !inferred.may_share && realized.may_share {
        mismatches.push(CoherenceMismatch::ParamMayShare {
            param_index: index,
            param_var,
            inferred: false,
            realized: true,
        });
    } else if inferred.may_share && !realized.may_share {
        tracing::info!(
            param_index = index,
            ?param_var,
            "conservative may_share inference"
        );
    }

    if inferred.iter_consumes != realized.iter_transfers {
        mismatches.push(CoherenceMismatch::ParamIterTransfer {
            param_index: index,
            param_var,
            inferred: inferred.iter_consumes,
            realized: realized.iter_transfers,
        });
    }
}

fn compare_effects(
    inferred: &MemoryContract,
    realized: &RealizedEffects,
    mismatches: &mut Vec<CoherenceMismatch>,
) {
    compare_effect(
        "may_allocate",
        inferred.effects.may_allocate,
        realized.may_allocate,
        mismatches,
    );
    compare_effect(
        "may_deallocate",
        inferred.effects.may_deallocate,
        realized.may_deallocate,
        mismatches,
    );
    compare_effect(
        "may_share",
        inferred.effects.may_share,
        realized.may_share,
        mismatches,
    );
}

fn compare_effect(
    field: &'static str,
    inferred: bool,
    realized: bool,
    mismatches: &mut Vec<CoherenceMismatch>,
) {
    if !inferred && realized {
        mismatches.push(CoherenceMismatch::EffectMismatch {
            field,
            inferred,
            realized,
        });
    } else if inferred && !realized {
        tracing::info!(field, "conservative effect inference");
    }
}

#[cfg(test)]
mod tests;
