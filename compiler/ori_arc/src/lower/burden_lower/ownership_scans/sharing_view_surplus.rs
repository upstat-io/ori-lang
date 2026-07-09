//! RL-1 surplus-inc suppression for seamless-slice RESULTS whose lineage is a
//! read-only in-place co-owner. Spec: Annex E §AIMS RL-1 + RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::borrowed_invoke_lineage::borrow_read_only_closure_vetted;

/// Slice-result `dst` vars whose FRESH-site `BurdenInc` is SURPLUS: a sharing-view
/// builtin (`slice` / `substring` / `take` / `drop`) self-incs the shared backing
/// buffer at runtime (`ori_rc_inc(original_data)`, rc 1 -> 2), so the result is an
/// owned CO-REFERENCE whose `+1` is the runtime's own inc. AIMS emits ONLY the
/// result's balancing last-use dec, never a FRESH-site inc — a second inc here
/// double-counts under sole-emitter Phase-7 lowering (net +1 LEAK; the shared
/// buffer never reaches rc 0).
///
/// SUPPRESS ONLY the read-only-in-place-co-owner shape — gated by THREE conditions
/// that, together, prove two concurrently-live co-owners each releasing once:
///
///  1. The slice RESULT lineage is a pure borrow-read co-owner
///     ([`borrow_read_only_closure_vetted`]): every use of the result + its
///     Let-Var aliases / Jump-arg hops is a borrow read (`.length` / `.first` /
///     borrowed-call arg / scalar `Project`); never materialized / moved into a
///     `Construct` / `Reuse` / `CollectionReuse` arg / `Set` value / `PartialApply`
///     capture / `Return` / owned-position consume.
///  2. The result lineage is NOT fed to ANOTHER sharing-view producer (the chained
///     `xs.take(4).drop(1)` shape — the intermediate result is both a result and a
///     receiver; its keep-alive inc funds the chain).
///  3. The RECEIVER (the slice source, arg 0's root) SURVIVES the slice call and is
///     independently read downstream (its own borrow-read use forward-reachable from
///     the slice site), OR the receiver's terminal-read release was RELOCATED past
///     the consuming `Invoke` (`relocated_borrowed_invoke_args`) — the relocated
///     dec releases the birth ref AFTER the call, so the runtime self-inc alone
///     carries the view and the FRESH-site inc is surplus. When the receiver dies
///     AT the slice with a NON-relocated inline dec (legacy pre-terminator
///     placement), the result's FRESH-site inc is the buffer's sole forward
///     carrier and is LOAD-BEARING — those shapes MUST keep the inc (the
///     over-fire boundary).
///
/// Empty when `ORI_DISABLE_SHARING_VIEW_SURPLUS_INC_SUPPRESS=1` (the burden path
/// falls back to the conservative `MaybeShared` FRESH-site inc — the pre-cure
/// behavior). Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`) + RL-2
/// (`RL2_release_exactly_once`).
pub(in crate::lower::burden_lower) fn compute_sharing_view_surplus_inc_dsts(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    relocated_borrowed_invoke_args: &FxHashMap<usize, FxHashSet<ArcVarId>>,
) -> FxHashSet<ArcVarId> {
    if std::env::var("ORI_DISABLE_SHARING_VIEW_SURPLUS_INC_SUPPRESS").as_deref() == Ok("1") {
        return FxHashSet::default();
    }
    let sharing_names = sharing_view_self_inc_names(interner);
    let mut dsts: FxHashSet<ArcVarId> = FxHashSet::default();

    let result_repr_admits = |dst: ArcVarId| -> bool {
        matches!(
            func.var_repr(dst),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };

    let relocated_vars: FxHashSet<ArcVarId> = relocated_borrowed_invoke_args
        .values()
        .flat_map(|vars| vars.iter().copied())
        .collect();

    let mut consider = |dst: ArcVarId, callee: &Name, args: &[ArcVarId]| {
        if !sharing_names.contains(callee) {
            return;
        }
        if !result_repr_admits(dst) {
            return;
        }
        let Some(&recv) = args.first() else {
            return;
        };
        // (1) result lineage is a pure read-only co-owner.
        if borrow_read_only_closure_vetted(func, dst).is_none() {
            return;
        }
        // A result whose OWN terminal-read release was relocated past its
        // consuming `Invoke` needs neither chain-funding (gate 2) nor a
        // surviving receiver (gate 3): the runtime self-inc is its single
        // funded ref and the relocated dec is its single release — the
        // FRESH-result inc is surplus unconditionally.
        let dst_relocated = relocated_vars.contains(&dst);
        // (2) result not fed to another sharing-view (chained slice).
        if !dst_relocated && result_feeds_sharing_view(func, &sharing_names, dst) {
            return;
        }
        // (3) the receiver source survives the slice + is read downstream, OR
        // its terminal-read release was relocated past the consuming Invoke
        // (the relocated dec releases the birth ref after the call, leaving
        // the runtime self-inc as the view's sole funding).
        if !dst_relocated
            && !receiver_survives_and_read_downstream(func, &sharing_names, recv)
            && !receiver_family_relocated(func, recv, &relocated_vars)
        {
            return;
        }
        dsts.insert(dst);
    };

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                consider(*dst, callee, args);
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            consider(*dst, callee, args);
        }
    }
    dsts
}

/// Seamless-slice builtins whose runtime self-incs the shared backing buffer:
/// `slice` / `substring` (the [`crate::borrow::sharing_builtin_names`] SSOT) plus
/// `take` / `drop` (also `make_slice_cap` views via `ori_list_slice_take` /
/// `ori_list_slice_drop`, which delegate to `ori_list_slice`). Matches
/// `emit_unified::sharing_view_relocation_names` exactly.
fn sharing_view_self_inc_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    let mut names = crate::borrow::sharing_builtin_names(interner);
    names.insert(interner.intern("take"));
    names.insert(interner.intern("drop"));
    names
}

/// True iff the slice RESULT lineage `dst` (its Let-Var aliases included) is
/// consumed as the RECEIVER (arg 0) of ANOTHER sharing-view producer — the chained
/// `xs.take(4).drop(1)` shape. The chain's intermediate result is both a result and
/// a receiver, needing joint chain accounting the surplus-strip does not model.
fn result_feeds_sharing_view(
    func: &ArcFunction,
    sharing_names: &FxHashSet<Name>,
    dst: ArcVarId,
) -> bool {
    // Lineage = dst + its whole-var Let aliases (a chained `.drop` reads the
    // `let ys = xs.take(..)` binding, not the raw Apply dst).
    let mut lineage: FxHashSet<ArcVarId> = FxHashSet::default();
    lineage.insert(dst);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst: ld,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if lineage.contains(src) && lineage.insert(*ld) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    let feeds = |callee: &Name, args: &[ArcVarId]| -> bool {
        sharing_names.contains(callee) && args.first().is_some_and(|a| lineage.contains(a))
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if feeds(callee, args) {
                    return true;
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if feeds(callee, args) {
                return true;
            }
        }
    }
    false
}

/// True iff the slice RECEIVER source (`recv`'s Construct / alias root) SURVIVES the
/// slice call and is INDEPENDENTLY READ downstream — proving two concurrently-live
/// co-owners (the source `xs` + the slice result `ys`), each releasing once. When
/// the source instead dies AT the slice (no non-slice borrow-read use), the result's
/// FRESH-site inc is the buffer's sole forward carrier and is LOAD-BEARING; decline.
///
/// "Read downstream" = a body / terminator use of the source ROOT (its Construct
/// def + whole-var Let aliases) that is NOT a sharing-view call arg and NOT a pure
/// RC bookkeeping op. Conservative: a single such read admits; none declines.
fn receiver_survives_and_read_downstream(
    func: &ArcFunction,
    sharing_names: &FxHashSet<Name>,
    recv: ArcVarId,
) -> bool {
    // Root the receiver to its Construct/alias-source family (whole-var Let aliases,
    // both directions, so `%7 = %5` and the receiver `%7` resolve to one family).
    let mut family: FxHashSet<ArcVarId> = FxHashSet::default();
    family.insert(recv);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if (family.contains(dst) && family.insert(*src))
                        || (family.contains(src) && family.insert(*dst))
                    {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    // Is a family member used as a sharing-view call arg (the slice consume itself)?
    let is_sharing_view_arg = |callee: &Name, args: &[ArcVarId], v: ArcVarId| -> bool {
        sharing_names.contains(callee) && args.first().is_some_and(|&a| a == v)
    };

    for block in &func.blocks {
        for instr in &block.body {
            // Skip the family's own alias-def edges + RC bookkeeping.
            match instr {
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
                | ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::BurdenInc { .. }
                | ArcInstr::BurdenDec { .. } => {}
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    // A non-receiver-arg read of a family member is a survival read.
                    if instr
                        .used_vars()
                        .iter()
                        .any(|&v| family.contains(&v) && !is_sharing_view_arg(callee, args, v))
                    {
                        return true;
                    }
                }
                _ => {
                    if instr.used_vars().iter().any(|v| family.contains(v)) {
                        return true;
                    }
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if block
                .terminator
                .used_vars()
                .iter()
                .any(|&v| family.contains(&v) && !is_sharing_view_arg(callee, args, v))
            {
                return true;
            }
        } else if block
            .terminator
            .used_vars()
            .iter()
            .any(|v| family.contains(v))
        {
            return true;
        }
    }
    false
}

/// True iff the slice RECEIVER (or a whole-var Let alias in its family) had its
/// terminal-read release RELOCATED to a consuming `Invoke`'s normal-successor
/// entry. Relocation only fires when the Invoke is the var's execution-final
/// read, so a relocated family member's birth ref is released AFTER every read
/// — the runtime self-inc alone funds the slice view.
fn receiver_family_relocated(
    func: &ArcFunction,
    recv: ArcVarId,
    relocated_vars: &FxHashSet<ArcVarId>,
) -> bool {
    if relocated_vars.is_empty() {
        return false;
    }
    let mut family: FxHashSet<ArcVarId> = FxHashSet::default();
    family.insert(recv);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if (family.contains(dst) && family.insert(*src))
                        || (family.contains(src) && family.insert(*dst))
                    {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    family.iter().any(|v| relocated_vars.contains(v))
}
