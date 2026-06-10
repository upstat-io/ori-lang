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

use ori_types::TypeRegistry;

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
/// `project_origins` is the Pass-1 `project_dst -> (project_src, field)` map
/// (`populate_moved_out_fields`); it names which alias lowered each projection.
pub(super) fn apply_sibling_moved_field_union(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    moved_out_fields_union: &FxHashMap<ArcVarId, FxHashSet<u32>>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    full_move_vars: &mut FxHashSet<ArcVarId>,
    partial_move_vars: &mut FxHashMap<ArcVarId, Vec<u32>>,
) {
    if sibling_moved_field_union_disabled() {
        return;
    }

    let groups = build_sibling_groups(func);

    for group in groups {
        apply_group(
            func,
            type_registry,
            &group,
            moved_out_fields_union,
            owned_vars_needing_rc,
            full_move_vars,
            partial_move_vars,
        );
    }
}

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
}

/// Build sibling-alias groups keyed to the alias-chain root. A group's siblings
/// are the `Let { Var }` dsts that (a) project a field out of an alias of the
/// root and (b) appear in the SAME block (co-reachable v1). The root is the
/// chain terminus: a block-param or a fresh `Construct` local reached by
/// following `Let { Var }` source edges.
fn build_sibling_groups(func: &ArcFunction) -> Vec<SiblingGroup> {
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

    // Resolve an alias dst to its chain root: follow `Let { Var }` source edges
    // until a block-param OR a fresh-`Construct` local is reached. `None` when
    // the chain has no such terminus (e.g. roots in a call result).
    let chain_root = |start: ArcVarId| -> Option<ArcVarId> {
        let mut cur = start;
        for _ in 0..func.var_types.len() {
            if block_params.contains(&cur) || construct_dsts.contains(&cur) {
                return Some(cur);
            }
            match alias_src.get(&cur) {
                Some(&next) => cur = next,
                None => return None,
            }
        }
        None
    };

    // Which block defines each block-param (the param's owning block index).
    let mut param_block: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for &(p, _) in &block.params {
            param_block.insert(p, block_idx);
        }
    }

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
    let mut by_root_block: FxHashMap<(ArcVarId, usize), Vec<ArcVarId>> = FxHashMap::default();
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
            let Some(root) = chain_root(*dst) else {
                continue;
            };
            // DIRECT-alias gate (the inc-aware / dead-end-2 boundary): the
            // carrier MUST be a one-hop `Let { Var }` of the root. An
            // intermediate alias (`let s = r; ... s.a ... s.b`, the
            // explicit-alias / late-use shape) carries its own kept whole-struct
            // duplication inc that the sibling partial-decs balance; suppressing
            // the siblings strands that inc (+1 leak). One-hop carriers of the
            // loop block-param have NO intermediate kept inc to strand.
            if alias_src.get(dst) != Some(&root) {
                continue;
            }
            // BACK-EDGE gate (the loop-carried shape; RCA shape-gate "PLUS a
            // loop back-edge"): the root MUST be a loop block-param whose owning
            // block lies on a CFG cycle. A fresh-`Construct`-rooted rebuild with
            // no back-edge (the `no_loop` straight-line shape) keeps its single
            // scope-exit release; suppressing it leaks the new struct's buffers.
            let Some(&root_block) = param_block.get(&root) else {
                continue;
            };
            if !block_on_cycle(func, root_block) {
                continue;
            }
            by_root_block
                .entry((root, block_idx))
                .or_default()
                .push(*dst);
        }
    }

    by_root_block
        .into_iter()
        .filter_map(|((root, block_idx), mut siblings)| {
            siblings.sort_unstable_by_key(|v| v.index());
            siblings.dedup();
            // A group needs >=2 distinct projection-carrier siblings (the >=2
            // plainly self-projected FIELDS shape gate).
            (siblings.len() >= 2).then_some(SiblingGroup {
                root,
                siblings,
                block_idx,
            })
        })
        .collect()
}

/// True iff `block_idx` lies on a CFG cycle — reachable from one of its own
/// successors (a loop header / body block carries a back-edge). The
/// loop-carried-rebuild gate; a straight-line `no_loop` rebuild block reaches no
/// successor that returns to it.
fn block_on_cycle(func: &ArcFunction, block_idx: usize) -> bool {
    use crate::graph::successor_block_ids;
    let Some(block) = func.blocks.get(block_idx) else {
        return false;
    };
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = successor_block_ids(&block.terminator)
        .into_iter()
        .map(crate::ir::ArcBlockId::index)
        .collect();
    while let Some(b) = stack.pop() {
        if b == block_idx {
            return true;
        }
        if !visited.insert(b) {
            continue;
        }
        if let Some(blk) = func.blocks.get(b) {
            for s in successor_block_ids(&blk.terminator) {
                stack.push(s.index());
            }
        }
    }
    false
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
) {
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
        return;
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
        return;
    }

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
    let owned_field_set = owned_top_level_fields(func, group.root, type_registry);

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
}

/// The set of top-level field indices (`OwnedField.field_path[0]`) carrying RC
/// in `var`'s burden. The grain the sibling union operates at; nested owned
/// fields are released by the outer drop-glue (top-level `field_path[0]`).
fn owned_top_level_fields(
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
