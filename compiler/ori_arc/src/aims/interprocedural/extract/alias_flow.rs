//! Alias-to-parameter flow: return-alias shapes, the alias map, and
//! callee-mediated consumption/containment detection.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::super::super::contract::{MemoryContract, ReturnAliasShape};
use super::super::super::lattice::AccessClass;

use super::build_definition_map;

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
pub(super) fn find_return_alias_shapes(
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

/// Capture-variant-deadness Project record (RL-2 closure-extract borrow-view):
/// for each param `i` that is a SUM whose discriminant is `Switch`ed, when EXACTLY
/// ONE Switch case arm returns `Project param.field` (a borrow-view of that
/// variant's payload) and EVERY OTHER reachable arm returns a fresh / non-aliasing
/// value, record `i → (variant_tag, field)`. This is the per-variant refinement
/// of `find_return_alias_shapes`: the whole-param `return_alias` POISONS to `None`
/// because the fresh-return arms disagree with the Project arm, but the Project
/// holds on the matching arm alone — admissible at a caller that proves the
/// captured value is `variant_tag` (the non-matching arms are then DEAD).
///
/// Detection (all hold per recorded param):
///  - the Switch scrutinee is `Project param.0` (the sum discriminant of `param`).
///  - EXACTLY ONE case `(tag, block)` Jumps the merge a value tracing to
///    `Project param.field` (the matching-variant payload borrow-view).
///  - every OTHER case + the default arm Jumps a value that does NOT trace to a
///    `Project` of ANY param (a fresh / literal / non-aliasing return — the poison
///    leaf the whole-param join chokes on). An arm that projects a DIFFERENT param
///    or a different field declines the whole record (ambiguous web).
///  - the matching case's payload field is NOT field 0 (field 0 is the
///    discriminant tag, never an owned payload borrow-view).
fn find_capture_variant_return_projections(
    func: &ArcFunction,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
    all_projects: &FxHashMap<ArcVarId, (ArcVarId, u32)>,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> FxHashMap<usize, (u64, u32)> {
    let mut out: FxHashMap<usize, (u64, u32)> = FxHashMap::default();
    // The Return value of the whole function — the per-arm leaves all flow here.
    let Some(return_value) = func.blocks.iter().find_map(|b| match &b.terminator {
        ArcTerminator::Return { value } => Some(*value),
        _ => None,
    }) else {
        return out;
    };
    // Per-case-block Jump-arg into the merge param: the value this arm contributes.
    // The merge block-param's alias_sources are the per-arm Jump args; we instead
    // trace each case block's OWN Jump terminator to the value it forwards.
    for block in &func.blocks {
        let ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        } = &block.terminator
        else {
            continue;
        };
        // The scrutinee must be `Project param.0` (the sum discriminant).
        let Some(&(disc_src, disc_field)) = all_projects.get(scrutinee) else {
            continue;
        };
        if disc_field != 0 {
            continue;
        }
        let Some(param_set) = alias_to_param.get(&disc_src) else {
            continue;
        };
        let Some(&param_idx) = param_set.iter().next().filter(|_| param_set.len() == 1) else {
            continue;
        };

        // Classify each arm's contributed leaf: a Project of THIS param's field,
        // or a fresh / non-aliasing leaf. A Project of a different param / field
        // (or a Direct alias of any param) declines the whole record.
        let mut project_arm: Option<(u64, u32)> = None;
        let mut declined = false;
        let mut classify_arm = |arm_block: crate::ir::ArcBlockId, tag: Option<u64>| {
            if declined {
                return;
            }
            // The value this arm forwards to the merge: the arm block's Jump arg
            // that flows (via the alias chain) to the function Return value. No
            // resolvable single leaf — conservatively a non-Project (fresh) arm.
            let Some(leaf) = arm_leaf_for_return(func, arm_block, return_value, alias_sources)
            else {
                return;
            };
            // A Project of THIS param's non-discriminant field is the matching
            // borrow-view arm; a Project of a different param / field 0 declines;
            // a non-Project leaf is a permitted fresh-return non-matching arm.
            match all_projects.get(&leaf) {
                Some(&(proj_src, field))
                    if field != 0
                        && alias_to_param
                            .get(&proj_src)
                            .is_some_and(|s| s.len() == 1 && s.contains(&param_idx)) =>
                {
                    match (tag, project_arm) {
                        // The matching-variant project arm (tag-carrying case).
                        (Some(t), None) => project_arm = Some((t, field)),
                        // A second project arm (or a project on the default) —
                        // ambiguous; decline.
                        _ => declined = true,
                    }
                }
                // A Project of a DIFFERENT param / field 0 — ambiguous web.
                Some(_) => declined = true,
                // A non-Project leaf (fresh / literal return) — the poison the
                // whole-param join chokes on; permitted, no record change.
                None => {}
            }
        };
        for &(tag, arm_block) in cases {
            classify_arm(arm_block, Some(tag));
        }
        classify_arm(*default, None);

        if declined {
            continue;
        }
        if let Some((tag, field)) = project_arm {
            // Equal record idempotent; a disagreeing second Switch on the same
            // param poisons the record (matches the contract join).
            match out.get(&param_idx).copied() {
                None => {
                    out.insert(param_idx, (tag, field));
                }
                Some(existing) if existing == (tag, field) => {}
                Some(_) => {
                    out.remove(&param_idx);
                }
            }
        }
    }
    out
}

/// Trace the value an arm block contributes to the function Return: the arm's
/// own `Jump` arg whose alias chain reaches `return_value`. Returns the leaf the
/// arm forwards (a `Project`-def var or a fresh def var); `None` when the arm has
/// no single resolvable contributing value.
fn arm_leaf_for_return(
    func: &ArcFunction,
    arm_block: crate::ir::ArcBlockId,
    return_value: ArcVarId,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
) -> Option<ArcVarId> {
    let block = func.blocks.get(arm_block.index())?;
    let ArcTerminator::Jump { args, .. } = &block.terminator else {
        return None;
    };
    // The Jump arg whose alias chain reaches the function Return value is the
    // leaf this arm contributes. Trace each arg forward through alias_sources
    // (Jump-arg → block-param edges + Let-Var aliases recorded there).
    for &arg in args {
        if arg == return_value || alias_reaches(arg, return_value, alias_sources) {
            // The arg's OWN leaf: descend its alias chain to the defining value.
            return Some(resolve_alias_leaf(arg, alias_sources));
        }
    }
    // Single-arg merge: the only contributed value (its chain forms the return).
    if args.len() == 1 {
        return Some(resolve_alias_leaf(args[0], alias_sources));
    }
    None
}

/// Does `from`'s alias chain reach `target` (forward through the recorded
/// `alias_sources` edges, which are stored target→sources)? We walk the reverse:
/// is `from` a source-leaf of `target`'s chain?
fn alias_reaches(
    from: ArcVarId,
    target: ArcVarId,
    alias_sources: &FxHashMap<ArcVarId, FxHashSet<ArcVarId>>,
) -> bool {
    let mut visited: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut stack = vec![target];
    while let Some(v) = stack.pop() {
        if v == from {
            return true;
        }
        if !visited.insert(v) {
            continue;
        }
        if let Some(srcs) = alias_sources.get(&v) {
            stack.extend(srcs.iter().copied());
        }
    }
    false
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

/// Public entry: compute the per-param capture-variant Project records.
pub(super) fn find_capture_variant_return_projections_entry(
    func: &ArcFunction,
    alias_to_param: &FxHashMap<ArcVarId, FxHashSet<usize>>,
) -> FxHashMap<usize, (u64, u32)> {
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
    find_capture_variant_return_projections(func, &alias_sources, &all_projects, alias_to_param)
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
