//! Post-realization contract refresh, deallocation closure, and FIP verification.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ContractMapExt, MemoryContract};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator};

/// Immutable inputs consumed together by the post-realization second pass.
#[derive(Clone, Copy)]
pub(super) struct SecondPassContext<'a, 'facts> {
    pub(super) functions: &'a [ArcFunction],
    pub(super) trmc_rewritten: &'a [Name],
    pub(super) reuse_updates: &'a [(Name, usize)],
    pub(super) classifier: &'a dyn crate::ArcClassification,
    pub(super) verify_arc: bool,
    pub(super) interner: &'a ori_ir::StringInterner,
    pub(super) builtins: &'a crate::borrow::BuiltinOwnershipSets,
    pub(super) exact_callables: &'a FxHashSet<Name>,
    pub(super) type_registry: &'a ori_types::TypeRegistry,
    pub(super) callable_boundaries:
        &'a crate::pipeline::callable_boundary::ValidatedCallableBoundaryFacts<'facts>,
}

/// Refresh TRMC contracts, update `may_deallocate`, and re-verify FIP.
///
/// Ordering: (1) TRMC contract refresh, (2) `may_deallocate` update,
/// (3) FIP recomputation, (4) FIP re-verification.
pub(super) fn run_second_pass(
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
        builtins,
        exact_callables,
        type_registry,
        callable_boundaries,
    } = context;

    if !trmc_rewritten.is_empty() {
        let _span = tracing::info_span!("trmc_contract_refresh").entered();
        for &name in trmc_rewritten {
            let Some(func) = functions.iter().find(|f| f.name == name) else {
                continue;
            };
            let state_map = crate::aims::intraprocedural::analyze_function(
                func,
                classifier,
                contracts,
                &[],
                Vec::new(),
            );
            let context_regions = crate::aims::normalize::detect_context_regions(func);
            let mut new_contract =
                crate::aims::interprocedural::extract_contract_with_call_ownership(
                    &crate::aims::interprocedural::ContractExtractionInput {
                        func,
                        state_map: &state_map,
                        classifier,
                        sigs: contracts,
                        scc_peers: &rustc_hash::FxHashSet::default(),
                        context_regions: &context_regions,
                        interner,
                        builtins,
                        exact_callables,
                        type_registry: Some(type_registry),
                    },
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
pub(super) fn reconcile_post_emission_may_deallocate(
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
