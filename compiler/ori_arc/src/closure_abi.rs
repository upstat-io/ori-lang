//! Frozen backend-neutral closure-call adaptation facts.
//!
//! Residual indirect calls have one convention: the caller retains ownership
//! of every explicit argument. A closure adapter bridges that borrowed
//! convention to the concrete target signature. This module is the single
//! owner of the bridge decision and of the logical ownership-credit topology;
//! physical projections map the frozen topology through their own layouts.

use std::fmt;
use std::hash::BuildHasher;

use ori_ir::Name;
use ori_types::{Idx, Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::aims::contract::{CalleeOwnerDemand, MemoryContract};
use crate::ir::{ArcFunction, ArcInstr, CtorKind};

mod callable_facts;
mod retain_plan;

pub use callable_facts::{
    freeze_function_callable_facts, ClosureValueSignature, FunctionCallableFacts,
};
use retain_plan::{Duplication, RetainPlanBuilder};
pub use retain_plan::{
    DuplicationFailure, RetainPlanEdge, RetainPlanId, RetainPlanKind, RetainPlanNode,
    RetainPlanTable,
};

/// Where an adapter obtains a target parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClosureAdapterSource {
    /// The value is stored in the closure environment, which remains owned by
    /// the closure while the target invocation is active.
    EnvironmentCapture,
    /// The value is supplied by an indirect caller under the borrowed
    /// residual-call convention.
    BorrowedCallArgument,
}

/// Exact logical ownership action before invoking the concrete target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClosureAdapterAction {
    /// The concrete target also borrows the value.
    Borrow,
    /// The concrete target owns a trivially copyable value.
    Copy,
    /// The concrete target needs one independent owner, created by the named
    /// backend-neutral retain topology.
    Retain(RetainPlanId),
}

/// One target parameter in a frozen closure adapter signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClosureAdapterSlot {
    pub source: ClosureAdapterSource,
    pub ty: Idx,
    pub demand: CalleeOwnerDemand,
    pub action: ClosureAdapterAction,
}

/// Complete target signature needed by a closure adapter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClosureAdapterPlan {
    capture_count: usize,
    slots: Box<[ClosureAdapterSlot]>,
}

impl ClosureAdapterPlan {
    /// Construct an unvalidated adapter fact. The executable artifact validates
    /// it against the realized target before a physical projection can consume
    /// it.
    #[must_use]
    pub fn from_slots(capture_count: usize, slots: Vec<ClosureAdapterSlot>) -> Self {
        Self {
            capture_count,
            slots: slots.into_boxed_slice(),
        }
    }

    /// Number of values loaded from the closure environment.
    #[must_use]
    pub const fn capture_count(&self) -> usize {
        self.capture_count
    }

    /// Number of explicit arguments accepted by the residual closure.
    #[must_use]
    pub fn explicit_arity(&self) -> usize {
        self.slots.len() - self.capture_count
    }

    /// Target parameters in concrete call order (captures, then explicit
    /// residual arguments).
    #[must_use]
    pub fn slots(&self) -> &[ClosureAdapterSlot] {
        &self.slots
    }

    /// Whether bypassing the adapter would omit a required logical retain.
    #[must_use]
    pub fn requires_retain(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot.action, ClosureAdapterAction::Retain(_)))
    }
}

/// Complete shared output of closure ABI freezing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrozenClosureAdapters {
    pub adapters: FxHashMap<Name, ClosureAdapterPlan>,
    pub retain_plans: RetainPlanTable,
    pub callable_facts: FxHashMap<Name, FunctionCallableFacts>,
}

/// Failure to freeze a total, executable closure adapter signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosureAbiError {
    MissingTarget {
        target: Name,
    },
    CaptureArityMismatch {
        target: Name,
        expected: usize,
        actual: usize,
    },
    CaptureTypeMismatch {
        target: Name,
        capture: usize,
        expected: Idx,
        actual: Idx,
    },
    InvalidCaptureCount {
        target: Name,
        captures: usize,
        parameters: usize,
    },
    ParameterMetadataMissing {
        target: Name,
        parameter: usize,
    },
    OwnedParameterNotShareable {
        target: Name,
        parameter: usize,
        ty: Idx,
        failure: DuplicationFailure,
    },
    MissingContract {
        target: Name,
    },
    ContractArityMismatch {
        target: Name,
        parameters: usize,
        contract_parameters: usize,
    },
}

impl fmt::Display for ClosureAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget { target } => write!(
                f,
                "closure target name_id={} is absent from the realized program",
                target.raw()
            ),
            Self::CaptureArityMismatch {
                target,
                expected,
                actual,
            } => write!(
                f,
                "closure target name_id={} requires {expected} captures, but a closure site supplies {actual}",
                target.raw()
            ),
            Self::CaptureTypeMismatch {
                target,
                capture,
                expected,
                actual,
            } => write!(
                f,
                "closure target name_id={} capture {capture} has type idx {}, but the target prefix requires type idx {}",
                target.raw(),
                actual.raw(),
                expected.raw()
            ),
            Self::InvalidCaptureCount {
                target,
                captures,
                parameters,
            } => write!(
                f,
                "closure target name_id={} declares {captures} captures but has only {parameters} parameters",
                target.raw()
            ),
            Self::ParameterMetadataMissing { target, parameter } => write!(
                f,
                "closure target name_id={} parameter {parameter} has no realized variable metadata",
                target.raw()
            ),
            Self::OwnedParameterNotShareable {
                target,
                parameter,
                ty,
                failure,
                ..
            } => write!(
                f,
                "closure target name_id={} owned parameter {parameter} (type idx {}) cannot cross the borrowed closure ABI: {}",
                target.raw(),
                ty.raw(),
                failure.reason()
            ),
            Self::MissingContract { target } => write!(
                f,
                "closure target name_id={} has no final memory contract",
                target.raw()
            ),
            Self::ContractArityMismatch {
                target,
                parameters,
                contract_parameters,
            } => write!(
                f,
                "closure target name_id={} has {parameters} parameters but {contract_parameters} final contract entries",
                target.raw()
            ),
        }
    }
}

impl std::error::Error for ClosureAbiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OwnedParameterNotShareable { failure, .. } => failure
                .conversion_source()
                .map(|source| source as &(dyn std::error::Error + 'static)),
            _ => None,
        }
    }
}

/// Freeze adapter plans for exactly the functions used as closure targets.
///
/// Every closure site is checked against the target's capture prefix. Unknown
/// targets and affine owned slots are errors rather than projection-specific
/// fallbacks.
pub fn freeze_closure_adapter_plans<S: BuildHasher>(
    functions: &[ArcFunction],
    contracts: &std::collections::HashMap<Name, MemoryContract, S>,
    pool: &Pool,
    type_registry: &TypeRegistry,
) -> Result<FrozenClosureAdapters, Vec<ClosureAbiError>> {
    let callable_facts = freeze_function_callable_facts(functions, pool);
    let by_name: FxHashMap<Name, &ArcFunction> = functions
        .iter()
        .map(|function| (function.name, function))
        .collect();
    let mut targets = Vec::new();
    let mut errors = Vec::new();

    for function in functions {
        for block in &function.blocks {
            for instruction in &block.body {
                let site = match instruction {
                    ArcInstr::PartialApply { func, args, .. }
                    | ArcInstr::Construct {
                        ctor: CtorKind::Closure { func },
                        args,
                        ..
                    } => Some((*func, args.as_slice())),
                    _ => None,
                };
                let Some((target, capture_args)) = site else {
                    continue;
                };
                let Some(target_function) = by_name.get(&target) else {
                    errors.push(ClosureAbiError::MissingTarget { target });
                    continue;
                };
                if target_function.num_captures != capture_args.len() {
                    errors.push(ClosureAbiError::CaptureArityMismatch {
                        target,
                        expected: target_function.num_captures,
                        actual: capture_args.len(),
                    });
                    continue;
                }
                validate_capture_types(function, target_function, capture_args, &mut errors);
                targets.push(target);
            }
        }
    }

    targets.sort_unstable_by_key(|name| name.raw());
    targets.dedup();
    let mut builder = RetainPlanBuilder::new(pool, type_registry);
    let mut adapters = FxHashMap::default();
    for target in targets {
        let function = by_name[&target];
        let Some(contract) = contracts.get(&target) else {
            errors.push(ClosureAbiError::MissingContract { target });
            continue;
        };
        match freeze_closure_adapter_plan(function, contract, &mut builder) {
            Ok(plan) => {
                adapters.insert(target, plan);
            }
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(FrozenClosureAdapters {
            adapters,
            retain_plans: builder.finish(),
            callable_facts,
        })
    } else {
        Err(errors)
    }
}

fn validate_capture_types(
    enclosing: &ArcFunction,
    target: &ArcFunction,
    capture_args: &[crate::ArcVarId],
    errors: &mut Vec<ClosureAbiError>,
) {
    for (capture, (&argument, parameter)) in capture_args.iter().zip(&target.params).enumerate() {
        let Some(&actual) = enclosing.var_types.get(argument.index()) else {
            errors.push(ClosureAbiError::CaptureTypeMismatch {
                target: target.name,
                capture,
                expected: parameter.ty,
                actual: Idx::NONE,
            });
            continue;
        };
        if actual != parameter.ty {
            errors.push(ClosureAbiError::CaptureTypeMismatch {
                target: target.name,
                capture,
                expected: parameter.ty,
                actual,
            });
        }
    }
}

fn freeze_closure_adapter_plan(
    function: &ArcFunction,
    contract: &MemoryContract,
    builder: &mut RetainPlanBuilder<'_>,
) -> Result<ClosureAdapterPlan, ClosureAbiError> {
    if function.num_captures > function.params.len() {
        return Err(ClosureAbiError::InvalidCaptureCount {
            target: function.name,
            captures: function.num_captures,
            parameters: function.params.len(),
        });
    }
    if contract.params.len() != function.params.len() {
        return Err(ClosureAbiError::ContractArityMismatch {
            target: function.name,
            parameters: function.params.len(),
            contract_parameters: contract.params.len(),
        });
    }

    let mut slots = Vec::with_capacity(function.params.len());
    for (parameter, (param, param_contract)) in
        function.params.iter().zip(&contract.params).enumerate()
    {
        if function.var_types.get(param.var.index()) != Some(&param.ty) {
            return Err(ClosureAbiError::ParameterMetadataMissing {
                target: function.name,
                parameter,
            });
        }
        let source = if parameter < function.num_captures {
            ClosureAdapterSource::EnvironmentCapture
        } else {
            ClosureAdapterSource::BorrowedCallArgument
        };
        let demand = param_contract.callee_owner_demand();
        let action = match demand {
            CalleeOwnerDemand::Borrow => ClosureAdapterAction::Borrow,
            CalleeOwnerDemand::WholeValue => match builder.duplication_for(param.ty) {
                Ok(Duplication::Copy) => ClosureAdapterAction::Copy,
                Ok(Duplication::Retain(plan)) => ClosureAdapterAction::Retain(plan),
                Err(failure) => {
                    return Err(ClosureAbiError::OwnedParameterNotShareable {
                        target: function.name,
                        parameter,
                        ty: param.ty,
                        failure,
                    });
                }
            },
        };
        slots.push(ClosureAdapterSlot {
            source,
            ty: param.ty,
            demand,
            action,
        });
    }

    Ok(ClosureAdapterPlan {
        capture_count: function.num_captures,
        slots: slots.into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests;
