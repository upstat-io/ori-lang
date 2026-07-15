//! Batch orchestration: run AIMS pipeline on all functions.
//!
//! Contains the batch entry point (`run_aims_pipeline_all`), the second-pass
//! TRMC contract refresh and FIP recomputation (`run_second_pass`), and
//! ownership application (`apply_aims_ownership`).

use std::hash::BuildHasher;

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{AimsPipelineConfig, CheckpointObserver};
use crate::aims::contract::{ContractMapExt, MemoryContract, ParamContract};
use crate::aims::lattice::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};
use crate::lower::ArcProblem;
use crate::ownership::Ownership;
use crate::pipeline::{ArcPipelineBatchOutcome, ArcPipelineContext};

/// Run one whole-program realization with immutable producer-authored
/// contracts for calls whose bodies live in other compiled units.
pub(crate) fn run_aims_pipeline_all_with_external_contracts<S: BuildHasher>(
    functions: &mut [ArcFunction],
    context: &ArcPipelineContext<'_, S>,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    run_aims_pipeline_all_impl(functions, context, None)
}

/// Run one whole-program realization while reporting stable phase snapshots.
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
    // Freeze typed primitive semantics before any interprocedural transfer
    // reads them. Each per-function pipeline validates the same table after
    // normalization; it does not re-resolve policy.
    crate::aims::freeze_primitive_facts(functions, classifier)?;

    // Step 1: interprocedural analysis -> MemoryContract per function.
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

    // Step 2: apply ownership to function parameters.
    {
        let _span = tracing::info_span!("apply_ownership").entered();
        apply_aims_ownership(functions, &contracts);
    }
    for function in functions.iter() {
        super::trace_pipeline_checkpoint(function, "ownership_applied", interner, observer);
    }

    // Set of function names in this compilation unit (the analyzed set),
    // sourced independently of the contracts map so the Site-8 IC-1
    // debug_assert can catch a local function missing from contracts.
    let func_names: FxHashSet<Name> = functions.iter().map(|f| f.name).collect();
    let exact_callables: FxHashSet<Name> = func_names
        .iter()
        .copied()
        .chain(external_contracts.keys().copied())
        .collect();

    // Steps 3-14: per-function pipeline.
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

    // Second pass: TRMC contract refresh -> may_deallocate -> FIP.
    run_second_pass(
        SecondPassContext {
            functions,
            trmc_rewritten: &execution.trmc_rewritten,
            reuse_updates: &execution.reuse_updates,
            classifier,
            verify_arc,
            interner,
            callable_boundaries: &callable_boundaries,
        },
        &mut contracts,
    )?;

    // Contract coherence oracle: verify inferred contracts match what the
    // realization pipeline actually emitted. Only under ORI_VERIFY_ARC=1.
    if verify_arc {
        execution.problems.extend(contract_coherence_problems(
            functions,
            &contracts,
            &execution.reuse_updates,
            interner,
        ));
    }

    trace_physical_rc_counts(functions.len(), execution.total_rc);

    // Builtin contracts participate in AIMS analysis but are not executable
    // program functions. Close the exported map to the realized body set so
    // the downstream artifact can enforce exact one-to-one coverage.
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
        let mismatches = crate::aims::verify::oracle::verify_coherence(
            function,
            contract,
            contracts,
            interner,
            u32::try_from(missed_reuses).unwrap_or(u32::MAX),
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
    // Preserve the historical tracing label for consumer compatibility. These
    // counters measure only the current compiled-counter adapter; they are not
    // AIMS facts or a shared-calculus conformance verdict.
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
    // Freeze backend-neutral semantic facts at the realization owner. The
    // executable artifact validates and orders these maps; it never re-runs
    // AIMS analysis at the transport seam.
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
    })
}

/// Immutable inputs consumed together by the post-realization second pass.
#[derive(Clone, Copy)]
struct SecondPassContext<'a, 'facts> {
    functions: &'a [ArcFunction],
    trmc_rewritten: &'a [Name],
    reuse_updates: &'a [(Name, usize)],
    classifier: &'a dyn crate::ArcClassification,
    verify_arc: bool,
    interner: &'a ori_ir::StringInterner,
    callable_boundaries:
        &'a crate::pipeline::callable_boundary::ValidatedCallableBoundaryFacts<'facts>,
}

/// Refresh TRMC contracts, update `may_deallocate`, and re-verify FIP.
///
/// Ordering: (1) TRMC contract refresh, (2) `may_deallocate` update,
/// (3) FIP recomputation, (4) FIP re-verification.
fn run_second_pass(
    context: SecondPassContext<'_, '_>,
    contracts: &mut FxHashMap<Name, MemoryContract>,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let SecondPassContext {
        functions,
        trmc_rewritten,
        reuse_updates,
        classifier,
        verify_arc,
        interner,
        callable_boundaries,
    } = context;

    // Phase 1: full contract refresh for TRMC-rewritten functions.
    // Re-run analysis + extraction on the rewritten IR to get accurate
    // ContextBehavior, FipContract, and EffectSummary.
    if !trmc_rewritten.is_empty() {
        let _span = tracing::info_span!("trmc_contract_refresh").entered();
        for &name in trmc_rewritten {
            // Find the rewritten function.
            let Some(func) = functions.iter().find(|f| f.name == name) else {
                continue;
            };
            // Re-analyze with current contracts as peer context.
            let state_map = crate::aims::intraprocedural::analyze_function(
                func,
                classifier,
                contracts,
                &[],
                Vec::new(),
            );
            let context_regions = crate::aims::normalize::detect_context_regions(func);
            // No SCC peers needed — TRMC rewrite is per-function and
            // the function's own contract is already in `contracts`.
            let mut new_contract = crate::aims::interprocedural::extract_contract(
                func,
                &state_map,
                classifier,
                contracts,
                &rustc_hash::FxHashSet::default(),
                &context_regions,
                interner,
            );
            callable_boundaries.constrain_contract(name, &mut new_contract);
            let old = contracts.get_mut_required(&name, "trmc_contract_refresh");
            tracing::debug!(
                func = name.raw(),
                old_unbounded = old.effects.has_unbounded_stack,
                new_unbounded = new_contract.effects.has_unbounded_stack,
                "TRMC full contract refresh"
            );
            *old = new_contract;
        }
    }

    // Phase 2: join post-emission may_deallocate facts, close the joined fact
    // over direct calls, then recompute every affected local FIP contract.
    {
        let _span = tracing::info_span!("post_emission_fip_update").entered();
        let downgrades =
            reconcile_post_emission_may_deallocate(functions, contracts, reuse_updates);
        if downgrades > 0 {
            tracing::info!(
                downgrades,
                "FIP contracts downgraded after may_deallocate update"
            );
        }
    }

    // Phase 3: re-verify FIP contracts with corrected data.
    {
        let _span = tracing::info_span!("post_emission_fip_verify").entered();
        debug_assert_eq!(
            functions.len(),
            reuse_updates.len(),
            "reuse_updates must match functions 1:1"
        );
        for (func, (update_name, missed_reuses)) in functions.iter().zip(reuse_updates.iter()) {
            debug_assert_eq!(
                func.name, *update_name,
                "reuse_updates order must match functions order"
            );
            let contract = contracts.get_required(&func.name, "post_emission_fip_verify");
            let evidence = crate::aims::realize::FipEvidence {
                fip_gates: vec![],
                missed_reuses: *missed_reuses,
            };
            let fip_errors =
                crate::aims::verify::fip::verify_fip_contract(func.name, contract, &evidence);
            if !fip_errors.is_empty() {
                for e in &fip_errors {
                    tracing::error!("FIP post-recompute verification failed: {e}");
                }
                if verify_arc {
                    // Second pass: ALL FIP errors are blocking because
                    // may_deallocate facts have been recomputed.
                    return Err(fip_errors
                        .into_iter()
                        .map(|e| crate::verify::VerifyError::FipStructural {
                            message: e.to_string(),
                        })
                        .collect());
                }
            }
        }
    }
    Ok(())
}

/// Merge realization-local deallocation evidence into the converged IC-5
/// summaries, propagate newly true facts to callers, and update local FIP.
///
/// `may_deallocate` is an OR-monotone effect flag. Post-emission evidence may
/// therefore promote a contract from `false` to `true`, but it must never erase
/// a `true` fact already inherited from a builtin, external contract, or callee.
fn reconcile_post_emission_may_deallocate(
    functions: &[ArcFunction],
    contracts: &mut FxHashMap<Name, MemoryContract>,
    reuse_updates: &[(Name, usize)],
) -> u32 {
    debug_assert_eq!(
        functions.len(),
        reuse_updates.len(),
        "reuse_updates must match functions 1:1"
    );

    for (function, (name, missed_reuses)) in functions.iter().zip(reuse_updates) {
        debug_assert_eq!(
            function.name, *name,
            "reuse_updates order must match functions order"
        );
        let contract = contracts.get_mut_required(name, "post_emission_may_deallocate_join");
        contract.effects.may_deallocate |= *missed_reuses > 0;
    }

    propagate_may_deallocate_to_callers(functions, contracts);

    let mut downgrades = 0u32;
    for (name, _) in reuse_updates {
        let contract = contracts.get_mut_required(name, "post_emission_fip_recompute");
        if crate::aims::verify::fip::recompute_fip_for_may_deallocate(contract) {
            downgrades += 1;
            tracing::debug!(
                func = name.raw(),
                "FIP contract downgraded to Never after may_deallocate update"
            );
        }
    }
    downgrades
}

/// Compute the least IC-5 closure of `may_deallocate` over realized direct-call
/// edges. The worklist naturally converges across recursive SCCs because the
/// boolean flag has only one promotion (`false -> true`).
fn propagate_may_deallocate_to_callers(
    functions: &[ArcFunction],
    contracts: &mut FxHashMap<Name, MemoryContract>,
) {
    let mut callers_by_callee: FxHashMap<Name, FxHashSet<Name>> = FxHashMap::default();
    for function in functions {
        for block in &function.blocks {
            for instruction in &block.body {
                if let ArcInstr::Apply { func: callee, .. } = instruction {
                    callers_by_callee
                        .entry(*callee)
                        .or_default()
                        .insert(function.name);
                }
            }
            if let ArcTerminator::Invoke { func: callee, .. } = &block.terminator {
                callers_by_callee
                    .entry(*callee)
                    .or_default()
                    .insert(function.name);
            }
        }
    }

    // Seed from every true contract, including builtins and immutable external
    // contracts. Their names occur as reverse-index keys even though they have
    // no body in `functions`.
    let mut worklist: Vec<Name> = contracts
        .iter()
        .filter_map(|(&name, contract)| contract.effects.may_deallocate.then_some(name))
        .collect();

    while let Some(callee) = worklist.pop() {
        let Some(callers) = callers_by_callee.get(&callee) else {
            continue;
        };
        for &caller in callers {
            let caller_contract =
                contracts.get_mut_required(&caller, "may_deallocate_call_closure");
            if !caller_contract.effects.may_deallocate {
                caller_contract.effects.may_deallocate = true;
                worklist.push(caller);
            }
        }
    }
}

/// Apply AIMS ownership annotations to function parameters.
///
/// Sets `ArcParam.ownership` on each function from its `MemoryContract`.
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

/// Convert a `ParamContract` access class to the `Ownership` enum used by
/// `ArcParam`. This bridges the AIMS contract representation with the
/// existing ARC IR parameter ownership field.
fn param_contract_to_ownership(pc: ParamContract) -> Ownership {
    match pc.access {
        AccessClass::Borrowed => Ownership::Borrowed,
        AccessClass::Owned => Ownership::Owned,
    }
}

#[cfg(test)]
mod tests;
