//! Per-same-alloc-rep lineage re-balance — the cross-var release-exactly-once
//! pass that the per-var DP-2/DP-3 elimination (`burden_elim.rs`) cannot
//! express.
//!
//! Groups burden ops by `same_alloc_reps` rep and decides, per rep, whether
//! removal alone can re-balance the lineage.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::intraprocedural::state_map::ApplyAliasSource;
use crate::aims::intraprocedural::AimsStateMap;
use crate::aims::lattice::dimensions::AccessClass;
use crate::aims::verify::burden_delta::{compute_burden_entry_nets, whole_var_dec_target};
use crate::graph::{compute_predecessors, forward_reachable, DominatorTree};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};

use super::LINEAGE_REBALANCE_DISABLED;
use super::SINGLE_RELEASE_AFTER_LAST_READ_DISABLED;

/// A whole-var burden op site located across the function: `(block_idx,
/// instr_idx)` of the instruction. Same as `OpSite` (in `burden_elim.rs`) but
/// named for the lineage-rebalance pass which keys ops by their lineage rep,
/// not their var.
type LineageOp = (usize, usize);

/// Whole-var burden inc/dec op sites of ONE same-alloc lineage rep, plus the
/// rep's fresh-self-alloc block footprint + loop membership — the grouping the
/// lineage re-balance decides removals over.
#[derive(Default)]
struct RepOps {
    inc_sites: Vec<LineageOp>,
    dec_sites: Vec<LineageOp>,
    /// Distinct vars that carry a whole-var burden op (inc OR dec) on this
    /// lineage — the alias-chain signature is ≥2.
    op_vars: FxHashSet<ArcVarId>,
    all_vars: FxHashSet<ArcVarId>,
    /// Per-block fresh-self-alloc count (block index → number of lineage
    /// allocations in that block). The `+1`s of the post-re-balance net.
    alloc_counts: FxHashMap<usize, i64>,
    touches_loop: bool,
}

/// Group every whole-var burden op (inc / dec) + fresh self-allocation by its
/// `same_alloc_reps` rep into [`RepOps`]. The single grouping pass the lineage
/// re-balance decides over; `loop_blocks` taints a rep touching any loop block.
fn group_lineage_ops(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    list_take_name: ori_ir::Name,
    loop_blocks: &FxHashSet<usize>,
) -> FxHashMap<ArcVarId, RepOps> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashMap<ArcVarId, RepOps> = FxHashMap::default();
    for (b, block) in func.blocks.iter().enumerate() {
        let in_loop = loop_blocks.contains(&b);
        for (i, instr) in block.body.iter().enumerate() {
            if let Some(dst) = super::super::emit_unified::fresh_rc_alloc_dst(
                instr,
                func,
                interner,
                list_take_name,
            ) {
                let entry = reps.entry(rep_of(dst)).or_default();
                *entry.alloc_counts.entry(b).or_insert(0) += 1;
                entry.all_vars.insert(dst);
                entry.touches_loop |= in_loop;
            }
            match instr {
                ArcInstr::BurdenInc { var } => {
                    let entry = reps.entry(rep_of(*var)).or_default();
                    entry.inc_sites.push((b, i));
                    entry.op_vars.insert(*var);
                    entry.all_vars.insert(*var);
                    entry.touches_loop |= in_loop;
                }
                _ => {
                    if let Some(var) = whole_var_dec_target(instr) {
                        let entry = reps.entry(rep_of(var)).or_default();
                        entry.dec_sites.push((b, i));
                        entry.op_vars.insert(var);
                        entry.all_vars.insert(var);
                        entry.touches_loop |= in_loop;
                    }
                }
            }
        }
        // An `Invoke`-terminator self-allocating builtin result FIRST lives at the
        // `normal` successor block (where Phase-5 prepends its fresh-site
        // `BurdenInc`); attribute the lineage alloc to that successor so the
        // re-balance net counts the allocation on the path the result is defined.
        // Spec: Annex E §AIMS RL-2.
        if let Some((dst, normal)) = super::super::emit_unified::fresh_rc_alloc_dst_terminator(
            &block.terminator,
            func,
            interner,
        ) {
            let nb = normal.index();
            let entry = reps.entry(rep_of(dst)).or_default();
            *entry.alloc_counts.entry(nb).or_insert(0) += 1;
            entry.all_vars.insert(dst);
            entry.touches_loop |= loop_blocks.contains(&nb);
        }
    }
    reps
}

/// Per-same-alloc-rep lineage re-balance — the cross-var release-exactly-once
/// pass that the per-var DP-2/DP-3 elimination cannot express.
///
/// For an alias chain (`let b = a; a == b`), Phase 5 emits BALANCED per-alias
/// inc/dec pairs on each Let-Var / borrow-operand alias of ONE allocation; the
/// per-var pass elides the borrow aliases' incs (DP-3: `Once+Linear`) but keeps
/// every dec (DP-2 fails: `Once`, not `Absent/Dead`), so the lineage's kept
/// incs/decs net to a non-zero per-path balance — a double-free on the alias
/// branch + leak on the dead branch. The per-var pass cannot see the cross-var
/// balance because it never receives `same_alloc_reps`.
///
/// This pass groups burden ops by `same_alloc_reps` rep and, for each rep it
/// can re-balance by removal ALONE, marks: elide ALL incs (the allocation
/// supplies the lineage's +1 — every alias inc is spurious) + keep EXACTLY ONE
/// dec that releases the allocation on EVERY alloc-reachable terminal path
/// (RL-2 `RL2_release_exactly_once`: a value allocated at RC = 1 nets 0 by one
/// release), eliding every other dec. The kept dec is verification-selected:
/// only a dec whose retention drives the lineage's per-path terminal net to 0
/// (via `compute_burden_entry_nets`) is accepted.
///
/// SCOPE (conservative, non-loop only): a rep is re-balanced ONLY when ALL hold,
/// else the per-var pass owns its vars unchanged:
/// - the rep has ≥1 fresh self-alloc member (it is a real allocation lineage);
/// - NO member's burden ops sit in a loop block — `same_alloc_reps` EXCLUDES the
///   Jump-phi back-edge by design, so a loop-carried value's release attributes
///   to a different rep and the per-path net mis-computes (the known blind spot
///   per the M-series dead-ends); loop-carried lineages defer to a later pass;
/// - eliding all incs + keeping exactly one dec yields a per-path terminal net
///   of 0 on every alloc-reachable terminal (else removal alone cannot balance
///   it — the missing release must be edge-emitted, out of scope here).
///
/// Returns the set of vars whose removals this pass owns; the caller's
/// per-var pass skips them. Spec: Annex E §AIMS RL-1 (alias inc spurious) +
/// RL-2 (one release).
pub(super) fn mark_lineage_rebalance_removals(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    remove: &mut [Vec<bool>],
) -> FxHashSet<ArcVarId> {
    let mut owned_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    // The lineage re-balance keeps exactly one dec as the RL-2 release. The
    // burden path is the sole real-RC emitter, so that kept dec is the correct
    // sole release. `ORI_DISABLE_LINEAGE_REBALANCE=1` is the bisection escape
    // hatch that defers every rep to the per-var pass.
    if *LINEAGE_REBALANCE_DISABLED {
        return owned_vars;
    }
    let list_take_name = super::super::emit_unified::for_yield_result_finalizer_name(interner);
    let preds = compute_predecessors(func);
    let dom = DominatorTree::build(func);
    let loop_blocks = compute_loop_blocks(func, &preds, &dom);

    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);

    // Forwarder-tainted reps partition into TWO classes via the callee
    // `ParamContract`/`ReturnContract` provenance:
    //
    // (1) TRANSFER-through-return forwarders (`ApplyAliasSource::Direct(arg)` whose
    //     callee param `transfers_through_return == true`, `@id<T>(x: T) -> T = x`):
    //     the callee OWN-CONSUMES the arg and transfers it back out (RL-34
    //     `transferOwnership`; RL-2 `Return` transfer terminal use), so the caller
    //     acquires the SAME allocation at the result `dst`. That acquisition IS the
    //     lineage `+1` (`transfer_forwarder_anchors` records the transfer block);
    //     the per-alias incs AND the Aggregate-result apply-result inc are spurious
    //     duplicates on top (RL-1: a single moved transfer, not a duplication). The
    //     rep IS re-balanceable: elide ALL incs + keep EXACTLY ONE POST-transfer
    //     release. The anchor at the transfer block makes the pre-transfer arg-side
    //     dec NON-forward-reachable, so `select_single_release_dec` never keeps it
    //     (keeping it frees the value before the `[own]` move — UAF).
    //
    // (2) Every OTHER apply-result alias — a `Project` borrow-view return (the
    //     callee returns `arg.field` it BORROWED), a `Conditional`/`Wrapped` shape,
    //     or a `Direct` whose contract does NOT prove `transfers_through_return` —
    //     stays EXCLUDED. Its result is not a uniform moved transfer of a single
    //     owned arg, so its result-inc is not a uniform spurious duplicate; eliding
    //     it under-counts the shared allocation → UAF. The per-var pass owns it.
    //
    // Spec: Annex E §AIMS RL-1 (genuine vs spurious inc) + RL-2 (release exactly
    // once) + RL-34 (tail-call ownership transfer).
    let transfer_forwarder_anchors = compute_transfer_forwarder_anchors(
        func,
        same_alloc_reps,
        state_map.apply_result_aliases(),
        contracts,
    );
    let forwarder_tainted_reps: FxHashSet<ArcVarId> = state_map
        .apply_result_aliases()
        .keys()
        .map(|&v| rep_of(v))
        .collect();

    let reps = group_lineage_ops(
        func,
        same_alloc_reps,
        interner,
        list_take_name,
        &loop_blocks,
    );

    // COW-mutated lineage reps DEFER to the per-var pass. The re-balance elides ALL
    // incs of a rep + keeps one release; for a lineage whose value flows into a
    // COW-mutation operand (`push`/`set`/`insert`/`remove`/`sort`/`reverse` at an
    // owned arg, collection `+`/concat, or a may-COW user-call arg) AND is re-read
    // after the consume, the fresh keep-alive inc is LOAD-BEARING (RL-1: raises the
    // runtime rc ≥ 2 so the `ori_rc_is_unique` COW protocol COPIES instead of
    // mutating in place). Eliding it lets the mutation hit a freed/aliased buffer →
    // double-free. The per-var pass (`classify_burden_ops` /
    // `mark_whole_var_removals`) is COW-aware (`inc_elidable_at_realization` /
    // `compute_elidable_fresh_self_alloc_incs`) and correctly KEEPS the inc. SSOT
    // detector reused — never duplicated. Spec: Annex E §AIMS RL-1.
    let cow_mutated_reps = super::super::emit_unified::compute_cow_mutated_lineage_reps(
        func,
        same_alloc_reps,
        interner,
        contracts,
    );

    for (rep, ops) in &reps {
        // A forwarder-tainted rep is un-excluded ONLY when it BOTH is a proven
        // owned-transfer-through-return forwarder AND has NO `fresh_rc_alloc_dst`
        // self-alloc — the AGGREGATE case where the transferred-in `Construct` is
        // not RcPtr/FatVal so Phase 5 KEEPS the spurious apply-result inc and the
        // lineage has no recognized `+1` (`box_list` / `option_list`). The
        // transfer anchor supplies that `+1` and the re-balance elides the spurious
        // inc + keeps one post-transfer release.
        //
        // An RcPtr/FatVal forwarder (`id<T>([int]) -> [int]`, or `borrow_if_use`
        // returning RcPtr lists) HAS a `fresh_rc_alloc_dst` self-alloc: Phase 5
        // already SUPPRESSED its apply-result inc (`compute_transfer_through_return
        // _results`, repr-gated to RcPtr/FatVal), so it was sound under the per-var
        // pass while EXCLUDED. Un-excluding it would let the self-alloc re-balance
        // fire on a multi-block forwarder lineage and mis-select the kept release
        // (a used-then-returned forwarder body corrupts the value) — keep it
        // excluded. Spec: Annex E §AIMS RL-1 + RL-2.
        let transfer_anchor_blocks = transfer_forwarder_anchors.get(rep);
        let is_aggregate_transfer_forwarder =
            transfer_anchor_blocks.is_some() && ops.alloc_counts.is_empty();
        let excluded_forwarder =
            forwarder_tainted_reps.contains(rep) && !is_aggregate_transfer_forwarder;
        // The lineage `+1` anchor: where the allocation is BORN owned in this rep.
        // A transfer forwarder THREADS an existing allocation through — it adds no
        // new `+1`. So the anchor is:
        //   - the `fresh_rc_alloc_dst` self-alloc block(s), when the rep has any
        //     (RcPtr/FatVal `Construct`/literal/collection-source — the canonical
        //     birth site). The forwarder transfer of such a value adds nothing.
        //   - ELSE, for an Aggregate transfer forwarder whose `Construct` is NOT
        //     `fresh_rc_alloc_dst`-recognized (repr-gated to RcPtr/FatVal), the
        //     transfer block(s) where the transferred-in value arrives owned at the
        //     result `dst` — the earliest point in the rep where the allocation is
        //     known to exist owned by the caller.
        // Using the self-alloc when present prevents a DOUBLE `+1` (born once +
        // transferred once) that would over-count an RcPtr forwarder's lineage.
        let mut alloc_delta: Vec<i64> = vec![0; func.blocks.len()];
        let alloc_blocks: Vec<usize> = if ops.alloc_counts.is_empty() {
            // Aggregate transfer forwarder: anchor `+1` at each transfer block
            // (one acquisition per block on every path through it).
            match transfer_anchor_blocks {
                Some(blocks) => {
                    for &b in blocks {
                        alloc_delta[b] = 1;
                    }
                    let mut v: Vec<usize> = blocks.clone();
                    v.sort_unstable();
                    v.dedup();
                    v
                }
                None => Vec::new(),
            }
        } else {
            for (&b, &c) in &ops.alloc_counts {
                alloc_delta[b] = c;
            }
            ops.alloc_counts.keys().copied().collect()
        };
        // Candidate gate: a loop-free lineage with a `+1` anchor whose burden ops
        // span ≥2 distinct SSA vars (the alias-chain / forwarder signature). A
        // single-var lineage is already handled by the per-var pass; a loop-carried
        // lineage's release attributes to a different rep (`same_alloc_reps`
        // excludes the back-edge) so its per-path net mis-computes; an excluded
        // forwarder's apply-result inc is not a uniform spurious duplicate; a
        // COW-mutated rep's fresh keep-alive inc is load-bearing (RL-1) and is
        // owned by the COW-aware per-var pass, not elidable here.
        if alloc_blocks.is_empty()
            || ops.touches_loop
            || ops.op_vars.len() < 2
            || excluded_forwarder
            || cow_mutated_reps.contains(rep)
        {
            continue;
        }
        // The SOLE discriminator: eliding ALL incs (alias-spurious by the rep's
        // same-allocation construction — every member shares the one rc, needs
        // no inc) + keeping EXACTLY ONE POST-anchor dec must drive the lineage's
        // per-path terminal net to 0 on every anchor-reachable terminal. Verified
        // per-path via `compute_burden_entry_nets` — never a flat op count (it
        // double-counts mutually-exclusive paths). A transferred lineage (alloc
        // returned / moved out → consumer decs, needs no local release) has NO
        // balancing single-dec and is rejected (the per-var pass keeps it). Spec:
        // Annex E §AIMS RL-1 (alias inc spurious) + RL-2 (release exactly once).
        let Some(kept_dec) = select_single_release_dec(
            func,
            &preds,
            &alloc_blocks,
            &alloc_delta,
            &ops.dec_sites,
            *rep,
            &rep_of,
        ) else {
            continue;
        };
        // Commit: elide all incs + every dec except the kept release.
        for &(b, i) in &ops.inc_sites {
            remove[b][i] = true;
        }
        for &(b, i) in &ops.dec_sites {
            if (b, i) != kept_dec {
                remove[b][i] = true;
            }
        }
        owned_vars.extend(ops.all_vars.iter().copied());

        if tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = interner.lookup(func.name),
                rep = rep.raw(),
                incs_elided = ops.inc_sites.len(),
                decs = ops.dec_sites.len(),
                kept_dec_block = kept_dec.0,
                "lineage re-balance: alias-chain release-exactly-once"
            );
        }
    }
    owned_vars
}

/// Per-rep transfer-through-return forwarder anchors: the rep of a generics
/// forwarder (`@id<T>(x: T) -> T = x`) → the block indices where the forwarder
/// RESULT is defined (the `Apply`/`Invoke` `dst`).
///
/// A forwarder unions its consumed arg and its result into ONE
/// `same_alloc_reps` rep via `ApplyAliasSource::Direct` (the callee returns the
/// param unchanged). The callee OWN-CONSUMES the arg and transfers it back out
/// (RL-34 `transferOwnership`: callee Owned param → no post-call dec; RL-2
/// `Return` is a transfer terminal use), so the caller acquires the SAME
/// allocation at the result `dst`. That acquisition IS the lineage's `+1` — the
/// transferred-in value arrives owned at `dst`, not at a fresh `Construct` (an
/// Aggregate forwarder result whose `Construct` `fresh_rc_alloc_dst` does not
/// count). The result-INC the Phase-5 walk emits for an Aggregate forwarder
/// result is a spurious duplicate ON TOP of that `+1` (RL-1: the value is moved
/// once through the forwarder, not duplicated), elidable by the re-balance.
///
/// Discriminator (the precise transfer-vs-alias-spurious distinction): the entry
/// is admitted ONLY for an `ApplyAliasSource::Direct(arg)` whose callee's
/// `ParamContract` for `arg`'s position has `transfers_through_return == true`
/// (the proven Return-flow transfer fact, `IcReturnContract` provenance). A
/// `Project` borrow-view return (the callee returns `arg.field` it borrowed) or
/// a `Conditional`/`Wrapped` shape is NOT admitted — its result is not a moved
/// transfer of a single owned arg, so its result-inc is not a uniform spurious
/// duplicate. Those reps stay excluded (the per-var pass owns them).
///
/// Spec: Annex E §AIMS RL-1 (genuine vs spurious inc) + RL-2 (release exactly
/// once) + RL-34 (tail-call ownership transfer).
fn compute_transfer_forwarder_anchors(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    apply_result_aliases: &FxHashMap<ArcVarId, ApplyAliasSource>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<ArcVarId, Vec<usize>> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    // arg-position → OWNED-transfer-through-return lookup for a callee's contract.
    //
    // BOTH conditions are required (the over-fire boundary):
    //   - `access == Owned`: the callee OWN-CONSUMES the param (RL-34
    //     `rl34_action .Owned = transferOwnership` — ownership moves through; no
    //     post-call dec). A `Borrowed` param is `decBeforeCall` (RL-34): the callee
    //     borrows a VIEW and returns it; the caller STILL OWNS the original, so the
    //     apply-result is a borrow alias (NOT a moved transfer-dup). A borrowed
    //     forwarder (`@borrow_if_use(xs: [int]) -> [int] = xs`, `xs` borrow-read
    //     then returned) sets `transfers_through_return` (the param flows to Return)
    //     yet keeps `access: Borrowed` — its rep must stay EXCLUDED; the per-var
    //     pass owns it. Without the `Owned` gate the re-balance over-fires on the
    //     borrowed-forwarder lineage.
    //   - `transfers_through_return`: the param flows to a `Return { value }`
    //     terminator (the proven `facts.return_flow` fact), so the caller acquires
    //     the same allocation at the result `dst`.
    let arg_transfers_through_return = |callee: &Name, arg: ArcVarId, args: &[ArcVarId]| -> bool {
        let Some(contract) = contracts.get(callee) else {
            return false;
        };
        args.iter()
            .zip(contract.params.iter())
            .any(|(&a, p)| a == arg && p.transfers_through_return && p.access == AccessClass::Owned)
    };
    // The anchor block is where the forwarder RESULT `dst` is first LIVE OWNED:
    //   - `Apply` instruction: `dst` is bound in the SAME block (anchor = that
    //     block).
    //   - `Invoke` TERMINATOR: `dst` is bound on the NORMAL-successor edge — it is
    //     NOT live in the Invoke's own block (the unwind edge does not bind it).
    //     Anchor = the normal successor. Anchoring at the Invoke's own block would
    //     make the pre-call arg-side dec forward-reachable (the Invoke block
    //     dominates everything), so the `select_single_release_dec` filter could
    //     keep that pre-transfer dec (UAF — it frees before the `[own]` move).
    let mut anchors: FxHashMap<ArcVarId, Vec<usize>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if let Some(ApplyAliasSource::Direct(arg)) = apply_result_aliases.get(dst) {
                    if arg_transfers_through_return(callee, *arg, args) {
                        // Apply result is live in its own block.
                        anchors
                            .entry(rep_of(*dst))
                            .or_default()
                            .push(block.id.index());
                    }
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            normal,
            ..
        } = &block.terminator
        {
            if let Some(ApplyAliasSource::Direct(arg)) = apply_result_aliases.get(dst) {
                if arg_transfers_through_return(callee, *arg, args) {
                    // Invoke result is bound on the normal-successor edge.
                    anchors
                        .entry(rep_of(*dst))
                        .or_default()
                        .push(normal.index());
                }
            }
        }
    }
    anchors
}

/// Select the ONE dec op site to keep as the lineage's RL-2 release: the dec
/// whose retention (with all incs + all OTHER decs elided) drives the lineage's
/// per-path terminal net to 0 on EVERY alloc-reachable terminal block, with no
/// merge disagreement. Returns `None` when no single dec satisfies this (the
/// lineage transfers out / needs a multi-branch release that removal alone
/// cannot supply — both defer to the per-var pass).
///
/// The kept release MUST be in a block forward-reachable from an alloc/transfer
/// anchor block. For a pure-alias-chain lineage every dec is post-alloc, so this
/// is a no-op. For a forwarder lineage anchored at the TRANSFER block (where the
/// caller acquires the transferred-in value at the result `dst`), it excludes
/// the pre-transfer arg-side decs: a dec there releases the value BEFORE it is
/// moved into the forwarder `[own]` arg, freeing a reference about to be
/// transferred — a use-after-free even when the per-path net verifies 0. Those
/// decs are elided with the rest; only a post-transfer dec is the RL-2 release.
///
/// The delta passed to the per-path net dataflow models the post-re-balance
/// lineage exactly: ALL incs elided (the only `+1`s are the per-block allocation
/// counts in `alloc_delta`) + the single kept dec's `−1`.
///
/// Candidates are ordered alloc-block-first: a dec in the allocation's own block
/// fires on every path through it (every path from the allocation passes through
/// it), so it is the most likely single dominating release.
fn select_single_release_dec(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    alloc_blocks: &[usize],
    alloc_delta: &[i64],
    dec_sites: &[LineageOp],
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> Option<LineageOp> {
    let alloc_reachable = forward_reachable(func, alloc_blocks);
    // The kept release must be forward-reachable from an anchor block — a
    // pre-anchor dec (the pre-transfer arg-side dec of a forwarder) frees the
    // value before it is moved into the forwarder `[own]` arg (UAF). Filter
    // those out before selection; they are elided with the other decs.
    let mut candidates: Vec<LineageOp> = dec_sites
        .iter()
        .copied()
        .filter(|&(b, _)| alloc_reachable.contains(&b))
        .collect();
    // RL-2 release-exactly-once: the kept release MUST sit after the lineage's
    // LAST read. A dec followed by a borrow-read of the rep on a forward path
    // frees the allocation while a later block still reads it — a use-after-free
    // / double-free (the `copy[0]` … `copy[1]` yield-identity shape: an early
    // alias's dec in bb3 frees the list buffer the bb7 `@__index(alias)` still
    // reads). The terminal-net check below only validates per-path BALANCE, not
    // read-ordering; this filter removes the unsafe candidates before selection.
    // Disabled by `ORI_DISABLE_SINGLE_RELEASE_AFTER_LAST_READ=1`.
    if !*SINGLE_RELEASE_AFTER_LAST_READ_DISABLED {
        candidates.retain(|&(b, i)| !dec_precedes_rep_read(func, rep, rep_of, b, i));
    }
    // Ordering: alloc-block decs first, then the rest (stable within each group).
    candidates.sort_by_key(|&(b, _)| usize::from(!alloc_blocks.contains(&b)));

    for &keep in &candidates {
        let mut delta = alloc_delta.to_vec();
        delta[keep.0] -= 1;
        let nets = compute_burden_entry_nets(func, preds, &delta);
        // Convergence guard: the all-paths-net-zero check below reads
        // `entry_net`, authoritative only on a converged result. On
        // `!converged` (freeze-on-disagree / cap exhaustion) the nets are stale
        // and `disagree_blocks` may be empty, so a single-release placement
        // derived from them would be non-deterministic. Decline this candidate
        // — a missed elision is a leak surfaced by the verifier, never a UAF
        // from a wrong release (Spec: Annex E §AIMS RL-2).
        if !nets.converged {
            continue;
        }
        if !nets.disagree_blocks.is_empty() {
            continue;
        }
        let mut all_zero = true;
        for (b, block) in func.blocks.iter().enumerate() {
            let Some(eb) = nets.entry_net[b] else {
                continue;
            };
            if !alloc_reachable.contains(&b) {
                continue;
            }
            if !matches!(
                block.terminator,
                ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
            ) {
                continue;
            }
            if eb + delta[b] != 0 {
                all_zero = false;
                break;
            }
        }
        if all_zero {
            return Some(keep);
        }
    }
    None
}

/// True iff a borrow-read of `rep` is forward-reachable from the dec at
/// `(dec_block, dec_idx)` — i.e. keeping the release there would free the
/// allocation before a later read (UAF). Scans: (1) the same block AFTER
/// `dec_idx` (body instrs + terminator), and (2) every block forward-reachable
/// across the dec block's successor edges (full body + terminator). Reuses the
/// borrow-read-of-rep SSOT (`instr_borrow_reads_rep` / `terminator_borrow_reads_rep`)
/// so the read predicate never drifts. Spec: Annex E §AIMS RL-2.
fn dec_precedes_rep_read(
    func: &ArcFunction,
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    dec_block: usize,
    dec_idx: usize,
) -> bool {
    use super::super::emit_unified::{instr_borrow_reads_rep, terminator_borrow_reads_rep};
    let dec_blk = &func.blocks[dec_block];
    // (1) Same block, strictly after the dec position.
    for instr in dec_blk.body.iter().skip(dec_idx + 1) {
        if instr_borrow_reads_rep(instr, rep_of, rep) {
            return true;
        }
    }
    if terminator_borrow_reads_rep(&dec_blk.terminator, rep_of, rep) {
        return true;
    }
    // (2) Strictly-forward blocks (successors of the dec block, transitively;
    // the dec block itself is excluded — its post-dec slice is covered above).
    let succ_starts: Vec<usize> = crate::graph::successor_block_ids(&dec_blk.terminator)
        .iter()
        .map(|s| s.index())
        .collect();
    let reachable = forward_reachable(func, &succ_starts);
    for b in reachable {
        if b == dec_block {
            continue;
        }
        let block = &func.blocks[b];
        for instr in &block.body {
            if instr_borrow_reads_rep(instr, rep_of, rep) {
                return true;
            }
        }
        if terminator_borrow_reads_rep(&block.terminator, rep_of, rep) {
            return true;
        }
    }
    false
}

/// Blocks that lie inside a natural loop: a block is in a loop iff it is the
/// target of a back-edge `b → h` (an edge whose head `h` dominates its tail `b`)
/// OR it can reach the back-edge tail while dominated by the head. Computed as
/// the union of every back-edge's natural-loop body.
fn compute_loop_blocks(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    dom: &DominatorTree,
) -> FxHashSet<usize> {
    let mut loop_blocks: FxHashSet<usize> = FxHashSet::default();
    let n = func.blocks.len();
    for (b, block) in func.blocks.iter().enumerate() {
        let tail = ArcBlockId::new(u32::try_from(b).unwrap_or(u32::MAX));
        for h in crate::graph::successor_block_ids(&block.terminator) {
            // Back-edge: successor `h` dominates the current block `b`.
            if dom.dominates(h, tail) {
                // Natural-loop body of back-edge `b → h`: `h` plus every block
                // that reaches `b` without passing through `h` (the standard
                // backward-reachability from the tail, bounded by the header).
                loop_blocks.insert(h.index());
                loop_blocks.insert(b);
                let mut stack = vec![b];
                while let Some(x) = stack.pop() {
                    if x == h.index() {
                        continue;
                    }
                    for &p in &preds[x] {
                        if p < n && loop_blocks.insert(p) {
                            stack.push(p);
                        }
                    }
                }
            }
        }
    }
    loop_blocks
}
