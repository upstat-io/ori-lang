//! RL-1 + RL-2 + RL-4 treatment for the BRANCHY RETAIN-ALIASING shape
//! (`let v = m[k]; if v.is_some() && v.unwrap().starts_with(..)`).
//!
//! # Shape
//!
//! A FRESH niche-family sum (`@__index` `Option<str>` result) whose payload is
//! read through `Let { Var }` aliases + niche-payload `Project` views AND
//! through an ACCESSOR-RETAIN call (`@unwrap`/`@get`, a builtin in
//! [`accessor_retain_builtin_names`](crate::borrow::accessor_retain_builtin_names)
//! that self-increments its extracted view — TF-4, the SAME allocation) across
//! a SHORT-CIRCUIT branchy CFG. The base walk emits the spurious dup-alias
//! keep-alive incs + an INLINE source dec BEFORE the accessor-retain call reads
//! the payload → use-after-free + double-free (exit -134:
//! `coll_map_index_int_str` / `set_str_union` / catch-cohort).
//!
//! The single-site [`super::compute_fresh_sum_live_extract_lineage`] does not
//! apply here: the allocation is live on the SHORT-CIRCUIT-BYPASS branch
//! (`v.is_some() && …`: the `None`/`false` arm reaches `Return` WITHOUT
//! passing the `Some`-path reader), so no single site dominates every normal
//! exit. This consumer reuses the SHARED MULTI-EXIT per-path placement
//! ([`super::super::multi_exit_borrow_view::place_per_path_releases_with`]).
//!
//! # Placement
//!
//! MULTI-REFERENCE placement: the accessor-retain chain holds ONE
//! retained reference PER hop on the SAME allocation — the niche-sum root (the
//! `@__index` self-inc on the Option payload) AND each accessor-retain result
//! (the `@unwrap`/`@get` self-inc on its extracted view). A single root-dec
//! releases ONLY the Option-payload reference and LEAKS the `@unwrap` reference.
//! The cure places a per-path release for EACH reference root (the niche-sum
//! root + every accessor-retain result member): RL-2 after the final read on
//! each READING path, RL-4 edge dec on each BYPASS edge — exactly-once-per-path
//! PER reference.
//!
//! # Admission
//!
//! Admission mirrors [`super::compute_fresh_sum_live_extract_lineage`] gates
//! (a)/(b)/(c) EXCEPT: the closure grows across accessor-retain results, the
//! all-borrow-reads vet admits an accessor-retain non-scalar result that is
//! itself a closure member (the payload view), the gate is "carries a borrow-
//! read" (the accessor-retain extract), and the per-path multi-reference
//! placement REPLACES the single-site gate (f). Pairwise-disjoint over the
//! candidate set. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4 + TF-4.
//!
//! Gate (d) closure growth/vetting lives in [`vet`]; gate (f') per-reference
//! release placement lives in [`placement`].

mod placement;
mod vet;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::graph::compute_predecessors;
use crate::ir::{ArcFunction, ArcVarId, ValueRepr};

use super::super::{compute_pairwise_overlap_flags, ForwarderReleasePos};
use super::site::{choose_release_site, release_site_sound};
use super::{collect_fresh_sum_roots, is_niche_family_sum, FreshSumLiveExtractLineage};
use placement::{accessor_retain_result, place_retain_aliasing_per_reference};
use vet::retain_aliasing_closure_vetted;

/// One retain-aliasing root's candidate admission (the branchy per-path sibling
/// of `Candidate`): the same-alloc closure + its per-terminal-path placed
/// releases, held until the gate (g) disjointness filter runs.
struct RaCandidate {
    members: FxHashSet<ArcVarId>,
    releases: Vec<((usize, ForwarderReleasePos), ArcVarId)>,
}

pub(in crate::lower::burden_lower) fn compute_retain_aliasing_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> FreshSumLiveExtractLineage {
    let mut out = FreshSumLiveExtractLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };
    let accessor_retain_names = crate::borrow::accessor_retain_builtin_names(interner);
    let preds = compute_predecessors(func);

    let mut candidates: Vec<RaCandidate> = Vec::new();
    let mut claimed: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in collect_fresh_sum_roots(func, contracts) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "retain-aliasing root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed + by-value sum repr.
        if !owned_vars_needing_rc.contains(&root)
            || claimed.contains(&root)
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
        // Gate (d): vetted same-alloc closure (accessor-retain results extend it).
        let Some(members) = retain_aliasing_closure_vetted(func, root, &accessor_retain_names)
        else {
            decline("d:closure-vet");
            continue;
        };
        // Gate (e'): at least one ACCESSOR-RETAIN result member (the
        // retain-aliasing shape; a closure with no accessor-retain read is the
        // live_extract / multi-exit family, owned by their own scans).
        if !members
            .iter()
            .any(|m| accessor_retain_result(func, *m, &accessor_retain_names))
        {
            decline("e:no-accessor-retain");
            continue;
        }
        // Gate (e2): single-site FORECLOSURE — this per-path consumer fires ONLY
        // when the single-site placement is structurally impossible (the branchy
        // reader-BYPASS shape: a normal-exit Return reachable from the root def
        // WITHOUT passing the lineage's reader, so no single site dominates every
        // normal exit — coll_map's `if v.is_some() && v.unwrap()...`). A
        // STRAIGHT-LINE accessor-retain (`let v = o.unwrap(); v == ..`) HAS a
        // viable single dominating site and is already balanced by the existing
        // machinery (RL-1 inc-elision + base-walk last-use dec) — firing the
        // per-path multi-reference treatment there DOUBLE-treats and regresses
        // it (the wrapper_rc_retain / predicate_stack_probe accessor-retain
        // cells). Decline when single-site is viable; defer to the status quo.
        let single_site_viable = choose_release_site(func, &members)
            .is_some_and(|(b, pos, _)| release_site_sound(func, &members, root, b, pos, &preds));
        if single_site_viable {
            decline("e2:single-site-viable");
            continue;
        }
        // Gate (f'): per-path MULTI-REFERENCE placement over DISJOINT live
        // ranges. Each retained reference (the niche-sum root for the `@__index`
        // self-inc + each accessor-retain result for its `@unwrap`/`@get`
        // self-inc) has its OWN live range: the root ref is live across its
        // Let-aliases and DIES at the accessor-retain call that reads it; the
        // accessor-retain result is BORN at that call and dies at ITS last read.
        // Running placement over the WHOLE `members` per ref over-emits — it would
        // dec the root at the accessor-retain RESULT's last-read site (root
        // already dead there) and dec the result on a BYPASS path where it was
        // never born (a use-before-def → malformed IR). Each ref is therefore
        // placed over its OWN sub-closure (the ref var + the alias/projection
        // members reachable WITHOUT crossing another accessor-retain hop), with
        // that sub-closure's reads as the final-read set. Spec: Annex E §AIMS
        // RL-2 + RL-4.
        let releases =
            place_retain_aliasing_per_reference(func, root, &members, &accessor_retain_names);
        if releases.is_empty() {
            decline("f:no-placement");
            continue;
        }
        claimed.extend(members.iter().copied());
        candidates.push(RaCandidate { members, releases });
    }

    // Gate (g): decline EVERY candidate overlapping another's closure.
    let overlapping = compute_pairwise_overlap_flags(&candidates, |c| &c.members);
    for (cand, overlaps) in candidates.into_iter().zip(overlapping) {
        if overlaps {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                gate = "g:closure-overlap",
                "retain-aliasing root declined"
            );
            continue;
        }
        out.suppressed_lineage_vars
            .extend(cand.members.iter().copied());
        for (site, var) in cand.releases {
            out.releases.entry(site).or_default().push(var);
        }
    }
    out
}
