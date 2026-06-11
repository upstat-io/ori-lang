//! Sibling-alias moved-field cross-check (RL-2): the per-field verdict that
//! cures the loop-carried struct self-rebuild double-free.
//!
//! `r = T { a: r.a, b: r.b }` lowers each self-projection through a DISTINCT
//! `Let { Var }` alias of the loop block-param; the moved-out-field scan
//! attributes each moved field ONLY to its own alias, so each sibling's
//! `BurdenDecPartial skip=[k]` releases the field its SIBLING transferred —
//! a double-free of a buffer carried to the next iteration.
//!
//! Cure (post-`compute_partial_move_vars`): group sibling aliases by chain
//! root; UNION their moved-out fields; WIDEN each sibling's `skip_fields`
//! with sibling-covered fields. Full coverage joins `full_move_vars` (dec +
//! FRESH-site inc suppressed via the `inc_suppressed_vars` coupling). The
//! alias-chain ROOT is NEVER a suppression target.
//!
//! Spec: Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec`: a transferred field's
//! obligation moves to the consumer; a dec on it double-releases).

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use ori_types::TypeRegistry;

use crate::aims::intraprocedural::project_aliases::{ParamEdgeArg, ProjectAliasTable};
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::super::burden_lookup::{idx_to_type_ref, lookup_burden};
use crate::lower::burden::{Burden, TypeRef};

#[cfg(test)]
use super::burden_carries_rc;
#[cfg(test)]
use super::ownership_scans::instr_owned_position_transfer_vars;

/// `ORI_DISABLE_SIBLING_MOVED_FIELD_UNION=1` restores per-alias moved-field
/// attribution: the sibling-alias cross-check post-process is skipped, so each
/// `Let { Var }` projection alias keeps the partial-dec for the SIBLING's
/// transferred field (the pre-fix double-free shape). Bisection surface:
/// isolates the loop-carried struct self-rebuild double-free to the
/// sibling-union post-process vs the rest of the Phase-5 walk. Default (unset):
/// the sibling moved-field union widens each alias's skip set. Spec: Annex E
/// §AIMS RL-2.
pub(super) fn sibling_moved_field_union_disabled() -> bool {
    std::env::var("ORI_DISABLE_SIBLING_MOVED_FIELD_UNION").as_deref() == Ok("1")
}

/// Apply the sibling-alias moved-field cross-check, mutating `partial_move_vars`
/// (widening each sibling's `skip_fields`) and `full_move_vars` (absorbing
/// siblings whose widened skip covers all owned RC-carrying fields). Runs after
/// `compute_partial_move_vars` and BEFORE `inc_suppressed_vars` is derived from
/// `full_move_vars`, so an absorbed sibling's FRESH-site dup-alias inc is
/// suppressed coherently with its dec.
///
/// Cross-block / multi-hop identity comes from the §1.9 unified alias table
/// (`alias_table` — the ONE same-allocation identity SSOT) + the
/// per-predecessor-edge attribution (`param_edge_args`: a block-param resolves
/// to THAT edge's Jump-arg, never the merged R4 union; back-edge unification
/// DECLINES). `dup_alias_dsts` gates multi-hop admission: a chain whose
/// INTERMEDIATE alias carries a kept RL-1 dup inc declines (suppressing the
/// downstream siblings would strand that inc, +1 leak). Spec: Annex E §AIMS
/// RL-1 + RL-2.
#[expect(
    clippy::too_many_arguments,
    reason = "verdict-pass plumbing mirrors the toggle-injected inner entry"
)]
pub(super) fn apply_sibling_moved_field_union(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    alias_table: &ProjectAliasTable,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) -> SiblingUnionOutcome {
    apply_sibling_moved_field_union_with(
        sibling_moved_field_union_disabled(),
        func,
        type_registry,
        moved_out_fields_union,
        owned_vars_needing_rc,
        alias_table,
        param_edge_args,
        dup_alias_dsts,
        full_move_vars,
        partial_move_vars,
    )
}

/// Toggle-injected body of [`apply_sibling_moved_field_union`]; `disabled`
/// carries the `ORI_DISABLE_SIBLING_MOVED_FIELD_UNION` verdict so tests
/// exercise the disabled path without mutating process-global env (env
/// mutation races parallel test threads reading the same var).
#[expect(
    clippy::too_many_arguments,
    reason = "verdict-pass plumbing mirrors the public entry"
)]
fn apply_sibling_moved_field_union_with(
    disabled: bool,
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    alias_table: &ProjectAliasTable,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) -> SiblingUnionOutcome {
    let mut outcome = SiblingUnionOutcome::default();
    if disabled {
        return outcome;
    }

    let groups = build_sibling_groups(func, alias_table, param_edge_args, dup_alias_dsts);

    for group in groups {
        if let Some(verdict) = apply_group(
            func,
            type_registry,
            &group,
            moved_out_fields_union,
            owned_vars_needing_rc,
            full_move_vars,
            partial_move_vars,
        ) {
            outcome.fired_roots.insert(group.root);
            if let Some(keeper) = verdict.keeper {
                outcome.keepers.insert(keeper);
            }
        }
    }
    outcome
}

/// Outcome of the sibling-union verdict pass.
#[derive(Default)]
pub(super) struct SiblingUnionOutcome {
    /// FIRED group roots (back-edge block-params whose verdict APPLIED — the
    /// per-iteration releases were suppressed/widened). Consumed by the RL-5
    /// rebuild-lineage dead-param release scan: with the in-loop decs
    /// suppressed, the loop's final value exits through the loop-exit Jump
    /// edge into a (possibly DEAD) successor block-param that becomes the
    /// lineage's sole terminal owner. Spec: Annex E §AIMS RL-5.
    pub(super) fired_roots: FxHashSet<ArcVarId>,
    /// Assigned KEEPERS (the one sibling per group whose whole `burden_dec`
    /// at last use balances the dup INTERMEDIATE's kept inc, RL-1). The
    /// keeper's own FRESH-site dup-alias inc is suppressed at Phase 5
    /// (`inc_suppressed_vars`) so its dec is the lineage's net release —
    /// encoded directly rather than relying on the Phase-6 DP-3 split (which
    /// the loop-carried pair-atomicity guard would otherwise ban). Spec:
    /// Annex E §AIMS RL-1 (`RL1_duplication_balanced`).
    pub(super) keepers: FxHashSet<ArcVarId>,
}

/// Verdict detail for one APPLIED group.
struct GroupVerdict {
    /// The assigned keeper sibling, when the group carried a dup intermediate.
    keeper: Option<ArcVarId>,
}

/// A projection carrier paired with the dup-inc-carrying INTERMEDIATE aliases
/// recorded on its chain (keeper-ledger input).
type CarrierWithDups = (ArcVarId, SmallVec<[ArcVarId; 2]>);

/// A sibling-alias group: the projection-carrier alias dsts that share one
/// alias-chain root and are CO-REACHABLE (same block — the conservative v1
/// per the fix-consensus single-releaser restriction).
struct SiblingGroup {
    /// The alias-chain root (loop block-param OR fresh-Construct local). NEVER
    /// a suppression target.
    root: ArcVarId,
    /// The projection-carrier sibling alias dsts (`Let { Var }` of `root` or of
    /// an intermediate alias of `root`) whose projection moved a field.
    siblings: Vec<ArcVarId>,
    /// The block all siblings live in (co-reachability v1 = same block).
    block_idx: usize,
    /// INTERMEDIATE Let aliases on the siblings' chains carrying a kept RL-1
    /// dup inc (`dup_alias_dsts`), mapped to the siblings whose chain passes
    /// through them. Each needs EXACTLY ONE balancing whole-var release among
    /// its siblings (the keeper) — `apply_group` assigns it or declines. Spec:
    /// Annex E §AIMS RL-1.
    dup_intermediates: FxHashMap<ArcVarId, Vec<ArcVarId>>,
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
fn build_sibling_groups(
    func: &ArcFunction,
    alias_table: &ProjectAliasTable,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
) -> Vec<SiblingGroup> {
    // `Let { Var }` source edges: alias dst -> its source.
    let mut alias_src: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    // Vars defined by a fresh `Construct` (any ctor) — chain-root terminus
    // candidates alongside block-params.
    let mut construct_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    // Block-param vars — the loop block-param chain-root terminus.
    let mut block_params: FxHashSet<ArcVarId> = FxHashSet::default();
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

    let chain_root = |start: ArcVarId| {
        resolve_chain_root(
            start,
            func.var_types.len(),
            &block_params,
            &construct_dsts,
            &alias_src,
            param_edge_args,
            dup_alias_dsts,
        )
    };
    debug_assert_let_edges_match_genuine_reps(&alias_src, alias_table);

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

/// Resolve an alias dst to its chain root: follow `Let { Var }` source edges —
/// passing through SINGLE-predecessor FORWARD block-params per the per-edge
/// attribution (the param IS that one edge's Jump-arg) — until a TERMINUS:
/// a block-param carrying a back-edge (the loop-carried root; traversal STOPS,
/// back-edge unification declines per the merge-point filtering) OR a
/// fresh-`Construct` local. `None` when the chain dead-ends (call result root)
/// or crosses a multi-predecessor FORWARD merge (a TRUE merge of distinct
/// allocations — never unified). INTERMEDIATE Let aliases carrying a kept RL-1
/// dup inc are RECORDED — `apply_group` assigns each ONE balancing whole-var
/// releaser (the keeper) or declines. Spec: Annex E §AIMS RL-1 + RL-2.
fn resolve_chain_root(
    start: ArcVarId,
    hop_limit: usize,
    block_params: &FxHashSet<ArcVarId>,
    construct_dsts: &FxHashSet<ArcVarId>,
    alias_src: &FxHashMap<ArcVarId, ArcVarId>,
    param_edge_args: &FxHashMap<ArcVarId, SmallVec<[ParamEdgeArg; 2]>>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
) -> Option<CarrierWithDups> {
    let mut cur = start;
    let mut dup_intermediates: SmallVec<[ArcVarId; 2]> = SmallVec::new();
    for hop in 0..hop_limit {
        if block_params.contains(&cur) {
            let edges = param_edge_args.get(&cur)?;
            if edges.iter().any(|e| e.is_back_edge) {
                // Loop-carried root terminus (back-edge param). The back-edge
                // itself is never traversed.
                return Some((cur, dup_intermediates));
            }
            if let [only] = edges.as_slice() {
                // Single-pred forward param: pure SSA rename — the param IS
                // this edge's Jump-arg (same allocation, per-edge attribution).
                cur = only.arg;
                continue;
            }
            // Multi-pred forward merge: a TRUE merge of (potentially) distinct
            // allocations. Decline.
            return None;
        }
        if construct_dsts.contains(&cur) {
            return Some((cur, dup_intermediates));
        }
        let next = *alias_src.get(&cur)?;
        // Intermediate Let alias (every chain node past the carrier itself):
        // a kept RL-1 dup inc on it needs a designated balancing release —
        // record for the keeper ledger.
        if hop > 0 && dup_alias_dsts.contains(&cur) {
            dup_intermediates.push(cur);
        }
        cur = next;
    }
    None
}

/// Let-hop chains (no param hops) agree with the table's GENUINE
/// same-allocation reps by construction — assert the two never drift (the
/// table is the identity SSOT).
fn debug_assert_let_edges_match_genuine_reps(
    alias_src: &FxHashMap<ArcVarId, ArcVarId>,
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
        for (&dst, &src) in alias_src {
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
        let _ = (alias_src, alias_table);
    }
}

/// Apply the per-field verdict to one sibling group: compute the sibling-union
/// of moved fields, widen each sibling's skip set, and absorb fully-covered
/// siblings into `full_move_vars`.
fn apply_group(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    group: &SiblingGroup,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) -> Option<GroupVerdict> {
    let decline = |gate: &str| {
        tracing::trace!(
            target: "ori_arc::aims::realize",
            fn_name = ?func.name,
            root = group.root.index(),
            block = group.block_idx,
            gate,
            "sibling-moved-field verdict declined"
        );
    };

    // Sum-payload guard: decline when any sibling projects through a sum
    // (variant-tagged) source whose variant context is not statically the
    // same across siblings. v1-conservative: a sibling whose root type is a
    // sum aggregate with >1 variant declines the whole group (cross-variant
    // unification is unsound; pin 15). The same-variant sum extraction
    // (pin 10) reaches the rebuild through a Construct local root, not a
    // direct sum-field projection, so it is admitted.
    if root_is_multivariant_sum(func, group.root, type_registry) {
        decline("sum-payload-multivariant");
        return None;
    }

    // Sibling UNION of moved-out fields: a field counts as moved when ANY
    // sibling transferred it (the Pass-3 union already accumulates a field per
    // carrier through any RL-2 transfer position). Restrict to fields the
    // GROUP's siblings transferred so an unrelated alias of the same root does
    // not pollute the union.
    let mut union_fields: FxHashSet<u32> = FxHashSet::default();
    for &sib in &group.siblings {
        if let Some(fields) = moved_out_fields_union.get(&sib) {
            union_fields.extend(fields.iter().copied());
        }
    }
    if union_fields.is_empty() {
        decline("empty-union");
        return None;
    }

    // Keeper ledger (RL-1 — the multi-hop / late-use-alias chain): an
    // INTERMEDIATE alias with a kept dup inc and a transfer-suppressed own dec
    // (its last use is a sibling's Let) is balanced by EXACTLY ONE whole-var
    // release among its siblings — the KEEPER, the LAST sibling through it
    // (all reads precede the release). The keeper is left UNTOUCHED (keeps its
    // whole `burden_dec` at last use; its own dup inc is the standard DP-3
    // Once-Affine elision target). v1 conservative declines: >1 dup
    // intermediate (one whole dec cannot balance two kept incs); a keeper that
    // MOVED a field (its dec is partial — cannot balance the whole inc); an
    // intermediate used outside the group's sibling Lets (the kept inc may be
    // balanced elsewhere); union not covering all owned fields (an uncovered
    // field needs the entering reference released too — one whole dec cannot
    // do both). Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`).
    let owned_field_set_precheck = owned_top_level_fields(func, group.root, type_registry);
    let keeper: Option<ArcVarId> = match assign_keeper(
        func,
        group,
        moved_out_fields_union,
        &union_fields,
        &owned_field_set_precheck,
    ) {
        Ok(keeper) => keeper,
        Err(gate) => {
            decline(gate);
            return None;
        }
    };

    // Per-FIELD verdict: for each sibling, widen its skip set with the union,
    // then classify. A sibling whose widened skip covers ALL its owned
    // RC-carrying top-level fields becomes a full-move (dec + inc suppressed);
    // a mixed-coverage sibling keeps a `BurdenDecPartial` with the widened skip
    // (releases only its still-uncovered fields).
    //
    // Single-releaser assignment: an uncovered RC field must be released by
    // EXACTLY ONE sibling's kept dec (else widening leaves it in multiple
    // siblings' release sets — double-free; OR drops it from all — leak). v1
    // same-block: assign each uncovered field to the LAST sibling (instruction
    // order) that still owns it, widening every earlier sibling's skip to
    // include it.
    let owned_field_set = owned_field_set_precheck;

    // The releaser per uncovered field: the highest-index sibling that owns the
    // field (same-block instruction order). Compute owned-by per sibling first.
    let mut releaser: FxHashMap<u32, ArcVarId> = FxHashMap::default();
    for &field in &owned_field_set {
        if union_fields.contains(&field) {
            // Covered by some sibling's transfer — no sibling keeps a dec for
            // it (the transfer discharges it). NOT an uncovered field.
            continue;
        }
        // Uncovered: assign to the last sibling (group.siblings is sorted
        // ascending by index, so .last() is the latest).
        if let Some(&last) = group.siblings.last() {
            releaser.insert(field, last);
        }
    }

    for &sib in &group.siblings {
        // The KEEPER stays untouched: its whole `burden_dec` at last use is
        // the designated balancing release for the dup intermediate's kept
        // inc (RL-1). Never widened, never absorbed into full_move_vars.
        if Some(sib) == keeper {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = group.root.index(),
                sibling = sib.index(),
                "sibling-moved-field verdict: keeper (whole dec kept)"
            );
            continue;
        }
        // The sibling's widened skip: every owned field EXCEPT the uncovered
        // fields this sibling is the assigned releaser for.
        let mut skip: Vec<u32> = owned_field_set
            .iter()
            .copied()
            .filter(|f| releaser.get(f) != Some(&sib))
            .collect();
        skip.sort_unstable();

        // Does the widened skip cover ALL owned RC fields? (i.e. this sibling
        // releases nothing.)
        let covers_all = owned_field_set.iter().all(|f| skip.contains(f));

        if covers_all {
            // Full move: suppress the dec entirely + (via the
            // `inc_suppressed_vars = full_move_vars` coupling) the FRESH-site
            // dup-alias inc. Remove any stale partial entry.
            partial_move_vars.remove(&sib);
            full_move_vars.insert(sib);
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = group.root.index(),
                sibling = sib.index(),
                "sibling-moved-field verdict: widened (full-move)"
            );
        } else {
            // Mixed coverage: keep a partial dec with the widened skip. Only
            // emit when the sibling carries RC (in owned_vars_needing_rc); a
            // borrowed/scalar carrier never had a dec.
            if owned_vars_needing_rc.contains(&sib) {
                partial_move_vars.insert(sib, skip.clone());
                full_move_vars.remove(&sib);
            }
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = ?func.name,
                root = group.root.index(),
                sibling = sib.index(),
                ?skip,
                "sibling-moved-field verdict: widened (partial, kept uncovered)"
            );
        }
    }
    Some(GroupVerdict { keeper })
}

/// Keeper-ledger assignment for one group (RL-1): pick the ONE sibling whose
/// whole `burden_dec` balances the dup INTERMEDIATE's kept inc, or name the
/// declining gate. `Ok(None)` = no dup intermediate (no keeper needed).
/// v1 conservative declines: >1 dup intermediate (one whole dec cannot balance
/// two kept incs); a keeper that MOVED a field (its dec is partial — cannot
/// balance the whole inc); an intermediate used outside the group's sibling
/// `Let` aliases (the kept inc may be balanced elsewhere); union not covering all owned
/// fields (an uncovered field needs the entering reference released too — one
/// whole dec cannot do both). Spec: Annex E §AIMS RL-1
/// (`RL1_duplication_balanced`).
fn assign_keeper(
    func: &ArcFunction,
    group: &SiblingGroup,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    union_fields: &FxHashSet<u32>,
    owned_field_set: &FxHashSet<u32>,
) -> Result<Option<ArcVarId>, &'static str> {
    let (&dup_int, through_sibs) = match group.dup_intermediates.len() {
        0 => return Ok(None),
        1 => match group.dup_intermediates.iter().next() {
            Some(kv) => kv,
            None => return Err("dup-intermediate-missing"),
        },
        _ => return Err("multi-dup-intermediate"),
    };
    let Some(&last) = through_sibs.last() else {
        return Err("dup-intermediate-no-sibling");
    };
    if moved_out_fields_union
        .get(&last)
        .is_some_and(|f| !f.is_empty())
    {
        return Err("dup-intermediate-keeper-moves");
    }
    if !owned_field_set.iter().all(|f| union_fields.contains(f)) {
        return Err("dup-intermediate-uncovered-field");
    }
    // Every use of the intermediate must be one of the group's sibling Lets —
    // else the kept inc may already be balanced by a release outside the
    // group (keeping the keeper's dec would double-release).
    for block in &func.blocks {
        for instr in &block.body {
            if !instr.used_vars().contains(&dup_int) {
                continue;
            }
            let is_group_let = matches!(
                instr,
                ArcInstr::Let { dst, value: ArcValue::Var(src), .. }
                    if *src == dup_int && group.siblings.contains(dst)
            );
            if !is_group_let {
                return Err("dup-intermediate-escapes-group");
            }
        }
        if block.terminator.used_vars().contains(&dup_int) {
            return Err("dup-intermediate-escapes-group");
        }
    }
    tracing::trace!(
        target: "ori_arc::aims::realize",
        fn_name = ?func.name,
        root = group.root.index(),
        dup_intermediate = dup_int.index(),
        keeper = last.index(),
        "sibling-moved-field keeper assigned (balances intermediate dup inc)"
    );
    Ok(Some(last))
}

/// The set of top-level field indices (`OwnedField.field_path[0]`) carrying RC
/// in `var`'s burden. The grain the sibling union operates at; nested owned
/// fields are released by the outer drop-glue (top-level `field_path[0]`).
/// Shared with the match-handoff extract-transfer verdict
/// (`extract_transfer`) — the same field grain.
pub(super) fn owned_top_level_fields(
    func: &ArcFunction,
    var: ArcVarId,
    type_registry: &TypeRegistry,
) -> FxHashSet<u32> {
    let ty: TypeRef = idx_to_type_ref(func.var_type(var), type_registry);
    let mut fields: FxHashSet<u32> = FxHashSet::default();
    if let Some(burden) = lookup_burden(ty, type_registry) {
        for of in burden.owned_fields() {
            if let Some(&top) = of.field_path.first() {
                fields.insert(top);
            }
        }
    }
    fields
}

/// True iff `var`'s type is a SUM aggregate with >1 variant (a multivariant
/// niche/tagged sum). Such a root's field projections may extract through
/// DIFFERENT variant arms across iterations (pin 15 cross-variant shape), which
/// the sibling union must DECLINE — unifying moved-field sets across distinct
/// variant tags is unsound. A struct (no variants) or single-variant sum is
/// safe to unify.
fn root_is_multivariant_sum(
    func: &ArcFunction,
    var: ArcVarId,
    type_registry: &TypeRegistry,
) -> bool {
    let ty: TypeRef = idx_to_type_ref(func.var_type(var), type_registry);
    let Some(burden) = lookup_burden(ty, type_registry) else {
        return false;
    };
    burden.variant_burdens().take(2).count() >= 2
}

/// True iff `var`'s type carries any RC. Thin wrapper over `burden_carries_rc`
/// for the var's looked-up burden — the gate the unit tests assert on.
#[cfg(test)]
pub(super) fn var_carries_rc(
    func: &ArcFunction,
    var: ArcVarId,
    type_registry: &TypeRegistry,
) -> bool {
    let ty: TypeRef = idx_to_type_ref(func.var_type(var), type_registry);
    lookup_burden(ty, type_registry)
        .as_ref()
        .is_some_and(burden_carries_rc)
}

/// True iff `instr` transfers `var` at an owned position (RL-2). The
/// Apply-owned-param transfer the matrix pin-11 unit test asserts on.
#[cfg(test)]
pub(super) fn instr_transfers_owned(instr: &ArcInstr, var: ArcVarId) -> bool {
    instr_owned_position_transfer_vars(instr).contains(&var)
}

#[cfg(test)]
mod tests;
