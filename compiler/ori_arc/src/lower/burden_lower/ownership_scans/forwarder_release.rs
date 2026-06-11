//! Forwarder-result under-release scan (RL-2): the scope-exit release for a
//! ttr forwarder RESULT whose lineage gets neither a FRESH inc nor any
//! scope-exit dec, plus the release-site selection helpers. Spec: Annex E
//! §AIMS RL-2 + RL-34.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::forwarder::{arg_owned_transfers_through_return, compute_alt_consumer_reps};
use super::union_find::ForwarderUnionFind;

/// RL-2 scope-exit release for a transfer-through-return forwarder RESULT whose
/// monomorphized result-type burden is EMPTY (`burden_carries_rc == false`), so the
/// result lineage was never collected into `owned_vars_needing_rc` and gets neither
/// a FRESH inc nor any scope-exit dec — leaking its transferred-in allocation when
/// the lineage is consumed only by a borrow-projection / borrow-read then dies.
///
/// Returns `(block_idx, instr_idx) -> [result_var]`: a single whole-var `BurdenDec`
/// emitted AFTER the lineage's last-use instruction. The whole-var dec lowers
/// (Phase 7) to a `RcDec` whose drop-glue recursively frees the result's owned
/// fields (`result_list`'s `[int]` Ok-payload) OR the result pointer itself
/// (`set_int`'s `{int}` buffer) — `RL2_release_exactly_once` holds: the transferred-in
/// allocation's single lifecycle `+1` is matched by this single `-1`.
///
/// THE ROOT: `@id<T>(x: T) -> T` instantiated returns a
/// distinct monomorphized result type `Idx` whose `UserBurdenSpec` is empty, so
/// `compute_owned_vars_needing_rc` skips the result lineage. RL-34 makes the caller
/// own the returned allocation; `RL2_borrowed_param_emits_caller_dec` mandates a dec
/// at the borrow-read last use. The dec is MISSING. This restores it (the impl
/// under-emits; the proven calculus is correct).
///
/// CONSERVATIVE GATES (the over-emission / double-free boundary — add ONLY when ALL hold):
///  (a) the var is an Apply/Invoke result whose callee `transfers_through_return ∧ Owned`
///      (the forwarder identity — RL-34) AND its forwarder-rep lineage exists;
///  (b) NO lineage class member is in `owned_vars_needing_rc` — there is NO existing
///      release (distinguishes from the `inherent` over-emission, where the result
///      `carries_rc` is true, IS in `owned_vars`, and already over-decs, so a release
///      here would deepen the double-free);
///  (c) the lineage rep is NOT in `alt_consumer_reps` — never consumed at an owned arg
///      position / returned / stored (a moved/transferred result's downstream consumer
///      decs it; adding a dec here double-frees);
///  (d) the lineage IS used (a genuinely-dead-immediately result is the dead-block-param
///      shape, owned by `compute_dead_forwarder_block_param_releases`).
/// When ANY gate is uncertain the release is NOT added — under-emission is the current
/// leak (no regression); over-emission is a DOUBLE-FREE (catastrophic).
pub(in crate::lower::burden_lower) fn compute_forwarder_result_under_release(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> {
    let mut uf = ForwarderUnionFind::build(func, contracts);
    let alt_consumer_reps = compute_alt_consumer_reps(func, contracts, &mut uf);

    // Collect the forwarder-result vars: Apply/Invoke `dst` whose callee owns-transfers
    // an arg through its return. Repr-admit RcPointer / FatValue / Aggregate (the
    // forwarded heap value or sum/struct wrapper); scalars carry no RC.
    let mut result_vars: Vec<ArcVarId> = Vec::new();
    let result_repr_admits = |dst: ArcVarId| -> bool {
        matches!(
            func.var_repr(dst),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue | ValueRepr::Aggregate)
        )
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if args
                    .iter()
                    .any(|&a| arg_owned_transfers_through_return(contracts, *callee, a, args))
                    && result_repr_admits(*dst)
                {
                    result_vars.push(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            if args
                .iter()
                .any(|&a| arg_owned_transfers_through_return(contracts, *callee, a, args))
                && result_repr_admits(*dst)
            {
                result_vars.push(*dst);
            }
        }
    }

    let mut out: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> = FxHashMap::default();
    let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for result_var in result_vars {
        let rep = uf.find(result_var);
        // Gate (a): forwarder-identity rep.
        if !uf.is_forwarder_rep.contains(&rep) {
            continue;
        }
        // Gate (c): not owned-transferred / returned / stored downstream.
        if alt_consumer_reps.contains(&rep) {
            continue;
        }
        // Gate (e) — the FRESH-Construct-owner boundary (the over-emission distinguisher).
        // Decline when the forwarder's FULL class (arg + result, via `class_members`)
        // contains a FRESH-allocating definition (`Construct`/`Reuse`/`CollectionReuse`/
        // `PartialApply`) whose dst is owned-RC-tracked: that fresh member OWNS the shared
        // allocation and retains its own scope-exit release, so a whole-var release here
        // double-frees it (the `host`/imported-generic shape: `%3 = Construct List` flows
        // `%5 = %3` → `identity(%5)` → result, and `%3`'s native release covers the alloc).
        // FIRE only when the allocation entered the class via a CALL RESULT (`@__collect_set`)
        // with no in-class FRESH-Construct owner — then no retained release exists (the
        // `set_int` shape). The class members are resolved BEFORE the result-side narrowing
        // because the over-fire owner is UPSTREAM of the result (the transferred-in arg).
        if class_has_fresh_construct_owner(func, &uf.class_members(rep), owned_vars_needing_rc) {
            continue;
        }
        // RESULT-SIDE lineage: the result var + its FORWARD Let-Var alias closure (the
        // `%10 = %8` continuation). DISTINCT from the full forwarder-rep class (which
        // ALSO unions the transferred-in ARG `%7` — whose own release is correctly
        // transfer-suppressed at the forwarder call). Gate (b) + the release-site walk
        // operate on the result-side lineage so the upstream arg's owned-RC membership
        // does NOT mask the genuinely-unreleased result.
        let result_lineage = result_side_lineage(func, result_var);
        // Gate (b): NO existing release on the RESULT-SIDE lineage (the carries_rc=false
        // bug). If any result-side member is owned-RC-tracked, the existing last-use
        // machinery already releases it — adding a dec here double-frees (the `inherent`
        // shape: its result `%7` carries_rc=true, IS in owned_vars, already over-decs).
        if result_lineage
            .iter()
            .any(|m| owned_vars_needing_rc.contains(m))
        {
            continue;
        }
        // Gate (c, cont.): the result lineage's payload may be PROJECTED to an
        // owned-RC var that the existing machinery releases (e.g. a `Box<[int]>`
        // forwarder whose `[int]` field is projected and owns its own dec). If any
        // payload-projection of a result-side member carries RC and is released, the
        // whole-var dec here would double-free that field. Decline.
        if lineage_has_released_projection(func, &result_lineage, owned_vars_needing_rc) {
            continue;
        }
        // Dedup: one release per distinct forwarder allocation (multiple result vars
        // can alias one rep through the union-find; a second dec double-frees).
        if !seen_reps.insert(rep) {
            continue;
        }
        // Find the result-side lineage's death point (just past its final borrow-read).
        // Gate (d): a lineage with NO use is the dead-block-param shape (owned elsewhere)
        // — skip.
        let Some(site) = lineage_release_site(func, &result_lineage) else {
            continue;
        };
        out.entry(site).or_default().push(result_var);
    }
    out
}

/// The RESULT-SIDE lineage of a forwarder result: the result var plus the FORWARD
/// closure over `Let { Var }` aliases (`%10 = %8`) AND borrow `Project` dsts
/// (`%14 = Project %9.1` — the Ok-payload of an Aggregate result). EXCLUDES the
/// transferred-in arg (upstream of the forwarder call) — that arg's own release is
/// transfer-suppressed at the call site and must NOT mask the result's missing release.
///
/// Forward-only fixpoint: seed with `result_var`; repeatedly add any `Let { dst = Var(m) }`
/// dst or `Project { dst, value: m }` dst whose source `m` is already a member, until no
/// growth. Gives the set of vars that share the result's allocation downstream of the
/// forwarder — the lineage that needs the single whole-var release + whose final use is
/// the death point.
fn result_side_lineage(func: &ArcFunction, result_var: ArcVarId) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(result_var);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value) && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
        }
        if !grew {
            break;
        }
    }
    members
}

/// True iff any member of the forwarder class is defined by a FRESH-allocating
/// instruction (`Construct` / `Reuse` / `CollectionReuse` / `PartialApply`) AND is in
/// `owned_vars_needing_rc`. Such a member OWNS the class's shared allocation and retains
/// its own scope-exit release — a whole-var forwarder-result release would double-free it.
///
/// The over-emission distinguisher between the genuinely-unreleased forwarder result
/// (allocation enters via a CALL RESULT — `@__collect_set` — with NO in-class FRESH-Construct
/// owner) and the already-released one (`host`: `%3 = Construct List` flows into the
/// forwarder, retains its native release). Resolving the FULL class (not result-side) is
/// load-bearing — the FRESH owner is UPSTREAM of the result (the transferred-in arg).
fn class_has_fresh_construct_owner(
    func: &ArcFunction,
    class_members: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let fresh_dst = match instr {
                ArcInstr::Construct { dst, .. }
                | ArcInstr::Reuse { dst, .. }
                | ArcInstr::CollectionReuse { dst, .. }
                | ArcInstr::PartialApply { dst, .. } => Some(*dst),
                _ => None,
            };
            if let Some(dst) = fresh_dst {
                if class_members.contains(&dst) && owned_vars_needing_rc.contains(&dst) {
                    return true;
                }
            }
        }
    }
    false
}

/// Where a forwarder-result whole-var `BurdenDec` lands relative to one block. The
/// release follows the lineage's FINAL borrow-read so the read completes before the
/// allocation is freed (no UAF).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ForwarderReleasePos {
    /// At the START of the block's body (the normal-successor of a borrowed
    /// terminator-`Invoke` whose arg was the lineage's final use — the borrowed call
    /// completed on the predecessor's terminator, the result is dead at this entry).
    BlockEntry,
    /// Immediately AFTER the body instruction at `instr_idx` (the lineage's final use
    /// was a body `Apply`/etc. borrow at that position).
    AfterInstr(usize),
}

/// True iff any `Project` of a lineage member produces an owned-RC dst that the
/// existing last-use machinery already releases (in `owned_vars_needing_rc`). Such a
/// projection's field is freed by its own dec; a whole-var release on the parent
/// would double-free it. Over-fire boundary for `compute_forwarder_result_under_release`.
fn lineage_has_released_projection(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if members.contains(value) && owned_vars_needing_rc.contains(dst) {
                    return true;
                }
            }
        }
    }
    false
}

/// Resolve the lineage's death point — the `(block_idx, pos)` just past its FINAL
/// borrow-read, where the whole-var `BurdenDec` lands. Walks every block in forward
/// CFG order; the last block carrying a member use wins (the lineage is straight-line
/// forwarder-then-borrow, so the textually-last use is the death point).
///
/// Two terminal shapes:
///  - the final use is a body instruction (e.g. `Apply @__index(.. [borrow])`) →
///    `AfterInstr(instr_idx)`;
///  - the final use is a BORROWED terminator-`Invoke` arg (e.g. `Invoke @len(.. [borrow])
///    normal bbN`) → the value survives the borrowed call and dies at the normal
///    successor's entry → `(normal_succ, BlockEntry)`.
///
/// An OWNED terminator-position use (Return / Jump-arg / owned Invoke-arg) is a
/// transfer, NOT a borrow-read — `compute_alt_consumer_reps` already excluded such a
/// lineage (gate c), so it cannot reach here; return `None` defensively if it does.
fn lineage_release_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> Option<(usize, ForwarderReleasePos)> {
    let mut site: Option<(usize, ForwarderReleasePos)> = None;
    for (block_idx, block) in func.blocks.iter().enumerate() {
        // Body-instruction uses (skip Let-Var alias re-binds: their dst is itself a
        // member, not a terminal read).
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Let {
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if members.contains(src) {
                    continue;
                }
            }
            if instr.used_vars().iter().any(|v| members.contains(v)) {
                site = Some((block_idx, ForwarderReleasePos::AfterInstr(instr_idx)));
            }
        }
        // Terminator borrow-use: a member at a BORROWED position of an `Invoke` /
        // `InvokeIndirect` (the result survives, dies at the normal successor's entry).
        let term = &block.terminator;
        if let ArcTerminator::Invoke { normal, .. } | ArcTerminator::InvokeIndirect { normal, .. } =
            term
        {
            for (pos, &v) in term.used_vars().iter().enumerate() {
                if members.contains(&v) && !term.is_owned_position(pos) {
                    site = Some((normal.index(), ForwarderReleasePos::BlockEntry));
                }
            }
        }
    }
    site
}
