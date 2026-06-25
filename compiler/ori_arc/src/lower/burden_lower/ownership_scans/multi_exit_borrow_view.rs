//! Multi-exit per-path borrow-view release scan (RL-1 + RL-2 + RL-4): a FRESH
//! closure (`Apply`/`Invoke` result, `Unique ∧ preserves_freshness`, non-
//! forwarder) consumed ONLY as `ApplyIndirect` borrow-views across a
//! short-circuit CFG. Places exactly ONE release per terminal path — an RL-2
//! dec after the execution-final borrow-read on each READING path, an RL-4 edge
//! dec on each SHORT-CIRCUIT-BYPASS path. Spec: Annex E §AIMS RL-1 + RL-2 + RL-4.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::Uniqueness;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::ForwarderReleasePos;

/// Result of [`compute_multi_exit_borrow_view_lineage`]: the same-alloc closure
/// to suppress (kills every borrow-view inc + the spurious pre-use source dec)
/// plus the per-terminal-path placed releases.
pub(in crate::lower::burden_lower) struct MultiExitBorrowViewLineage {
    /// Every var in an admitted borrow-view closure — removed from
    /// `owned_vars_needing_rc` so the only releases are the placed decs below.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, pos) -> [dec var]` — one whole-var `BurdenDec` per terminal
    /// path: `AfterInstr` past the final borrow-read on a READING path (on the
    /// read var), `BlockEntry` on a SHORT-CIRCUIT-BYPASS successor (on the
    /// root). Merged into the `forwarder_result_releases` emission surface.
    pub releases: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>>,
}

/// One admitted root's lineage facts, held until the pairwise-disjoint gate
/// runs over the full candidate set.
struct Candidate {
    root: ArcVarId,
    members: FxHashSet<ArcVarId>,
    releases: Vec<((usize, ForwarderReleasePos), ArcVarId)>,
}

/// RL-1 + RL-2 + RL-4 multi-exit treatment for the short-circuit closure-
/// borrow-view shape (`let p = make_gt(5); p(10) && !p(3)`).
///
/// A FRESH closure allocation (an `Apply`/`Invoke` result whose callee contract
/// reports `return_info` uniqueness Unique and `preserves_freshness` true and NO
/// `transfers_through_return` param — TF-7 + IC-4, a freshly-minted
/// `PartialApply`) read through `Let { Var }` aliases and consumed ONLY at
/// `ApplyIndirect` BORROW-receiver sites (pos 0, where `is_owned_position`
/// returns false) names ONE allocation across the whole closure. Per RL-1 the
/// borrow-view never duplicates, so every dup/keep-alive inc the per-var
/// multi-use cardinality proxy emits is spurious; per RL-2/RL-4 the allocation
/// owes EXACTLY ONE release per terminal path. The per-var split (DP-2/DP-3)
/// frees the lineage's first var at its last var-use — which on the reading path
/// is the `%dup = %root` alias-bind placed BEFORE the execution-final
/// `ApplyIndirect` read — a use-after-free + double-free (SIGABRT exit 134).
///
/// The cure removes the WHOLE closure from `owned_vars_needing_rc` (every inc +
/// the spurious pre-use source dec) and places per-terminal-path releases via
/// allocation-level RL-2/RL-4 liveness over the lineage:
///  - a block with a borrow-read whose lineage is DEAD at block exit gets a
///    whole-var `BurdenDec` AFTER its final borrow-read (`RL2_release_exactly_once`
///    — no UAF, the read completes first);
///  - a CFG edge `B -> S` where the lineage is live at `B`'s exit but dead at
///    `S`'s entry (and not a Jump-arg handoff) gets a whole-var `BurdenDec` at
///    `S`'s entry (`RL4_edge_dec_decision`). The edge dec targets the ROOT (it
///    dominates every block, so it is in scope at `S`).
///
/// Admission gates (ALL hold per closure; ANY failure declines the root and
/// keeps current behavior — the status-quo double-free is the migration floor,
/// never a regression introduced here):
///  (a) FRESH closure root: an `Apply` body dst OR `Invoke` terminator dst whose
///      callee contract has `return_info.uniqueness == Unique ∧
///      preserves_freshness == true` AND NO param `transfers_through_return` (a
///      forwarder/aliasing return is NOT fresh — proven-structural
///      same-alloc identity, not a use-count/type proxy).
///  (b) root in `owned_vars_needing_rc` (heap-carrying, RC-tracked) AND
///      `FatValue` repr (closure two-word value).
///  (c) vetted borrow-view closure ([`borrow_view_closure_vetted`]): grow over
///      `Let { Var }` aliases; EVERY use of every member is either an alias-edge
///      or an `ApplyIndirect` at the BORROWED receiver pos 0 — any owned-consume
///      position, `Set`/`SetTag`, `Construct`/`Reuse`/`PartialApply` capture,
///      `Project`, `Return`/`Jump`/`Branch`/`Switch`/`Invoke` terminator use, or
///      `Apply`/`Invoke`/`InvokeIndirect` arg declines (the compensated-lineage
///      trap: a closure with an owned use is the decoupled pair-atomic shape).
///  (d) at least one borrow-read exists (a closure never read is a different
///      shape).
///  (e) pairwise-DISJOINT closures: two admitted roots sharing any member
///      converge on one allocation web; each admission would place its own
///      releases, double-freeing the web. ALL roots of an overlapping group
///      decline (status quo preserved).
pub(in crate::lower::burden_lower) fn compute_multi_exit_borrow_view_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> MultiExitBorrowViewLineage {
    let mut out = MultiExitBorrowViewLineage {
        suppressed_lineage_vars: FxHashSet::default(),
        releases: FxHashMap::default(),
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut claimed_roots: FxHashSet<ArcVarId> = FxHashSet::default();

    for root in collect_fresh_closure_roots(func, contracts) {
        let decline = |gate: &str| {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = root.index(),
                gate,
                "multi-exit borrow-view root declined"
            );
        };
        // Gate (b): heap-carrying + not already claimed + FatValue closure repr.
        if !owned_vars_needing_rc.contains(&root)
            || claimed_roots.contains(&root)
            || !matches!(func.var_repr(root), Some(ValueRepr::FatValue))
        {
            decline("b:owned/claimed/repr");
            continue;
        }
        // Gate (c): vetted borrow-view closure.
        let Some(members) = borrow_view_closure_vetted(func, root) else {
            decline("c:closure-vet");
            continue;
        };
        // Gate (d): at least one borrow-read.
        if !members_have_borrow_read(func, &members) {
            decline("d:no-borrow-read");
            continue;
        }
        // RL-2 + RL-4 per-terminal-path placement over allocation liveness.
        let releases = place_per_path_releases(func, root, &members);
        if releases.is_empty() {
            decline("f:no-placement");
            continue;
        }
        claimed_roots.extend(members.iter().copied());
        candidates.push(Candidate {
            root,
            members,
            releases,
        });
    }

    // Gate (e): decline EVERY candidate whose closure overlaps another's.
    let overlapping: Vec<bool> = candidates
        .iter()
        .map(|c| {
            candidates
                .iter()
                .filter(|o| !std::ptr::eq(*o, c))
                .any(|o| !c.members.is_disjoint(&o.members))
        })
        .collect();
    for (cand, overlaps) in candidates.into_iter().zip(overlapping) {
        if overlaps {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = cand.root.index(),
                gate = "e:closure-overlap",
                "multi-exit borrow-view root declined"
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

/// Gate (a): candidate roots — `Apply` body dsts + `Invoke` terminator dsts
/// whose callee contract hands the caller a FRESH owned closure
/// (`return_info.uniqueness == Unique ∧ preserves_freshness`) without
/// transferring any arg through the return.
fn collect_fresh_closure_roots(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<ArcVarId> {
    let fresh_closure_callee = |callee: &Name| -> bool {
        contracts.get(callee).is_some_and(|c| {
            c.return_info.uniqueness == Uniqueness::Unique
                && c.return_info.preserves_freshness
                && !c.params.iter().any(|p| p.transfers_through_return)
        })
    };
    let mut roots: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if fresh_closure_callee(callee) {
                    roots.push(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if fresh_closure_callee(callee) {
                roots.push(*dst);
            }
        }
    }
    roots
}

/// Gate (c): grow the same-alloc closure from `root` over `Let { Var }` aliases,
/// then vet that every member use is an alias-edge or an `ApplyIndirect`
/// borrowed-receiver read. `None` on any vet failure.
fn borrow_view_closure_vetted(func: &ArcFunction, root: ArcVarId) -> Option<FxHashSet<ArcVarId>> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(root);
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
/// (`Let { Var(src) }`, `src ∈ members`) or an `ApplyIndirect` at the BORROWED
/// receiver position (pos 0). Any other appearance — owned-consume position,
/// store, capture, projection, terminator use, or call arg — declines.
fn member_uses_all_borrow_views(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let touches = instr.used_vars().iter().any(|v| members.contains(v));
            if !touches {
                continue;
            }
            match instr {
                // Alias-edge: the closure's own internal hop.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if members.contains(src) => {}
                // Borrow-view read: the member MUST be the closure receiver
                // (pos 0, borrowed) and MUST NOT appear as an owned arg.
                ArcInstr::ApplyIndirect { closure, args, .. } => {
                    if !members.contains(closure) {
                        return false;
                    }
                    if args.iter().any(|a| members.contains(a)) {
                        return false;
                    }
                }
                // Any other instruction that touches a member is an owned
                // consume / store / capture / projection / call arg — decline.
                _ => return false,
            }
        }
        // A member at ANY terminator position is an escape (Return / Jump arg /
        // Branch / Switch / Invoke arg / InvokeIndirect) — decline.
        if block
            .terminator
            .used_vars()
            .iter()
            .any(|v| members.contains(v))
        {
            return false;
        }
    }
    true
}

/// Gate (d): true iff some member is read at an `ApplyIndirect` borrowed
/// receiver position.
fn members_have_borrow_read(func: &ArcFunction, members: &FxHashSet<ArcVarId>) -> bool {
    func.blocks.iter().any(|b| {
        b.body.iter().any(|instr| {
            matches!(instr, ArcInstr::ApplyIndirect { closure, .. } if members.contains(closure))
        })
    })
}

/// RL-2 + RL-4 placement over allocation-level liveness. A block carrying a
/// borrow-read whose lineage is dead at block exit gets a dec after its final
/// read (on the read var); a CFG edge crossing from live-out to dead-in gets an
/// edge dec at the successor entry (on the root).
fn place_per_path_releases(
    func: &ArcFunction,
    root: ArcVarId,
    members: &FxHashSet<ArcVarId>,
) -> Vec<((usize, ForwarderReleasePos), ArcVarId)> {
    // Multi-exit closure shape: the per-block final borrow-read is an
    // `ApplyIndirect` at the borrowed receiver (`closure ∈ members`). The
    // closure is vetted (`member_uses_all_borrow_views`) to never appear at a
    // terminator, so the terminator-read predicate yields `None` — byte-
    // identical to the pre-terminator-read placement core.
    place_per_path_releases_with(
        func,
        root,
        members,
        |instr, members| match instr {
            ArcInstr::ApplyIndirect { closure, .. } if members.contains(closure) => Some(*closure),
            _ => None,
        },
        |_term, _members| None,
    )
}

/// Per-path placement core, PARAMETERIZED over TWO final-borrow-read detectors:
/// `body_final_read(instr, members)` for a borrow-read at a BODY instruction,
/// and `term_final_read(terminator, members)` for a borrow-read at a
/// may-unwind TERMINATOR-`Invoke` (the accessor-retain shape:
/// `Invoke @unwrap(member [borrow])` / `Invoke @starts_with(member [borrow])`).
/// The `ApplyIndirect`-receiver consumer ([`place_per_path_releases`]) and the
/// accessor-retain consumer share this one allocation-liveness placement.
///
/// A BODY read places its RL-2 dec `AfterInstr` past the read (the read
/// completes first — no UAF). A TERMINATOR-`Invoke` read completes on the
/// NORMAL edge, so its RL-2 dec lands at the normal-successor `BlockEntry`
/// (the read is over once control reaches the normal successor); the dec
/// targets the root (it dominates every successor). The terminator read's
/// UNWIND edge is owned by the RL-4 live-out → dead-in edge logic + the
/// downstream Category-2 `deadAtSucc` conjunct — disjoint edges, no double
/// release. Spec: Annex E §AIMS RL-2 + RL-4.
pub(super) fn place_per_path_releases_with(
    func: &ArcFunction,
    root: ArcVarId,
    members: &FxHashSet<ArcVarId>,
    body_final_read: impl Fn(&ArcInstr, &FxHashSet<ArcVarId>) -> Option<ArcVarId>,
    term_final_read: impl Fn(&ArcTerminator, &FxHashSet<ArcVarId>) -> Option<ArcVarId>,
) -> Vec<((usize, ForwarderReleasePos), ArcVarId)> {
    let n = func.blocks.len();
    // Per-block: index of the final BODY borrow-read + the var read there (per
    // the `body_final_read` shape); whether the block births the root.
    let mut final_read: Vec<Option<(usize, ArcVarId)>> = vec![None; n];
    // Per-block: the NORMAL successor of a TERMINATOR-`Invoke` borrow-read (per
    // the `term_final_read` shape). The block is a lineage USE; its release
    // lands at the normal-successor `BlockEntry` (after the read completes).
    let mut term_read_normal_succ: Vec<Option<usize>> = vec![None; n];
    let mut births_root: Vec<bool> = vec![false; n];
    // The root's defining-`Invoke` UNWIND successor: the root is bound on the
    // NORMAL edge only, so on the unwind edge the allocation never existed (the
    // call threw before producing it). Excluding that edge from liveness +
    // placement keeps an edge dec off the unwind handler (a use-before-def).
    let mut root_def_unwind_succ: Option<usize> = None;
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if let Some(read_var) = body_final_read(instr, members) {
                final_read[b] = Some((i, read_var));
            }
            if matches!(instr, ArcInstr::Apply { dst, .. } if *dst == root) {
                births_root[b] = true;
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            normal,
            unwind,
            ..
        } = &block.terminator
        {
            // A terminator-`Invoke` reading a member (per `term_final_read`)
            // is a lineage USE whose RL-2 release lands at the normal succ.
            if term_final_read(&block.terminator, members).is_some() {
                term_read_normal_succ[b] = Some(normal.index());
            }
            if *dst == root {
                births_root[b] = true;
                root_def_unwind_succ = Some(unwind.index());
            }
        }
    }

    // Allocation-level liveness fixpoint: a block is a USE iff it carries a
    // body borrow-read OR a terminator-`Invoke` borrow-read; a block KILLS
    // upward liveness iff it births the root.
    let mut live_in = vec![false; n];
    let mut live_out = vec![false; n];
    let preds_succs: Vec<Vec<usize>> = func
        .blocks
        .iter()
        .map(|b| {
            let drop_unwind = matches!(
                &b.terminator,
                ArcTerminator::Invoke { dst, .. } if *dst == root
            )
            .then_some(root_def_unwind_succ)
            .flatten();
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .filter(|&s| s < n && Some(s) != drop_unwind)
                .collect()
        })
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..n).rev() {
            let lo = preds_succs[b].iter().any(|&s| live_in[s]);
            let li = final_read[b].is_some()
                || term_read_normal_succ[b].is_some()
                || (lo && !births_root[b]);
            if lo != live_out[b] || li != live_in[b] {
                live_out[b] = lo;
                live_in[b] = li;
                changed = true;
            }
        }
    }

    let mut placements: Vec<((usize, ForwarderReleasePos), ArcVarId)> = Vec::new();
    for b in 0..n {
        // RL-2 (body read): a borrow-read block dead at exit releases after its
        // final read.
        if let Some((idx, read_var)) = final_read[b] {
            if !live_out[b] {
                placements.push(((b, ForwarderReleasePos::AfterInstr(idx)), read_var));
            }
        }
        // RL-2 (terminator-`Invoke` read): the read completes on the normal
        // edge, so when the lineage is dead at the normal successor's entry the
        // release lands there (on the root). A live normal successor keeps the
        // lineage; the later reader owns the release. The unwind edge is handled
        // by the RL-4 conjunct below.
        if let Some(normal_succ) = term_read_normal_succ[b] {
            if normal_succ < n && !live_in[normal_succ] {
                placements.push(((normal_succ, ForwarderReleasePos::BlockEntry), root));
            }
        }
        // RL-4: a CFG edge crossing from live-out to dead-in releases at the
        // successor entry (on the root — it dominates every successor).
        if live_out[b] {
            for &s in &preds_succs[b] {
                if !live_in[s] {
                    placements.push(((s, ForwarderReleasePos::BlockEntry), root));
                }
            }
        }
    }
    // Exactly-once-per-(site, var): the terminator-read normal-successor RL-2
    // placement and the RL-4 dead-in edge placement can name the SAME
    // `(normal_succ, BlockEntry, root)` (a terminator-read block live-out on
    // one edge with a dead normal successor) — dedupe so one allocation is
    // released once per path (`RL2_release_exactly_once`).
    let mut seen: FxHashSet<(usize, ForwarderReleasePos, ArcVarId)> = FxHashSet::default();
    placements.retain(|&((b, pos), v)| seen.insert((b, pos, v)));
    placements
}
