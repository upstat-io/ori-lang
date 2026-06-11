//! Dead-block-param releases (RL-5 + RL-4): forwarder-identity dead-param
//! releases, the rebuild-lineage (sibling-union-fired) dead-param releases,
//! and their rep-resolution gates. Spec: Annex E §AIMS RL-5 + RL-4 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::forwarder::compute_alt_consumer_reps;
use super::function_used_vars;
use super::union_find::ForwarderUnionFind;

/// Per-block representative block-param vars needing exactly ONE RL-5 dead-at-entry
/// release for a forwarder-identity allocation.
///
/// Shape: an Owned non-scalar allocation `A` forwarded through a transfer-through-return
/// callee (`@id<T>(x: T) -> T = x`, `ParamContract.transfers_through_return ∧
/// access == Owned`) reaches a merge/return block's DEAD block-params (`Cardinality =
/// Absent` — the param is bound by the block but used nowhere on any forward path) via
/// `Jump` args on every predecessor edge. The Jump-arg → Owned-param handoff suppresses
/// the source's last-use dec (RL-4 Jump-arg exemption: ownership transfers to the
/// successor block-param), and that successor param is dead, so RL-5
/// (`AimsProof.Realization::RL5_dead_at_entry_cleanup`: an Owned non-scalar `Absent`
/// param gets ONE immediate dec) is the lineage's sole release point. The Phase-5 walk
/// emits no RL-5 dec → the allocation leaks (`RL5_cleanup_balanced` is violated: the
/// param entered with a live RC=1 reference that is never released).
///
/// One allocation reaching N dead params is the dedup case: the forwarder identity (the
/// Invoke/Apply result aliases its `[own]` arg, `id(x)=x`) makes `%4` (the source) and
/// `%7` (the forwarder result) the SAME RC=1 allocation, passed as TWO Jump args (`Jump
/// bb(.., %4, %7)`) to two dead params. RL-5's balance proof is PER-ALLOCATION (one
/// `[inc, dec]` per allocation, not per param): emitting a dec on BOTH params is
/// `[inc, dec, dec]` = net −1 = double-free. The cure emits EXACTLY ONE dec per distinct
/// source allocation reaching the block (dedup by the forwarder-identity rep), returning
/// one representative dead-param var per `(block, rep)`.
///
/// Forwarder-identity gate (the over-fire boundary): the dead-param's source allocation
/// MUST be reached through a forwarder edge (an Invoke/Apply transfer-through-return
/// result aliasing its owned arg). A plain `let d = Numbers(..); match d` dead param
/// (`compute_*` no Invoke-forwarder) is NOT admitted — its lineage is a distinct
/// (non-forwarder) dead-block-param sub-root; admitting it here would emit a release the
/// per-var path does not expect on the borrow-view sum-payload shapes (double-free).
///
/// Edge-release gate (the unwind subtlety): the rep is admitted ONLY when NO predecessor
/// edge into the block already releases the allocation — when an arm consumed the lineage
/// (e.g. an unwind `Resume` edge whose Phase-8a `unwind_cleanup` decs the value, or a
/// match arm that decs the inner payload) the block-entry dec would double the release.
/// The Phase-5 walk's union-find identity is conservative: a rep is admitted only when
/// the lineage's sole transfer is the forwarder + the dead-param handoff, with no
/// alternate release on any incoming edge.
pub(in crate::lower::burden_lower) fn compute_dead_forwarder_block_param_releases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<usize, Vec<ArcVarId>> {
    let mut uf = ForwarderUnionFind::build(func, contracts);
    let used = function_used_vars(func);
    let alt_consumer_reps = compute_alt_consumer_reps(func, contracts, &mut uf);

    let mut out: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
        for &(param_var, _) in &block.params {
            if used.contains(&param_var) {
                continue; // not dead
            }
            // L-9 repr-aware admission gate (same SSOT as
            // `owned_vars_needing_rc`): a provably-Scalar-repr dead param
            // carries no allocation to release; an RL-5 dec on it can never
            // lower and survives as VF-1 ledger residue.
            if super::super::is_provably_scalar_repr(func, param_var) {
                continue;
            }
            let Some(rep) = dead_param_forwarder_rep(func, &mut uf, block_idx, param_var) else {
                continue;
            };
            if alt_consumer_reps.contains(&rep) {
                continue;
            }
            if seen_reps.insert(rep) {
                out.entry(block_idx).or_default().push(param_var);
            }
        }
    }
    out
}

/// Resolve a DEAD block-param's source allocation to a forwarder-identity rep, or
/// `None` when it is not a single forwarder lineage. The param carries no union edge
/// (it is a phi sink), so its allocation is the rep of the args feeding its position
/// across every `Jump` predecessor; admitted only when every feeding edge resolves to
/// ONE rep AND that rep is a forwarder identity.
fn dead_param_forwarder_rep(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    block_idx: usize,
    param_var: ArcVarId,
) -> Option<ArcVarId> {
    let rep = dead_param_single_feeding_rep(func, uf, block_idx, param_var)?;
    uf.is_forwarder_rep.contains(&rep).then_some(rep)
}

/// Resolve a DEAD block-param to the SINGLE union-find rep feeding its position
/// across every `Jump` predecessor, or `None` when the feeding edges resolve to
/// zero or more than one rep (a genuine phi over distinct allocations). This is
/// the gate-free core: callers apply their own admission gate (forwarder-identity
/// for [`dead_param_forwarder_rep`]; sum-aggregate-Construct for
/// [`compute_construct_fed_dead_param_lineage`]).
pub(super) fn dead_param_single_feeding_rep(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    block_idx: usize,
    param_var: ArcVarId,
) -> Option<ArcVarId> {
    let pos = func.blocks[block_idx]
        .params
        .iter()
        .position(|&(p, _)| p == param_var)?;
    let mut feeding_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut any_feed = false;
    for pred in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &pred.terminator {
            if target.index() == block_idx {
                if let Some(&arg) = args.get(pos) {
                    any_feed = true;
                    feeding_reps.insert(uf.find(arg));
                }
            }
        }
    }
    if !any_feed || feeding_reps.len() != 1 {
        return None;
    }
    feeding_reps.iter().next().copied()
}

/// `ORI_DISABLE_REBUILD_LINEAGE_DEAD_PARAM_RELEASE=1` declines the RL-5
/// dead-at-entry release for a sibling-union-fired loop-carried rebuild lineage
/// reaching a dead loop-exit block-param ([`compute_rebuild_lineage_dead_param_releases`]).
/// With the toggle set, the loop's final value leaks when the loop-carried var
/// is unused after the loop (the union suppressed the in-loop decs; no
/// post-loop use means no last-use dec). Spec: Annex E §AIMS RL-5.
pub(in crate::lower::burden_lower) fn rebuild_lineage_dead_param_release_disabled() -> bool {
    std::env::var("ORI_DISABLE_REBUILD_LINEAGE_DEAD_PARAM_RELEASE").as_deref() == Ok("1")
}

/// RL-5 dead-at-entry release for a sibling-union-fired loop-carried rebuild
/// lineage: the union's verdict suppressed the lineage's per-iteration releases
/// (full-move widening / keeper assignment), so the loop's FINAL value exits
/// through the loop-exit `Jump` edge into a successor block-param that becomes
/// the lineage's sole terminal owner. When that param is DEAD (no uses — the
/// loop-carried var is unused after the loop), no last-use dec ever fires and
/// the final struct's fields leak. Emit EXACTLY ONE whole-var `BurdenDec` at
/// the dead param's block entry (RL-5: an Owned non-scalar block param with
/// `Cardinality = Absent` at entry receives an immediate dec).
///
/// Root-kind scoping (the over-fire boundary): ONLY params fed by a Jump-arg
/// whose genuine same-allocation rep matches a FIRED union root (the
/// back-edge loop param whose group verdict applied). Gates:
/// - the feeding edge is FORWARD (a back-edge re-entry is the loop carrying
///   the value to the next iteration, never a death point);
/// - the target param has exactly ONE incoming edge (single-predecessor v1 —
///   a true merge may receive a live allocation on another path);
/// - the param is DEAD (`function_used_vars` miss) and not provably-Scalar
///   repr (L-9: a scalar-repr dec can never lower);
/// - one release per distinct arriving rep per block (a second same-rep dead
///   param would double-release the same arriving allocation);
/// - NO live same-rep param in the same target block (the live alias's own
///   last-use dec is the release).
///
/// Spec: Annex E §AIMS RL-5 + RL-2.
pub(in crate::lower::burden_lower) fn compute_rebuild_lineage_dead_param_releases(
    func: &ArcFunction,
    fired_rebuild_roots: &FxHashSet<ArcVarId>,
    genuine_same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    param_edge_args: &FxHashMap<
        ArcVarId,
        smallvec::SmallVec<[crate::aims::intraprocedural::project_aliases::ParamEdgeArg; 2]>,
    >,
) -> FxHashMap<usize, Vec<ArcVarId>> {
    let mut out: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    if rebuild_lineage_dead_param_release_disabled() || fired_rebuild_roots.is_empty() {
        return out;
    }
    let rep = |v: ArcVarId| genuine_same_alloc_reps.get(&v).copied().unwrap_or(v);
    let root_reps: FxHashSet<ArcVarId> = fired_rebuild_roots.iter().map(|&r| rep(r)).collect();
    let used = function_used_vars(func);

    let mut seen_reps_per_block: FxHashMap<usize, FxHashSet<ArcVarId>> = FxHashMap::default();
    for (pred_idx, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Jump { target, args } = &block.terminator else {
            continue;
        };
        let target_idx = target.index();
        let Some(target_block) = func.blocks.get(target_idx) else {
            continue;
        };
        for (i, &arg) in args.iter().enumerate() {
            let arg_rep = rep(arg);
            if !root_reps.contains(&arg_rep) {
                continue;
            }
            let Some(&(param_var, _)) = target_block.params.get(i) else {
                continue;
            };
            let Some(edges) = param_edge_args.get(&param_var) else {
                continue;
            };
            // Single-predecessor + FORWARD edge gates.
            let [only] = edges.as_slice() else {
                continue;
            };
            if only.is_back_edge || only.pred_block != pred_idx {
                continue;
            }
            if used.contains(&param_var) {
                continue; // live — its own last-use dec releases
            }
            if super::super::is_provably_scalar_repr(func, param_var) {
                continue;
            }
            // NO live same-rep sibling param in the target block: the live
            // alias's last-use dec is the release; a dec here double-frees.
            let live_same_rep_sibling = target_block
                .params
                .iter()
                .any(|&(p, _)| p != param_var && used.contains(&p) && rep(p) == arg_rep);
            if live_same_rep_sibling {
                continue;
            }
            if seen_reps_per_block
                .entry(target_idx)
                .or_default()
                .insert(arg_rep)
            {
                tracing::trace!(
                    target: "ori_arc::aims::realize",
                    fn_name = ?func.name,
                    pred_block = pred_idx,
                    target_block = target_idx,
                    param = param_var.index(),
                    "rebuild-lineage dead-param release placed (RL-5)"
                );
                out.entry(target_idx).or_default().push(param_var);
            }
        }
    }
    out
}
