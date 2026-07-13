//! Alias-to-parameter flow: return-alias shapes, the alias map, and
//! callee-mediated consumption/containment detection.
//!
//! Return-alias shape detection lives in [`return_alias_shapes`] — split out
//! to keep this module under the 500-line hygiene cap.

mod return_alias_shapes;

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::super::super::contract::MemoryContract;
use super::super::super::lattice::AccessClass;

use super::build_definition_map;

pub(super) use return_alias_shapes::find_return_alias_shapes;

/// Build the multi-valued alias map: variable → set of parameter indices
/// it aliases. Covers Let{Var} aliases, Select conditional aliases,
/// Jump-arg → block-parameter passing, and (when `sigs` is provided)
/// Apply destinations whose callee transfers a parameter through its return.
/// Iterates to fixed point.
///
/// Public to the crate so the realization phase can reuse the same alias
/// resolution that interprocedural extraction relies on. Both phases need
/// to ask "which parameter indices does this variable alias?" — two
/// independent alias-tracing implementations would duplicate the algorithm.
///
/// `sigs` enables BUG-04-090 transitive `transfers_through_return`
/// propagation: when callee `g(x)` has `g.x.transfers_through_return = true`,
/// then `let r = g(arg)` makes `r` alias whatever params `arg` aliases.
/// This makes multi-hop forwarder chains (`wrap` calls `id`) transitively
/// mark the caller's params for return-transfer suppression. Pass `None`
/// from realization-side callers that only need the local alias structure.
pub(crate) fn build_alias_to_param_map(
    func: &ArcFunction,
    param_vars: &FxHashMap<ArcVarId, usize>,
    sigs: Option<&FxHashMap<Name, MemoryContract>>,
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
                changed |= absorb_instr_aliases(instr, &mut alias_to_param, sigs);
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_params = &func.blocks[target.index()].params;
                for (arg, &(param_var, _)) in args.iter().zip(target_params.iter()) {
                    changed |= absorb_alias(*arg, param_var, &mut alias_to_param);
                }
            }
            // Invoke is a terminator that defines `dst` on the normal edge.
            // Same transitive transfers_through_return propagation as Apply.
            if let ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } = &block.terminator
            {
                if let Some(sigs_map) = sigs {
                    changed |= absorb_callee_return_transfer(
                        *dst,
                        *callee,
                        args,
                        sigs_map,
                        &mut alias_to_param,
                    );
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
    sigs: Option<&FxHashMap<Name, MemoryContract>>,
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
        // BUG-04-090 transitivity: Apply { dst, callee, args } where the
        // callee's contract marks param i as `transfers_through_return`.
        // The callee returns args[i], so dst aliases whatever args[i]
        // aliases. SCC topological order guarantees the callee's contract
        // is already in `sigs` when we process the caller.
        ArcInstr::Apply {
            dst,
            func: callee,
            args,
            ..
        } => {
            if let Some(sigs_map) = sigs {
                absorb_callee_return_transfer(*dst, *callee, args, sigs_map, alias_to_param)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// BUG-04-090 transitivity helper: when `callee` has any param marked
/// `transfers_through_return`, propagate the corresponding arg's alias set
/// to `dst`. Used by both `absorb_instr_aliases` (for `Apply`) and the
/// terminator-loop in `build_alias_to_param_map` (for `Invoke`).
///
/// Multi-param case: if `callee` has both param 0 AND param 1 marked
/// `transfers_through_return`, the callee's return value may alias either
/// arg at runtime — `dst` is the union of both arg alias sets. Per
/// Select-style join.
fn absorb_callee_return_transfer(
    dst: ArcVarId,
    callee: Name,
    args: &[ArcVarId],
    sigs: &FxHashMap<Name, MemoryContract>,
    alias_to_param: &mut FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> bool {
    let Some(callee_contract) = sigs.get(&callee) else {
        return false;
    };
    let mut grew = false;
    for (i, &arg) in args.iter().enumerate() {
        let transfers = callee_contract
            .params
            .get(i)
            .is_some_and(|p| p.transfers_through_return);
        if !transfers {
            continue;
        }
        grew |= absorb_alias(arg, dst, alias_to_param);
    }
    grew
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
pub(super) fn find_consumed_via_callees(
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
/// Owned (the own-params-using-args borrow-inference rule) AND get
/// `transfers_through_return = true` for the BUG-04-090 fix — the gate
/// reads this STRUCTURAL fact (Return-trace only), kept distinct from
/// the Apply/Invoke consumption set.
pub(super) fn find_return_flow_params(
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

/// Detect parameters that flow into a transitive-drop variant payload that
/// is returned (path-c population).
///
/// Walks each `Return { value }` terminator, traces `value` to its defining
/// instruction, and when that instruction is `Construct { ctor, args,.. }`
/// or `PartialApply { args,.. }` whose result is a transitive-drop variant,
/// records every parameter whose alias appears in `args`.
///
/// Distinct from `find_return_flow_params` (Direct return — `Return { v }`
/// where `v` aliases a param) and from `find_return_alias_shapes` (which
/// records `Direct` / `Project` aliasing where the result IS an alias of
/// the param). This function captures the case where the result CONTAINS
/// the param as a constructed variant payload — e.g.,
/// `@wrap_ok (m: T) -> Result<T, E> = Ok(m)`. Here `m` is contained in
/// `Ok(m)`'s payload, but the result is NOT an alias of `m`; it's a fresh
/// allocation whose RC slot encloses `m`'s.
///
/// Used by `extract_contract` to populate
/// `ParamContract::return_payload_contains_param` (the `any` set), which the
/// burden-path transitive-drop alias machinery (`intraprocedural/apply_aliases.rs`
/// aliasing-params filter + `post_convergence.rs::materialize_transitive_drop_singleton_classes`)
/// consumes to admit the param's caller-side transitive-drop containment even
/// when its access is `Borrowed`.
pub(super) fn find_payload_containment_params(
    func: &ArcFunction,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> PayloadContainment {
    let return_values: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .filter_map(|b| match &b.terminator {
            ArcTerminator::Return { value } => Some(*value),
            _ => None,
        })
        .collect();
    if return_values.is_empty() {
        return PayloadContainment::default();
    }
    let def_map = build_definition_map(func);
    let mut containment: FxHashSet<usize> = FxHashSet::default();
    for &ret_var in &return_values {
        // Trace through Let { Var } aliases to find the defining
        // Construct/PartialApply (if any).
        let mut current = ret_var;
        let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
        loop {
            if !visited.insert(current) {
                break; // cycle guard
            }
            let Some(instr) = def_map.get(&current) else {
                break;
            };
            match instr {
                ArcInstr::Let {
                    value: crate::ir::ArcValue::Var(v),
                    ..
                } => {
                    current = *v;
                }
                ArcInstr::Construct {
                    dst, ctor, args, ..
                } => {
                    // Phase ordering: var_rc_strategies is not populated
                    // during interprocedural contract extraction (it's
                    // computed later in the per-function pipeline).
                    // We use CtorKind as a structural proxy: EnumVariant
                    // is the ctor that yields transitive-drop variant
                    // payloads. The consumer
                    // (`post_convergence.rs::materialize_transitive_drop_singleton_classes`)
                    // re-checks is_transitive_drop_strategy on the call dst at
                    // the caller's site, so being permissive here only
                    // populates the contract field; the consumer guards real
                    // class materialization.
                    let is_variant = matches!(ctor, crate::ir::CtorKind::EnumVariant { .. });
                    tracing::debug!(
                        func = ?func.name,
                        ret_var = ret_var.raw(),
                        dst = dst.raw(),
                        is_variant_ctor = is_variant,
                        "path-c: payload-containment Construct candidate"
                    );
                    if !is_variant {
                        break;
                    }
                    for arg in args {
                        if let Some(param_indices) = alias_to_param.get(arg) {
                            for &idx in param_indices {
                                containment.insert(idx);
                                tracing::debug!(
                                    func = ?func.name,
                                    arg = arg.raw(),
                                    param_idx = idx,
                                    "path-c: param flows into returned transitive-drop payload"
                                );
                            }
                        }
                    }
                    break;
                }
                ArcInstr::PartialApply { args, .. } => {
                    // PartialApply produces a closure environment, which
                    // is a transitive-drop container per CtorKind::Closure
                    // semantics in the realize-side strategy assignment.
                    for arg in args {
                        if let Some(param_indices) = alias_to_param.get(arg) {
                            for &idx in param_indices {
                                containment.insert(idx);
                            }
                        }
                    }
                    break;
                }
                _ => break,
            }
        }
    }
    PayloadContainment { any: containment }
}

/// Payload-containment facts per [`find_payload_containment_params`]:
/// `any` = contained on SOME return path (OR semantics — feeds
/// `return_payload_contains_param`).
#[derive(Default)]
pub(super) struct PayloadContainment {
    pub(super) any: FxHashSet<usize>,
}
