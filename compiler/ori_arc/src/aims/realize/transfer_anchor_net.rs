//! Phase 6.99 — transfer-anchor credit net (RL-34 + RL-2 + RL-1).
//!
//! Per-rep alloc-aware NET over the result-side lineage of a PROVEN
//! forwarder call. Two anchor classes (see `anchors`):
//!
//! - **Direct** — `transfers_through_return ∧ Owned ∧ Direct`: the caller
//!   acquires the SAME allocation back at the result `dst` (`+1` on the
//!   NORMAL successor of the anchor call — RL-34 transfer, not a fresh
//!   birth).
//! - **Wrapped** — `return_payload_contains_param` proven on EVERY return
//!   path with a same-allocation wrapper result (`Ok(m)` / `Some(m)`): the
//!   returned wrapper carries exactly ONE reference on the payload's
//!   allocation (RL-1 borrowed-store mint or RL-2 owned transfer-through);
//!   the wrapper, its payload args, and any live EXTRACTION (`Project`
//!   destructure back out, TF-4 same allocation) form ONE coupled lineage.
//!
//! Each `[own]` arg is a `-1` hand-off (RL-2 ownership-transferring terminal
//! use), each fresh member definition is the allocation `+1` (wrapped reps
//! also admit a proven fresh-owned NON-anchor call result), and surviving
//! whole-var burden/RC ops count `±1`. The proven invariant
//! (`RL2_release_exactly_once` + `RL3_elision_net_preserving`): every
//! Return-reaching path nets EXACTLY 0, the running intra-block net stays
//! `>= 1` at every member use point (a value must be alive when read /
//! transferred / released), and unwind paths (`Resume` / `Unreachable`) net
//! `>= 0` (status-quo unwind leaks preserved; a negative = double-free,
//! never introduced).
//!
//! Three repair modes, strictly net-verified (when the net cannot be proven,
//! change NOTHING):
//! - **Removal**: a single surviving fresh-site keep-alive inc whose removal
//!   drives every Return terminal to 0 (the spurious RL-1 inc on a lineage
//!   whose only `+1` is the transferred-in credit).
//! - **Placement**: ONE whole-var `BurdenDec` after the lineage's
//!   execution-final value-read (RC ops are NOT value-reads per TF-11) when
//!   every Return terminal nets `+1` and the placed release drives all to 0.
//! - **Combined** (wrapped reps only): exactly ONE removal candidate whose
//!   removal PLUS the unique placed release drives every Return terminal to
//!   0 — the wrapped two-deficit shape (a spurious live-across inc AND the
//!   wrapper's carried credit both unreleased).
//!
//! Runs LAST in the burden-strip pipeline (after Phase 6.98) so every
//! normal-path repair pass computes against a byte-identical baseline.
//!
//! Toggles: `ORI_DISABLE_TRANSFER_ANCHOR_CREDIT_NET=1` bypasses the pass;
//! `ORI_DISABLE_WRAPPED_CREDIT_ANCHOR=1` declines the Wrapped class only.
//!
//! Spec: Annex E §AIMS RL-34 + RL-2 + RL-1 + RL-comp.

mod anchors;
mod model;
mod uses;
mod verify;
mod views;

use std::sync::LazyLock;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId, ValueRepr};

use super::emit_unified::compute_jump_threaded_reps;
use anchors::{collect_anchors, Anchor, AnchorKind, CreditSite};
use model::{build_lineage_model, FreshSiteInc, LineageModel};
use verify::{block_in_cycle, classify_model, final_value_read_sinks, Change, NetVerdict, PlaceAt};
use views::same_alloc_wrapper_type;

/// `ORI_DISABLE_TRANSFER_ANCHOR_CREDIT_NET=1` bypasses the Phase-6.99
/// transfer-anchor credit-net repair. Read once at first access.
static TRANSFER_ANCHOR_CREDIT_NET_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_TRANSFER_ANCHOR_CREDIT_NET").as_deref() == Ok("1"));

/// One net-verified repair on an anchored lineage.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CreditRepair {
    /// Remove the surviving fresh-site keep-alive inc at `blocks[block].body[instr_idx]`.
    RemoveInc { block: usize, instr_idx: usize },
    /// Append `BurdenDec { var }` at the END of `blocks[block].body`.
    PlaceDecEndOfBody { block: usize, var: ArcVarId },
    /// Prepend `BurdenDec { var }` at the FRONT of `blocks[block].body`.
    PlaceDecBlockFront { block: usize, var: ArcVarId },
}

/// Apply the Phase-6.99 transfer-anchor credit-net repairs to `func`.
pub(super) fn apply_transfer_anchor_credit_net(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if *TRANSFER_ANCHOR_CREDIT_NET_DISABLED {
        return;
    }
    let repairs =
        compute_transfer_anchor_credit_repairs(func, pool, interner, contracts, same_alloc_reps);
    // Removals first, per block in DESCENDING instr order (stable indices);
    // placements are index-independent (end-push / front-insert).
    let mut removals: Vec<(usize, usize)> = repairs
        .iter()
        .filter_map(|r| match r {
            CreditRepair::RemoveInc { block, instr_idx } => Some((*block, *instr_idx)),
            _ => None,
        })
        .collect();
    removals.sort_unstable_by(|a, b| b.cmp(a));
    for (b, i) in removals {
        if let Some(block) = func.blocks.get_mut(b) {
            if i < block.body.len() {
                block.body.remove(i);
            }
        }
    }
    for repair in &repairs {
        match repair {
            CreditRepair::PlaceDecEndOfBody { block, var } => {
                if let Some(b) = func.blocks.get_mut(*block) {
                    b.body.push(ArcInstr::BurdenDec { var: *var });
                }
            }
            CreditRepair::PlaceDecBlockFront { block, var } => {
                if let Some(b) = func.blocks.get_mut(*block) {
                    b.body.insert(0, ArcInstr::BurdenDec { var: *var });
                }
            }
            CreditRepair::RemoveInc { .. } => {}
        }
    }
}

/// Compute the net-verified repair plan. Pure; split out for direct unit pins.
pub(crate) fn compute_transfer_anchor_credit_repairs(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> Vec<CreditRepair> {
    let anchors = collect_anchors(func, pool, contracts);
    if anchors.is_empty() {
        return Vec::new();
    }

    // Lineage union: jump-threaded reps (Let-Var + Jump-arg + the committed
    // same-alloc seed) PLUS the anchor dst<->arg transfer edges. Local
    // read-only threading — `compute_same_alloc_reps` itself untouched.
    let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let mut edges: Vec<(ArcVarId, ArcVarId)> = jt_reps.iter().map(|(&m, &r)| (m, r)).collect();
    edges.sort_unstable();
    for (a, b) in edges {
        uf_union(&mut parent, a, b);
    }
    for anchor in &anchors {
        for &arg in &anchor.payload_args {
            uf_union(&mut parent, anchor.dst, arg);
        }
    }
    union_wrapped_extractions(func, pool, &anchors, &mut parent);
    let rep_of = |v: ArcVarId| uf_find_ro(&parent, v);

    // Group anchors by final rep, deterministically ordered.
    let mut by_rep: FxHashMap<ArcVarId, Vec<&Anchor>> = FxHashMap::default();
    for anchor in &anchors {
        by_rep.entry(rep_of(anchor.dst)).or_default().push(anchor);
    }
    let mut reps: Vec<ArcVarId> = by_rep.keys().copied().collect();
    reps.sort_unstable();

    let preds = crate::graph::compute_predecessors(func);
    let mut repairs: Vec<CreditRepair> = Vec::new();

    for rep in reps {
        let rep_anchors = &by_rep[&rep];
        let Some(model) = build_lineage_model(func, pool, contracts, &rep_of, rep, rep_anchors)
        else {
            trace_verdict(func, interner, rep, "declined-unmodeled-use");
            continue;
        };
        trace_model_events(func, interner, rep, &model);
        match classify_model(func, &preds, &model, Change::default()) {
            NetVerdict::Clean => {
                trace_verdict(func, interner, rep, "clean");
                continue;
            }
            NetVerdict::Unprovable => {
                trace_verdict(func, interner, rep, "declined-unprovable");
                continue;
            }
            NetVerdict::LeakOnly => {}
        }

        // Mode 1 — removal: exactly ONE fresh-site inc whose removal proves.
        let winners: Vec<&FreshSiteInc> = model
            .fresh_site_incs
            .iter()
            .filter(|cand| {
                let change = Change {
                    remove: Some((cand.block, cand.event_idx)),
                    place: None,
                    extra_places: [None; 3],
                };
                classify_model(func, &preds, &model, change) == NetVerdict::Clean
            })
            .collect();
        if winners.len() == 1 {
            trace_verdict(func, interner, rep, "removed-fresh-site-inc");
            repairs.push(CreditRepair::RemoveInc {
                block: winners[0].block,
                instr_idx: winners[0].instr_idx,
            });
            continue;
        }

        // Mode 2 — placement: a unique execution-final value-read sink.
        let (verdict, repair) = try_place_release(func, &preds, &model, None);
        if let Some(repair) = repair {
            trace_verdict(func, interner, rep, verdict);
            repairs.push(repair);
            continue;
        }

        // Mode 2b — multi-path placement: the read sink covers only its own
        // path; the transferred-in credit survives unread on sibling
        // branch/switch arms (RL-4 dead edges). Complement the sink release
        // with BlockFront releases at single-pred dead-frontier blocks,
        // jointly verified all-Return-paths-0.
        if rep_anchors.len() == 1 {
            if let Some(multi) = try_multi_path_placement(func, &preds, &model, rep_anchors[0]) {
                trace_verdict(func, interner, rep, "placed-multi-path-releases");
                repairs.extend(multi);
                continue;
            }
        }

        // Mode 3 — combined removal + placement (wrapped reps only).
        if rep_anchors.iter().any(|a| a.kind == AnchorKind::Wrapped) {
            if let Some(combined) = try_combined_repair(func, &preds, &model) {
                trace_verdict(func, interner, rep, "removed-inc-and-placed-release");
                repairs.extend(combined);
                continue;
            }
        }
        trace_verdict(func, interner, rep, verdict);
    }
    repairs
}

/// Union the same-allocation EXTRACTIONS of wrapped-anchor reps: a
/// non-scalar `Project` off a member whose type is a proven same-allocation
/// wrapper re-names the SAME allocation (TF-4) — for wrapped reps the
/// extracted payload (the `Ok(inner) -> inner` destructure) is a MEMBER of
/// the coupled lineage, not a single-block view. Fixpoint: an extraction
/// may itself be projected (nested wrappers).
fn union_wrapped_extractions(
    func: &ArcFunction,
    pool: &Pool,
    anchors: &[Anchor],
    parent: &mut FxHashMap<ArcVarId, ArcVarId>,
) {
    if !anchors.iter().any(|a| a.kind == AnchorKind::Wrapped) {
        return;
    }
    loop {
        let wrapped_roots: FxHashSet<ArcVarId> = anchors
            .iter()
            .filter(|a| a.kind == AnchorKind::Wrapped)
            .map(|a| uf_find_ro(parent, a.dst))
            .collect();
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                let ArcInstr::Project { dst, value, .. } = instr else {
                    continue;
                };
                if matches!(func.var_repr(*dst), Some(ValueRepr::Scalar)) {
                    continue;
                }
                if !wrapped_roots.contains(&uf_find_ro(parent, *value)) {
                    continue;
                }
                if !same_alloc_wrapper_type(func, *value, pool) {
                    continue;
                }
                if uf_find_ro(parent, *dst) != uf_find_ro(parent, *value) {
                    uf_union(parent, *dst, *value);
                    grew = true;
                }
            }
        }
        if !grew {
            return;
        }
    }
}

/// Mode-3 combined repair (the wrapped two-deficit shape): a removal AND
/// the unique placed release, jointly verified. Every winning combo is
/// INDEPENDENTLY proven (all Return paths net 0 + the aliveness walk under
/// the joint change), and the placed release is identical across combos
/// (the final-sink selector is forward-reachability-ordered and unique per
/// model) — combos differ only in WHICH equivalently proven inc is
/// removed, so the smallest `(block, instr_idx)` winner is the canonical
/// deterministic pick. Returns `None` when no combo proves.
fn try_combined_repair(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    model: &LineageModel,
) -> Option<[CreditRepair; 2]> {
    let mut combos: Vec<(&FreshSiteInc, CreditRepair)> = model
        .fresh_site_incs
        .iter()
        .filter_map(|cand| {
            let (v, r) = try_place_release(func, preds, model, Some((cand.block, cand.event_idx)));
            if v == "placed-release" {
                r.map(|r| (cand, r))
            } else {
                None
            }
        })
        .collect();
    combos.sort_by_key(|(c, _)| (c.block, c.instr_idx));
    combos.into_iter().next().map(|(winner, place)| {
        [
            CreditRepair::RemoveInc {
                block: winner.block,
                instr_idx: winner.instr_idx,
            },
            place,
        ]
    })
}

/// Placement mode: locate the unique execution-final value-read sink and
/// verify ONE whole-var release there (jointly with `base_remove`, the
/// Mode-3 combined removal) nets every path 0. Returns the trace verdict +
/// the placement repair (`None` repair = declined).
fn try_place_release(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    model: &LineageModel,
    base_remove: Option<(usize, usize)>,
) -> (&'static str, Option<CreditRepair>) {
    let (verdict, change, repair) = sink_placement_candidate(func, preds, model, base_remove);
    let (Some(change), Some(repair)) = (change, repair) else {
        return (verdict, None);
    };
    if classify_model(func, preds, model, change) == NetVerdict::Clean {
        ("placed-release", Some(repair))
    } else {
        ("declined-placement-unproven", None)
    }
}

/// The unique-sink placement candidate (unverified): the change + repair a
/// caller then classifies, or a decline verdict.
fn sink_placement_candidate(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    model: &LineageModel,
    base_remove: Option<(usize, usize)>,
) -> (&'static str, Option<Change>, Option<CreditRepair>) {
    let sinks = final_value_read_sinks(func, &model.read_blocks);
    if sinks.len() != 1 {
        return ("declined-sink-count", None, None);
    }
    let sink = sinks[0];
    if block_in_cycle(func, sink) {
        return ("declined-sink-in-cycle", None, None);
    }
    let read = &model.read_blocks[&sink];
    let (change, repair) = if let Some(var) = read.terminator {
        // Terminator borrowed-Invoke read: the value is read DURING the call —
        // release on the (single-pred) normal successor (RL-4).
        let crate::ir::ArcTerminator::Invoke { normal, .. } = &func.blocks[sink].terminator else {
            return ("declined-terminator-shape", None, None);
        };
        let succ = normal.index();
        if preds.get(succ).map(Vec::len) != Some(1) {
            return ("declined-multi-pred-successor", None, None);
        }
        (
            Change {
                remove: base_remove,
                place: Some((succ, PlaceAt::BlockFront)),
                extra_places: [None; 3],
            },
            CreditRepair::PlaceDecBlockFront { block: succ, var },
        )
    } else if read.view_terminator {
        // The sink's TERMINATOR reads a same-allocation VIEW: an end-of-body
        // release would free the allocation the terminator is about to read.
        return ("declined-view-terminator-read", None, None);
    } else {
        // A member body-read anchors the release directly; a view-only sink
        // anchors it on the admitted same-allocation VIEW instead (a
        // whole-var dec on the view releases the shared allocation — the
        // unified member+view ledger prices both at -1).
        let Some(var) = read.last_body.or(read.last_body_view) else {
            return ("declined-view-only-sink", None, None);
        };
        (
            Change {
                remove: base_remove,
                place: Some((sink, PlaceAt::EndOfBody)),
                extra_places: [None; 3],
            },
            CreditRepair::PlaceDecEndOfBody { block: sink, var },
        )
    };
    ("candidate", Some(change), Some(repair))
}

/// Mode 2b: the unique read sink's release + `BlockFront` releases at
/// single-pred DEAD-FRONTIER blocks (no read reachable, predecessor still
/// reaches one) where the transferred-in credit would otherwise survive to a
/// Return unread. The anchor dst names each frontier release (defined at the
/// anchor terminator, so it dominates every block the credit reaches);
/// dominance is verified against the anchor's normal successor. Jointly
/// verified all-Return-paths-0 before any repair is emitted.
fn try_multi_path_placement(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    model: &LineageModel,
    anchor: &Anchor,
) -> Option<Vec<CreditRepair>> {
    let (_, change, sink_repair) = sink_placement_candidate(func, preds, model, None);
    let (mut change, sink_repair) = (change?, sink_repair?);

    // can_reach_read[b]: some read block is forward-reachable from b.
    let n = func.blocks.len();
    let mut can_reach_read = vec![false; n];
    for &b in model.read_blocks.keys() {
        can_reach_read[b] = true;
    }
    loop {
        let mut grew = false;
        for (b, block) in func.blocks.iter().enumerate() {
            if can_reach_read[b] {
                continue;
            }
            let reaches = crate::graph::successor_block_ids(&block.terminator)
                .iter()
                .any(|s| can_reach_read[s.index()]);
            if reaches {
                can_reach_read[b] = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    // Frontier releases require the Invoke-anchor shape: `dst` is bound at
    // the NORMAL successor's entry, which then must dominate each frontier.
    let CreditSite::BlockEntry {
        block: anchor_normal,
    } = anchor.credit
    else {
        return None;
    };
    let dom = crate::graph::DominatorTree::build(func);
    let normal_id = crate::ir::ArcBlockId::new(u32::try_from(anchor_normal).ok()?);

    let mut frontier: Vec<usize> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        if can_reach_read[b]
            || matches!(
                block.terminator,
                crate::ir::ArcTerminator::Resume | crate::ir::ArcTerminator::Unreachable
            )
        {
            continue;
        }
        let [pred] = preds.get(b)?.as_slice() else {
            continue;
        };
        if !can_reach_read[*pred] {
            continue;
        }
        if !dom.dominates(
            normal_id,
            crate::ir::ArcBlockId::new(u32::try_from(b).ok()?),
        ) {
            continue;
        }
        frontier.push(b);
    }
    if frontier.is_empty() || frontier.len() > change.extra_places.len() {
        return None;
    }
    for (slot, &b) in change.extra_places.iter_mut().zip(frontier.iter()) {
        *slot = Some((b, PlaceAt::BlockFront));
    }
    if classify_model(func, preds, model, change) != NetVerdict::Clean {
        return None;
    }
    let mut repairs = vec![sink_repair];
    for &b in &frontier {
        repairs.push(CreditRepair::PlaceDecBlockFront {
            block: b,
            var: anchor.dst,
        });
    }
    Some(repairs)
}

fn trace_verdict(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    rep: ArcVarId,
    verdict: &'static str,
) {
    tracing::trace!(
        target: "ori_arc::aims::realize",
        fn_name = interner.lookup(func.name),
        rep = rep.raw(),
        verdict,
        "transfer-anchor credit-net verdict"
    );
}

/// Trace one rep's ordered per-block ledger events (`+N`/`-N` deltas; `r` =
/// aliveness-required value-read). Zero overhead when the target is disabled.
fn trace_model_events(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    rep: ArcVarId,
    model: &model::LineageModel,
) {
    if !tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        return;
    }
    let fmt = |events: &[model::Event]| -> String {
        events
            .iter()
            .map(|e| match (e.alive, e.delta) {
                (true, 0) => "r".to_owned(),
                (_, d) => format!("{d:+}"),
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    for (b, ev) in model.events.iter().enumerate() {
        if ev.entry_credit == 0 && ev.body.is_empty() && ev.term.is_empty() {
            continue;
        }
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = interner.lookup(func.name),
            rep = rep.raw(),
            block = b,
            entry_credit = ev.entry_credit,
            body = fmt(&ev.body),
            term = fmt(&ev.term),
            "transfer-anchor credit-net ledger"
        );
    }
}

fn uf_find_ro(parent: &FxHashMap<ArcVarId, ArcVarId>, v: ArcVarId) -> ArcVarId {
    let mut cur = v;
    loop {
        let p = *parent.get(&cur).unwrap_or(&cur);
        if p == cur {
            return cur;
        }
        cur = p;
    }
}

fn uf_union(parent: &mut FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) {
    let ra = uf_find_ro(parent, a);
    let rb = uf_find_ro(parent, b);
    if ra != rb {
        parent.insert(ra, rb);
    }
}

#[cfg(test)]
mod tests;
