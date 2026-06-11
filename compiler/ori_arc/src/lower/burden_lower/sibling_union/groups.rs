//! Sibling-alias group construction: build the per-(root, block) groups of
//! projection-carrier `Let { Var }` aliases sharing one alias-chain root, via
//! the shared chain-root walk. Spec: Annex E §AIMS RL-1 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::aims::intraprocedural::project_aliases::{ParamEdgeArg, ProjectAliasTable};
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::super::chain_root::{
    ChainRootIndex, ChainRootPolicy, ChainRootWithDups, ForwardMergePolicy,
};

/// A projection carrier paired with the dup-inc-carrying INTERMEDIATE aliases
/// recorded on its chain (keeper-ledger input).
pub(super) type CarrierWithDups = ChainRootWithDups;

/// A sibling-alias group: the projection-carrier alias dsts that share one
/// alias-chain root and are CO-REACHABLE (same block — the conservative v1
/// per the fix-consensus single-releaser restriction).
pub(super) struct SiblingGroup {
    /// The alias-chain root (loop block-param OR fresh-Construct local). NEVER
    /// a suppression target.
    pub(super) root: ArcVarId,
    /// The projection-carrier sibling alias dsts (`Let { Var }` of `root` or of
    /// an intermediate alias of `root`) whose projection moved a field.
    pub(super) siblings: Vec<ArcVarId>,
    /// The block all siblings live in (co-reachability v1 = same block).
    pub(super) block_idx: usize,
    /// INTERMEDIATE Let aliases on the siblings' chains carrying a kept RL-1
    /// dup inc (`dup_alias_dsts`), mapped to the siblings whose chain passes
    /// through them. Each needs EXACTLY ONE balancing whole-var release among
    /// its siblings (the keeper) — `apply_group` assigns it or declines. Spec:
    /// Annex E §AIMS RL-1.
    pub(super) dup_intermediates: FxHashMap<ArcVarId, Vec<ArcVarId>>,
}

/// Build sibling-alias groups keyed to the alias-chain root. A group's siblings
/// are the `Let { Var }` dsts that (a) project a field out of an alias of the
/// root and (b) appear in the SAME block (co-reachable v1). The root is the
/// chain terminus: a back-edge-carrying block-param or a fresh `Construct`
/// local, reached by following `Let { Var }` source edges plus
/// SINGLE-predecessor FORWARD block-param hops (per-predecessor-edge
/// attribution — `param_edge_args`). Multi-predecessor forward merges and
/// chains whose INTERMEDIATE Let alias carries a kept RL-1 dup inc
/// (`dup_alias_dsts`) DECLINE. Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn build_sibling_groups(
    func: &ArcFunction,
    alias_table: &ProjectAliasTable,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
) -> Vec<SiblingGroup> {
    // Shared chain-root index + walk ([`ChainRootIndex`]), parameterized to the
    // sibling-union policy: a back-edge-carrying block-param is the terminus
    // UNCONDITIONALLY (traversal STOPS; back-edge unification declines per the
    // merge-point filtering); SINGLE-predecessor FORWARD block-params hop
    // through (the param IS that one edge's Jump-arg); a multi-predecessor
    // FORWARD merge DECLINES (a TRUE merge of distinct allocations — never
    // unified); a fresh-`Construct` local is a terminus; INTERMEDIATE Let
    // aliases carrying a kept RL-1 dup inc are RECORDED for the keeper ledger.
    // `None` when the chain dead-ends (call result root). Spec: Annex E §AIMS
    // RL-1 + RL-2.
    let idx = ChainRootIndex::build(func);
    let policy = ChainRootPolicy {
        same_arg_renames_params: false,
        forward_merge: ForwardMergePolicy::Decline,
        construct_terminus: true,
        dup_alias_dsts: Some(dup_alias_dsts),
    };
    let chain_root = |start: ArcVarId| idx.resolve_root(start, param_edge_args, &policy);
    idx.debug_assert_let_edges_match_genuine_reps(alias_table);

    // The set of alias dsts that carry a PROJECTION (the carrier reads a field
    // out of itself) — over EVERY `Project`, INCLUDING scalar-field projections.
    // A direct alias of the root projecting only a scalar field still carries a
    // WHOLE `burden_dec` on the shared struct (the mixed heap+scalar shape: its
    // dec releases the heap field a sibling moved out — a double-free). Built
    // from the IR directly (NOT `project_origins`, which gates scalar dsts out
    // per Pass-1 L-9). A direct alias of the root with no projection is a plain
    // re-bind (the `explicit_alias` intermediate `let s = r`), not a sibling.
    let mut carrier_projects: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { value, .. } = instr {
                carrier_projects.insert(*value);
            }
        }
    }

    // Group every DIRECT `Let { Var }` alias of a loop-block-param root that
    // carries a projection, by (root, block). A sibling is the alias `dst` (the
    // `value` an `ArcInstr::Project` reads) — including a sibling that projects
    // ONLY a scalar field (it carries a WHOLE `burden_dec` on the shared struct,
    // releasing the heap field a sibling moved out; the mixed heap+scalar shape).
    let mut by_root_block: FxHashMap<(ArcVarId, usize), Vec<CarrierWithDups>> =
        FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(_),
                ..
            } = instr
            else {
                continue;
            };
            // The carrier MUST itself be projected (a rebuild sibling reads a
            // field out of the loop-carried struct alias).
            if !carrier_projects.contains(dst) {
                continue;
            }
            let Some((root, dup_intermediates)) = chain_root(*dst) else {
                continue;
            };
            // INTERMEDIATE-hop admission (table-resolved, supersedes the
            // one-hop DIRECT-alias gate): a multi-hop carrier is admitted with
            // its chain's kept-dup-inc intermediates RECORDED — `apply_group`
            // assigns each intermediate ONE balancing whole-var releaser (the
            // keeper) or declines the group (the inc-aware boundary: an
            // intermediate's kept inc whose own dec is transfer-suppressed is
            // balanced by the sibling decs; suppressing ALL of them strands
            // the inc, +1 leak).
            //
            // BACK-EDGE gate (the loop-carried shape; RCA shape-gate "PLUS a
            // loop back-edge"): the root MUST be a block-param carrying a
            // back-edge (a loop header). A fresh-`Construct`-rooted rebuild
            // with no back-edge (the `no_loop` straight-line shape) keeps its
            // single scope-exit release; suppressing it leaks the new struct's
            // buffers. Per-predecessor-edge attribution is the SSOT for the
            // back-edge classification.
            if !param_edge_args
                .get(&root)
                .is_some_and(|edges| edges.iter().any(|e| e.is_back_edge))
            {
                continue;
            }
            by_root_block
                .entry((root, block_idx))
                .or_default()
                .push((*dst, dup_intermediates));
        }
    }

    by_root_block
        .into_iter()
        .filter_map(|((root, block_idx), carriers)| {
            let mut dup_intermediates: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
            let mut siblings: Vec<ArcVarId> = Vec::with_capacity(carriers.len());
            for (sib, dups) in carriers {
                siblings.push(sib);
                for d in dups {
                    dup_intermediates.entry(d).or_default().push(sib);
                }
            }
            siblings.sort_unstable_by_key(|v| v.index());
            siblings.dedup();
            for sibs in dup_intermediates.values_mut() {
                sibs.sort_unstable_by_key(|v| v.index());
                sibs.dedup();
            }
            // A group needs >=2 distinct projection-carrier siblings (the >=2
            // plainly self-projected FIELDS shape gate).
            (siblings.len() >= 2).then_some(SiblingGroup {
                root,
                siblings,
                block_idx,
                dup_intermediates,
            })
        })
        .collect()
}
