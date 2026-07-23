//! Contract construction from converged intraprocedural state maps.
//!
//! After backward dataflow analysis converges, [`extract_contract`] reads the
//! per-parameter demand at the function entry point and determines return
//! value uniqueness to produce a [`MemoryContract`].

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{
    ContextBehavior, ContextRegion, EffectSummary, FipContract, MemoryContract, ParamContract,
};
use crate::aims::intraprocedural::{compute_requires_unique_params, AimsStateMap};
use crate::aims::lattice::{AccessClass, AimsState, Uniqueness};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::tail_call::has_non_tail_recursive_calls;
use crate::ArcClassification;

use super::param_facts::{detect_param_facts, ParamFacts};
use super::return_contract::extract_return_info;

/// Complete converged state and callable authority needed for one contract.
pub(crate) struct ContractExtractionInput<'a> {
    pub(crate) func: &'a ArcFunction,
    pub(crate) state_map: &'a AimsStateMap,
    pub(crate) classifier: &'a dyn ArcClassification,
    pub(crate) sigs: &'a FxHashMap<Name, MemoryContract>,
    pub(crate) scc_peers: &'a FxHashSet<Name>,
    pub(crate) context_regions: &'a [ContextRegion],
    pub(crate) interner: &'a ori_ir::StringInterner,
    pub(crate) builtins: &'a crate::borrow::BuiltinOwnershipSets,
    pub(crate) exact_callables: &'a FxHashSet<Name>,
}

/// Extract a [`MemoryContract`] from a converged intraprocedural state map.
///
/// Reads the backward-computed demand at the function entry point for each
/// parameter, and determines return value uniqueness from the function's
/// Return terminators.
///
/// # Parameters
///
/// - `scc_peers` — names of all functions in the same SCC (empty for
///   non-recursive functions). Used to determine `has_unbounded_stack`
///   via syntactic tail-position analysis.
/// - `context_regions` — TRMC context regions detected by the normalization
///   pass. Used to compute `ContextBehavior` fields.
/// - `interner` — string interner for protocol-builtin name lookup
///   (`@iter` / `ori_iter_drop`), consumed by `find_iter_consume_params`.
#[cfg(test)]
pub(crate) fn extract_contract(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    scc_peers: &FxHashSet<Name>,
    context_regions: &[ContextRegion],
    interner: &ori_ir::StringInterner,
) -> MemoryContract {
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    extract_contract_with_call_ownership(&ContractExtractionInput {
        func,
        state_map,
        classifier,
        sigs,
        scc_peers,
        context_regions,
        interner,
        builtins: &builtins,
        exact_callables: &FxHashSet::default(),
    })
}

/// Production contract extraction with exact callable identities and the
/// registry-derived builtin ownership authority supplied by the SCC driver.
pub(crate) fn extract_contract_with_call_ownership(
    input: &ContractExtractionInput<'_>,
) -> MemoryContract {
    let func = input.func;
    let state_map = input.state_map;
    let classifier = input.classifier;
    let sigs = input.sigs;
    let scc_peers = input.scc_peers;
    let context_regions = input.context_regions;
    let interner = input.interner;
    let builtins = input.builtins;
    let exact_callables = input.exact_callables;
    // Build a map of param_var → param_index for lookup.
    let param_vars: FxHashMap<ArcVarId, usize> = func
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| (p.var, i))
        .collect();

    // Structural scans supply access-class facts that backward cardinality
    // analysis cannot derive; field semantics live on [`ParamFacts`].
    let facts = detect_param_facts(
        func,
        sigs,
        &param_vars,
        classifier,
        builtins,
        exact_callables,
        interner,
    );

    let params: Vec<ParamContract> = func
        .params
        .iter()
        .enumerate()
        .map(|(i, param)| {
            if classifier.is_scalar(param.ty) {
                // Scalar parameters don't participate in RC.
                // Use conservative access to avoid confusion — scalars
                // have no RC obligations regardless.
                return ParamContract::CONSERVATIVE;
            }
            let state = state_map.var_state_at_block_entry(func.entry, param.var);
            param_contract_for(i, &state, &facts)
        })
        .collect();

    let return_info = extract_return_info(func, classifier, sigs, interner);

    let mut effects = state_map.effect_summary();

    // Recursive functions have constant stack only when every recursive call
    // within the SCC is syntactically tail-positioned.
    let has_unbounded_stack = if scc_peers.is_empty() {
        false
    } else {
        has_non_tail_recursive_calls(func, scc_peers)
    };
    effects.has_unbounded_stack = has_unbounded_stack;

    // FBIP inference from converged effect summary.
    // A function is FBIP if it never allocates on any code path.
    let is_fbip = !effects.may_allocate;

    // Token balance classifies FIP, but certification also requires constant
    // stack. Post-emission missed-reuse evidence supplies the final
    // deallocation verdict; allocation-free FBIP remains immediately valid.
    let fip = if has_unbounded_stack {
        // Unbounded stack growth → cannot be Certified regardless of
        // allocation balance. Downgrade to Never (conservative).
        FipContract::Never
    } else if is_fbip {
        // No allocations at all → trivially FIP (FBIP is stronger than FIP).
        FipContract::Certified
    } else if !effects.may_share {
        // Function doesn't share references. Check token balance for FIP.
        let requires_unique = compute_requires_unique_params(state_map, func);
        let consumed_count = requires_unique.iter().filter(|&&r| r).count();
        let construct_count = state_map.fip_construct_count() as usize;
        let any_requires_unique = requires_unique.iter().any(|&r| r);

        if consumed_count >= construct_count && any_requires_unique {
            // Token balanced, but some params need caller-guaranteed uniqueness
            // for their memory to be reusable. Conditional FIP.
            FipContract::Conditional {
                requires_unique_params: requires_unique,
            }
        } else if consumed_count >= construct_count {
            // Token balanced and no param requires uniqueness — all reuse
            // comes from local deaths. Unconditionally FIP.
            FipContract::Certified
        } else {
            // Net allocation is bounded: function allocates more than it
            // reuses, but the count is known. FIPTree's fip(n) pattern.
            let net = construct_count.saturating_sub(consumed_count);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "net allocation count fits u16 in practice"
            )]
            let n = net.min(u16::MAX as usize) as u16;
            FipContract::Bounded(n)
        }
    } else {
        FipContract::Never
    };

    let context_behavior = compute_context_behavior(func, context_regions, effects);

    MemoryContract {
        params,
        return_info,
        effects,
        context_behavior,
        fip,
        is_fbip,
    }
}

/// Build one non-scalar [`ParamContract`] from the converged entry state +
/// the structural [`ParamFacts`] for param index `i`.
///
/// Access is upgraded to Owned when the param flows to a consuming callee
/// OR a Direct Return (the param IS the returned value). Project-return
/// params are NOT promoted — body-derived classification prevails per
/// Spec: §1.4 Uniqueness.
fn param_contract_for(i: usize, state: &AimsState, facts: &ParamFacts) -> ParamContract {
    let access = if facts.consumed.contains(&i) {
        AccessClass::Owned
    } else {
        state.access
    };
    // INVARIANT: Return shape is structural; caller ownership decides whether
    // it suppresses an argument release or requires compensation.
    let return_alias = facts.return_alias_shapes.get(&i).copied();
    ParamContract {
        access,
        consumption: state.consumption,
        cardinality: state.cardinality,
        // v1: locality from backward demand
        locality_bound: state.locality,
        // Publish the converged sharing proof so realized retains and the
        // caller-visible contract describe the same ownership effect.
        may_escape: false,
        may_share: state.uniqueness != Uniqueness::Unique
            || (access == AccessClass::Borrowed && facts.owner_credit.contains(&i)),
        // Caller-side uniqueness: set to MaybeShared by default.
        // Tightened to Unique by post-fixpoint demand propagation
        // when all callers satisfy the condition.
        uniqueness: Uniqueness::MaybeShared,
        // Direct structural return flow, not freshness, gates scope-exit
        // decrement suppression and implies `return_alias == Some(Direct)`.
        transfers_through_return: facts.return_flow.contains(&i),
        return_alias,
        // Payload containment means the parameter enters a returned
        // transitive-drop payload; unlike `return_alias`, the result need not
        // be the same allocation.
        return_payload_contains_param: facts.payload_containment.contains(&i),
        // RL-2 iter-consume transfer fact (proven sound:
        // `AimsProof.Realization::RL2_iter_consuming_caller_dec_splits`).
        iter_consumes: facts.iter_consume.contains(&i),
        // RL-2: surviving borrowed collection arguments remain caller-owned;
        // COW-mutated parameters are excluded.
        borrowed_read_only: facts.borrowed_read_only.contains(&i),
        // RL-1 + RL-2 borrowed-COW-consume-at-death fact — the caller's
        // class-ledger boundary classification funds one distinct reference
        // per call site.
        borrowed_cow_consumed: facts.borrowed_cow_consumed.contains(&i),
        // MUTATOR-only refinement (excludes the builtin `iter`) — the
        // borrowed-`Invoke` lineage gate (c3) declines on it.
        borrowed_cow_mutated: facts.borrowed_cow_mutated.contains(&i),
        // The canonical structural recognizer replaces this conservative
        // value when it publishes the local witness and caller projection.
        exact_transfer: crate::aims::contract::ExactTransferState::Unproven,
    }
}

// Context behavior computation

/// Compute [`ContextBehavior`] from detected TRMC context regions.
///
/// When no context regions exist (non-TRMC function), returns `default`.
/// When regions exist:
/// - `preserves_context`: true if any context variable flows to a Return
/// - `consumes_hole`: true if any context region has a hole field write
///   (always true by definition — the region is detected because a recursive
///   call fills the hole)
/// - `requires_unique_context`: always `true` (modulo-cons instantiation)
/// - `may_resume_nonlinearly`: `effects.may_share` (conservative)
fn compute_context_behavior(
    func: &ArcFunction,
    context_regions: &[ContextRegion],
    effects: EffectSummary,
) -> ContextBehavior {
    if context_regions.is_empty() {
        return ContextBehavior::default();
    }

    // Collect all context variables for return-flow check.
    let context_vars: FxHashSet<ArcVarId> = context_regions.iter().map(|r| r.context_var).collect();

    // Check if any context variable flows to a Return terminator.
    // This indicates the function preserves the context (returns it
    // rather than consuming/dropping it).
    let preserves_context = func.blocks.iter().any(|block| {
        if let ArcTerminator::Return { value } = &block.terminator {
            context_vars.contains(value)
        } else {
            false
        }
    });

    // By definition, every detected ContextRegion has a hole field that
    // is filled by a recursive call — that's what makes it a TRMC candidate.
    let consumes_hole = true;

    ContextBehavior {
        preserves_context,
        consumes_hole,
        requires_unique_context: true, // modulo-cons instantiation only
        may_resume_nonlinearly: effects.may_share,
    }
}

/// Build a map from variable to its defining instruction.
///
/// In ARC IR's SSA form, each variable is defined exactly once.
/// Block parameters, function parameters, and Invoke-defined variables
/// are not included (Invoke is a terminator, handled separately).
pub(super) fn build_definition_map(func: &ArcFunction) -> FxHashMap<ArcVarId, &ArcInstr> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                map.insert(dst, instr);
            }
        }
    }
    map
}

/// Build a map from Invoke-defined variables to their callee names.
///
/// Invoke terminators define a dst variable in the normal successor block.
/// This map captures those definitions separately from instruction definitions.
pub(super) fn build_invoke_def_map(func: &ArcFunction) -> FxHashMap<ArcVarId, Name> {
    let mut map = FxHashMap::default();
    for block in &func.blocks {
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            map.insert(*dst, *callee);
        }
    }
    map
}
