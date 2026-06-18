//! Backward-liveness dataflow feeding `emit_burden_ops`: per-block live-out
//! sets restricted to `owned_vars_needing_rc`, plus the per-block gen/kill
//! inputs the fixpoint consumes.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};
use crate::ownership::Ownership;

/// Compute per-block live-out sets restricted to `owned_vars_needing_rc`.
///
/// Standard backward liveness (`live_out(B) = ∪ live_in(S)` over successors `S`;
/// `live_in(B) = gen(B) ∪ (live_out(B) − kill(B))`), filtered to vars that carry
/// an owned-heap burden. Mirrors `crate::liveness::compute_liveness`'s gen/kill
/// shape (an `Invoke` `dst` is a definition at its `normal` successor's entry,
/// like a block param) but is keyed on the burden walk's own
/// `owned_vars_needing_rc` set rather than the `ArcClassification` `needs_rc`
/// predicate — no parallel ownership tracker (AIMS Invariant 5): the set is the
/// burden walk's existing owned-RC classification.
///
/// Consumed by `emit_last_use_decs` + `emit_terminator_burden_decs` to suppress
/// the in-block last-use `BurdenDec` for a var live-out of the block per
/// `Spec: Annex E §AIMS RL-4` (the dec belongs on the dying CFG edge / at the
/// dead-out block, not unconditionally in a block the value outlives).
pub(in crate::lower::burden_lower) fn compute_live_out_owned(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> Vec<FxHashSet<ArcVarId>> {
    compute_owned_liveness(func, owned_vars_needing_rc).live_out
}

/// Per-block backward-liveness result restricted to `owned_vars_needing_rc`:
/// both the `live_in` (used before any def in the block, or in a reachable
/// successor) and `live_out` (live on entry to some successor) sets, indexed by
/// block index. `compute_live_out_owned` projects out `live_out`;
/// `compute_dead_owned_param_branch_releases` consumes BOTH for the RL-4
/// per-edge dead-param release.
pub(in crate::lower::burden_lower) struct OwnedLiveness {
    pub(in crate::lower::burden_lower) live_in: Vec<FxHashSet<ArcVarId>>,
    pub(in crate::lower::burden_lower) live_out: Vec<FxHashSet<ArcVarId>>,
}

/// Run the owned-var backward-liveness fixpoint and return both `live_in` and
/// `live_out` per block. SSOT for the burden walk's owned-liveness; consumed by
/// `compute_live_out_owned` (RL-4 live-out suppression) and
/// `compute_dead_owned_param_branch_releases` (RL-4 dead-param release).
pub(in crate::lower::burden_lower) fn compute_owned_liveness(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> OwnedLiveness {
    let n = func.blocks.len();
    let (gen, kill) = compute_owned_gen_kill(func, owned_vars_needing_rc);

    // Fixed-point backward dataflow: `live_out(B) = ∪ live_in(S)`,
    // `live_in(B) = gen(B) ∪ (live_out(B) − kill(B))`.
    let mut live_in: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut live_out: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for b in (0..n).rev() {
            let mut new_out: FxHashSet<ArcVarId> = FxHashSet::default();
            for succ in crate::graph::successor_block_ids(&func.blocks[b].terminator) {
                let si = succ.index();
                if si < n {
                    new_out.extend(live_in[si].iter().copied());
                }
            }
            let mut new_in = gen[b].clone();
            for &var in &new_out {
                if !kill[b].contains(&var) {
                    new_in.insert(var);
                }
            }
            if new_in != live_in[b] || new_out != live_out[b] {
                changed = true;
                live_in[b] = new_in;
                live_out[b] = new_out;
            }
        }
    }
    OwnedLiveness { live_in, live_out }
}

/// Per-block `(gen, kill)` sets for `compute_live_out_owned`, restricted to
/// `owned_vars_needing_rc`. `gen` = vars used before any definition in the
/// block; `kill` = vars defined in the block (incl. block params + the
/// `Invoke` `dst` bound at the normal-successor entry).
fn compute_owned_gen_kill(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> (Vec<FxHashSet<ArcVarId>>, Vec<FxHashSet<ArcVarId>>) {
    let n = func.blocks.len();
    let invoke_defs = crate::graph::collect_invoke_defs(func);
    let mut gen: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    let mut kill: Vec<FxHashSet<ArcVarId>> = vec![FxHashSet::default(); n];
    for (b, block) in func.blocks.iter().enumerate() {
        let g = &mut gen[b];
        let k = &mut kill[b];
        for &(param_var, _) in &block.params {
            if owned_vars_needing_rc.contains(&param_var) {
                k.insert(param_var);
            }
        }
        if let Some(dsts) = invoke_defs.get(&block.id) {
            for &dst in dsts {
                if owned_vars_needing_rc.contains(&dst) {
                    k.insert(dst);
                }
            }
        }
        for instr in &block.body {
            for var in instr.used_vars() {
                if owned_vars_needing_rc.contains(&var) && !k.contains(&var) {
                    g.insert(var);
                }
            }
            if let Some(dst) = instr.defined_var() {
                if owned_vars_needing_rc.contains(&dst) {
                    k.insert(dst);
                }
            }
        }
        for var in block.terminator.used_vars() {
            if owned_vars_needing_rc.contains(&var) && !k.contains(&var) {
                g.insert(var);
            }
        }
    }
    (gen, kill)
}

/// Per-block RL-4 dead-at-entry releases for Owned non-scalar FUNCTION-params that die
/// crossing a CFG edge without transfer.
///
/// Shape: a callee with multiple Owned non-scalar params returns ONE of them
/// path-sensitively (`triple<T>(c, x, y, z) -> if c == 0 then x else if c == 1 then y else
/// z`). On the branch returning `x`, the non-returned params `y` and `z` are DEAD — never
/// used, never returned, never transferred on that path. The Phase-5 walk emits no release
/// for them (their only last-use is a transfer-return on a DIFFERENT branch, RL-2
/// transfer-suppressed there), so they leak (RL-4 is violated: a value live at a block's
/// exit but dead at a successor's entry, not handed off, needs an edge dec).
///
/// RL-4 per-edge model (`AimsProof.Realization::RL4_edge_dec_decision` +
/// `RL4_edge_release_balanced`): a param `p` dying across the `pred → B` edge gets EXACTLY
/// ONE release — emitted at `B`'s entry. The release lands when ALL of:
///  - `p` is a FUNCTION param with `Ownership::Owned` (a caller-owned RC=1 reference);
///  - `p ∈ owned_vars_needing_rc` (heap-RC-carrying; scalars excluded per DP-1 / VF-1);
///  - `p ∈ live_out(pred)` AND `p ∉ live_in(B)` — `p` was live entering `B` and is dead in
///    `B`'s entire forward sub-CFG (the dead-at-entry condition);
///  - `p` is NOT transferred on the `pred → B` edge: not a `Jump` arg (RL-4 Jump-arg
///    exemption — ownership transfers to the successor block-param) and not an owned arg /
///    `dst` of an `Invoke` terminator targeting `B`.
///
/// Single-predecessor gate (the over-fire boundary): `B` is admitted ONLY when it has
/// EXACTLY ONE predecessor. Block-entry placement equals the `pred → B` edge release iff
/// `B` is single-pred; for a merge block (`p` live on one incoming edge, dead on another)
/// a block-entry dec would fire on the live edge too → double-free. When uncertain
/// (multi-pred) the release is NOT emitted — under-emission preserves the current leak (no
/// regression); over-emission is a double-free. The `triple` family's per-branch leaf
/// blocks are all single-pred, so the gate is exact there.
///
/// Distinct from `compute_dead_forwarder_block_param_releases` / the construct-fed pass
/// (which release dead BLOCK-params reached via Jump args from a forwarder/Construct): this
/// releases a FUNCTION-param dead on a branch it never reaches a merge through. The
/// `p ∈ live_out(pred) ∧ p ∉ live_in(B)` test is the edge-deadness; the function-param +
/// Owned gate bounds it to caller-owned params. Spec: Annex E §AIMS RL-4 + RL-2.
pub(in crate::lower::burden_lower) fn compute_dead_owned_param_branch_releases(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<usize, Vec<ArcVarId>> {
    // Owned non-scalar function params that carry heap RC — release candidates.
    let mut candidates: Vec<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Owned && owned_vars_needing_rc.contains(&p.var))
        .map(|p| p.var)
        .collect();
    // RL-4 edge-deadness ALSO governs a FRESH `Construct` owned non-scalar LOCAL
    // dead crossing a branch edge (`if c then p1.first else p2.first` — the
    // UNTAKEN parent aggregate dies on its own merge edge with no release).
    // Reuses the SAME single-pred + `live_out(pred) ∧ ¬live_in(B)` edge-deadness
    // + Jump-transfer-exemption gates as the param scan. Disjoint from the
    // construct-fed-dead-param family (Jump-arg-threaded, exempted by
    // `transferred_to_block`) and `branch_release` (funded-duplicate consume).
    // Gated by `ORI_DISABLE_FRESH_CONSTRUCT_DEAD_BRANCH_RELEASE=1`. Spec: Annex
    // E §AIMS RL-4 (`RL4_edge_dec_decision`, P1) + RL-2.
    if std::env::var("ORI_DISABLE_FRESH_CONSTRUCT_DEAD_BRANCH_RELEASE").as_deref() != Ok("1") {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Construct { dst, .. } = instr {
                    if owned_vars_needing_rc.contains(dst)
                        && !matches!(func.var_repr(*dst), Some(ValueRepr::Scalar))
                    {
                        candidates.push(*dst);
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return FxHashMap::default();
    }

    let liveness = compute_owned_liveness(func, owned_vars_needing_rc);
    let n = func.blocks.len();

    // Single-predecessor map: block index -> its sole predecessor index (None when 0 or
    // >=2 preds). Built from successor edges so the entry block (0 preds) and every merge
    // block (>=2 preds) map to None and are skipped.
    let mut pred_count: Vec<u32> = vec![0; n];
    let mut sole_pred: Vec<Option<usize>> = vec![None; n];
    for (b, block) in func.blocks.iter().enumerate() {
        for succ in crate::graph::successor_block_ids(&block.terminator) {
            let si = succ.index();
            if si < n {
                pred_count[si] += 1;
                sole_pred[si] = Some(b);
            }
        }
    }

    let mut out: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    for b in 0..n {
        // Single-predecessor gate: block-entry placement == the `pred -> b` edge release.
        if pred_count[b] != 1 {
            continue;
        }
        let Some(pred) = sole_pred[b] else {
            continue;
        };
        let transferred = transferred_to_block(&func.blocks[pred].terminator, func.blocks[b].id);
        for &p in &candidates {
            // Edge-deadness: live entering `b`, dead in `b`'s forward sub-CFG.
            if !liveness.live_out[pred].contains(&p) {
                continue;
            }
            if liveness.live_in[b].contains(&p) {
                continue;
            }
            // RL-4 transfer exemption: a Jump-arg / owned-Invoke-arg on the edge hands
            // ownership to the successor — no edge dec.
            if transferred.contains(&p) {
                continue;
            }
            out.entry(b).or_default().push(p);
        }
    }
    out
}

/// Vars transferred from `pred`'s terminator to the successor block `target` — the RL-4
/// Jump-arg exemption set. A `Jump`/`Branch`/`Switch`/`Invoke` whose terminator carries
/// `target` as a successor and passes a var as a `Jump` arg (ownership handoff to the
/// successor block-param) or an owned `Invoke`/`InvokeIndirect` arg / `dst` is excluded
/// from the dead-param release on the `pred -> target` edge. Conservative: any var at any
/// owned terminator position is treated as transferred (never under-excludes a genuine
/// handoff).
pub(super) fn transferred_to_block(
    term: &ArcTerminator,
    target: crate::ir::ArcBlockId,
) -> FxHashSet<ArcVarId> {
    let mut transferred: FxHashSet<ArcVarId> = FxHashSet::default();
    match term {
        ArcTerminator::Jump { target: t, args } if *t == target => {
            transferred.extend(args.iter().copied());
        }
        ArcTerminator::Invoke {
            dst, args, normal, ..
        } if *normal == target => {
            transferred.insert(*dst);
            transferred.extend(args.iter().copied());
        }
        ArcTerminator::InvokeIndirect {
            dst,
            closure,
            args,
            normal,
            ..
        } if *normal == target => {
            transferred.insert(*dst);
            transferred.insert(*closure);
            transferred.extend(args.iter().copied());
        }
        _ => {}
    }
    transferred
}
