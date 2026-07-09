//! FRESH-sum LIVE-EXTRACT same-alloc lineage treatment (RL-1 + RL-2): the
//! match-extract sibling of `compute_construct_fed_dead_param_lineage`.
//! Shape, over-emission mechanism, cure, and admission gates are documented
//! on [`compute_fresh_sum_live_extract_lineage`]. The branchy RETAIN-ALIASING
//! sibling scan lives in [`retain_aliasing`]; root/vetting utilities shared by
//! both live in [`shared`]; release-site placement (gate (f)) lives in
//! [`site`].

mod retain_aliasing;
mod shared;
mod site;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId, ValueRepr};

use super::super::is_provably_scalar_repr;
use super::{function_used_vars, ForwarderReleasePos};
use site::{choose_release_site, release_site_sound};

pub(in crate::lower::burden_lower) use retain_aliasing::compute_retain_aliasing_lineage;
use shared::same_alloc_closure_vetted;
pub(in crate::lower::burden_lower) use shared::{collect_fresh_sum_roots, is_niche_family_sum};

/// One root's candidate admission, held until the gate (g) disjointness
/// filter runs over the full candidate set.
struct Candidate {
    root: ArcVarId,
    members: FxHashSet<ArcVarId>,
    site_block: usize,
    site_pos: ForwarderReleasePos,
    dec_var: ArcVarId,
    /// True iff this web carries a DEPTH-≥2 niche-of-niche projection — a member
    /// niche-family-sum `Project`ed FROM another member niche-sum (the
    /// `catch(catch(panic))` double-wrap: `%inner = Project %outer.1` where BOTH
    /// `%outer` and `%inner` are niche-family sums over the same leaf
    /// allocation). The base walk over-emits a CASCADE dec at EACH nest level for
    /// this shape (the -134 double-free). The overlapping-group MERGE that places
    /// ONE release fires ONLY for a group carrying this flag. A FLAT single-wrap
    /// web (a niche-sum carrier `Project`ed directly to a LEAF `str` / `[T]`) is
    /// base-walk-correct and keeps the gate (g) decline (the
    /// `match_alias::test_match_arm_alias_result_str` / `_unwind_path_alias` /
    /// `_option_intlist_select_branch_return` over-fire boundary). Spec: Annex E
    /// §AIMS RL-2 + TF-4.
    nested_niche_extract: bool,
}

/// Detect the DEPTH-≥2 niche-of-niche projection in `members`: a non-scalar
/// `Project { dst, value }` where `value` is a member niche-family sum AND `dst`
/// is ALSO a niche-family sum (a transparent niche wrapper projected out of
/// another, the `catch(catch(panic))` double-wrap). A flat single-wrap web
/// projects its carrier directly to a LEAF (`str` / `[T]`), so `dst` is NOT a
/// niche-family sum and this returns false.
fn has_nested_niche_projection(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if members.contains(value)
                    && members.contains(dst)
                    && !is_provably_scalar_repr(func, *dst)
                    && is_niche_family_sum(func, *value, type_registry)
                    && is_niche_family_sum(func, *dst, type_registry)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Union-find with path compression: the representative root of `x`.
fn uf_find(parent: &mut [usize], x: usize) -> usize {
    let mut r = x;
    while parent[r] != r {
        r = parent[r];
    }
    let mut c = x;
    while parent[c] != r {
        let next = parent[c];
        parent[c] = r;
        c = next;
    }
    r
}

/// Partition [`Candidate`] indices into connected components by member-set
/// overlap (a member shared by two candidates joins their component). Each
/// returned `Vec<usize>` is a component's candidate indices.
fn connected_overlap_groups(candidates: &[Candidate]) -> Vec<Vec<usize>> {
    let n = candidates.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in (i + 1)..n {
            if !candidates[i].members.is_disjoint(&candidates[j].members) {
                let ri = uf_find(&mut parent, i);
                let rj = uf_find(&mut parent, j);
                if ri != rj {
                    parent[ri] = rj;
                }
            }
        }
    }
    let mut groups: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    for i in 0..n {
        let r = uf_find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups.into_values().collect()
}

/// Emit one gate-(g) decline trace per root in a declined merge group.
fn trace_group_decline(
    func: &ArcFunction,
    candidates: &[Candidate],
    group: &[usize],
    gate: &'static str,
) {
    for &i in group {
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            root = candidates[i].root.index(),
            gate,
            "fresh-sum live-extract root declined"
        );
    }
}

/// Result of [`compute_fresh_sum_live_extract_lineage`]: the same-alloc
/// closure to suppress (Part A) + the single placed release (Part B).
pub(in crate::lower::burden_lower) struct FreshSumLiveExtractLineage {
    /// Every var in an admitted fresh-sum live-extract closure. All carry
    /// spurious keep-alive incs / misplaced releases on ONE allocation;
    /// removed from `owned_vars_needing_rc` so the sole release is the
    /// placed dec below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — exactly ONE whole-var `BurdenDec`
    /// per admitted closure, on the lineage var read at the final site,
    /// placed AFTER that read. Merged into the `forwarder_result_releases`
    /// emission surface (same `ForwarderReleasePos` placement contract).
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// RL-1 + RL-2 treatment for the FRESH-sum live-extract match shape
/// (`match result { Some(s) -> s, .. }` + downstream borrow-reads).
///
/// A FRESH niche-family sum allocation read through `Let { Var }` aliases +
/// a niche-payload `Project` borrow-view and EXTRACTED LIVE to a merge
/// block-param names ONE allocation across the whole closure (TF-4: `Project`
/// is a borrow; a niche-family sum carries no allocation of its own). Per
/// RL-1 NO duplication exists, yet the `use_counts >= 2` proxy classes the
/// Let-Var alias as a duplication and the FRESH-site result inc survives:
/// 2 spurious keep-alive incs + 2 misplaced releases (the sum's arm dec
/// before the live reads + the extract's last-use dec before its final
/// transitive read) net +1 — the caller-owned reference is never released
/// (`RL2_release_exactly_once` violated; leak).
///
/// The cure removes the WHOLE closure from `owned_vars_needing_rc` (both
/// incs + both misplaced releases) and emits EXACTLY ONE whole-var
/// `BurdenDec` on the lineage var read at the closure's execution-FINAL
/// borrow-read, placed AFTER that read (`RL2_dec_at_last_use` — no UAF). The
/// dec targets the final-read var (NOT the root): on a merge path where the
/// carrier param received a DIFFERENT allocation (the `None -> "fallback"`
/// arm's literal), the dec releases that path's allocation — per-path
/// correct where a root dec would no-op and leak the literal.
///
/// Admission gates (ALL must hold per closure; ANY failure declines the root
/// and keeps current behavior — the status-quo leak is FAR safer than an
/// over-fire UAF / double-free):
///  (a) FRESH sum root: a `Construct { ctor: EnumVariant }` dst, OR an
///      `Apply` / `Invoke` dst whose callee contract reports
///      `return_info.uniqueness ∈ {Unique, MaybeShared}` (the same condition
///      under which `fresh_site_burden_inc_dst` emits the spurious result
///      inc) with NO `transfers_through_return` param (a forwarder result is
///      the ARG's allocation — owned by the forwarder scans, disjoint family).
///  (b) root in `owned_vars_needing_rc` (heap-carrying; auto-declines roots
///      claimed by the construct-fed suppression, which runs first) with
///      `Aggregate` repr (an `RcPointer` sum is a BOXED wrapper needing its
///      own release — out of family).
///  (c) niche-family sum type ([`is_niche_family_sum`]): the wrapper's RC
///      identity IS the single live payload, so one release frees the web.
///  (d) vetted same-alloc closure ([`same_alloc_closure_vetted`]): every
///      member use is a borrow-read; any consume / store / capture /
///      `Select`-`IsShared`-`Reset`-`Reuse` use / `Return` / indirect-call
///      arg / non-scalar borrowed-call result declines.
///  (e) at least one closure member is a LIVE block-param (the live extract;
///      a dead-params-only closure is the RL-5 dead-param family).
///  (f) execution-final single release ([`choose_release_site`] +
///      [`release_site_sound`]): the dec lands after the closure's final
///      borrow-read on every normal exit; unwind paths (`Resume`) are exempt
///      (status-quo leak there, no new double-free — the closure carries no
///      other release).
///  (g) pairwise-DISJOINT closures: two admitted roots whose closures share
///      any member converge on ONE per-path allocation web (both match arms
///      `Construct` into the same merge carrier — the caught-panic
///      `catch(expr:)` Ok/Err shape); each admission would place its own dec
///      at the shared final read, double-freeing the web. ALL roots of an
///      overlapping group decline (status quo preserved).
pub(in crate::lower::burden_lower) fn compute_fresh_sum_live_extract_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
) -> FreshSumLiveExtractLineage {
    let mut out = FreshSumLiveExtractLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let used = function_used_vars(func);
    let preds = compute_predecessors(func);

    // Per-root candidate admissions; gate (g) filters before application.
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut claimed_roots: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in collect_fresh_sum_roots(func, contracts) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "fresh-sum live-extract root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed + by-value sum repr.
        if !owned_vars_needing_rc.contains(&root)
            || claimed_roots.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::Aggregate))
        {
            decline("b:owned/claimed/repr");
            continue;
        }
        // Gate (c): niche-family sum burden.
        if !is_niche_family_sum(func, root, type_registry) {
            decline("c:niche-family-sum");
            continue;
        }
        // Gate (d): vetted same-alloc closure.
        let Some(members) = same_alloc_closure_vetted(func, root) else {
            decline("d:closure-vet");
            continue;
        };
        // Gate (e): a LIVE block-param member (the live extract).
        let has_live_param = func.blocks.iter().any(|b| {
            b.params
                .iter()
                .any(|&(p, _)| members.contains(&p) && used.contains(&p))
        });
        if !has_live_param {
            decline("e:live-extract-param");
            continue;
        }
        // Gate (e2): a live-extracted `Aggregate` payload member is admitted ONLY
        // when it is itself a NICHE-FAMILY SUM (a transparent niche wrapper
        // sharing the leaf allocation); a by-value struct / tuple aggregate with
        // its OWN owned heap fields declines.
        //
        // Gate (c)'s "one release frees the whole web" premise holds when every
        // member names ONE allocation. A leaf `FatVal` / `RcPtr` (`str` / `[T]`)
        // payload always does. A recursively niche-family sum payload
        // (`Result<Result<never, str>, str>` extracting the inner
        // `Result<never, str>`, or `Option<Option<T>>`) ALSO does — each niche
        // wrapper carries no allocation of its own (`is_niche_family_sum`), so the
        // whole nest is one leaf allocation and the web's sole release frees it
        // once (`RL2_release_exactly_once`). A by-value struct / tuple payload
        // (`Option<Node>` where `Node` is a struct with owned heap fields) is
        // INLINE in the niche but recursively owns DISTINCT heap children: the
        // wrapper's own scope-exit release + the live-extracted struct's own
        // release are TWO distinct releases; the keep-alive inc between them is
        // LOAD-BEARING per RL-1 (`RL1_duplication_balanced`); suppressing it
        // leaves the allocation net -1 (double-free) — the over-fire boundary the
        // negative `probe_nested_construct_payload_extracted_live` pin guards.
        // Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.
        let non_transparent_aggregate_payload = func.blocks.iter().any(|b| {
            b.params.iter().any(|&(p, _)| {
                members.contains(&p)
                    && used.contains(&p)
                    && matches!(func.var_repr(p), Some(ValueRepr::Aggregate))
                    && !is_niche_family_sum(func, p, type_registry)
            })
        });
        if non_transparent_aggregate_payload {
            decline("e2:aggregate-payload-extract");
            continue;
        }
        // The DEPTH-≥2 niche-of-niche projection flag gating the gate-(g) merge
        // (the catch(catch(panic)) double-wrap; flat single-wrap webs are false).
        let nested_niche_extract = has_nested_niche_projection(func, &members, type_registry);
        // Gate (f): the placed single release.
        let Some((site_block, site_pos, dec_var)) = choose_release_site(func, &members) else {
            decline("f:release-site");
            continue;
        };
        if !release_site_sound(func, &members, root, site_block, site_pos, &preds) {
            decline("f:site-soundness");
            continue;
        }
        claimed_roots.extend(members.iter().copied());
        candidates.push(Candidate {
            root,
            members,
            site_block,
            site_pos,
            dec_var,
            nested_niche_extract,
        });
    }

    apply_overlap_group_releases(func, &candidates, &preds, &mut out);
    out
}

/// Gate (g): partition candidates into connected overlap components and apply
/// their releases into `out`. A SINGLE-candidate component keeps its
/// already-vetted per-root placement (the established flat live-extract
/// behavior). A MULTI-candidate (overlapping) component is a shared-allocation
/// web; MERGE it into ONE release placement ONLY when it carries the DEPTH-≥2
/// `nested_niche_extract` (the `catch(catch(panic))` double-wrap where the base
/// walk over-emits a cascade dec at each nest level). A flat single-wrap
/// overlapping web (leaf payloads, `Some(s) -> s` Ok/Err merging two arms) is
/// BASE-WALK-CORRECT and keeps the historical gate-(g) decline (the
/// `match_alias::*` over-fire boundary). When the merged web has no sound single
/// release site, the component declines. Spec: Annex E §AIMS RL-2
/// (`RL2_release_exactly_once`) + TF-4.
fn apply_overlap_group_releases(
    func: &ArcFunction,
    candidates: &[Candidate],
    preds: &[Vec<usize>],
    out: &mut FreshSumLiveExtractLineage,
) {
    for group in connected_overlap_groups(candidates) {
        if group.len() == 1 {
            let c = &candidates[group[0]];
            out.suppressed_lineage_vars
                .extend(c.members.iter().copied());
            out.releases
                .entry((c.site_block, c.site_pos))
                .or_default()
                .push(c.dec_var);
            continue;
        }
        if !group.iter().any(|&i| candidates[i].nested_niche_extract) {
            trace_group_decline(func, candidates, &group, "g:closure-overlap");
            continue;
        }
        let mut union_members: FxHashSet<ArcVarId> = FxHashSet::default();
        for &i in &group {
            union_members.extend(candidates[i].members.iter().copied());
        }
        let Some((b, pos, var)) = choose_release_site(func, &union_members) else {
            trace_group_decline(func, candidates, &group, "g:merged-release-site");
            continue;
        };
        let all_sound = group
            .iter()
            .all(|&i| release_site_sound(func, &union_members, candidates[i].root, b, pos, preds));
        if !all_sound {
            trace_group_decline(func, candidates, &group, "g:merged-site-soundness");
            continue;
        }
        out.suppressed_lineage_vars
            .extend(union_members.iter().copied());
        out.releases.entry((b, pos)).or_default().push(var);
    }
}
