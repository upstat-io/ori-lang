//! Closure-extract borrow-view release scan (RL-2 + RL-4): the interprocedural
//! sibling of [`compute_multi_exit_borrow_view_lineage`]. `ApplyIndirect` results
//! that are PROVEN same-allocation borrow-views of ONE captured field of a closure
//! env (the lambda's `return_alias = Project { field }` contract over its capture
//! param) name ONE allocation. The base walk decs EACH result borrow-view, so a
//! closure invoked N times double-frees the captured payload; this scan suppresses
//! the surplus result decs and places EXACTLY ONE release per terminal path.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::{MemoryContract, ReturnAliasShape};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, ValueRepr};

/// Result of [`compute_closure_extract_borrow_view_lineage`]: the same-alloc
/// borrow-view result lineage to suppress (every surplus per-result dec). The
/// canonical release is the closure env's OWN scope-exit dec, whose drop cascade
/// frees the captured field — so this scan PLACES NO release, it only suppresses.
pub(in crate::lower::burden_lower) struct ClosureExtractBorrowViewLineage {
    /// Every var in an admitted borrow-view result lineage — removed from
    /// `owned_vars_needing_rc` so the N surplus per-result decs vanish. The
    /// captured field's single release is the closure env's drop cascade.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
}

/// One admitted web's lineage facts, held until the pairwise-disjoint gate runs
/// over the full candidate set. A web = the set of `ApplyIndirect` results (and
/// their aliases) that project the SAME captured field of ONE closure capture.
struct Web {
    members: FxHashSet<ArcVarId>,
}

/// RL-2 + RL-4 treatment for the interprocedural closure-extract borrow-view
/// shape (`let f = () -> { match captured { Err(e) -> e, .. } }; let a = f();
/// let b = f(); a.contains(..); b.contains(..)`).
///
/// A closure captures `Result<int, str>::Err(str)` / `Option<str>::Some(str)`
/// and is `ApplyIndirect`-invoked N times; the lambda returns the captured str
/// via a match-Switch-extract-to-block-param Return, recorded by
/// `find_return_alias_shapes` as `return_alias = Project { field }` over the
/// capture param. Each `ApplyIndirect` result is therefore a TF-4 borrow-view of
/// the SAME captured-str allocation (canonical owner = the closure env). Per the
/// joint-release theorem EXACTLY one release should survive across all N results;
/// the base walk decs each result (the per-result `burden_dec`) — N decs on one
/// allocation, double-free on call 2 (exit 134).
///
/// The captured field's CANONICAL owner is the closure env — the env captured
/// the payload by value (`PartialApply @lambda(%cap)` where `%cap` holds the
/// payload), so the env's OWN scope-exit dec drops the env, and the env's drop
/// CASCADE-FREES the captured payload exactly once. The N `ApplyIndirect` result
/// borrow-views therefore owe NO release of their own — every per-result dec is
/// surplus. The cure SUPPRESSES the whole result lineage (removes it from
/// `owned_vars_needing_rc`, killing the N surplus decs) and PLACES NO release;
/// the closure env's drop is the single release (`RL2_release_exactly_once`, the
/// joint-release theorem with the closure env as canonical owner).
///
/// Admission gates (ALL hold per web; ANY failure declines — the status-quo
/// double-free is the migration floor, never a regression introduced here):
///  (a) PROVEN same-allocation identity: the `ApplyIndirect`'s resolved lambda
///      contract has a capture param with `return_alias = Project { field }` (a
///      contract edge, NOT a use-count / type-membership proxy — dead-end #150).
///  (b) the closure ENV (the `PartialApply` root capturing the payload) is in
///      `owned_vars_needing_rc` — it gets its standard scope-exit dec whose drop
///      cascade frees the captured payload. An env that is NOT released (escaped
///      / stored) would leave the payload un-freed if the result decs are
///      suppressed, so the web declines.
///  (c) every result root in `owned_vars_needing_rc` (heap-carrying, RC-tracked)
///      AND `FatValue` repr (str borrow-view two-word value).
///  (d) BORROW-ONLY lineage ([`borrow_view_result_vetted`]): grow over
///      `Let { Var }` aliases; EVERY use of every member is an alias-edge or a
///      BORROWED arg (`@contains` / any borrowed call/invoke arg). Any owned-
///      position consume / store / capture / `Project` / `Select` / `Return` /
///      `Jump`/`Branch`/`Switch` terminator use / owned call arg declines (the
///      fresh-sum live-extract over-fire boundary, dead-ends #174/#175): an
///      owned-position escape means the result holds its OWN ref and suppressing
///      its dec leaks / double-frees.
///  (e) at least one borrow-read exists.
///  (f) pairwise-DISJOINT webs: two webs sharing any member converge on one
///      allocation. ALL members of an overlapping group decline (status quo).
pub(in crate::lower::burden_lower) fn compute_closure_extract_borrow_view_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> ClosureExtractBorrowViewLineage {
    let mut out = ClosureExtractBorrowViewLineage {
        suppressed_lineage_vars: FxHashSet::default(),
    };

    // Gate (a): collect closure-extract borrow-view result roots, grouped by the
    // `(closure-env-root, field)` they project — one web per captured field.
    let webs_by_key = collect_borrow_view_result_roots(func, contracts);
    if webs_by_key.is_empty() {
        return out;
    }

    let mut webs: Vec<Web> = Vec::new();
    let mut claimed: FxHashSet<ArcVarId> = FxHashSet::default();
    for ((env_root, _capture, _field), roots) in webs_by_key {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                gate,
                "closure-extract borrow-view web declined"
            );
        };
        // Gate (b): the closure ENV is released — its scope-exit dec drops the
        // env and cascade-frees the captured payload (the single release). An
        // env not in `owned_vars_needing_rc` (escaped / stored) is never
        // dropped, so suppressing the result decs would LEAK — decline.
        if !owned_vars_needing_rc.contains(&env_root) {
            decline("b:env-not-released");
            continue;
        }
        // Gate (c): every result root heap-carrying + FatValue + not claimed.
        if roots.iter().any(|r| {
            !owned_vars_needing_rc.contains(r)
                || claimed.contains(r)
                || !matches!(func.var_repr(*r), Some(ValueRepr::FatValue))
        }) {
            decline("c:owned/claimed/repr");
            continue;
        }
        // Gate (d): grow + vet the borrow-only result lineage across all roots.
        let Some(members) = borrow_view_result_vetted(func, &roots) else {
            decline("d:lineage-vet");
            continue;
        };
        // Gate (e): at least one borrow-read.
        if !members_have_borrow_read(func, &members) {
            decline("e:no-borrow-read");
            continue;
        }
        claimed.extend(members.iter().copied());
        webs.push(Web { members });
    }

    // Gate (f): decline EVERY web whose lineage overlaps another's.
    let overlapping: Vec<bool> = webs
        .iter()
        .map(|w| {
            webs.iter()
                .filter(|o| !std::ptr::eq(*o, w))
                .any(|o| !w.members.is_disjoint(&o.members))
        })
        .collect();
    for (web, overlaps) in webs.into_iter().zip(overlapping) {
        if overlaps {
            continue;
        }
        out.suppressed_lineage_vars
            .extend(web.members.iter().copied());
    }
    out
}

/// Gate (a): `ApplyIndirect` result dsts whose resolved lambda contract proves a
/// capture-param `return_alias = Project { field }` — grouped by the
/// `(closure-env-var, capture-var, field)` web they share. All results projecting
/// the SAME captured field of the SAME closure env name ONE allocation. The
/// closure-env-var is the `PartialApply` def var the `ApplyIndirect`'s closure
/// operand resolves to (its scope-exit dec is the canonical release).
fn collect_borrow_view_result_roots(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<(ArcVarId, ArcVarId, u32), Vec<ArcVarId>> {
    let closure_def_map = crate::rc_insert::closure_resolve::build_closure_def_map(&func.blocks);
    let construct_variant = build_construct_variant_map(func);
    let mut webs: FxHashMap<(ArcVarId, ArcVarId, u32), Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::ApplyIndirect { dst, closure, .. } = instr else {
                continue;
            };
            let Some((target, capture_args)) =
                crate::rc_insert::closure_resolve::resolve_to_partial_apply_idx_eq(
                    *closure,
                    &closure_def_map,
                    &func.var_types,
                )
            else {
                continue;
            };
            // The closure env's own RC slot — the `PartialApply` def var the
            // `closure` operand aliases (trace `Let { Var }` aliases). Its
            // scope-exit dec drops the env + cascade-frees the captured payload.
            let Some(env_root) = resolve_partial_apply_def_var(*closure, &closure_def_map) else {
                continue;
            };
            let Some(contract) = contracts.get(&target) else {
                continue;
            };
            // The capture params are the contract's LEADING params (capture-first
            // ordering, matching `populate_apply_result_aliases`). A capture param
            // resolves to a `(param_idx, field)` Project of its captured payload by
            // EITHER:
            //  (1) `return_alias = Project { field }` — every return path projects
            //      the SAME field (the whole-param shape), OR
            //  (2) `capture_variant_return_project = Some((tag, field))` AND the
            //      capture arg is a single-variant `Construct Variant(T.tag)(..)` —
            //      the per-variant refinement: the non-matching arms are DEAD here,
            //      so the live return resolves to `Project { field }` (the cell's
            //      `Option<str>::Some` capture-and-extract shape). The whole-param
            //      `return_alias` POISONS on the fresh-return non-matching arm, so
            //      this caller-site variant-deadness proof is the sole admission.
            // Require EXACTLY ONE projecting capture param so the web key is
            // unambiguous (a multi-capture-project lambda is a deferred shape).
            let projecting: Vec<(usize, u32)> = contract
                .params
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if let Some(ReturnAliasShape::Project { field }) = p.return_alias {
                        return Some((i, field));
                    }
                    // Per-variant refinement: the capture arg must be a single-
                    // variant Construct of the recorded variant tag.
                    let (tag, field) = p.capture_variant_return_project?;
                    let &capture = capture_args.get(i)?;
                    let variant = construct_variant.get(&capture)?;
                    (u64::from(*variant) == tag).then_some((i, field))
                })
                .collect();
            if projecting.len() != 1 {
                continue;
            }
            let (param_idx, field) = projecting[0];
            let Some(&capture) = capture_args.get(param_idx) else {
                continue;
            };
            webs.entry((env_root, capture, field))
                .or_default()
                .push(*dst);
        }
    }
    webs
}

/// Map each var defined by `Construct EnumVariant { variant }` to its variant
/// index — the caller-site discriminant proof the capture-variant-deadness
/// refinement consumes (gate (2) of [`collect_borrow_view_result_roots`]).
fn build_construct_variant_map(func: &ArcFunction) -> FxHashMap<ArcVarId, u32> {
    let mut out: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct {
                dst,
                ctor: CtorKind::EnumVariant { variant, .. },
                ..
            } = instr
            {
                out.insert(*dst, *variant);
            }
        }
    }
    out
}

/// Trace a closure operand var through `Let { Var }` aliases to the var whose
/// definition is the `PartialApply` (the closure env's own RC slot). `None` when
/// the closure is a block-param merge / non-`PartialApply` def (a closure whose
/// env identity is not a single resolvable var declines the web).
fn resolve_partial_apply_def_var(
    closure: ArcVarId,
    def_map: &FxHashMap<ArcVarId, crate::rc_insert::closure_resolve::ResolvedDef>,
) -> Option<ArcVarId> {
    use crate::rc_insert::closure_resolve::ResolvedDef;
    let mut var = closure;
    let mut guard = 0usize;
    loop {
        guard += 1;
        if guard > func_block_alias_chain_cap() {
            return None;
        }
        match def_map.get(&var)? {
            ResolvedDef::PartialApply { .. } => return Some(var),
            ResolvedDef::Alias(src) => var = *src,
            ResolvedDef::BlockParam { .. } | ResolvedDef::Other => return None,
        }
    }
}

/// Conservative alias-chain depth cap for [`resolve_partial_apply_def_var`] — a
/// chain longer than this declines (defends against a cyclic / pathological
/// `Let`-alias chain without a full visited-set).
const fn func_block_alias_chain_cap() -> usize {
    1024
}

/// Gate (d): grow the borrow-view result lineage from the roots over `Let { Var }`
/// aliases, then vet that every member use is an alias-edge or a BORROWED arg.
/// `None` on any owned-position / escape use.
fn borrow_view_result_vetted(
    func: &ArcFunction,
    roots: &[ArcVarId],
) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = roots.iter().copied().collect();
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if members.contains(src) && members.insert(*dst) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    member_uses_all_borrow_views(func, &members).then_some(members)
}

/// Gate (c) vetting core: true iff EVERY use of every member is an alias-edge
/// (`Let { Var(src) }`, `src ∈ members`) or a BORROWED arg of an `Apply`/`Invoke`
/// (any arg position that is NOT an owned position). Any owned-position consume,
/// store (`Construct`/`Reuse`/`Set`), capture (`PartialApply`), `Project`,
/// `Select`, terminator operand (`Return`/`Jump`/`Branch`/`Switch`/`Invoke` arg
/// at an owned position), or `ApplyIndirect` arg declines.
fn member_uses_all_borrow_views(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches {
                continue;
            }
            match instr {
                // Alias-edge: the lineage's own internal hop.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // Borrowed call: every member arg MUST be at a BORROWED position.
                ArcInstr::Apply { args, .. } => {
                    if args
                        .iter()
                        .enumerate()
                        .any(|(pos, a)| members.contains(a) && instr.is_owned_position(pos))
                    {
                        return false;
                    }
                }
                // Any other instruction touching a member is an owned consume /
                // store / capture / projection / indirect-call arg — decline.
                _ => return false,
            }
        }
        // A member at the borrowed args of an `Invoke` terminator is a borrow-read
        // (the `@contains` shape lowers to `Invoke` on the unwind path); any other
        // terminator position (owned `Invoke` arg, `Return`/`Jump`/`Branch`/
        // `Switch` operand, `InvokeIndirect`) is an escape — decline.
        match &block.terminator {
            ArcTerminator::Invoke { args, .. } => {
                let term = &block.terminator;
                if args
                    .iter()
                    .enumerate()
                    .any(|(pos, a)| members.contains(a) && term.is_owned_position(pos))
                {
                    return false;
                }
            }
            other => {
                if other.used_vars().iter().any(|v| members.contains(v)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Gate (d): true iff some member is read at a borrowed `Apply`/`Invoke` arg.
fn members_have_borrow_read(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    func.blocks.iter().any(|b| {
        b.body.iter().any(|instr| match instr {
            ArcInstr::Apply { args, .. } => args
                .iter()
                .enumerate()
                .any(|(pos, a)| members.contains(a) && !instr.is_owned_position(pos)),
            _ => false,
        }) || match &b.terminator {
            ArcTerminator::Invoke { args, .. } => args
                .iter()
                .enumerate()
                .any(|(pos, a)| members.contains(a) && !b.terminator.is_owned_position(pos)),
            _ => false,
        }
    })
}
