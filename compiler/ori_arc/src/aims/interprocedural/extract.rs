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
    ContextBehavior, ContextRegion, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use super::super::intraprocedural::compute_requires_unique_params;
use super::super::intraprocedural::AimsStateMap;
use super::super::lattice::{AccessClass, Uniqueness};

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
///   pass. Used to compute `ContextBehavior` fields (Section 13.1).
pub(crate) fn extract_contract(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    scc_peers: &FxHashSet<Name>,
    context_regions: &[ContextRegion],
) -> MemoryContract {
    // Build a map of param_var → param_index for lookup.
    let param_vars: FxHashMap<ArcVarId, usize> = func
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| (p.var, i))
        .collect();

    // Compute which parameters flow (possibly through Let aliases) to a
    // callee that consumes them at an Owned position. The backward analysis
    // doesn't propagate access class through Apply instructions (it only
    // tracks cardinality demand), so parameters that reach a consuming
    // builtin (e.g., `iter`) stay at Borrowed. This post-hoc upgrade
    // catches those cases.
    //
    // `return_flow_params` is a SUBSET of `consumed_params` containing ONLY
    // params traced to a `Return { value }` terminator. Distinct from the
    // broader `consumed_params` set (which mixes Apply/Invoke-arg
    // consumption with Return-flow alias). Used to populate
    // `transfers_through_return` on `ParamContract` per BUG-04-090's fix:
    // params that flow through Return must NOT receive scope-exit RcDec
    // because ownership transfers through the return value.
    let (consumed_params, return_flow_params) = detect_consumed_params(func, sigs, &param_vars);

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
            // Upgrade access to Owned if the parameter flows to a consuming
            // callee. This ensures the interprocedural contract correctly
            // reflects that the function consumes (not just borrows) the
            // parameter's data.
            let access = if consumed_params.contains(&i) {
                AccessClass::Owned
            } else {
                state.access
            };
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
                // (Section 09.1) when all callers satisfy the condition.
                uniqueness: Uniqueness::MaybeShared,
                // BUG-04-090: param flows directly to a `Return { value }`
                // terminator. Gates scope-exit dec suppression in the
                // realize walk. Computed from the STRUCTURAL Return-flow
                // alias fact (NOT from `preserves_freshness` which is
                // currently spec-inverted per `aims-rules.md §1.9`).
                transfers_through_return: return_flow_params.contains(&i),
            }
        })
        .collect();

    let return_info = extract_return_info(func, classifier, sigs);

    let mut effects = state_map.effect_summary();

    // Section 12.2: Constant stack verification.
    // Non-recursive functions have constant stack by definition. Recursive
    // functions have constant stack only if ALL recursive calls (to self
    // or mutual-recursion partners) are in syntactic tail position.
    let has_unbounded_stack = if scc_peers.is_empty() {
        false
    } else {
        has_non_tail_recursive_calls(func, scc_peers)
    };
    effects.has_unbounded_stack = has_unbounded_stack;

    // Section 09.2: FBIP inference from converged effect summary.
    // A function is FBIP if it never allocates on any code path.
    let is_fbip = !effects.may_allocate;

    // Section 09.2: FIP natural detection from converged state.
    // Token balance determines FIP classification without a separate pass.
    //
    // FP² Theorem 2 requires `!may_allocate && !may_deallocate` for full FIP.
    // At contract extraction time, `may_deallocate` is optimistic (`false`) —
    // the true value is computed post-emission from `FipEvidence.missed_reuses`
    // and applied in the second pass of `run_aims_pipeline_all()` (Section 12.1).
    // The FBIP fast path (`!may_allocate → Certified`) is always valid: if the
    // function never allocates, it trivially never deallocates.
    //
    // Section 12.2: FIP also requires constant stack — `has_unbounded_stack`
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

// Context behavior computation (Section 13.1)

/// Compute [`ContextBehavior`] from detected TRMC context regions.
///
/// When no context regions exist (non-TRMC function), returns `default()`.
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
fn detect_consumed_params(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    param_vars: &FxHashMap<ArcVarId, usize>,
) -> (FxHashSet<usize>, FxHashSet<usize>) {
    let alias_to_param = build_alias_to_param_map(func, param_vars);
    let mut consumed = find_consumed_via_callees(func, sigs, &alias_to_param);
    let return_flow = find_return_flow_params(func, &alias_to_param);
    // Returned params are still Owned (Lean ownParamsUsingArgs); fold the
    // return-flow set into `consumed` so the caller transfers ownership.
    consumed.extend(&return_flow);
    (consumed, return_flow)
}

/// Build the multi-valued alias map: variable → set of parameter indices
/// it aliases. Covers Let{Var} aliases, Select conditional aliases, and
/// Jump-arg → block-parameter passing. Iterates to fixed point.
fn build_alias_to_param_map(
    func: &ArcFunction,
    param_vars: &FxHashMap<ArcVarId, usize>,
) -> FxHashMap<ArcVarId, FxHashSet<usize>> {
    let mut alias_to_param: FxHashMap<ArcVarId, FxHashSet<usize>> = param_vars
        .iter()
        .map(|(&v, &idx)| {
            let mut set = FxHashSet::default();
            set.insert(idx);
            (v, set)
        })
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                changed |= absorb_instr_aliases(instr, &mut alias_to_param);
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_params = &func.blocks[target.index()].params;
                for (arg, &(param_var, _)) in args.iter().zip(target_params.iter()) {
                    changed |= absorb_alias(*arg, param_var, &mut alias_to_param);
                }
            }
        }
    }
    alias_to_param
}

/// Absorb alias edges from a single instruction. Returns true if any
/// destination set grew.
fn absorb_instr_aliases(
    instr: &ArcInstr,
    alias_to_param: &mut FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> bool {
    match instr {
        // Let { dst, Var(src) } — direct alias
        ArcInstr::Let {
            dst,
            value: crate::ir::ArcValue::Var(src),
            ..
        } => absorb_alias(*src, *dst, alias_to_param),
        // Select { dst, true_val, false_val } — conditional alias.
        // Either branch may flow to dst at runtime; track BOTH.
        ArcInstr::Select {
            dst,
            true_val,
            false_val,
            ..
        } => {
            let a = absorb_alias(*true_val, *dst, alias_to_param);
            let b = absorb_alias(*false_val, *dst, alias_to_param);
            a || b
        }
        _ => false,
    }
}

/// Extend `dst`'s alias set with `src`'s. Returns true if `dst`'s set grew.
fn absorb_alias(
    src: ArcVarId,
    dst: ArcVarId,
    alias_to_param: &mut FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> bool {
    let Some(src_set) = alias_to_param.get(&src).cloned() else {
        return false;
    };
    let dst_set = alias_to_param.entry(dst).or_default();
    let before = dst_set.len();
    dst_set.extend(src_set);
    dst_set.len() != before
}

/// Scan Apply / Invoke call sites for arguments that alias a parameter
/// and flow to a callee with an Owned parameter contract. Returns the
/// set of parameter indices consumed via callees.
fn find_consumed_via_callees(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> FxHashSet<usize> {
    let mut consumed = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                absorb_owned_callee_args(*callee, args, sigs, alias_to_param, &mut consumed);
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            absorb_owned_callee_args(*callee, args, sigs, alias_to_param, &mut consumed);
        }
    }
    consumed
}

/// For each arg position where the callee parameter is Owned and the arg
/// aliases function parameters, record those parameter indices in `consumed`.
fn absorb_owned_callee_args(
    callee: Name,
    args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    consumed: &mut FxHashSet<usize>,
) {
    let callee_contract = sigs.get(&callee);
    for (pos, &arg) in args.iter().enumerate() {
        let Some(param_indices) = alias_to_param.get(&arg) else {
            continue;
        };
        let callee_owned = callee_contract.is_some_and(|c| {
            c.params
                .get(pos)
                .is_some_and(|p| p.access == AccessClass::Owned)
        });
        if callee_owned {
            for &idx in param_indices {
                consumed.insert(idx);
            }
        }
    }
}

/// Identify parameters that flow to a Return terminator (directly or
/// through Let / Jump-arg / Select alias chains). These params must be
/// Owned (Lean 4 `Borrow.lean` `ownParamsUsingArgs`) AND get
/// `transfers_through_return = true` for the BUG-04-090 fix — the gate
/// reads this STRUCTURAL fact (Return-trace only), kept distinct from
/// the Apply/Invoke consumption set.
fn find_return_flow_params(
    func: &ArcFunction,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> FxHashSet<usize> {
    let mut return_flow: FxHashSet<usize> = FxHashSet::default();
    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            if let Some(param_indices) = alias_to_param.get(value) {
                for &idx in param_indices {
                    return_flow.insert(idx);
                }
            }
        }
    }
    return_flow
}

// Return info extraction

/// Determine return value uniqueness from the function's Return terminators.
///
/// Walks all Return terminators, traces each returned variable to its
/// definition instruction, and determines uniqueness based on how the
/// value was produced. Results from all return paths are joined.
fn extract_return_info(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> ReturnContract {
    // Build definition map: var → defining instruction.
    let def_map = build_definition_map(func);

    // Build Invoke definition map: dst → callee name
    // (Invoke is a terminator, not an instruction).
    let invoke_defs = build_invoke_def_map(func);

    // Collect parameter variables for identity checks.
    let param_vars: rustc_hash::FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();

    let mut return_uniqueness = None::<Uniqueness>;
    let mut all_preserve_freshness = true;

    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            let (uniq, preserves) = var_uniqueness(
                *value,
                &def_map,
                &invoke_defs,
                &param_vars,
                classifier,
                sigs,
            );

            return_uniqueness = Some(match return_uniqueness {
                None => uniq,
                Some(prev) => prev.join(uniq),
            });

            if !preserves {
                all_preserve_freshness = false;
            }
        }
    }

    match return_uniqueness {
        Some(uniqueness) => ReturnContract {
            uniqueness,
            preserves_freshness: all_preserve_freshness,
            ..ReturnContract::CONSERVATIVE
        },
        // No Return terminators (e.g., infinite loop) — conservative.
        None => ReturnContract::CONSERVATIVE,
    }
}

/// Determine the uniqueness of a variable based on its definition.
///
/// Returns `(Uniqueness, preserves_freshness)`.
fn var_uniqueness(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, &ArcInstr>,
    invoke_defs: &FxHashMap<ArcVarId, Name>,
    param_vars: &rustc_hash::FxHashSet<ArcVarId>,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(instr) = def_map.get(&var) {
        match instr {
            // Fresh construction, closure, COW reuse, or scalar check → unique.
            ArcInstr::Construct { .. }
            | ArcInstr::Reuse { .. }
            | ArcInstr::PartialApply { .. }
            | ArcInstr::CollectionReuse { .. }
            | ArcInstr::IsShared { .. } => (Uniqueness::Unique, true),

            // Direct call → use callee's return contract.
            ArcInstr::Apply { func: callee, .. } => callee_return_uniqueness(*callee, sigs),

            // Let binding → trace through to source.
            ArcInstr::Let { value, ty, .. } => match value {
                crate::ir::ArcValue::Var(source) => {
                    if param_vars.contains(source) {
                        // Returning a parameter → preserves freshness
                        // (if caller passes unique, return is unique).
                        (Uniqueness::MaybeShared, true)
                    } else {
                        var_uniqueness(*source, def_map, invoke_defs, param_vars, classifier, sigs)
                    }
                }
                crate::ir::ArcValue::Literal(_) => (Uniqueness::Unique, true),
                crate::ir::ArcValue::PrimOp { .. } => {
                    if classifier.is_scalar(*ty) {
                        (Uniqueness::Unique, true)
                    } else {
                        (Uniqueness::MaybeShared, false)
                    }
                }
            },

            // Indirect call, projection, select, RC/mutation ops → conservative.
            ArcInstr::ApplyIndirect { .. }
            | ArcInstr::Project { .. }
            | ArcInstr::Select { .. }
            | ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. }
            | ArcInstr::Reset { .. } => (Uniqueness::MaybeShared, false),
        }
    } else if let Some(callee) = invoke_defs.get(&var) {
        // Defined by an Invoke terminator → use callee's return contract.
        callee_return_uniqueness(*callee, sigs)
    } else if param_vars.contains(&var) {
        // Returning a parameter directly → uniqueness depends on caller.
        (Uniqueness::MaybeShared, true)
    } else {
        // Block parameter or unknown definition → conservative.
        (Uniqueness::MaybeShared, false)
    }
}

/// Look up a callee's return uniqueness from its contract.
fn callee_return_uniqueness(
    callee: Name,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> (Uniqueness, bool) {
    if let Some(contract) = sigs.get(&callee) {
        (
            contract.return_info.uniqueness,
            contract.return_info.preserves_freshness,
        )
    } else {
        (Uniqueness::MaybeShared, false)
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
