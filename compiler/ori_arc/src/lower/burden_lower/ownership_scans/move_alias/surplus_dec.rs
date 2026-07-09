//! Same-allocation surplus-dec suppression arms consumed by
//! [`super::compute_transfer_via_move_alias`]: the single-alias
//! borrow-view-dst keystone + the joint borrow-projection arm, plus the
//! shared same-allocation-identity helpers both arms and
//! [`super::multi_borrow_view`] depend on.

use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::intraprocedural::state_map::ApplyAliasSource;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};
use crate::lower::burden::{Burden, TypeRef};
use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

use super::super::instr_transfer_vars;
use super::SurplusDecInputs;

/// Run both same-allocation surplus-dec suppression arms (each RL-2
/// release-once), each behind its own toggle. Returns the union of `%s` (surplus
/// source) sets to mark transferred. Spec: Annex E §AIMS RL-2 + TF-4.
pub(super) fn collect_surplus_dec_srcs(
    func: &ArcFunction,
    inp: &SurplusDecInputs<'_>,
) -> FxHashSet<ArcVarId> {
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    if !super::super::super::borrow_view_dst_surplus_dec_suppress_disabled() {
        out.extend(collect_borrow_view_dst_surplus_dec_srcs(func, inp));
    }
    if !super::super::super::project_return_surplus_owner_dec_suppress_disabled() {
        out.extend(collect_project_return_surplus_owner_dec_srcs(func, inp));
    }
    out
}

/// Borrow-view-dst surplus-dec suppression source set (RL-2 release-once). A
/// use-once owned source `%s` whose sole `Let { Var }` alias `%d` is a
/// same-allocation borrow-view (`genuine_same_alloc_reps` proves identity) that
/// is live downstream: `%s`'s own scope-exit dec is the SURPLUS same-allocation
/// dec (the lineage's single release is the edge-cleanup dec on the shared rep).
/// Returns the `%s` set to mark transferred. Spec: Annex E §AIMS RL-2.
fn collect_borrow_view_dst_surplus_dec_srcs(
    func: &ArcFunction,
    inp: &SurplusDecInputs<'_>,
) -> FxHashSet<ArcVarId> {
    let owned_vars_needing_rc = inp.owned_vars_needing_rc;
    let use_counts = inp.use_counts;
    let last_use_points = inp.last_use_points;
    let src_has_dead_alias = inp.src_has_dead_alias;
    let param_vars = inp.param_vars;
    let genuine_same_alloc_reps = inp.same_alloc.genuine_same_alloc_reps;
    let mut entry_count: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut last_pos: FxHashMap<ArcVarId, (usize, usize)> = FxHashMap::default();
    for &(var, b, i) in last_use_points {
        *entry_count.entry(var).or_default() += 1;
        last_pos.insert(var, (b, i));
    }
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            let (dst, src) = (*dst, *src);
            // `%s` owned-RC + use-once + this alias is its single-block last use +
            // no dead alias (a dead alias's ref is discharged by the kept terminal
            // dec, never the borrow-view path) + not a param.
            let single_block_last_use = entry_count.get(&src).copied().unwrap_or(0) == 1
                && last_pos.get(&src) == Some(&(block_idx, instr_idx));
            if !owned_vars_needing_rc.contains(&src)
                || use_counts.get(&src).copied().unwrap_or(0) != 1
                || !single_block_last_use
                || src_has_dead_alias.contains(&src)
                || param_vars.contains(&src)
            {
                continue;
            }
            // `%d` is a SAME-ALLOCATION borrow-view that is LIVE downstream.
            let same_alloc = genuine_same_alloc_reps.get(&dst).copied()
                == genuine_same_alloc_reps.get(&src).copied()
                && genuine_same_alloc_reps.contains_key(&dst);
            let dst_is_borrow_view = !owned_vars_needing_rc.contains(&dst);
            let dst_live = use_counts.get(&dst).copied().unwrap_or(0) >= 1;
            if same_alloc && dst_is_borrow_view && dst_live {
                out.insert(src);
            }
        }
    }
    out
}

/// Joint borrow-projection surplus-owner-dec suppression source set (RL-2
/// release-once + TF-4). A borrowed-receiver callee that returns `arg.field`
/// (apply-result `ApplyAliasSource::Project { arg, field }` — `@unwrap(b) =
/// b.value`) hands the caller's OWNED result `%d` a borrow-view of the SAME
/// single allocation as `%s = arg`'s SOLE owned RC field. The base walk emits
/// BOTH `%s`'s scope-exit dec (its drop recurses into the field) AND `%d`'s own
/// scope-exit dec at the joint lineage's last use — two releases of one
/// allocation (the joint borrow-projection double-free, exit 134). The proven
/// joint release theorem: one allocation, exactly one release. The owned result
/// `%d` carries the single release at its TRUE last use (which dominates `%s`'s
/// premature drop); `%s`'s own dec is the SURPLUS same-allocation release —
/// suppress it. Returns the `%s` set to mark transferred. Spec: Annex E §AIMS
/// RL-2 (`RL2_release_exactly_once`) + TF-4 (Project borrow-view identity).
///
/// SAME-ALLOCATION-IDENTITY discriminators bound the family (NOT a use-count /
/// type-membership proxy — each is a structural property of the lineage):
///
/// - **`return_alias = Project { field }`**: the callee's PROVEN contract that
///   the result IS `arg.field` (TF-4 borrow-view of `arg`'s field). Direct /
///   Conditional / Wrapped aliases are handled by other arms; only the
///   `Project` apply-result alias keys this arm.
/// - **SOLE owned RC field**: `arg`'s burden has EXACTLY ONE owned RC field, and
///   it is the returned `field`. A multi-field struct returning one field would
///   LEAK its other owned fields if `arg`'s whole-var dec were suppressed —
///   declined here (a `BurdenDecPartial skip=[field]` is the multi-field
///   extension, deferred to its own cycle). The single-owned-field gate makes
///   `arg`'s whole-var drop a no-op once the field transfers out, so whole-var
///   suppression is sound.
/// - **`%d` live downstream**: the result is used past the call (`use_count ≥
///   1`), so it carries the joint lineage's SINGLE release at its true last use
///   (whether `%d` is a fresh owned result or released by the projected-result
///   borrow-view machinery). A dead result aliases no surviving reference — the
///   surplus would be the result's, not `%s`'s; declined.
/// - **`%s` use-once owned non-param, no dead alias**: matches the keystone's
///   surplus-source gates — a use-once owned source whose single use is the
///   borrowed-receiver call, with no dead duplicate alias whose ref the kept
///   terminal dec must discharge.
fn collect_project_return_surplus_owner_dec_srcs(
    func: &ArcFunction,
    inp: &SurplusDecInputs<'_>,
) -> FxHashSet<ArcVarId> {
    let owned_vars_needing_rc = inp.owned_vars_needing_rc;
    let use_counts = inp.use_counts;
    let src_has_dead_alias = inp.src_has_dead_alias;
    let param_vars = inp.param_vars;
    let apply_result_aliases = inp.same_alloc.apply_result_aliases;
    let type_registry = inp.same_alloc.type_registry;
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for (&dst, source) in apply_result_aliases {
        let ApplyAliasSource::Project { arg, field } = source else {
            continue;
        };
        let (arg, field) = (*arg, *field);
        // `%d` (the call result) is the joint lineage's release carrier: it is
        // live downstream (used past the call), so it carries the SINGLE release
        // at its true last use. Whether `%d` is tracked in `owned_vars_needing_rc`
        // (a fresh owned result) or released by the projected-result borrow-view
        // machinery, the buffer's one release lands at `%d`'s last use — `%s`'s
        // own drop is the surplus regardless. Require only `%d` live (used ≥ 1).
        if use_counts.get(&dst).copied().unwrap_or(0) == 0 {
            continue;
        }
        // `%s` (= the consumed `arg`) is a use-once owned non-param source whose
        // surplus drop is the box's recursive field-drop. No dead alias (its ref
        // would otherwise be discharged by the kept terminal dec).
        if !owned_vars_needing_rc.contains(&arg)
            || use_counts.get(&arg).copied().unwrap_or(0) != 1
            || src_has_dead_alias.contains(&arg)
            || param_vars.contains(&arg)
        {
            continue;
        }
        // SOLE-owned-RC-field gate: `arg`'s burden has exactly ONE owned RC
        // field, and it is the returned `field`. Whole-var suppression is sound
        // only when no OTHER owned field would leak. A multi-field struct
        // returning one field declines (deferred to a partial-dec extension).
        if !arg_sole_owned_rc_field_is(func, arg, field, type_registry) {
            continue;
        }
        out.insert(arg);
    }
    out
}

/// `arg`'s monomorphized burden has EXACTLY ONE owned RC field, and it is
/// `field`. Used by `collect_project_return_surplus_owner_dec_srcs` to gate
/// whole-var dec suppression on the single-owned-field case (whole-var drop
/// becomes a no-op once that field transfers out via the projected return) AND by
/// `walk::extend_owner_last_use_for_borrow_views` to gate the owner-drop
/// borrow-view-liveness placement relocation on the same discriminator.
pub(in crate::lower::burden_lower) fn arg_sole_owned_rc_field_is(
    func: &ArcFunction,
    arg: ArcVarId,
    field: u32,
    type_registry: &TypeRegistry,
) -> bool {
    let arg_ty: TypeRef = idx_to_type_ref(func.var_types[arg.index()], type_registry);
    let Some(burden) = lookup_burden(arg_ty, type_registry) else {
        return false;
    };
    // Each owned field carries a `field_path` (Cow<[u32]>); a TOP-LEVEL owned
    // field has a single-element path `[idx]`. The sole-owned-field gate requires
    // exactly one owned field, at the top level, equal to the returned `field`.
    let owned: Vec<Vec<u32>> = burden
        .owned_fields()
        .map(|f| f.field_path.to_vec())
        .collect();
    owned.len() == 1 && owned[0].as_slice() == [field]
}

/// Whether every use of `alias` across the function is a `Project` that reads
/// `alias` as its source (a borrow-read field extraction, TF-4) whose projected
/// dst is a pure BORROW-VIEW — NOT in `owned_vars_needing_rc`, so it is never
/// independently released (the `.name` / `.settings` read passed straight to a
/// `[borrow]` call). Returns false when `alias` is consumed at any other position
/// (owned call arg, Construct arg, Set, Return, another Let alias, terminator
/// operand) OR when ANY projected field is OWNED-RC (an extracted heap field with
/// its OWN release lineage — `let s_field = m.s` over `Mixed { s: str, xs: [int] }`
/// where `s_field` / `xs_field` get their own scope-exit decs). In the owned-field
/// case the owner's drop is NOT surplus — the fields transferred out and the owner
/// drop is suppressed by other arms; suppressing it here loses a release (leak).
pub(super) fn alias_use_is_borrow_view_project_only(
    func: &ArcFunction,
    alias: ArcVarId,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    let mut saw_use = false;
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Project { dst, value, .. } if *value == alias => {
                    // The projected field must be a pure borrow-view: NOT
                    // independently owned-RC, AND consumed only as a borrow-read
                    // (no `Let { Var }` extraction alias of it, which would move
                    // the field out into its own released lineage — the `Mixed`
                    // `let s_field = m.s` shape where the owner's drop is NOT
                    // surplus because the fields transferred out).
                    if owned_vars_needing_rc.contains(dst)
                        || !projected_field_is_borrow_read_only(func, *dst)
                    {
                        return false;
                    }
                    saw_use = true;
                }
                other => {
                    if other.used_vars().contains(&alias) {
                        return false;
                    }
                }
            }
        }
        if block.terminator.used_vars().contains(&alias) {
            return false;
        }
    }
    saw_use
}

/// Whether the projected field `field_dst` is consumed ONLY as a borrow-read
/// (used at an Invoke/Apply borrowed-arg position, or not at all), with NO
/// `Let { Var }` extraction alias and NO owned-position consume. A field extracted
/// into a downstream `Let` alias (`let s_field = m.s`) becomes its own released
/// lineage, so the owner's whole-var drop is NOT surplus over the borrow-view
/// aliases — declines the multi-borrow-view-alias arm.
fn projected_field_is_borrow_read_only(func: &ArcFunction, field_dst: ArcVarId) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                // A `Let { Var(field_dst) }` is an extraction alias — the field is
                // moved into its own released lineage.
                ArcInstr::Let {
                    value: ArcValue::Var(src),
                    ..
                } if *src == field_dst => return false,
                // Any owned-position consume of the field (Construct arg / owned
                // call arg / `Set` value) transfers it out.
                other => {
                    if instr_transfer_vars(other, func).contains(&field_dst) {
                        return false;
                    }
                }
            }
        }
        // A terminator owned-move (Return value / Jump arg) transfers the field
        // out; a borrowed Invoke arg (`Invoke @length(%f [borrow])`) does NOT and
        // is the permitted borrow-read consume.
        match &block.terminator {
            ArcTerminator::Return { value } if *value == field_dst => return false,
            ArcTerminator::Jump { args, .. } if args.contains(&field_dst) => return false,
            ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
                let used = block.terminator.used_vars();
                for (pos, &v) in used.iter().enumerate() {
                    if v == field_dst && block.terminator.is_owned_position(pos) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}
