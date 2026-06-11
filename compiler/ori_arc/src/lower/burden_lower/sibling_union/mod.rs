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
#[cfg(test)]
use crate::ir::ArcInstr;
use crate::ir::{ArcFunction, ArcVarId};

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

mod groups;
mod verdict;

use groups::build_sibling_groups;
// Test-only re-export so the `tests` child resolves it via `super::`.
#[cfg(test)]
use groups::SiblingGroup;
use verdict::apply_group;

#[cfg(test)]
mod tests;
