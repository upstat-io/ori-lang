//! Contract extraction from converged intraprocedural state maps.
//!
//! After backward dataflow analysis converges, [`extract_contract`] reads the
//! per-parameter demand at the function entry point and determines return
//! value uniqueness to produce a [`MemoryContract`].

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::tail_call::has_non_tail_recursive_calls;
use crate::ArcClassification;

use super::super::contract::{
    ContextBehavior, ContextRegion, FipContract, MemoryContract, ParamContract, ReturnAliasShape,
};
use super::super::intraprocedural::compute_requires_unique_params;
use super::super::intraprocedural::AimsStateMap;
use super::super::lattice::{AccessClass, Uniqueness};

mod alias_flow;
mod param_facts;
mod return_contract;

pub(crate) use alias_flow::build_alias_to_param_map;
use alias_flow::{
    find_consumed_via_callees, find_payload_containment_params, find_return_alias_shapes,
    find_return_flow_params,
};
use param_facts::{find_borrowed_read_only_params, find_iter_consume_params};
use return_contract::extract_return_info;

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
pub(crate) fn extract_contract(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    scc_peers: &FxHashSet<Name>,
    context_regions: &[ContextRegion],
    interner: &ori_ir::StringInterner,
) -> MemoryContract {
    // Build a map of param_var → param_index for lookup.
    let param_vars: FxHashMap<ArcVarId, usize> = func
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| (p.var, i))
        .collect();

    // Per-param facts (consume/return/alias/containment/iter/borrowed-RO).
    // The backward analysis only tracks cardinality demand through Apply
    // instructions, so these structural scans supply the access-class facts
    // the state map cannot; field semantics live on [`ParamFacts`].
    let facts = detect_param_facts(func, sigs, &param_vars, interner);

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

    let return_info = extract_return_info(func, classifier, sigs);

    let mut effects = state_map.effect_summary();

    // Constant stack verification.
    // Non-recursive functions have constant stack by definition. Recursive
    // functions have constant stack only if ALL recursive calls (to self
    // or mutual-recursion partners) are in syntactic tail position.
    let has_unbounded_stack = if scc_peers.is_empty() {
        false
    } else {
        has_non_tail_recursive_calls(func, scc_peers)
    };
    effects.has_unbounded_stack = has_unbounded_stack;

    // FBIP inference from converged effect summary.
    // A function is FBIP if it never allocates on any code path.
    let is_fbip = !effects.may_allocate;

    // FIP natural detection from converged state.
    // Token balance determines FIP classification without a separate pass.
    //
    // FP² Theorem 2 requires `!may_allocate && !may_deallocate` for full FIP.
    // At contract extraction time, `may_deallocate` is optimistic (`false`) —
    // the true value is computed post-emission from `FipEvidence.missed_reuses`
    // and applied in the second pass of `run_aims_pipeline_all`.
    // The FBIP fast path (`!may_allocate → Certified`) is always valid: if the
    // function never allocates, it trivially never deallocates.
    //
    // FIP also requires constant stack — `has_unbounded_stack`
    // must be `false` for Certified. Functions with non-tail recursion that
    // are allocation-balanced get Bounded, not Certified.
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
fn param_contract_for(
    i: usize,
    state: &super::super::lattice::AimsState,
    facts: &ParamFacts,
) -> ParamContract {
    let access = if facts.consumed.contains(&i) {
        AccessClass::Owned
    } else {
        state.access
    };
    // return_alias is populated whenever the param structurally flows
    // to a Direct or Project Return, regardless of access. The shape
    // is a caller-side signal: when the caller passes the arg Owned,
    // `apply_aliases::install_alias_entry` records the alias entry so
    // the realize walk suppresses the caller's scope-exit dec on the
    // arg. Borrowed-callee + Borrowed-caller-arg pairs leave the
    // signal unused. Owned-only callee-side compensation is gated
    // separately in `realize::emit_unified::build_return_project_inc_targets`.
    let return_alias = facts.return_alias_shapes.get(&i).copied();
    ParamContract {
        access,
        consumption: state.consumption,
        cardinality: state.cardinality,
        // v1: locality from backward demand
        locality_bound: state.locality,
        // v1 conservative — refined in 03.3 escape/share analysis
        may_escape: false,
        may_share: false,
        // Caller-side uniqueness: set to MaybeShared by default.
        // Tightened to Unique by post-fixpoint demand propagation
        // when all callers satisfy the condition.
        uniqueness: Uniqueness::MaybeShared,
        // Param flows directly to a `Return { value }` terminator —
        // gates scope-exit dec suppression in the realize walk.
        // Computed from the STRUCTURAL Return-flow alias fact (NOT from
        // `preserves_freshness`, which is currently spec-inverted).
        // Invariant: `transfers_through_return == true` IFF
        // `return_alias == Some(Direct)`.
        transfers_through_return: facts.return_flow.contains(&i),
        return_alias,
        // Structural payload-containment fact — the param flows into a
        // returned transitive-drop variant payload. Distinct from
        // `return_alias`, which covers the result being an ALIAS of the
        // param. Consumed by `populate_class_payload_of_with_liveness`.
        return_payload_contains_param: facts.payload_containment.contains(&i),
        // Per-path wrap proof — contained in the returned EnumVariant
        // payload on EVERY return path. Gates the Phase-6.99 wrapped
        // transfer-anchor credit (RL-1 mint / RL-2 transfer-through).
        return_payload_contains_param_all_paths: facts.payload_containment_all_paths.contains(&i),
        // RL-2 iter-consume transfer fact (proven sound:
        // `AimsProof.Realization::RL2_iter_consuming_caller_dec_splits`).
        iter_consumes: facts.iter_consume.contains(&i),
        // RL-2 borrowed-read-only fact: a surviving Borrowed collection arg
        // is `ApplyToBorrowedParam` (caller decs); COW-mutated params are
        // excluded. Proven sound:
        // `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec`.
        borrowed_read_only: facts.borrowed_read_only.contains(&i),
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
    effects: super::super::contract::EffectSummary,
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

/// Detect parameters that flow (possibly through Let aliases) to a callee
/// that consumes them at an Owned position.
///
/// The backward analysis only tracks cardinality demand for Apply arguments,
/// not access class. Parameters passed to consuming builtins (like `iter`)
/// retain Borrowed access in the state map. This scan upgrades them.
///
/// Walks all blocks, tracks Let alias chains from param vars, and checks
/// Apply/Invoke call sites against callee contracts in `sigs`.
///
/// `alias_to_param` is multi-valued: a single variable may alias multiple
/// parameters via merge-block phi nodes. Branch+Jump merges both x and y
/// into the merge-block's param r, so r aliases BOTH parameter indices.
/// Select aliases work the same way (`true_val` and `false_val` both reach
/// dst at runtime); the Select arm below tracks both as alias sources.
/// Per-param facts detected for contract construction by [`detect_param_facts`].
struct ParamFacts {
    /// Params consumed via callees OR returned — promoted to `Owned` access.
    consumed: FxHashSet<usize>,
    /// Params traced to a `Return { value }` terminator (Direct case).
    /// Subset of `consumed`; sets `transfers_through_return` so the param
    /// receives no scope-exit `RcDec` (ownership transfers through the return).
    return_flow: FxHashSet<usize>,
    /// Per-param `ReturnAliasShape` (Direct or Project) — the caller-side
    /// signal for the `apply_result_aliases` side-table. Direct entries
    /// mirror `return_flow`; Project entries detect `Return { value }` whose
    /// definition is a `Project` off the param's var.
    return_alias_shapes: FxHashMap<usize, ReturnAliasShape>,
    /// Params flowing into a returned transitive-drop variant payload
    /// (e.g., `wrap_ok(m) = Ok(m)`) — contained IN the result, not aliased
    /// to it. Consumed by the `class_payload_of` liveness gate.
    payload_containment: FxHashSet<usize>,
    /// Subset of `payload_containment`: params contained in the returned
    /// `EnumVariant` payload on EVERY return path — the per-path wrap proof
    /// consumed by the Phase-6.99 wrapped transfer-anchor admission.
    payload_containment_all_paths: FxHashSet<usize>,
    /// Params iter-consumed (`@iter` -> `ori_iter_drop`) inside the body —
    /// the RL-2 inward ownership transfer.
    iter_consume: FxHashSet<usize>,
    /// Params that flow ONLY to borrowed positions in the body (no
    /// owned-position COW/transfer consumer) — the affirmative read-only
    /// complement of `consumed` + `iter_consume` feeding the caller
    /// carve-out gate.
    borrowed_read_only: FxHashSet<usize>,
}

/// Detect every structural [`ParamFacts`] set in one pass over the body
/// (per-field semantics on the struct).
///
/// Owned-only invariant: the consume / return-flow sets feed `access`
/// promotion. Borrowed-param Project returns must NOT set `return_alias`;
/// this is enforced by extending `consumed` to include Project-Return
/// params, which routes them through the Owned access path in
/// [`param_contract_for`].
fn detect_param_facts(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    param_vars: &FxHashMap<ArcVarId, usize>,
    interner: &ori_ir::StringInterner,
) -> ParamFacts {
    let alias_to_param = build_alias_to_param_map(func, param_vars, Some(sigs));
    let mut consumed = find_consumed_via_callees(func, sigs, &alias_to_param);
    let return_flow = find_return_flow_params(func, &alias_to_param);
    let return_alias_shapes = find_return_alias_shapes(func, &alias_to_param);
    let containment = find_payload_containment_params(func, &alias_to_param);
    let iter_consume = find_iter_consume_params(func, sigs, &alias_to_param, interner);
    let borrowed_read_only = find_borrowed_read_only_params(func, sigs, &alias_to_param, interner);
    // Direct-return params are promoted to Owned (Lean 4 `ownParamsUsingArgs`
    // pattern): the param IS the returned value, so the caller takes
    // ownership of the param's allocation through the return slot.
    //
    // Project-return params are NOT promoted — body-derived classification
    // prevails. A function whose body is only `Project p.f → Return` does
    // not consume `p`; promoting `p` to Owned would violate Spec: §1.4
    // Uniqueness. Caller-side dec-suppression (when the caller passes the
    // arg Owned) is handled by `apply_aliases::install_alias_entry` reading
    // `ParamContract.return_alias`, independent of the param's own access.
    //
    // Payload-containment params are NOT promoted to Owned: the param is
    // contained IN a returned construct, not aliased TO the result. The
    // caller's edge-recording gate (post_convergence.rs) reads
    // `return_payload_contains_param` to record the class_payload_of edge
    // even for Borrowed-access params.
    consumed.extend(&return_flow);
    ParamFacts {
        consumed,
        return_flow,
        return_alias_shapes,
        payload_containment: containment.any,
        payload_containment_all_paths: containment.all_paths,
        iter_consume,
        borrowed_read_only,
    }
}

/// Build a map from variable to its defining instruction.
///
/// In ARC IR's SSA form, each variable is defined exactly once.
/// Block parameters, function parameters, and Invoke-defined variables
/// are not included (Invoke is a terminator, handled separately).
fn build_definition_map(func: &ArcFunction) -> FxHashMap<ArcVarId, &ArcInstr> {
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
fn build_invoke_def_map(func: &ArcFunction) -> FxHashMap<ArcVarId, Name> {
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
