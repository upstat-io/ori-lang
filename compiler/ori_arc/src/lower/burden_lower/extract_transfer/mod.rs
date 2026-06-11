//! Match-handoff extract-transfer attribution (RL-2): the all-arms verdict
//! that cures the sum-payload loop-rebuild double-free.
//!
//! `r = Pair { o: Some(extracted), b: r.b }` where `extracted = match r.o
//! { Some(xs) -> xs, None -> [] }` extracts the sum field's PAYLOAD through
//! the match block-param handoff and re-wraps it into the rebuild construct.
//! The carrier's `BurdenDecPartial` still releases the sum field — freeing a
//! payload whose ownership already transferred (RL-2 `ConstructArg`) into the
//! new loop-carried struct: a double-free.
//!
//! Cure (post-sibling-union): a still-released sum field counts MOVED when
//! EVERY switch arm over the field's tag either (a) transfers the extracted
//! payload into the rebuild construct on an UNCONDITIONAL arm path (owning
//! Construct-arg position, per-edge block-param attribution), or (b) is a
//! payload-less-variant arm (vacuous). ANY arm with a conditional / partial
//! flow DECLINES — the release stays (the dropped-payload path needs it).
//!
//! Spec: Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec`: a transferred
//! payload's obligation moves to the consumer; a dec on it double-releases;
//! `RL2_nontransfer_kinds_dec`: a non-transferred payload keeps its release).

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use ori_types::TypeRegistry;

use crate::aims::intraprocedural::project_aliases::{ParamEdgeArg, ProjectAliasTable};
use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use super::chain_root::{ChainRootIndex, ChainRootPolicy, ForwardMergePolicy};
use super::sibling_union::owned_top_level_fields;

/// `ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER=1` restores the per-field
/// attribution WITHOUT the all-arms extract-transfer verdict: a rebuild
/// carrier's `BurdenDecPartial` keeps releasing a sum field whose payload was
/// extracted through the match block-param handoff and re-wrapped into the
/// rebuild construct (the pre-fix double-free shape). Bisection surface:
/// isolates the sum-payload match-rebuild double-free to this verdict vs the
/// rest of the Phase-5 walk. Default (unset): the all-arms verdict widens the
/// carrier's skip set. Spec: Annex E §AIMS RL-2.
pub(super) fn match_handoff_extract_transfer_disabled() -> bool {
    std::env::var("ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER").as_deref() == Ok("1")
}

/// Apply the all-arms match-handoff extract-transfer verdict, mutating
/// `partial_move_vars` (widening a carrier's `skip_fields` with all-arms-
/// transferred sum fields) and `full_move_vars` (absorbing carriers whose
/// widened skip covers every owned RC field). Runs after
/// `apply_sibling_moved_field_union` and BEFORE `inc_suppressed_vars` is
/// derived from `full_move_vars`.
pub(super) fn apply_match_handoff_extract_transfer(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    alias_table: &ProjectAliasTable,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) {
    apply_match_handoff_extract_transfer_with(
        match_handoff_extract_transfer_disabled(),
        func,
        type_registry,
        owned_vars_needing_rc,
        alias_table,
        param_edge_args,
        full_move_vars,
        partial_move_vars,
    );
}

/// Toggle-injected body of [`apply_match_handoff_extract_transfer`]; `disabled`
/// carries the `ORI_DISABLE_MATCH_HANDOFF_EXTRACT_TRANSFER` verdict so tests
/// exercise the disabled path without mutating process-global env.
#[expect(
    clippy::too_many_arguments,
    reason = "verdict-pass plumbing mirrors the public entry"
)]
pub(super) fn apply_match_handoff_extract_transfer_with(
    disabled: bool,
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    alias_table: &ProjectAliasTable,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) {
    if disabled {
        return;
    }
    let idx = FuncIndex::build(func);
    idx.chain
        .debug_assert_let_edges_match_genuine_reps(alias_table);

    let carriers: Vec<(ArcVarId, Vec<u32>)> = partial_move_vars
        .iter()
        .map(|(&c, skip)| (c, skip.clone()))
        .collect();
    for (carrier, skip) in carriers {
        if !owned_vars_needing_rc.contains(&carrier) {
            continue;
        }
        // The carrier MUST resolve to a back-edge block-param root (the
        // loop-carried rebuild lineage). Non-loop shapes keep their release.
        let Some(root) = idx.resolve_root(carrier, param_edge_args) else {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                carrier = carrier.index(),
                "match-handoff verdict declined: no loop-carried root"
            );
            continue;
        };
        // The merge block holding the carrier's Let definition — the rebuild
        // + partial-dec site.
        let Some(&merge_block) = idx.def_block.get(&carrier) else {
            continue;
        };
        let owned_fields = owned_top_level_fields(func, carrier, type_registry);
        let mut widened: Vec<u32> = skip.clone();
        for &field in &owned_fields {
            if skip.contains(&field) {
                continue;
            }
            if all_arms_transfer_field(
                func,
                type_registry,
                &idx,
                param_edge_args,
                carrier,
                root,
                merge_block,
                field,
            ) {
                widened.push(field);
                tracing::trace!(
                    target: "ori_arc::aims::realize",
                    fn_name = ?func.name,
                    carrier = carrier.index(),
                    field,
                    "match-handoff extract-transfer verdict: field counts moved (all arms transfer)"
                );
            }
        }
        if widened.len() == skip.len() {
            continue;
        }
        widened.sort_unstable();
        let covers_all = owned_fields.iter().all(|f| widened.contains(f));
        if covers_all {
            partial_move_vars.remove(&carrier);
            full_move_vars.insert(carrier);
        } else {
            partial_move_vars.insert(carrier, widened);
        }
    }
}

/// Structural per-function indexes the verdict consumes.
struct FuncIndex {
    /// Shared chain-root index (`Let { Var }` source edges + block-params).
    chain: ChainRootIndex,
    /// var -> defining block index (instr dsts only; params excluded).
    def_block: FxHashMap<ArcVarId, usize>,
    /// Project dst -> (source value, field).
    project_of: FxHashMap<ArcVarId, (ArcVarId, u32)>,
}

impl FuncIndex {
    fn build(func: &ArcFunction) -> Self {
        let chain = ChainRootIndex::build(func);
        let mut def_block = FxHashMap::default();
        let mut project_of = FxHashMap::default();
        for (block_idx, block) in func.blocks.iter().enumerate() {
            for instr in &block.body {
                if let Some(dst) = instr.defined_var() {
                    def_block.insert(dst, block_idx);
                }
                if let ArcInstr::Project {
                    dst, value, field, ..
                } = instr
                {
                    project_of.insert(*dst, (*value, *field));
                }
            }
        }
        Self {
            chain,
            def_block,
            project_of,
        }
    }

    /// Resolve `start` to its loop-carried chain root via the shared walk
    /// ([`ChainRootIndex::resolve_root`]), parameterized to the match-handoff
    /// policy: a block-param whose EVERY incoming edge passes the SAME arg is
    /// a pure rename hop (per-edge attribution, regardless of edge cycle
    /// classification); a back-edge-carrying param (init edge + iteration
    /// back-edge disagree) IS the root terminus; a disagreeing FORWARD merge
    /// resolves all-edges-agree (the match merge passes the SAME loop struct
    /// from every arm). `None` when the chain dead-ends or edges disagree.
    /// Spec: Annex E §AIMS RL-1 + RL-2 (per-edge attribution).
    fn resolve_root(
        &self,
        start: ArcVarId,
        param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    ) -> Option<ArcVarId> {
        let policy = ChainRootPolicy {
            same_arg_renames_params: true,
            forward_merge: ForwardMergePolicy::AllEdgesAgree,
            construct_terminus: false,
            dup_alias_dsts: None,
        };
        self.chain
            .resolve_root(start, param_edge_args, &policy)
            .map(|(root, _)| root)
    }
}

mod gates;

use gates::all_arms_transfer_field;

#[cfg(test)]
mod tests;
