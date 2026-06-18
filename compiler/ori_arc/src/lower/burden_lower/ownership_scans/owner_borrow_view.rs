//! TF-14 owner-drop borrow-view-liveness placement extension: an owned
//! sole-owned-RC-field aggregate whose projected borrow-view is read at a
//! borrowed-`Invoke` terminator arg must release its field AFTER the borrow-read
//! (the owner's `[AggFields]` drop cascade-frees the field). Relocates the owner's
//! whole-var release to the borrowed-`Invoke` normal-successor `BlockEntry` and
//! suppresses the owner's own in-block dec. Gated on the SAME same-allocation
//! sole-owned-field + fresh-Construct-root + borrow-only-non-forwarder
//! discriminators as the surplus-dec arms (`move_alias.rs`), plus single-pred +
//! non-loop placement gates. Spec: Annex E §AIMS TF-14 + RL-1 + RL-2 + RL-4.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::TypeRegistry;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

use super::super::BurdenLowerCtx;
use super::move_alias::arg_sole_owned_rc_field_is;

/// TF-14 owner-drop placement extension: relocate a fresh sole-owned-RC-field
/// aggregate's whole-var release past the last read of its `Project` borrow-view.
///
/// A `Project { dst: view, value: agg }` is a TF-4 borrow-view of `agg`'s field;
/// `agg`'s whole-var `BurdenDec` (its `[AggFields]`/`[InlineEnum]` drop-glue)
/// cascade-FREES that field. When the borrow-view `view` (or a `Let{Var}` alias
/// of it) is read at a borrowed-`Invoke` terminator arg AFTER `agg`'s own
/// syntactic last use — `let xs = c.items; xs.fold(..)` — placing `agg`'s drop at
/// `agg`'s last use frees the field the borrow-read still consults: a
/// use-after-free. TF-14 `TF14_propagation_spec` joins the source's liveness with
/// the view's; RL-2 `RL2_release_exactly_once` keeps exactly one release, placed
/// past the borrow-read. The owner release is relocated to the borrowed-call's
/// normal-successor `BlockEntry` and `agg`'s own in-block dec is suppressed.
///
/// Per `agg` the decision cascade lives in `admit_owner_borrow_view_relocation`,
/// gated on the SAME same-allocation discriminators the surplus-dec arms
/// (`move_alias.rs`) use: sole-owned-RC-field + fresh struct/tuple `Construct`
/// root + borrow-only-non-forwarder lineage, plus single-pred + non-loop
/// placement. Returns the suppress set + the placed-release map for
/// `compute_owned_rc_filter` to merge. Spec: Annex E §AIMS TF-14 + RL-1 + RL-2 +
/// RL-4.
pub(in crate::lower::burden_lower) fn extend_owner_last_use_for_borrow_views(
    ctx: &BurdenLowerCtx<'_>,
    func: &ArcFunction,
    type_registry: &TypeRegistry,
) -> OwnerDropBorrowViewExtension {
    let mut out = OwnerDropBorrowViewExtension::default();
    if super::super::owner_drop_borrow_view_liveness_disabled() {
        return out;
    }
    // `agg -> [(view_dst, field), ...]`: every non-take `Project { dst, value: agg,
    // field }` of an aggregate to its borrow-view dst + projected field index.
    // (take-projects move ownership out; they are not borrow-views and never extend
    // the owner's drop.)
    let mut agg_views: FxHashMap<ArcVarId, Vec<(ArcVarId, u32)>> = FxHashMap::default();
    // `src -> [dst]`: `Let { dst, Var(src) }` alias edges, for the view alias
    // closure (a borrow-view aliased via `let xs = view` is the same allocation).
    let mut let_alias_fwd: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Project {
                    dst, value, field, ..
                } => {
                    agg_views.entry(*value).or_default().push((*dst, *field));
                }
                ArcInstr::Let {
                    dst,
                    value: crate::ir::ArcValue::Var(src),
                    ..
                } => {
                    let_alias_fwd.entry(*src).or_default().push(*dst);
                }
                _ => {}
            }
        }
    }
    if agg_views.is_empty() {
        return out;
    }
    // Per-var single last-use position (one entry per var per block in
    // `last_use_points`; for the same-block extension we read the var's last-use
    // in `agg`'s block). Build a (var, block) -> instr index map.
    let mut last_use_in_block: FxHashMap<(ArcVarId, usize), usize> = FxHashMap::default();
    for &(var, b, i) in &ctx.last_use_points {
        last_use_in_block.insert((var, b), i);
    }

    // For each aggregate with a borrow-view, the per-agg gate cascade in
    // `admit_owner_borrow_view_relocation` decides whether to relocate the owner's
    // release to the borrowed-`Invoke` normal successor.
    for (&agg, views) in &agg_views {
        if let Some(succ) = admit_owner_borrow_view_relocation(
            ctx,
            func,
            type_registry,
            agg,
            views,
            &let_alias_fwd,
            &last_use_in_block,
        ) {
            out.suppress.insert(agg);
            out.releases
                .entry((succ, super::ForwarderReleasePos::BlockEntry))
                .or_default()
                .push(agg);
        }
    }
    out
}

/// Per-`agg` gate cascade for [`extend_owner_last_use_for_borrow_views`]. Returns
/// `Some(succ_block)` when `agg`'s whole-var `[AggFields]` release must relocate to
/// the borrowed-`Invoke` normal-successor block entry (and `agg`'s own in-block dec
/// be suppressed); `None` (decline) otherwise. Each gate matches a surplus-dec arm
/// discriminator (sole-owned-field / fresh-Construct-root / borrow-only-non-forwarder)
/// plus the borrowed-`Invoke`-view + single-pred + non-loop placement gates. Spec:
/// Annex E §AIMS TF-14 + RL-1 + RL-2 + RL-4.
fn admit_owner_borrow_view_relocation(
    ctx: &BurdenLowerCtx<'_>,
    func: &ArcFunction,
    type_registry: &TypeRegistry,
    agg: ArcVarId,
    views: &[(ArcVarId, u32)],
    let_alias_fwd: &FxHashMap<ArcVarId, Vec<ArcVarId>>,
    last_use_in_block: &FxHashMap<(ArcVarId, usize), usize>,
) -> Option<usize> {
    // SOLE-owned-RC-field gate: a unique projected field across this agg's views,
    // equal to `agg`'s EXACTLY-ONE owned RC field. A multi-owned-field struct (the
    // other fields would leak / the relocated dec over-releases) or an ambiguous
    // multi-field projection declines.
    let mut proj_field: Option<u32> = None;
    for &(_, field) in views {
        match proj_field {
            None => proj_field = Some(field),
            Some(f) if f == field => {}
            Some(_) => return None,
        }
    }
    let field = proj_field?;
    if !arg_sole_owned_rc_field_is(func, agg, field, type_registry) {
        return None;
    }
    // `agg` must be an INLINE `Aggregate` whose `[AggFields]` whole-var drop
    // cascade-frees the projected field, AND a FRESH-LOCAL struct/tuple `Construct`
    // root (NOT a call result aliasing a forwarder param). Same fresh-root + repr
    // requirements the surplus-dec arms use.
    if !matches!(func.var_repr(agg), Some(crate::ir::ValueRepr::Aggregate))
        || !agg_is_fresh_struct_construct(func, agg)
    {
        return None;
    }
    let (agg_block, agg_instr) = ctx
        .last_use_points
        .iter()
        .find(|(v, _, _)| *v == agg)
        .map(|(_, b, i)| (*b, *i))?;
    // The view lineage: view dsts + transitive Let-Var aliases.
    let mut lineage: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut stack: Vec<ArcVarId> = views.iter().map(|(d, _)| *d).collect();
    while let Some(v) = stack.pop() {
        if !lineage.insert(v) {
            continue;
        }
        if let Some(children) = let_alias_fwd.get(&v) {
            stack.extend(children.iter().copied());
        }
    }
    // BORROW-ONLY + NON-FORWARDER vetting: neither `agg` NOR any lineage member is
    // consumed at an owned position / transferred out at a terminator (a moved-out
    // field or a forwarder/transfer-through-return result whose release is NOT its
    // `[AggFields]` drop — relocating then double-frees).
    if agg_or_lineage_consumed_at_owned_position(func, agg, &lineage) {
        return None;
    }
    // The view's max same-block last-use must be strictly later than `agg`'s own.
    let extended_instr = lineage
        .iter()
        .filter_map(|m| last_use_in_block.get(&(*m, agg_block)).copied())
        .max()
        .unwrap_or(agg_instr)
        .max(agg_instr);
    if extended_instr <= agg_instr {
        return None;
    }
    // ONLY the borrowed-`Invoke`-terminator view-read shape: the borrowed call
    // completes on this block's terminator and the buffer survives to the normal
    // successor. A body-instruction / owned-terminator extension declines.
    let term_idx = func.blocks[agg_block].body.len();
    if extended_instr != term_idx
        || !lineage_member_at_borrowed_invoke_terminator(func, agg_block, &lineage)
    {
        return None;
    }
    let crate::ir::ArcTerminator::Invoke { normal, .. } = &func.blocks[agg_block].terminator else {
        return None;
    };
    let succ = normal.index();
    // Single-pred successor (a multi-pred merge release would over-release on other
    // predecessors) + non-loop placement (a per-iteration fresh `agg` needs a
    // per-iteration release, not a single relocated one — the loop-carried back-edge
    // net is distinct infrastructure).
    if predecessor_count(func, succ) != 1
        || block_in_cycle(func, agg_block)
        || block_in_cycle(func, succ)
    {
        return None;
    }
    tracing::trace!(
        target: "ori_arc::aims::realize",
        agg = agg.index(),
        field,
        from_instr = agg_instr,
        succ,
        "owner-drop borrow-view liveness: sole-owned-field aggregate, \
         borrowed-Invoke view — owner release relocated to normal-successor \
         block entry"
    );
    Some(succ)
}

/// Output of [`extend_owner_last_use_for_borrow_views`]: the owner aggregates
/// whose own in-block last-use `BurdenDec` is SUPPRESSED (the borrowed-`Invoke`
/// case relocates the release to the normal successor) + the placed-release map
/// keyed by `(succ_block, ForwarderReleasePos::BlockEntry)`. Merged by
/// `compute_owned_rc_filter` into the `forwarder_result_releases` surface +
/// `transferred` suppression set. Spec: Annex E §AIMS TF-14 + RL-2 + RL-4.
#[derive(Default)]
pub(in crate::lower::burden_lower) struct OwnerDropBorrowViewExtension {
    pub(in crate::lower::burden_lower) suppress: FxHashSet<ArcVarId>,
    pub(in crate::lower::burden_lower) releases:
        FxHashMap<(usize, super::ForwarderReleasePos), Vec<ArcVarId>>,
}

/// True iff some `lineage` member is used at a BORROWED arg position of `block`'s
/// `Invoke` / `InvokeIndirect` terminator (the borrow-read that survives to the
/// normal successor — RL-2 the callee does not free a borrowed arg).
fn lineage_member_at_borrowed_invoke_terminator(
    func: &ArcFunction,
    block: usize,
    lineage: &FxHashSet<ArcVarId>,
) -> bool {
    let term = &func.blocks[block].terminator;
    if !matches!(
        term,
        crate::ir::ArcTerminator::Invoke { .. } | crate::ir::ArcTerminator::InvokeIndirect { .. }
    ) {
        return false;
    }
    for (pos, &var) in term.used_vars().iter().enumerate() {
        if lineage.contains(&var) && !term.is_owned_position(pos) {
            return true;
        }
    }
    false
}

/// True iff `agg` resolves (through `Let { Var }` aliases) to a FRESH struct/tuple
/// `Construct` (rc=1 owner of a freshly-built field) — NOT a call result /
/// `Project` / param. Excludes the forwarder-result-aliasing-a-param case
/// (`Apply`/`Invoke` dst whose release is the forwarder-result machinery's),
/// matching the surplus-dec arms' fresh-root requirement. The owner aggregate is
/// commonly a `Let`-Var alias of the `Construct` dst (`%6 = %4` where
/// `%4 = Construct Struct(..)`), so the chain is followed. Spec: Annex E §AIMS
/// RL-1 + TF-3.
fn agg_is_fresh_struct_construct(func: &ArcFunction, agg: ArcVarId) -> bool {
    // Build the defining instruction per dst (one pass).
    let mut def: FxHashMap<ArcVarId, &ArcInstr> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { dst, .. } | ArcInstr::Let { dst, .. } => {
                    def.insert(*dst, instr);
                }
                _ => {}
            }
        }
    }
    let mut cur = agg;
    for _ in 0..64 {
        match def.get(&cur) {
            Some(ArcInstr::Construct { ctor, .. }) => {
                return matches!(
                    ctor,
                    crate::ir::CtorKind::Struct(_) | crate::ir::CtorKind::Tuple
                );
            }
            Some(ArcInstr::Let {
                value: crate::ir::ArcValue::Var(src),
                ..
            }) => cur = *src,
            _ => return false,
        }
    }
    false
}

/// True iff `block` lies on a CFG cycle — `block` is reachable from one of its own
/// successors (a loop body / header re-reaches itself). Used to decline the
/// per-iteration loop-carried fresh-root case (needs per-iteration release, not a
/// single relocated one).
fn block_in_cycle(func: &ArcFunction, block: usize) -> bool {
    use crate::graph::successor_block_ids;
    let n = func.blocks.len();
    if block >= n {
        return false;
    }
    let mut seen: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = successor_block_ids(&func.blocks[block].terminator)
        .into_iter()
        .map(crate::ir::ArcBlockId::index)
        .collect();
    while let Some(b) = stack.pop() {
        if b == block {
            return true;
        }
        if b >= n || !seen.insert(b) {
            continue;
        }
        stack.extend(
            successor_block_ids(&func.blocks[b].terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index),
        );
    }
    false
}

/// Count the CFG predecessors of `block` (Jump / Branch / Switch / Invoke edges).
fn predecessor_count(func: &ArcFunction, block: usize) -> usize {
    use crate::graph::successor_block_ids;
    let mut count = 0;
    for b in &func.blocks {
        if successor_block_ids(&b.terminator)
            .into_iter()
            .any(|s| s.index() == block)
        {
            count += 1;
        }
    }
    count
}

/// True iff `agg` OR any view-lineage member is consumed at an OWNED position —
/// either an owned instruction position (`instr_transfer_vars`: `Construct` /
/// `Reuse` / `CollectionReuse` arg / owned `Apply`/`Invoke` arg / `Set.value`) OR
/// a terminator transfer-out (the `Return` value, a `Jump` arg, a `Branch` /
/// `Switch` operand, or an OWNED `Invoke`/`InvokeIndirect` arg). An owned consume
/// means the projected field MOVED out (the field is no longer freed solely by
/// `agg`'s `[AggFields]` drop) OR `agg` is a forwarder / transfer-through-return
/// result whose release is NOT its own drop — either case makes relocating the
/// owner release an over-release. The borrow-only + non-forwarder gate the
/// surplus-dec arms apply, lifted to the placement face. Spec: Annex E §AIMS
/// RL-1 + RL-2.
fn agg_or_lineage_consumed_at_owned_position(
    func: &ArcFunction,
    agg: ArcVarId,
    lineage: &FxHashSet<ArcVarId>,
) -> bool {
    let is_member = |v: ArcVarId| v == agg || lineage.contains(&v);
    for block in &func.blocks {
        for instr in &block.body {
            for v in super::instr_transfer_vars(instr, func) {
                if is_member(v) {
                    return true;
                }
            }
        }
        let term = &block.terminator;
        match term {
            // The whole returned value / jumped value / branched scrutinee transfers
            // out (NOT a borrowed-Invoke arg — that survives to the successor and is
            // the admitted shape). A member at an OWNED Invoke/InvokeIndirect arg is
            // also a transfer-out.
            crate::ir::ArcTerminator::Return { value } if is_member(*value) => return true,
            crate::ir::ArcTerminator::Jump { args, .. } if args.iter().any(|a| is_member(*a)) => {
                return true;
            }
            crate::ir::ArcTerminator::Branch { cond, .. } if is_member(*cond) => return true,
            crate::ir::ArcTerminator::Switch { scrutinee, .. } if is_member(*scrutinee) => {
                return true;
            }
            crate::ir::ArcTerminator::Invoke { .. }
            | crate::ir::ArcTerminator::InvokeIndirect { .. } => {
                for (pos, &var) in term.used_vars().iter().enumerate() {
                    if is_member(var) && term.is_owned_position(pos) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}
