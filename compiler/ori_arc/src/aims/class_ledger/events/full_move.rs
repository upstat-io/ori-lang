//! Arm-local FULL-MOVE detection and event rebooking (the branch-exclusive
//! rebuild shape): an aggregate whose every owned field is projected and
//! consumed exactly once into ONE `Construct` transfers WHOLE — its Reads
//! rebook as the move-out Consume (RL-2 `ConstructArg` transfer).

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, FieldPath, NodeIdx};
use crate::aims::intraprocedural::ledger_events::EventSite;
use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcTerminator, ArcVarId};

use super::{ClassEvent, ClassEvents, EventKind};

/// One arm-local FULL MOVE (the branch-exclusive rebuild shape): in
/// `block`, every owned field of one aggregate class is projected and
/// consumed exactly once as an arg of the ONE `Construct` at
/// `construct_index` (outside the class). The aggregate's reference
/// transfers WHOLE into the new construct — the RL-2 `ConstructArg`
/// transfer (`FD_moveout_is_committed_transfer`; the full-skip cell of
/// `FD_skipset_sound`).
pub(crate) struct FullMoveArm {
    pub(crate) block: usize,
    pub(crate) construct_index: usize,
    /// `(body index, projection dst)` of each member-moving `Project`.
    pub(crate) projections: Vec<(usize, ArcVarId)>,
    /// The moved aggregate's class rep.
    pub(crate) class_rep: NodeIdx,
    /// The projected-from member var (the rebooked consume's subject).
    pub(crate) src_var: ArcVarId,
}

/// Detect every arm-local full move in `func` (pure-IR pre-pass, before
/// per-class event extraction). Per block: ONE `Construct` consuming
/// projection dsts; all those `Project`s read ONE aggregate class; each dst
/// used exactly once (at the construct); the projected field set equals the
/// aggregate burden's owned top-level field set; no OTHER use of any
/// aggregate member var in the block (class-internal `Let` aliases and the
/// `Project`s themselves permitted). Fail-closed on any mismatch.
pub(crate) fn detect_full_move_arms(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
) -> Vec<FullMoveArm> {
    let mut arms = Vec::new();
    for block in 0..func.blocks.len() {
        if let Some(arm) = full_move_arm_in_block(func, partition, type_registry, block) {
            arms.push(arm);
        }
    }
    arms
}

/// The [`FullMoveArm`] in `block`, when the shape holds (see
/// [`detect_full_move_arms`]).
fn full_move_arm_in_block(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    block: usize,
) -> Option<FullMoveArm> {
    use crate::ir::ArcInstr;

    let blk = func.blocks.get(block)?;
    // Projections in this block, keyed by dst.
    let mut projections: Vec<(usize, ArcVarId, ArcVarId, u32)> = Vec::new();
    for (i, instr) in blk.body.iter().enumerate() {
        if let ArcInstr::Project {
            dst, value, field, ..
        } = instr
        {
            projections.push((i, *dst, *value, *field));
        }
    }
    // ONE Construct consuming projection dsts.
    let mut construct_index: Option<usize> = None;
    for (i, instr) in blk.body.iter().enumerate() {
        let ArcInstr::Construct { args, .. } = instr else {
            continue;
        };
        if !projections
            .iter()
            .any(|&(_, pdst, _, _)| args.contains(&pdst))
        {
            continue;
        }
        if construct_index.is_some() {
            return None;
        }
        construct_index = Some(i);
    }
    let cidx = construct_index?;
    let ArcInstr::Construct {
        dst: construct_dst,
        args: construct_args,
        ..
    } = blk.body.get(cidx)?
    else {
        return None;
    };
    // The moved projections: those consumed by the construct. All must read
    // ONE class; each dst used exactly once (at the construct).
    let moved: Vec<(usize, ArcVarId, ArcVarId, u32)> = projections
        .iter()
        .copied()
        .filter(|&(_, pdst, _, _)| construct_args.contains(&pdst))
        .collect();
    let (_, _, first_src, _) = *moved.first()?;
    let first_node = partition.register_node(first_src, FieldPath::whole_var());
    let class_rep = partition.rep_of(first_node);
    let construct_node = partition.register_node(*construct_dst, FieldPath::whole_var());
    if partition.rep_of(construct_node) == class_rep {
        return None;
    }
    for &(_, pdst, src, _) in &moved {
        let src_node = partition.register_node(src, FieldPath::whole_var());
        if partition.rep_of(src_node) != class_rep {
            return None;
        }
        if construct_args.iter().filter(|&&arg| arg == pdst).count() != 1 {
            return None;
        }
        let other_use = blk
            .body
            .iter()
            .enumerate()
            .any(|(i, instr)| i != cidx && instr.uses_var(pdst))
            || blk.terminator.uses_var(pdst);
        if other_use {
            return None;
        }
    }
    if !class_uses_confined_to_moves(func, partition, class_rep, block, &moved) {
        return None;
    }
    if moved_class_shares_edge_source(func, partition, class_rep) {
        return None;
    }
    if !moved_fields_cover_owned(func, type_registry, first_src, &moved) {
        return None;
    }
    tracing::trace!(
        target: "ori_arc::aims::class_ledger",
        block,
        construct_index = cidx,
        "full-move arm detected: every owned field moved into one Construct"
    );
    Some(FullMoveArm {
        block,
        construct_index: cidx,
        projections: moved.iter().map(|&(i, pdst, _, _)| (i, pdst)).collect(),
        class_rep,
        src_var: first_src,
    })
}

/// No OTHER use of the moved aggregate in `block`: permitted uses are the
/// moved `Project`s themselves and `Let` aliases inside the tracked set.
/// The tracked set is the union of the class's partition members AND the
/// block-local `Let`-alias closure of the projected-from vars — the
/// per-source partition can split runtime-same-allocation lineages into
/// sibling classes (a loop-header init param vs the iteration param), and a
/// terminator hand-off of ANY alias of the moved aggregate means the value
/// survives the arm (the loop-header-merge-read over-fire: rebooking there
/// releases a field the next iteration still reads).
fn class_uses_confined_to_moves(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class_rep: NodeIdx,
    block: usize,
    moved: &[(usize, ArcVarId, ArcVarId, u32)],
) -> bool {
    use crate::ir::{ArcInstr, ArcValue};

    let Some(blk) = func.blocks.get(block) else {
        return false;
    };
    let mut tracked: FxHashSet<ArcVarId> = {
        let nodes = partition.nodes_snapshot();
        nodes
            .iter()
            .filter(|(_, path, _)| path.is_whole_var())
            .filter(|&&(_, _, node)| partition.rep_of(node) == class_rep)
            .map(|&(var, _, _)| var)
            .collect()
    };
    for &(_, _, src, _) in moved {
        tracked.insert(src);
    }
    // Close over block-local `Let { Var }` edges in BOTH directions until
    // fixpoint: an alias of a tracked var and the var an alias reads are
    // the SAME runtime value.
    loop {
        let mut grew = false;
        for instr in &blk.body {
            let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            if tracked.contains(dst) && tracked.insert(*src) {
                grew = true;
            }
            if tracked.contains(src) && tracked.insert(*dst) {
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    for (i, instr) in blk.body.iter().enumerate() {
        // The aggregate must not be BORN in the arm block: a same-block
        // birth keeps the aggregate's own release here (its events are not
        // all Reads, so the rebook cannot apply) while the field credits
        // would still inject — the half-applied booking double-funds the
        // store (the no-loop single-reassign shape). `Let` aliases define
        // without birthing and stay permitted.
        if !matches!(instr, ArcInstr::Let { .. })
            && instr
                .defined_var()
                .is_some_and(|dst| tracked.contains(&dst))
        {
            return false;
        }
        for &member in &tracked {
            if !instr.uses_var(member) {
                continue;
            }
            let permitted = matches!(instr, ArcInstr::Project { .. })
                && moved.iter().any(|&(idx, _, _, _)| idx == i)
                || matches!(
                    instr,
                    ArcInstr::Let { value: ArcValue::Var(v), .. } if *v == member
                ) && matches!(instr, ArcInstr::Let { dst, .. } if tracked.contains(dst));
            if !permitted {
                return false;
            }
        }
    }
    tracked
        .iter()
        .all(|&member| !blk.terminator.uses_var(member))
}

/// Whether ANY `Jump` edge feeds a param of the moved class AND a param of
/// a DIFFERENT class from same-class args — the two lineages may alias ONE
/// runtime allocation on that edge (the loop-header init param vs iteration
/// param shape), so a full move through one lineage strands the other's
/// reads. Decline the arm.
fn moved_class_shares_edge_source(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    class_rep: NodeIdx,
) -> bool {
    for blk in &func.blocks {
        let ArcTerminator::Jump { target, args } = &blk.terminator else {
            continue;
        };
        let Some(target_blk) = func.blocks.get(target.index()) else {
            continue;
        };
        let mut moved_arg_reps: Vec<NodeIdx> = Vec::new();
        let mut other_arg_reps: Vec<NodeIdx> = Vec::new();
        for (i, &(param, _)) in target_blk.params.iter().enumerate() {
            let Some(&arg) = args.get(i) else {
                continue;
            };
            let param_node = partition.register_node(param, FieldPath::whole_var());
            let arg_node = partition.register_node(arg, FieldPath::whole_var());
            let arg_rep = partition.rep_of(arg_node);
            if partition.rep_of(param_node) == class_rep {
                moved_arg_reps.push(arg_rep);
            } else {
                other_arg_reps.push(arg_rep);
            }
        }
        if moved_arg_reps
            .iter()
            .any(|rep| other_arg_reps.contains(rep))
        {
            return true;
        }
    }
    false
}

/// The moved field set equals the aggregate burden's owned top-level field
/// set (non-empty).
fn moved_fields_cover_owned(
    func: &ArcFunction,
    type_registry: &ori_types::TypeRegistry,
    src_var: ArcVarId,
    moved: &[(usize, ArcVarId, ArcVarId, u32)],
) -> bool {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    let Some(&src_ty) = func.var_types.get(src_var.index()) else {
        return false;
    };
    let Some(BurdenRef::User(user)) =
        lookup_burden(idx_to_type_ref(src_ty, type_registry), type_registry)
    else {
        return false;
    };
    let owned: FxHashSet<u32> = user
        .owned_fields
        .iter()
        .filter_map(|f| f.field_path.first().copied())
        .collect();
    if owned.is_empty() {
        return false;
    }
    let moved_fields: FxHashSet<u32> = moved.iter().map(|&(_, _, _, field)| field).collect();
    moved_fields == owned
}

/// Extraction-credit sites `(block, body_index)` this class picks up from
/// the detected full-move arms: the member-moving `Project`s whose dst is a
/// member of THIS class (the field-view side of the transfer — the field's
/// reference rides the aggregate's move, so the extraction re-acquires it
/// and no duplication inc is owed).
pub(crate) fn full_move_credit_sites(
    partition: &mut BirthSitePartition,
    arms: &[FullMoveArm],
    class: NodeIdx,
) -> Vec<(usize, usize)> {
    let class_rep = partition.rep_of(class);
    let mut sites = Vec::new();
    for arm in arms {
        for &(index, dst) in &arm.projections {
            let node = partition.register_node(dst, FieldPath::whole_var());
            if partition.rep_of(node) == class_rep {
                sites.push((arm.block, index));
            }
        }
    }
    sites
}

/// The aggregate side of the full-move rebook: when THIS class is a
/// detected arm's moved aggregate and its events in that block are all
/// Reads, they rebook to ONE move-out Consume at the Construct site — the
/// per-path owed counts then agree at the downstream merge. Fail-closed:
/// any other event shape leaves the block untouched (the per-class verify
/// walk re-checks the rebooked stream independently).
pub(crate) fn apply_full_move_rebook(
    partition: &mut BirthSitePartition,
    arms: &[FullMoveArm],
    class: NodeIdx,
    events: &mut ClassEvents,
) {
    let class_rep = partition.rep_of(class);
    for arm in arms {
        if arm.class_rep != class_rep {
            continue;
        }
        let Some(evs) = events.per_block.get_mut(arm.block) else {
            continue;
        };
        if evs.is_empty() || !evs.iter().all(|ev| ev.kind == EventKind::Read) {
            continue;
        }
        evs.clear();
        evs.push(ClassEvent {
            site: EventSite::Body(arm.construct_index),
            kind: EventKind::Consume,
            var: Some(arm.src_var),
            delta: -1,
            floor: 1,
        });
        events.books_runtime_grounded = false;
        tracing::trace!(
            target: "ori_arc::aims::class_ledger",
            block = arm.block,
            construct_index = arm.construct_index,
            "full-move arm rebooked: the class's Reads become its move-out Consume"
        );
    }
}
