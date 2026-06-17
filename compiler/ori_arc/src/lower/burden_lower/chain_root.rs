//! Loop-carried chain-root resolution (RL-1 + RL-2): the ONE `alias_src` +
//! `block_params` IR-walk index and the ONE parameterized chain walk shared by
//! the sibling-union and match-handoff extract-transfer verdicts.
//!
//! A chain follows `Let { Var }` source edges — passing through block-params
//! per the per-predecessor-edge attribution (`param_edge_args`) — until a
//! TERMINUS: a back-edge-carrying block-param (the loop-carried root) or,
//! when the policy admits it, a fresh-`Construct` local. Per-consumer
//! variation (multi-pred forward-merge handling, same-arg rename ordering,
//! dup-intermediate recording) is a [`ChainRootPolicy`] parameterization of
//! the same walk. Spec: Annex E §AIMS RL-1 + RL-2 (per-edge attribution).

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::aims::intraprocedural::project_aliases::{ParamEdgeArg, ProjectAliasTable};
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

/// A resolved chain root paired with the dup-inc-carrying INTERMEDIATE aliases
/// recorded on the chain (keeper-ledger input; empty when the policy records
/// none).
pub(super) type ChainRootWithDups = (ArcVarId, SmallVec<[ArcVarId; 2]>);

/// Bounded all-edges-agree recursion: each param hop recurses once per edge;
/// chains are short (the rebuild handoff spans a handful of hops).
const MAX_EDGE_RECURSION_DEPTH: usize = 32;

/// Disagreeing multi-predecessor FORWARD-merge handling for the chain walk.
pub(super) enum ForwardMergePolicy {
    /// A TRUE merge of (potentially) distinct allocations — never unified;
    /// the walk declines (`None`). Single-pred forward params still hop (the
    /// param IS that one edge's Jump-arg, per-edge attribution).
    Decline,
    /// Resolve every edge's arg; ALL edges agreeing on ONE root is a pure
    /// rename, not a true merge. Disagreement declines. Generalizes the
    /// single-pred hop to the all-edges-agree multi-pred case (the match
    /// merge passes the SAME loop struct from every arm).
    AllEdgesAgree,
}

/// Per-consumer parameterization of [`ChainRootIndex::resolve_root`].
pub(super) struct ChainRootPolicy<'a> {
    /// Pass through a block-param whose EVERY incoming edge carries the SAME
    /// arg (pure rename) BEFORE the back-edge terminus check — a merge inside
    /// the loop body has its edges classified back-edges because the body
    /// reaches them around the loop; same-arg edges stay one allocation.
    /// `false`: a back-edge-carrying param is the terminus unconditionally.
    pub(super) same_arg_renames_params: bool,
    /// Disagreeing multi-pred FORWARD-merge handling.
    pub(super) forward_merge: ForwardMergePolicy,
    /// Fresh-`Construct` dsts are chain-root termini (alongside back-edge
    /// block-params).
    pub(super) construct_terminus: bool,
    /// INTERMEDIATE Let aliases carrying a kept RL-1 dup inc are RECORDED on
    /// the returned chain (the keeper-ledger input — each needs a designated
    /// balancing release). `None` records nothing.
    pub(super) dup_alias_dsts: Option<&'a FxHashSet<ArcVarId>>,
}

/// Structural per-function chain-root index: the `Let { Var }` source edges +
/// block-param + fresh-`Construct` sets the chain walk traverses.
pub(super) struct ChainRootIndex {
    /// `Let { dst, Var(src) }` edges: dst -> src.
    pub(super) alias_src: FxHashMap<ArcVarId, ArcVarId>,
    /// All block-param vars.
    pub(super) block_params: FxHashSet<ArcVarId>,
    /// Vars defined by a fresh `Construct` (any ctor) — chain-root terminus
    /// candidates alongside block-params when the policy admits them.
    pub(super) construct_dsts: FxHashSet<ArcVarId>,
}

impl ChainRootIndex {
    pub(super) fn build(func: &ArcFunction) -> Self {
        let mut alias_src = FxHashMap::default();
        let mut block_params = FxHashSet::default();
        let mut construct_dsts = FxHashSet::default();
        for block in &func.blocks {
            for &(p, _) in &block.params {
                block_params.insert(p);
            }
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } => {
                        alias_src.insert(*dst, *src);
                    }
                    ArcInstr::Construct { dst, .. } => {
                        construct_dsts.insert(*dst);
                    }
                    _ => {}
                }
            }
        }
        Self {
            alias_src,
            block_params,
            construct_dsts,
        }
    }

    /// Let-hop chains (no param hops) agree with the §1.9 unified alias
    /// table's GENUINE same-allocation reps by construction — assert the two
    /// never drift (the table is the identity SSOT).
    pub(super) fn debug_assert_let_edges_match_genuine_reps(
        &self,
        alias_table: &ProjectAliasTable,
    ) {
        #[cfg(debug_assertions)]
        {
            let rep = |v: ArcVarId| {
                alias_table
                    .genuine_same_alloc_reps
                    .get(&v)
                    .copied()
                    .unwrap_or(v)
            };
            for (&dst, &src) in &self.alias_src {
                debug_assert_eq!(
                    rep(dst),
                    rep(src),
                    "Let{{Var}} edge %{} = %{} missing from genuine_same_alloc_reps",
                    dst.index(),
                    src.index(),
                );
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = (self, alias_table);
        }
    }

    /// Resolve `start` to its loop-carried chain root: follow `Let { Var }`
    /// source edges — passing through block-params per the policy and the
    /// per-predecessor-edge attribution — until a TERMINUS: a block-param
    /// carrying a back-edge (the loop-carried root; the back-edge itself is
    /// never traversed) or, when `construct_terminus` admits it, a fresh
    /// `Construct` local. `None` when the chain dead-ends (call result root)
    /// or a disagreeing multi-pred FORWARD merge declines per the policy.
    /// Spec: Annex E §AIMS RL-1 + RL-2 (per-edge attribution).
    pub(super) fn resolve_root(
        &self,
        start: ArcVarId,
        param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
        policy: &ChainRootPolicy<'_>,
    ) -> Option<ChainRootWithDups> {
        self.resolve_root_inner(start, param_edge_args, policy, 0)
    }

    fn resolve_root_inner(
        &self,
        start: ArcVarId,
        param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
        policy: &ChainRootPolicy<'_>,
        depth: usize,
    ) -> Option<ChainRootWithDups> {
        if depth > MAX_EDGE_RECURSION_DEPTH {
            return None;
        }
        let mut cur = start;
        let mut dup_intermediates: SmallVec<[ArcVarId; 2]> = SmallVec::new();
        // Cycle guard only: any acyclic chain has at most one hop per Let edge
        // plus one per block-param; a CFG cycle terminates at its back-edge
        // param before the bound fires.
        let hop_limit = self.alias_src.len() + self.block_params.len() + 1;
        for hop in 0..hop_limit {
            if self.block_params.contains(&cur) {
                let edges = param_edge_args.get(&cur)?;
                if policy.same_arg_renames_params {
                    // Pure rename: EVERY incoming edge passes the SAME arg —
                    // the param IS that allocation on every path, regardless
                    // of edge cycle classification.
                    let first = edges.first()?.arg;
                    if first != cur && edges.iter().all(|e| e.arg == first) {
                        cur = first;
                        continue;
                    }
                }
                if edges.iter().any(|e| e.is_back_edge) {
                    // Loop-carried root terminus (back-edge param). The
                    // back-edge itself is never traversed; back-edge
                    // unification declines per the merge-point filtering.
                    return Some((cur, dup_intermediates));
                }
                match policy.forward_merge {
                    ForwardMergePolicy::Decline => {
                        if let [only] = edges.as_slice() {
                            // Single-pred forward param: pure SSA rename — the
                            // param IS this edge's Jump-arg (same allocation,
                            // per-edge attribution).
                            cur = only.arg;
                            continue;
                        }
                        // Multi-pred forward merge: a TRUE merge of
                        // (potentially) distinct allocations. Decline.
                        return None;
                    }
                    ForwardMergePolicy::AllEdgesAgree => {
                        // Forward merge with disagreeing args: every edge's
                        // arg must resolve to one root.
                        let mut agreed: Option<ArcVarId> = None;
                        for e in edges {
                            let (r, _) =
                                self.resolve_root_inner(e.arg, param_edge_args, policy, depth + 1)?;
                            match agreed {
                                None => agreed = Some(r),
                                Some(prev) if prev == r => {}
                                Some(_) => return None,
                            }
                        }
                        return agreed.map(|r| (r, dup_intermediates));
                    }
                }
            }
            if policy.construct_terminus && self.construct_dsts.contains(&cur) {
                return Some((cur, dup_intermediates));
            }
            let next = *self.alias_src.get(&cur)?;
            // Intermediate Let alias (every chain node past the carrier
            // itself): a kept RL-1 dup inc on it needs a designated balancing
            // release — record for the keeper ledger.
            if hop > 0 {
                if let Some(dups) = policy.dup_alias_dsts {
                    if dups.contains(&cur) {
                        dup_intermediates.push(cur);
                    }
                }
            }
            cur = next;
        }
        None
    }
}
