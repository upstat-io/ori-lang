//! Return-alias shape detection: [`find_return_alias_shapes`] and its
//! private resolution helpers.
//!
//! Split out of [`super`] to keep the alias-flow module under the 500-line
//! hygiene cap.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::super::super::super::contract::{MemoryContract, ReturnAliasShape};

/// Compute `ReturnAliasShape` for every param that flows to a Return
/// terminator, either Direct (param IS the returned value via Let / Select /
/// Jump-arg / Apply-transfers_through_return aliasing) or Project (Return
/// value is the dst of `Project { value: src, field }` where `src` aliases
/// a param).
///
/// Multi-path join via `ReturnAliasShape::join`: Direct is TOP and absorbs
/// Project; incomparable Project paths (different field indices) join to
/// Direct. See `ReturnAliasShape::join` for the lattice chain.
///
/// Used by `detect_param_facts` to produce the per-param shape map that
/// `extract_contract` writes into `ParamContract::return_alias`. The shape
/// is then consumed by the caller-side `apply_result_aliases` population at
/// the Apply site.
pub(in super::super) fn find_return_alias_shapes(
    func: &ArcFunction,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
    sigs: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<usize, ReturnAliasShape> {
    // Collect Return-value vars first (most blocks have no Return); skip
    // the Project-def scan entirely when there are no Returns.
    let return_values: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .filter_map(|b| match &b.terminator {
            ArcTerminator::Return { value } => Some(*value),
            _ => None,
        })
        .collect();
    if return_values.is_empty() {
        return FxHashMap::default();
    }
    // Build a small map only for Project instructions whose dst is returned.
    // Avoids walking every Project — most projections do not flow to Return.
    let project_returns: FxHashMap<ArcVarId, (ArcVarId, u32)> = func
        .blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter_map(|instr| match instr {
            ArcInstr::Project {
                dst, value, field, ..
            } if return_values.contains(dst) => Some((*dst, (*value, *field))),
            _ => None,
        })
        .collect();

    // Every `Project` dst (not only directly-returned ones) keyed to its
    // `(source, field)`. Used by `resolve_indirect_project_return` to recognize a
    // Project reached through a Let-Var / Jump-arg alias chain (the match-extract
    // -through-block-param return shape), not only a direct `Return (Project dst)`.
    let all_projects: FxHashMap<ArcVarId, (ArcVarId, u32)> = func
        .blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter_map(|instr| match instr {
            ArcInstr::Project {
                dst, value, field, ..
            } => Some((*dst, (*value, *field))),
            _ => None,
        })
        .collect();
    let alias_sources = build_return_alias_source_map(func);
    // Every `Apply` / `Invoke` call-result dst keyed to its `(callee, args)`.
    // Used by the forwarder-Project handling in this function: a Return value
    // forwarding a callee's `Project` return-alias result inherits the same
    // `Project { field }`.
    let call_results = build_call_result_map(func);
    // Every struct `Construct` dst keyed to its field args. Used by the
    // construct-project round-trip branch: `Project (Construct args) field`
    // resolves to `args[field]` (the field IS the arg allocation, TF-3 + TF-4).
    let struct_constructs = build_struct_construct_map(func);

    let mut shapes: FxHashMap<usize, ReturnAliasShape> = FxHashMap::default();
    for block in &func.blocks {
        let ArcTerminator::Return { value } = &block.terminator else {
            continue;
        };
        // Direct: the Return value's alias-chain reaches a param.
        if let Some(param_indices) = alias_to_param.get(value) {
            for &idx in param_indices {
                join_shape_into(&mut shapes, idx, ReturnAliasShape::Direct);
            }
        }
        // Project: Return value is the dst of `Project { value: src, field }`
        // where `src` (or `src`'s alias chain) reaches a param.
        if let Some(&(proj_src, field)) = project_returns.get(value) {
            if let Some(param_indices) = alias_to_param.get(&proj_src) {
                for &idx in param_indices {
                    join_shape_into(&mut shapes, idx, ReturnAliasShape::Project { field });
                }
            }
        }
        // Indirect Project: the Return value is a block-param fed (via Jump-arg
        // and Let-Var alias chain) by a `Project src.field` whose `src` aliases a
        // param — the match-Switch-extract-to-block-param return shape. Resolved
        // ONLY when EVERY alias-chain source is the SAME `(param, field)` Project;
        // any source that is a fresh / non-aliasing value poisons to no shape so a
        // caller never suppresses a release on a path that returns a fresh value.
        if !project_returns.contains_key(value) {
            if let Some((idx, field)) = resolve_indirect_project_return(
                func,
                *value,
                &alias_sources,
                &all_projects,
                alias_to_param,
            ) {
                join_shape_into(&mut shapes, idx, ReturnAliasShape::Project { field });
            }
        }
        // Forwarder Project: the Return value's alias-chain leaf is the result
        // of an `Apply` / `Invoke @callee(args)` whose callee returns
        // `Project { field }` of its param `args[i]`. The forwarder returns that
        // borrow-view UNCHANGED, so THIS function's param indices that `args[i]`
        // aliases inherit `Project { field }` (forwarder-transitivity of the
        // same-allocation-identity relation — proven net-0 single-release in
        // scratch `ForwardedProjectReturn.forwarded_joint_release_exactly_once`,
        // governing rules RL-2 `RL2_release_exactly_once` + TF-4 borrow-view).
        let leaf = resolve_alias_leaf(*value, &alias_sources);
        if let Some((callee, args)) = call_results.get(&leaf) {
            if let Some(callee_contract) = sigs.get(callee) {
                for (arg_pos, &arg) in args.iter().enumerate() {
                    let Some(ReturnAliasShape::Project { field }) = callee_contract
                        .params
                        .get(arg_pos)
                        .and_then(|p| p.return_alias)
                    else {
                        continue;
                    };
                    if let Some(param_indices) = alias_to_param.get(&arg) {
                        for &idx in param_indices {
                            join_shape_into(&mut shapes, idx, ReturnAliasShape::Project { field });
                        }
                    }
                }
            }
        }
        // Construct-project round-trip Direct: the Return value is a chain of
        // `Project src.field` hops that resolve through struct `Construct`s back
        // to a param. `Project (Construct args) field == args[field]` (the field
        // IS the arg allocation — no copy, TF-3 Construct + TF-4 Project
        // borrow-view), so a param inc'd into a struct and projected back out
        // flows out UNCHANGED — the return ALIASES the param (Direct). The
        // round-trip is the identity on the field allocation at any nesting depth
        // (proven `ConstructProjectRoundtrip.nest_roundtrip_is_identity` +
        // `cure_restores_balance`; governing TF-3 + TF-4 + RL-2
        // `RL2_release_exactly_once`). Recording Direct defers the caller's
        // premature param drop past the returned value's last use. Resolved ONLY
        // when EVERY hop is a struct-construct-project round-trip terminating in
        // ONE param index (a fresh / non-aliasing leaf records no shape, so a
        // caller never suppresses a release on a genuinely-fresh return path).
        if let Some(idx) = resolve_construct_project_roundtrip(
            *value,
            &alias_sources,
            &all_projects,
            &struct_constructs,
            alias_to_param,
        ) {
            join_shape_into(&mut shapes, idx, ReturnAliasShape::Direct);
        }
    }
    shapes
}

/// Per-var backward alias sources for return-value tracing: the immediate
/// vars a value can be equal to via `Let { Var(src) }` aliases and
/// `Jump`-arg → block-param edges. Distinct from `build_alias_to_param_map`
/// (which folds param indices); this keeps raw `ArcVarId` sources so the
/// indirect-Project trace can reach a `Project` definition through the chain.
fn build_return_alias_source_map(func: &ArcFunction) -> FxHashMap<ArcVarId, FxHashSet<ArcVarId>> {
    let mut sources: FxHashMap<ArcVarId, FxHashSet<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::Var(src),
                ..
            } = instr
            {
                sources.entry(*dst).or_default().insert(*src);
            }
        }
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_params = &func.blocks[target.index()].params;
            for (arg, &(param_var, _)) in args.iter().zip(target_params.iter()) {
                sources.entry(param_var).or_default().insert(*arg);
            }
        }
    }
    sources
}

/// Build the `Apply` / `Invoke` call-result map: each call result `dst` keyed
/// to its `(callee, args)`. Used by the forwarder-Project branch in
/// `find_return_alias_shapes` to recognize a Return value that forwards a
/// callee's `Project` return-alias result. `Invoke` defines `dst` on its normal
/// edge — included alongside `Apply`.
fn build_call_result_map(func: &ArcFunction) -> FxHashMap<ArcVarId, (Name, Vec<ArcVarId>)> {
    let mut out: FxHashMap<ArcVarId, (Name, Vec<ArcVarId>)> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                out.insert(*dst, (*callee, args.clone()));
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            out.insert(*dst, (*callee, args.clone()));
        }
    }
    out
}

/// Build the struct/tuple `Construct` map: each dst keyed to its field args.
/// Only `CtorKind::Struct` and `CtorKind::Tuple` are positional aggregates where
/// `Project (Construct args) i == args[i]` holds unconditionally (no tag, no
/// copy). `EnumVariant` is EXCLUDED — its projection is variant-conditional, so
/// the round-trip identity does not hold blindly.
fn build_struct_construct_map(func: &ArcFunction) -> FxHashMap<ArcVarId, Vec<ArcVarId>> {
    let mut out: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            {
                if matches!(
                    ctor,
                    crate::ir::CtorKind::Struct(_) | crate::ir::CtorKind::Tuple
                ) {
                    out.insert(*dst, args.clone());
                }
            }
        }
    }
    out
}

/// Resolve a Return value to a single param index when it is a chain of
/// `Project src.field` hops that each resolve through a struct/tuple `Construct`
/// back to ONE param — the construct-project round-trip Direct alias.
///
/// At each hop: resolve `value`'s alias-leaf; if it aliases a param, the chain
/// terminates Direct on that param. Otherwise, if the leaf is a `Project
/// src.field` whose `src`'s alias-leaf is a struct/tuple `Construct`, step to
/// that construct's `args[field]` and continue. Any other leaf (a fresh
/// non-construct value, a non-aliasing `Project` source, a multi-param ambiguity)
/// resolves to `None` — so a caller never suppresses a release on a genuinely
/// fresh return path. Bounded by a visited set (the IR is acyclic in this chain).
fn resolve_construct_project_roundtrip(
    return_value: ArcVarId,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
    all_projects: &FxHashMap<ArcVarId, (ArcVarId, u32)>,
    struct_constructs: &FxHashMap<ArcVarId, Vec<ArcVarId>>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> Option<usize> {
    let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
    let resolved = resolve_value(
        return_value,
        alias_sources,
        all_projects,
        struct_constructs,
        &mut visited,
    )?;
    // Terminates Direct only when the round-trip-resolved value aliases exactly
    // ONE param (the whole param flows out unchanged through the construct-
    // project chain). Multi-param ambiguity or a non-param value declines.
    let param_indices = alias_to_param.get(&resolved)?;
    if param_indices.len() == 1 {
        return param_indices.iter().next().copied();
    }
    None
}

/// Recursively fold a var to the value it equals under the construct-project
/// round-trip identity `Project (Construct args) field == value_of(args[field])`.
/// Resolves `Let`/`Jump` aliases (via `resolve_alias_leaf`), then: if the leaf is
/// `Project src.field` whose `src`'s value is a struct/tuple `Construct`, recurse
/// into `value_of(args[field])`; otherwise the leaf IS the value (a param, a
/// fresh construct, an opaque call result). Bounded by a visited set.
fn resolve_value(
    var: ArcVarId,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
    all_projects: &FxHashMap<ArcVarId, (ArcVarId, u32)>,
    struct_constructs: &FxHashMap<ArcVarId, Vec<ArcVarId>>,
    visited: &mut FxHashSet<ArcVarId>,
) -> Option<ArcVarId> {
    let leaf = resolve_alias_leaf(var, alias_sources);
    if !visited.insert(leaf) {
        return None; // cycle guard
    }
    // `Project src.field`: resolve `src`'s value; if it is a struct/tuple
    // Construct, the projection IS that construct's `args[field]` value.
    if let Some(&(proj_src, field)) = all_projects.get(&leaf) {
        let src_val = resolve_value(
            proj_src,
            alias_sources,
            all_projects,
            struct_constructs,
            visited,
        )?;
        if let Some(args) = struct_constructs.get(&src_val) {
            let arg = *args.get(field as usize)?;
            return resolve_value(arg, alias_sources, all_projects, struct_constructs, visited);
        }
        // A Project whose source is NOT a construct (an opaque borrow-view of a
        // param / call result) — the value is the leaf itself (not a round-trip).
        return Some(leaf);
    }
    // Not a Project — the leaf is the value (param, construct dst, call result).
    Some(leaf)
}

/// Resolve a Return value to a single `(param_index, field)` Project alias when
/// EVERY alias-chain leaf is the SAME `Project src.field` with `src` aliasing
/// ONE param index. Returns `None` (poison) the moment ANY leaf is a non-Project
/// value, a Project of a different `(src-param, field)`, or a Project whose
/// source does not alias a param — so the resulting `Project` contract holds on
/// EVERY return path (all-paths soundness; a fresh-value path never resolves).
fn resolve_indirect_project_return(
    func: &ArcFunction,
    return_value: ArcVarId,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
    all_projects: &FxHashMap<ArcVarId, (ArcVarId, u32)>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> Option<(usize, u32)> {
    // Collect the alias-chain leaves: vars with no further alias source. Each
    // leaf is the actual value returned on some path.
    let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut stack: Vec<ArcVarId> = vec![return_value];
    let mut resolved: Option<(usize, u32)> = None;
    while let Some(var) = stack.pop() {
        if !visited.insert(var) {
            continue;
        }
        match alias_sources.get(&var) {
            Some(srcs) if !srcs.is_empty() => {
                // Interior alias node — descend to its sources.
                for &s in srcs {
                    stack.push(s);
                }
            }
            _ => {
                // Leaf: the value returned on this path. A provably-`Scalar`-repr
                // leaf carries no RC and owns no allocation (an unreachable
                // panic-arm unit placeholder threaded into the merge block-param);
                // it cannot conflict with a `Project` borrow-view treatment, so it
                // is SKIPPED (not poison). Every OTHER leaf MUST be a `Project` of
                // ONE param's SAME field; anything else (a fresh / non-aliasing
                // RC-carrying value — the genuine fresh-return path) poisons.
                if matches!(func.var_repr(var), Some(crate::ir::ValueRepr::Scalar)) {
                    continue;
                }
                let (proj_src, field) = *all_projects.get(&var)?;
                let param_indices = alias_to_param.get(&proj_src)?;
                if param_indices.len() != 1 {
                    return None;
                }
                let idx = *param_indices.iter().next()?;
                match resolved {
                    None => resolved = Some((idx, field)),
                    Some(prev) if prev == (idx, field) => {}
                    Some(_) => return None,
                }
            }
        }
    }
    resolved
}

/// Descend `var`'s alias chain to its deepest single source leaf. Stops at a var
/// with no further alias source (the defining `Project` / fresh def).
fn resolve_alias_leaf(
    var: ArcVarId,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
) -> ArcVarId {
    let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut cur = var;
    loop {
        if !visited.insert(cur) {
            return cur;
        }
        // A single alias source — descend; zero or multiple sources is the leaf.
        match alias_sources.get(&cur).and_then(|srcs| {
            (srcs.len() == 1)
                .then(|| srcs.iter().next().copied())
                .flatten()
        }) {
            Some(next) => cur = next,
            None => return cur,
        }
    }
}

/// Multi-path join helper: insert `new` for `idx`, joining with any prior
/// shape per `ReturnAliasShape::join` semantics.
fn join_shape_into(
    shapes: &mut FxHashMap<usize, ReturnAliasShape>,
    idx: usize,
    new: ReturnAliasShape,
) {
    let prev = shapes.get(&idx).copied();
    if let Some(joined) = ReturnAliasShape::join(prev, Some(new)) {
        shapes.insert(idx, joined);
    }
}
