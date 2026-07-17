//! Retired immediate-emission contract logic kept only for regression tests.
//!
//! Production impl methods are members of the same closed AIMS batch as every
//! other executable body. This module models the superseded per-method repair
//! path so its historical edge cases remain pinned without exposing that path
//! in a production build.
//!
//! INVARIANT: the published contract never claims behavior the compiled
//! method does not have. The method's own pipeline runs with the
//! `ensure_own_contract` conservative seed (params Owned, no contract-driven
//! dec suppression), so only callee-independent structural facts (an owned
//! param returned directly) may be published; every ownership-cooperative
//! field stays at its conservative default.
//!
//! Diagnostics share the `ori_arc::aims::interprocedural` target with the
//! fixed-point driver because both report the same contract computation.

use std::collections::hash_map::Entry;

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use crate::verify::VerifyError;
use crate::ArcClassification;

use super::super::builtins::seed_builtin_contracts;
use super::super::contract::{MemoryContract, ReturnAliasShape};
use super::scc_driver::analyze_scc_single;

/// Compute the as-compiled contract for each analysis-lowered impl method.
///
/// Each function is analyzed in isolation (builtin contracts seeded; unknown
/// callees conservative) and the extracted contract is sanitized: every param
/// starts from `ParamContract::CONSERVATIVE`; only the structural
/// `transfers_through_return` + `return_alias == Some(Direct)` pair is
/// copied through. Keys are the analysis function names.
///
/// The shared AIMS input seam freezes an empty primitive-fact table exactly
/// once and validates every non-empty table without repair before analysis.
/// Invalid or partial frozen evidence rejects the batch.
pub(super) fn compute_impl_method_contracts(
    funcs: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    builtins: &BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
) -> Result<FxHashMap<Name, MemoryContract>, Vec<VerifyError>> {
    if funcs.is_empty() {
        return Ok(FxHashMap::default());
    }
    super::super::freeze_primitive_facts(funcs, classifier)?;
    let mut seed: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    seed_builtin_contracts(&mut seed, builtins, interner);
    let exact_callables: FxHashSet<Name> = funcs.iter().map(|func| func.name).collect();

    let mut out: FxHashMap<Name, MemoryContract> = FxHashMap::default();
    for func in funcs {
        let extracted = analyze_scc_single(
            func,
            classifier,
            &seed,
            interner,
            builtins,
            &exact_callables,
            &crate::pipeline::callable_boundary::ValidatedCallableBoundaryFacts::empty(),
        );
        out.insert(func.name, sanitize_to_as_compiled(&extracted));
    }

    if tracing::enabled!(target: "ori_arc::aims::interprocedural", tracing::Level::DEBUG) {
        for (name, contract) in &out {
            let ttr: Vec<usize> = contract
                .params
                .iter()
                .enumerate()
                .filter(|(_, p)| p.transfers_through_return)
                .map(|(i, _)| i)
                .collect();
            tracing::debug!(
                target: "ori_arc::aims::interprocedural",
                fn_name = interner.lookup(*name),
                params = contract.params.len(),
                transfers_through_return_params = ?ttr,
                "impl-method as-compiled contract computed",
            );
        }
    }

    Ok(out)
}

/// Project an extracted contract onto the as-compiled shape: conservative in
/// every dimension except the structural Direct return-flow pair.
fn sanitize_to_as_compiled(extracted: &MemoryContract) -> MemoryContract {
    let mut out = MemoryContract::conservative(extracted.params.len());
    for (sanitized, full) in out.params.iter_mut().zip(&extracted.params) {
        // Direct-only: a Project / containment alias stays conservative —
        // the caller-side machinery for those shapes assumes contract-aware
        // callee compilation the immediate-emit path does not perform.
        if full.transfers_through_return && full.return_alias == Some(ReturnAliasShape::Direct) {
            sanitized.transfers_through_return = true;
            sanitized.return_alias = Some(ReturnAliasShape::Direct);
        }
    }
    out
}

/// Augment a base contract map with receiver-resolved impl-method bindings
/// for ONE function's call sites.
///
/// A bare callee name binds to an impl-method contract iff ALL hold:
/// - the name is not the function's own name (self-recursion stays
///   conservative),
/// - the name has no entry in `base` (free functions, monos, and seeded
///   builtins keep their existing contract),
/// - EVERY `Apply` / `Invoke` site in `func` with that callee name resolves
///   through its receiver type (`args[0]`) to the SAME impl-method contract
///   in `impl_contracts`.
///
/// Any unresolved or divergent site poisons the name (conservative
/// no-contract treatment, the status quo). Returns `None` when no binding
/// fires — callers pass the base map through unchanged.
pub(super) fn augment_contracts_with_impl_callees(
    func: &ArcFunction,
    base: &FxHashMap<Name, MemoryContract>,
    impl_contracts: &FxHashMap<(Idx, Name), MemoryContract>,
    pool: &Pool,
) -> Option<FxHashMap<Name, MemoryContract>> {
    if impl_contracts.is_empty() {
        return None;
    }
    let candidate_names: FxHashSet<Name> = impl_contracts.keys().map(|&(_, m)| m).collect();

    let mut bindings: FxHashMap<Name, (Idx, Name)> = FxHashMap::default();
    let mut poisoned: FxHashSet<Name> = FxHashSet::default();

    let mut consider = |callee: Name, args: &[ArcVarId]| {
        if !candidate_names.contains(&callee) || poisoned.contains(&callee) {
            return;
        }
        if callee == func.name || base.contains_key(&callee) {
            poisoned.insert(callee);
            return;
        }
        let Some(&recv) = args.first() else {
            // No receiver to disambiguate with (associated-function shape).
            poisoned.insert(callee);
            return;
        };
        let recv_ty = pool.resolve_fully(func.var_type(recv));
        let key = (recv_ty, callee);
        if impl_contracts.contains_key(&key) {
            match bindings.entry(callee) {
                Entry::Occupied(prior) if *prior.get() != key => {
                    poisoned.insert(callee);
                }

                Entry::Vacant(slot) => {
                    slot.insert(key);
                }

                Entry::Occupied(_) => {}
            }
        } else {
            // A same-named site that is NOT this impl method (builtin,
            // derived, other type) — the name is ambiguous in this function.
            tracing::debug!(
                target: "ori_arc::aims::interprocedural",
                caller = ?func.name,
                callee_bare = ?callee,
                recv_ty = ?recv_ty,
                known_keys = ?impl_contracts.keys().filter(|(_, m)| *m == callee).collect::<Vec<_>>(),
                "impl-method contract binding declined — receiver key miss",
            );
            poisoned.insert(callee);
        }
    };

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                consider(*callee, args);
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            consider(*callee, args);
        }
    }

    bindings.retain(|name, _| !poisoned.contains(name));

    // Own-contract binding: when `func` IS an analysis-lowered impl method
    // (receiver-resolved by its own self-param type, same keying family as the
    // call-site path), its as-compiled sanitized contract becomes its OWN
    // entry. Phase-5's own-contract consumers (the RL-2 transfer-source-dec
    // strip on `transfers_through_return` params; the RL-1/RL-34
    // forwarder-identity alias transparency) then see the SAME structural
    // Direct-ttr pair the method's callers already see — a multi-block-used
    // ttr param otherwise keeps a scope-exit dec the caller's view says cannot
    // exist (caller/callee over-release on the moved-through allocation).
    // Declines when the bare name has a `base` entry (a free function owns the
    // name) or any call site in `func` carries the name (`poisoned` — the
    // self-name receiver could resolve to a DIFFERENT type's method).
    if !base.contains_key(&func.name) && !poisoned.contains(&func.name) {
        if let Some(recv) = func.params.first() {
            let key = (pool.resolve_fully(func.var_type(recv.var)), func.name);
            if impl_contracts.contains_key(&key) {
                bindings.insert(func.name, key);
            }
        }
    }

    if bindings.is_empty() {
        return None;
    }

    let mut augmented = base.clone();
    for (bare, key) in &bindings {
        let contract = &impl_contracts[key];
        if tracing::enabled!(target: "ori_arc::aims::interprocedural", tracing::Level::DEBUG) {
            let ttr: Vec<usize> = contract
                .params
                .iter()
                .enumerate()
                .filter(|(_, p)| p.transfers_through_return)
                .map(|(i, _)| i)
                .collect();
            tracing::debug!(
                target: "ori_arc::aims::interprocedural",
                caller = ?func.name,
                callee_bare = ?bare,
                recv_idx = ?key.0,
                transfers_through_return_params = ?ttr,
                "impl-method contract bound for caller",
            );
        }
        augmented.insert(*bare, contract.clone());
    }
    Some(augmented)
}
