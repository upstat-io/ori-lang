//! Whole-program AIMS orchestration.
//!
//! The batch freezes external contracts, computes interprocedural contracts,
//! realizes each function, refreshes TRMC contracts, and freezes one artifact.

use std::hash::BuildHasher;

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{AimsPipelineConfig, CheckpointObserver};
use crate::aims::contract::{ContractMapExt, MemoryContract, ParamContract};
use crate::aims::lattice::AccessClass;
use crate::ir::ArcFunction;
#[cfg(test)]
use crate::ir::{ArcInstr, ArcTerminator};
use crate::lower::ArcProblem;
use crate::ownership::Ownership;
use crate::pipeline::{ArcPipelineBatchOutcome, ArcPipelineContext};

#[cfg(test)]
use second_pass::reconcile_post_emission_may_deallocate;
use second_pass::{run_second_pass, SecondPassContext};

mod second_pass;

/// Realizes a whole program with immutable contracts for external call bodies.
pub(crate) fn run_aims_pipeline_all_with_external_contracts<S: BuildHasher>(
    functions: &mut [ArcFunction],
    context: &ArcPipelineContext<'_, S>,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    run_aims_pipeline_all_impl(functions, context, None)
}

/// Realizes a whole program while reporting stable checkpoints.
pub(crate) fn run_aims_pipeline_all_with_observer<'a, S: BuildHasher>(
    functions: &mut [ArcFunction],
    context: &ArcPipelineContext<'a, S>,
    observer: &'a CheckpointObserver<'a>,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    run_aims_pipeline_all_impl(functions, context, Some(observer))
}

fn run_aims_pipeline_all_impl<'a, S: BuildHasher>(
    functions: &mut [ArcFunction],
    context: &ArcPipelineContext<'a, S>,
    observer: Option<&'a CheckpointObserver<'a>>,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    let classifier = context.classifier;
    let interner = context.interner;
    let pool = context.pool;
    let builtins = context.builtins;
    let type_registry = context.type_registry;
    let callable_boundary_facts = context.callable_boundaries;
    let verify_arc = context.verify_arc;
    let external_contracts = context.external_contracts;
    let callable_boundaries =
        callable_boundary_facts
            .validate(functions, pool)
            .map_err(|errors| {
                errors
                    .into_iter()
                    .map(crate::verify::VerifyError::from)
                    .collect::<Vec<_>>()
            })?;
    // INVARIANT: Interprocedural transfer reads one frozen primitive policy table.
    crate::aims::freeze_primitive_facts(functions, classifier)?;

    let mut contracts = {
        let _span = tracing::info_span!("analyze_program").entered();
        crate::aims::interprocedural::analyze_program_with_external_contracts_and_boundaries(
            functions,
            classifier,
            builtins,
            interner,
            external_contracts,
            &callable_boundaries,
        )
    };

    {
        let _span = tracing::info_span!("apply_ownership").entered();
        apply_aims_ownership(functions, &contracts);
    }
    for function in functions.iter() {
        super::trace_pipeline_checkpoint(function, "ownership_applied", interner, observer);
    }

    // INVARIANT: Local-callable coverage is derived independently of contract insertion.
    let func_names: FxHashSet<Name> = functions.iter().map(|f| f.name).collect();
    let exact_callables: FxHashSet<Name> = func_names
        .iter()
        .copied()
        .chain(external_contracts.keys().copied())
        .collect();

    let config = AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        func_names: &func_names,
        exact_callables: &exact_callables,
        pool,
        interner,
        builtins,
        verify_arc,
        observer,
        type_registry,
    };

    let mut execution = run_function_pipelines(functions, &config)?;

    run_second_pass(
        SecondPassContext {
            functions,
            trmc_rewritten: &execution.trmc_rewritten,
            reuse_updates: &execution.reuse_updates,
            classifier,
            verify_arc,
            interner,
            builtins,
            exact_callables: &exact_callables,
            callable_boundaries: &callable_boundaries,
        },
        &mut contracts,
    )?;

    if verify_arc {
        execution.problems.extend(contract_coherence_problems(
            functions,
            &contracts,
            &execution.reuse_updates,
            interner,
        ));
    }

    trace_physical_rc_counts(functions.len(), execution.total_rc);

    // INVARIANT: Frozen contracts cover exactly the realized function bodies.
    contracts.retain(|name, _| func_names.contains(name));

    freeze_batch_outcome(
        functions,
        contracts,
        execution.problems,
        pool,
        type_registry,
    )
}

struct PipelineExecution {
    problems: Vec<ArcProblem>,
    reuse_updates: Vec<(Name, usize)>,
    trmc_rewritten: Vec<Name>,
    total_rc: crate::pipeline::rc_count::RcOpCount,
}

fn run_function_pipelines(
    functions: &mut [ArcFunction],
    config: &AimsPipelineConfig<'_>,
) -> Result<PipelineExecution, Vec<crate::verify::VerifyError>> {
    let mut execution = PipelineExecution {
        problems: Vec::new(),
        reuse_updates: Vec::new(),
        trmc_rewritten: Vec::new(),
        total_rc: crate::pipeline::rc_count::RcOpCount::default(),
    };
    for function in functions {
        let result = super::run_aims_pipeline(function, config)?;
        execution.problems.extend(result.problems);
        execution
            .reuse_updates
            .push((function.name, result.missed_reuses));
        if result.was_trmc_rewritten {
            execution.trmc_rewritten.push(function.name);
        }
        let rc = crate::pipeline::rc_count::count_rc_ops(function);
        execution.total_rc.inc += rc.inc;
        execution.total_rc.dec += rc.dec;
    }
    Ok(execution)
}

fn contract_coherence_problems(
    functions: &[ArcFunction],
    contracts: &FxHashMap<Name, MemoryContract>,
    reuse_updates: &[(Name, usize)],
    interner: &ori_ir::StringInterner,
) -> Vec<ArcProblem> {
    let _span = tracing::info_span!("contract_coherence_oracle").entered();
    let mut problems = Vec::new();
    for (function, &(_, missed_reuses)) in functions.iter().zip(reuse_updates) {
        let contract = contracts.get_required(&function.name, "contract_coherence_oracle");
        let Ok(missed_reuses) = u32::try_from(missed_reuses) else {
            unreachable!("missed-reuse count exceeds the u32 coherence-oracle domain");
        };
        let mismatches = crate::aims::verify::oracle::verify_coherence(
            function,
            contract,
            contracts,
            interner,
            missed_reuses,
        );
        let unsafe_mismatches: Vec<_> = mismatches
            .into_iter()
            .filter(crate::aims::verify::oracle::CoherenceMismatch::is_unsafe)
            .collect();
        if !unsafe_mismatches.is_empty() {
            problems.push(ArcProblem::ContractCoherenceViolation {
                func_name: interner.lookup(function.name).to_owned(),
                mismatches: unsafe_mismatches,
            });
        }
    }
    problems
}

fn trace_physical_rc_counts(function_count: usize, total_rc: crate::pipeline::rc_count::RcOpCount) {
    // INVARIANT: This trace measures the compiled-counter adapter, not an AIMS fact.
    tracing::debug!(
        functions = function_count,
        rc_inc = total_rc.inc,
        rc_dec = total_rc.dec,
        rc_total = total_rc.total(),
        "AIMS pipeline RC operation totals"
    );
}

fn freeze_batch_outcome(
    functions: &[ArcFunction],
    contracts: FxHashMap<Name, MemoryContract>,
    problems: Vec<ArcProblem>,
    pool: &ori_types::Pool,
    type_registry: &ori_types::TypeRegistry,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    // INVARIANT: Backends consume frozen facts and do not rerun AIMS analysis.
    let function_effects = functions
        .iter()
        .map(|function| {
            let contract = contracts.get_required(&function.name, "freeze_function_effects");
            (function.name, contract.function_effect_facts(function))
        })
        .collect();
    let fresh_return_facts = functions
        .iter()
        .map(|function| {
            let contract = contracts.get_required(&function.name, "freeze_fresh_return_facts");
            (function.name, contract.fresh_self_allocation_facts())
        })
        .collect();
    let param_disjointness = functions
        .iter()
        .map(|function| {
            let param_types: Vec<_> = function.params.iter().map(|param| param.ty).collect();
            (
                function.name,
                crate::aims::realize::rl31_disjoint::prove_param_disjointness(&param_types, pool),
            )
        })
        .collect();
    let yield_allocations = functions
        .iter()
        .map(|function| (function.name, function.yield_allocations.clone()))
        .collect();
    let frozen_closure_adapters =
        crate::freeze_closure_adapter_plans(functions, &contracts, pool, type_registry).map_err(
            |errors| {
                errors
                    .into_iter()
                    .map(crate::verify::VerifyError::ClosureAbi)
                    .collect::<Vec<_>>()
            },
        )?;

    Ok(ArcPipelineBatchOutcome {
        problems,
        contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        closure_adapters: frozen_closure_adapters.adapters,
        retain_plans: frozen_closure_adapters.retain_plans,
        callable_facts: frozen_closure_adapters.callable_facts,
        yield_allocations,
    })
}

/// Applies each [`MemoryContract`] parameter access class to the corresponding IR parameter.
pub(crate) fn apply_aims_ownership(
    functions: &mut [ArcFunction],
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    for func in functions {
        let contract = contracts.get_required(&func.name, "apply_aims_ownership");
        for (param, pc) in func.params.iter_mut().zip(&contract.params) {
            param.ownership = param_contract_to_ownership(*pc);
        }
    }
}

/// Maps parameter access into the IR ownership carrier.
fn param_contract_to_ownership(pc: ParamContract) -> Ownership {
    match pc.access {
        AccessClass::Borrowed => Ownership::Borrowed,
        AccessClass::Owned => Ownership::Owned,
    }
}

#[cfg(test)]
mod tests;
