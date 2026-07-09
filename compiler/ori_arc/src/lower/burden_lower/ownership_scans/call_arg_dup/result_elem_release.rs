//! Call-result-aggregate element FINAL-READ release designation:
//! [`compute_call_result_element_final_read_releases`]. Spec: Annex E §AIMS
//! RL-2.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::super::dup_inc::{collect_use_sites, is_burden_op};
use super::super::successor_reachable_blocks;

/// `ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE=1` skips the FINAL-READ release
/// designation for multi-read call-result-aggregate elements
/// ([`compute_call_result_element_final_read_releases`] returns empty),
/// restoring the keep-alive-pair arrangement (one leaked element per
/// multi-read lineage). Bisection surface: isolates a returned-tuple element
/// leak / double-free to the designated release vs the rest of the Phase-5
/// walk. Spec: Annex E §AIMS RL-2.
static RESULT_ELEM_FINAL_READ_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_RESULT_ELEM_FINAL_READ_RELEASE").as_deref() == Ok("1")
});

/// FINAL-READ release designation for multi-read elements of a caller-owned
/// call-result aggregate (RL-2 `RL2_release_exactly_once`).
///
/// Shape: `let (a, b) = make(); ... a.len() ... a[0]` — the call result `%0`
/// is an `Aggregate`-repr tuple carrying NO burden ops (tuple types compose no
/// whole-var burden, so the aggregate emits no `AggFields` drop), its element
/// `Project` dsts are borrowed views (excluded from `owned_vars_needing_rc`),
/// and every read goes through a fresh `Let { Var }` alias of the projection.
/// A SINGLE-read element's alias keeps a lone last-use `BurdenDec` (the
/// element's release). A MULTI-read element's aliases are dup-aliases
/// (`use_counts >= 2`): each gets a net-0 keep-alive pair (or no ops at all
/// for a sole-use borrowed-`Invoke`-arg alias), so the element's caller-owned
/// reference is NEVER released — one leaked element per multi-read lineage.
///
/// The cure designates the lineage's EXECUTION-FINAL read alias as the
/// release carrier: returned aliases are (a) re-included in
/// `owned_vars_needing_rc` (the borrowed-arg-alias exclusion would drop a
/// final `@len`-read alias) and (b) inserted into `inc_suppressed_vars` (the
/// dup-alias keep-alive inc is suppressed), leaving exactly ONE last-use
/// `BurdenDec` on the lineage — the transferred-in element's single release.
///
/// Decline fences (each leaves the lineage on today's arrangement):
/// - the call result has no contract, a `transfers_through_return` param (the
///   result aliases an arg — not a fresh acquisition), or a non-`Unique`
///   return (the caller may not own the only reference);
/// - the aggregate base or the projection carries burden ops of its own
///   (membership in `owned_vars_needing_rc` — a whole-var release exists, a
///   designated alias dec would double-free);
/// - any lineage member is consumed at an owned position, returned, or passed
///   as a `Jump` arg (the element escapes — its release belongs elsewhere);
/// - a use of the projection that is not a `Let { Var }` alias;
/// - no UNIQUE execution-final alias (loop-carried reads re-reach every site
///   via the back-edge; branch forks have per-arm finals) — conservative.
pub(in crate::lower::burden_lower) fn compute_call_result_element_final_read_releases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    if *RESULT_ELEM_FINAL_READ_RELEASE_DISABLED {
        return FxHashSet::default();
    }
    // Fresh caller-owned call-result aggregates: contract-vetted Apply /
    // Invoke dsts with Aggregate repr and no whole-var burden membership.
    let fresh_result = |callee: Name, dst: ArcVarId| -> bool {
        matches!(func.var_repr(dst), Some(ValueRepr::Aggregate))
            && !owned_vars_needing_rc.contains(&dst)
            && contracts.get(&callee).is_some_and(|c| {
                c.return_info.uniqueness == crate::aims::lattice::Uniqueness::Unique
                    && !c.params.iter().any(|p| p.transfers_through_return)
            })
    };
    let mut fresh_aggregates: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if fresh_result(*callee, *dst) {
                    fresh_aggregates.insert(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if fresh_result(*callee, *dst) {
                fresh_aggregates.insert(*dst);
            }
        }
    }
    if fresh_aggregates.is_empty() {
        return FxHashSet::default();
    }
    // Element projections of those aggregates: RcPtr/FatValue borrowed views
    // with no burden membership of their own.
    let mut elem_sources: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if fresh_aggregates.contains(value)
                    && !owned_vars_needing_rc.contains(dst)
                    && matches!(
                        func.var_repr(*dst),
                        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                    )
                {
                    elem_sources.insert(*dst);
                }
            }
        }
    }
    if elem_sources.is_empty() {
        return FxHashSet::default();
    }
    let use_sites = collect_use_sites(func);
    let (aliases_of, declined) = vet_element_read_lineages(func, &elem_sources);
    // Execution-final unique alias per multi-read lineage.
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut releases: FxHashSet<ArcVarId> = FxHashSet::default();
    for (&src, aliases) in &aliases_of {
        if declined.contains(&src) {
            continue;
        }
        // Multi-read lineages only: the single-read alias already carries its
        // lone last-use dec on today's arrangement.
        if use_sites.get(&src).map_or(0, Vec::len) < 2 || aliases.len() < 2 {
            continue;
        }
        // Lineage use-site union (source + aliases).
        let mut lineage_sites: Vec<(usize, usize)> = Vec::new();
        for member in std::iter::once(&src).chain(aliases.iter()) {
            if let Some(sites) = use_sites.get(member) {
                lineage_sites.extend(sites.iter().copied());
            }
        }
        let mut finals: Vec<ArcVarId> = Vec::new();
        for &a in aliases {
            let Some(sites) = use_sites.get(&a) else {
                continue;
            };
            // The alias's LAST use site: later-in-block beats earlier;
            // terminator sites are usize::MAX. Cross-block ordering is settled
            // by the execution-final check below, not here.
            let Some(&(lb, li)) = sites.last() else {
                continue;
            };
            let reachable = reachable_cache
                .entry(lb)
                .or_insert_with(|| successor_reachable_blocks(func, lb));
            let used_after = lineage_sites
                .iter()
                .any(|&(ub, ui)| (ub == lb && ui > li) || reachable.contains(&ub));
            if !used_after {
                finals.push(a);
            }
        }
        if let [one] = finals.as_slice() {
            releases.insert(*one);
        }
    }
    releases
}

/// Per-source alias collection + decline vetting for
/// [`compute_call_result_element_final_read_releases`]: returns
/// `(aliases_of, declined)` — the `Let { Var }` read aliases per element
/// source, plus the sources DECLINED by an escaping shape (a non-alias source
/// use, an owned-position alias consume, a `Set.value`, a re-alias of an
/// alias, or any member at a Return / Jump / owned terminator position).
fn vet_element_read_lineages(
    func: &ArcFunction,
    elem_sources: &FxHashSet<ArcVarId>,
) -> (FxHashMap<ArcVarId, Vec<ArcVarId>>, FxHashSet<ArcVarId>) {
    // Per-source alias collection + vetting.
    let mut aliases_of: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut alias_src: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let mut declined: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if is_burden_op(instr) {
                continue;
            }
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if elem_sources.contains(src) {
                    aliases_of.entry(*src).or_default().push(*dst);
                    alias_src.insert(*dst, *src);
                    continue;
                }
                // A re-alias of an alias hides downstream reads from the
                // lineage-site union — decline the lineage (conservative).
                if let Some(&s) = alias_src.get(src) {
                    declined.insert(s);
                }
            }
            // A non-alias use of a source declines its lineage; an
            // owned-position consume of an alias declines its lineage.
            for (pos, &v) in instr.used_vars().iter().enumerate() {
                if elem_sources.contains(&v) {
                    declined.insert(v);
                }
                if let Some(&s) = alias_src.get(&v) {
                    if instr.is_owned_position(pos) {
                        declined.insert(s);
                    }
                }
            }
            if let ArcInstr::Set { value, .. } = instr {
                if let Some(&s) = alias_src.get(value) {
                    declined.insert(s);
                }
            }
        }
        // Terminator vetting: a lineage member at a Return / Jump / owned
        // Invoke position escapes — decline. Borrowed Invoke args are reads.
        let term = &block.terminator;
        let term_is_call = matches!(
            term,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. }
        );
        for (pos, &v) in term.used_vars().iter().enumerate() {
            let member_src = if elem_sources.contains(&v) {
                Some(v)
            } else {
                alias_src.get(&v).copied()
            };
            if let Some(s) = member_src {
                if elem_sources.contains(&v) || !term_is_call || term.is_owned_position(pos) {
                    declined.insert(s);
                }
            }
        }
    }
    (aliases_of, declined)
}
