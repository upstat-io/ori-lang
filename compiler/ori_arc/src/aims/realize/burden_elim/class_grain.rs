//! Phase-6 class-grain whole-pair elision (RL-22/RL-23/RL-25 with T3
//! sibling-liveness evidence).
//!
//! A whole-var `BurdenInc`/`BurdenDec` PAIR the per-var DP-2/DP-3 pass KEPT
//! (a `Many`-cardinality keep-alive bracket) is removable when a live
//! same-allocation-class SIBLING holds the interior running count at >= 1
//! across the pair's span: the sibling's birth inc dominates the pair and its
//! own release lands after the pair's dec, so the pair is a redundant
//! keep-alive on an allocation another reference already keeps alive
//! (`AimsProof.RunningCount::T3_sibling_liveness_is_dominating_inc_evidence`
//! + `keep_alive_redundancy_sound_iff_whole_pair`).
//!
//! # Soundness boundaries (each a proven kill direction)
//!
//! - WHOLE-PAIR ONLY: the inc-only split is proven net-negative + freed-early
//!   (`T3_inc_only_net_negative`); this pass marks a var's incs AND decs
//!   together or not at all.
//! - SAME CLASS ONLY: sibling evidence comes exclusively from the birth-site
//!   partition (PV-1 `samerep_birthsite_sound`) — two provably-distinct
//!   allocations never justify each other's elision (a mis-classified
//!   sibling would reopen the 290-UAF class one grain coarser).
//! - MUTATE EXCLUSION: a class any member of which feeds a COW mutation
//!   (`Set`/`SetTag` base, `IsShared`, `Reset`/`Reuse`) keeps every pair —
//!   COW load-bearing incs are never elided (DP-5/DP-9 count the siblings).
//! - SAME-BLOCK SPAN: v1 admits only pairs whose every site sits in ONE
//!   block, with the sibling's supporting birth strictly before the span and
//!   its kept release strictly after, in that block's program order — the
//!   dominating-inc + no-intervening-dec evidence is positional, not
//!   inferred (RL-23 cross-merge joins are a later widening).
//! - NON-CANDIDATE SIBLING: the supporting sibling must itself survive this
//!   pass (no mutual justification — two pairs never elide each other).
//!
//! Additive removal-only: per-var DP-2/DP-3 verdicts are UNCHANGED (CH-2
//! intact); this pass only widens the removal set. Spec: Annex E §AIMS RL-22
//! + §12 (T3).

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::aims::intraprocedural::AimsStateMap;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use super::WholeVarBalance;

/// `ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION=1` declines the Phase-6 class-grain
/// whole-pair elision, restoring the per-var-only disposition (every kept
/// keep-alive pair survives). Bisects a class-grain-elision regression vs the
/// per-var DP-2/DP-3 path. Default (unset): the pass runs.
// Env: ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION — declines Phase-6 class-grain whole-pair elision for bisection, debug-only
static CLASS_GRAIN_PAIR_ELISION_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION").as_deref() == Ok("1"));

/// Mark class-grain whole-pair removals.
///
/// Runs AFTER `mark_whole_var_removals` (consumes its keep verdicts; touches
/// only pairs every one of whose sites is still un-removed) and BEFORE
/// compaction. `remove` is only ever widened.
pub(super) fn mark_class_grain_whole_pair_removals(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    balances: &FxHashMap<ArcVarId, WholeVarBalance>,
    rebalanced_vars: &FxHashSet<ArcVarId>,
    remove: &mut [Vec<bool>],
) {
    mark_class_grain_whole_pair_removals_gated(
        *CLASS_GRAIN_PAIR_ELISION_DISABLED,
        func,
        state_map,
        balances,
        rebalanced_vars,
        remove,
    );
}

/// Toggle seam: `disabled = true` declines the whole pass (the
/// `ORI_DISABLE_CLASS_GRAIN_PAIR_ELISION=1` disposition), unit-pinned
/// independent of the process-cached `LazyLock` read.
pub(super) fn mark_class_grain_whole_pair_removals_gated(
    disabled: bool,
    func: &ArcFunction,
    state_map: &AimsStateMap,
    balances: &FxHashMap<ArcVarId, WholeVarBalance>,
    rebalanced_vars: &FxHashSet<ArcVarId>,
    remove: &mut [Vec<bool>],
) {
    if disabled {
        return;
    }
    let Some(partition) = state_map.birth_site_partition() else {
        return;
    };
    // rep_of/same_rep path-compress (&mut); the side table on the state map is
    // read-only after Step 4b (PL-5), so resolve reps on a local clone.
    let mut partition = partition.clone();

    // Whole-var partition node per var (field-path nodes are per-field grain;
    // class identity for a whole-var pair is the whole-var node's rep).
    let mut whole_node: FxHashMap<ArcVarId, _> = FxHashMap::default();
    for (var, path, node) in partition.nodes_snapshot() {
        if path.is_whole_var() {
            whole_node.insert(var, node);
        }
    }

    // MUTATE-tainted class reps: any member var feeding a COW-relevant
    // instruction taints its whole class (conservative — DP-5/DP-9 count
    // sibling references, so a keep-alive pair on a mutated class is
    // load-bearing).
    let mut mutate_tainted: FxHashSet<_> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            // Reuse recycling is tainted via its paired `Reset { var }` (the
            // recycled allocation); the Reset token itself is SCALAR (TF-10a)
            // and never carries a partition node.
            let base = match instr {
                ArcInstr::Set { base, .. } | ArcInstr::SetTag { base, .. } => *base,
                ArcInstr::IsShared { var, .. } | ArcInstr::Reset { var, .. } => *var,
                ArcInstr::CollectionReuse { old_var, .. } => *old_var,
                _ => continue,
            };
            if let Some(&node) = whole_node.get(&base) {
                mutate_tainted.insert(partition.rep_of(node));
            }
        }
    }

    let candidates = collect_candidates(
        func,
        &mut partition,
        &whole_node,
        &mutate_tainted,
        balances,
        rebalanced_vars,
        remove,
    );
    // Sibling evidence: a NON-candidate same-class var defined before the
    // span (same block, or a function param) whose KEPT whole-var dec sits
    // strictly after the span in the same block. Non-candidate siblings only
    // — no mutual justification.
    let params: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let candidate_vars: FxHashSet<ArcVarId> = candidates.keys().copied().collect();
    for (var, cand) in &candidates {
        let block = &func.blocks[cand.block];
        let evidence = balances.iter().any(|(sib, sib_bal)| {
            if sib == var || candidate_vars.contains(sib) {
                return false;
            }
            let Some(&sib_node) = whole_node.get(sib) else {
                return false;
            };
            if partition.rep_of(sib_node) != cand.rep {
                return false;
            }
            // Dominating birth: param, or defined in this block before the span.
            let born_before = params.contains(sib)
                || block.body[..cand.first_site]
                    .iter()
                    .any(|instr| instr.defined_var() == Some(*sib));
            if !born_before {
                return false;
            }
            // +1-establishment (T3's dominating-inc): the sibling's NET kept
            // contribution at span entry must be >= 1. Birth counts +1 for a
            // param or an owning (non-alias) def; each kept inc before the
            // span adds +1; each kept dec before the span subtracts 1 — a
            // sibling that both funded AND released before the span may
            // enter it at 0, and a later kept dec alone is no evidence.
            let birth: i64 = if params.contains(sib) {
                1
            } else if sib_bal.inc_sites.is_empty() {
                // No dup incs: the before-span def IS the allocation birth.
                1
            } else {
                // Alias-shaped sibling (its +1s are its dup incs).
                0
            };
            let incs_before: i64 = sib_bal
                .inc_sites
                .iter()
                .filter(|&&(b, i)| b == cand.block && i < cand.first_site && !remove[b][i])
                .count() as i64;
            let decs_before: i64 = sib_bal
                .dec_sites
                .iter()
                .filter(|&&(b, i)| b == cand.block && i < cand.first_site && !remove[b][i])
                .count() as i64;
            if birth + incs_before - decs_before < 1 {
                return false;
            }
            // T3's live-across-the-span premise: the sibling holds the
            // interior count >= 1 for the WHOLE span — any kept release
            // landing inside the span (a multi-release sibling) breaks it,
            // so the evidence demands: no kept dec within the span, and a
            // kept plain whole-var dec strictly after it (same block; a
            // single basic block executes straight-line, so no other
            // block's dec can interleave).
            let released_within_span = sib_bal.dec_sites.iter().any(|&(b, i)| {
                b == cand.block && i >= cand.first_site && i <= cand.last_site && !remove[b][i]
            });
            if released_within_span {
                return false;
            }
            sib_bal.dec_sites.iter().any(|&site| {
                let (b, i) = site;
                b == cand.block
                    && i > cand.last_site
                    && !remove[b][i]
                    && is_plain_burden_dec(func, site)
            })
        });
        if evidence {
            let balance = &balances[var];
            for &(b, i) in balance.inc_sites.iter().chain(&balance.dec_sites) {
                remove[b][i] = true;
            }
            tracing::debug!(
                target: "ori_arc::aims::realize",
                ?var,
                "class-grain whole-pair elided (T3 sibling-liveness evidence)"
            );
        }
    }
}

/// A site is a plain whole-var `BurdenDec` — NOT a `BurdenDecPartial` /
/// `BurdenDecVariant` slice drop (T3 reasons over whole-var releases only).
fn is_plain_burden_dec(func: &ArcFunction, (block, idx): (usize, usize)) -> bool {
    matches!(
        func.blocks[block].body.get(idx),
        Some(ArcInstr::BurdenDec { .. })
    )
}

/// A kept whole-var pair eligible for class-grain elision: single-block span
/// plus its allocation-class rep.
struct Candidate {
    block: usize,
    first_site: usize,
    last_site: usize,
    rep: NodeIdx,
}

/// Collect candidate pairs: kept (un-removed) whole-var inc+dec brackets,
/// single block, plain-`BurdenDec`-only, non-rebalanced, class-known, class
/// not mutate-tainted.
fn collect_candidates(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    whole_node: &FxHashMap<ArcVarId, NodeIdx>,
    mutate_tainted: &FxHashSet<NodeIdx>,
    balances: &FxHashMap<ArcVarId, WholeVarBalance>,
    rebalanced_vars: &FxHashSet<ArcVarId>,
    remove: &[Vec<bool>],
) -> FxHashMap<ArcVarId, Candidate> {
    let mut candidates: FxHashMap<ArcVarId, Candidate> = FxHashMap::default();
    for (var, balance) in balances {
        if rebalanced_vars.contains(var) {
            continue;
        }
        // A pair-atomic ALIAS dst is admissible ONLY as a COMPLETE balanced
        // pair (inc AND dec both present + kept): T3 licenses removing a
        // balanced keep-alive bracket WHOLE under root-liveness evidence —
        // the root IS the same-class sibling. An inc-ONLY alias (its dec
        // transfer-suppressed at Phase 5; the inc funds a consumer's
        // cross-var release) has empty dec_sites and is excluded by the
        // pair-completeness gate below, never elided (RL-1
        // `RL1_duplication_balanced`).
        if balance.inc_sites.is_empty() || balance.dec_sites.is_empty() {
            continue;
        }
        // T3 is a WHOLE-VAR pair theorem: dec_sites upstream folds
        // `BurdenDecPartial`/`BurdenDecVariant` alongside plain `BurdenDec`;
        // a partial/variant drop releases only a slice of the allocation, so
        // a bracket containing one is outside the whole-pair proof and never
        // admitted.
        if balance
            .dec_sites
            .iter()
            .any(|&site| !is_plain_burden_dec(func, site))
        {
            continue;
        }
        let sites: Vec<_> = balance
            .inc_sites
            .iter()
            .chain(&balance.dec_sites)
            .copied()
            .collect();
        if sites.iter().any(|&(b, i)| remove[b][i]) {
            continue; // another pass already owns part of this var's ops
        }
        let block = sites[0].0;
        if sites.iter().any(|&(b, _)| b != block) {
            continue; // v1: same-block spans only
        }
        let Some(&node) = whole_node.get(var) else {
            continue;
        };
        let rep = partition.rep_of(node);
        if mutate_tainted.contains(&rep) {
            continue;
        }
        let first_site = sites.iter().map(|&(_, i)| i).min().unwrap_or(0);
        let last_site = sites.iter().map(|&(_, i)| i).max().unwrap_or(0);
        candidates.insert(
            *var,
            Candidate {
                block,
                first_site,
                last_site,
                rep,
            },
        );
    }
    candidates
}
