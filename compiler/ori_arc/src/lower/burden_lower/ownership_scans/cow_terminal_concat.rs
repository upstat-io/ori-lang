//! RL-1 surplus-inc suppression for a FRESH local consumed ONLY as a TERMINAL
//! `Binary(Add)` concat operand. Spec: Annex E §AIMS RL-1 + RL-2.

use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, LitValue, PrimOp, ValueRepr};

/// Fresh-local `dst` vars whose FRESH-site `BurdenInc` is SURPLUS: a fresh
/// rc=1 value (a `Let { Literal::String }` heap literal, or a fresh
/// `Construct`/`Reuse`/`CollectionReuse` collection) consumed EXACTLY ONCE,
/// as an operand of a `Binary(Add)` concat (`a + b` -> `ori_str_concat` /
/// `ori_list_concat_cow`), with NO further use.
///
/// The runtime concat helper BORROWS the operand (`a` passed by pointer) and
/// the caller emits exactly one dec after the call (RL-2 `ApplyToBorrowedParam`);
/// the operand is therefore move-once-linear (`incElidable`, DP-3) — its single
/// reference moves into the borrowed-then-caller-dec consume, so the keep-alive
/// `BurdenInc` is surplus and would leave the rc=1 allocation at net +1 under
/// sole-emitter Phase-7 lowering (alloc rc=1 + inc rc=2 - one dec rc=1 -> LEAK).
/// Suppress ONLY the fresh-site inc; the operand's own last-use dec (and the
/// edge-cleanup unwind dec) — mutually exclusive across the normal vs unwind
/// path of the concat's enclosing may-unwind region — remain the single release.
///
/// The over-fire boundary (each declines a load-bearing sibling):
///  - `dst` is consumed EXACTLY ONCE in the whole function (single-use); a
///    re-read AFTER the concat (`let s = a + b; a.starts_with(..)`) makes the
///    keep-alive inc LOAD-BEARING — it raises rc >= 2 so `ori_str_concat`
///    COPIES instead of mutating `a` in place, preserving the later read. A
///    multi-use `dst` is excluded by the single-use count.
///  - the single use IS a `Binary(Add)` operand — never an owned-position
///    consume (Construct / store / owned call arg / `Set` value / `PartialApply`
///    capture / `Return` / `Jump` arg), never a push/set/insert COW mutator,
///    never a may-COW user-call arg.
///  - `dst` is FatValue/RcPointer (carries an RC header).
///
/// Empty when `ORI_DISABLE_COW_TERMINAL_CONCAT_INC_ELISION=1` (the burden path
/// falls back to the conservative fresh-site inc — the pre-cure leak). Spec:
/// Annex E §AIMS RL-1 (`RL1_duplication_balanced`, `incElidable`) + RL-2
/// (`RL2_release_exactly_once`).
pub(in crate::lower::burden_lower) fn compute_cow_terminal_concat_inc_dsts(
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    if std::env::var("ORI_DISABLE_COW_TERMINAL_CONCAT_INC_ELISION").as_deref() == Ok("1") {
        return FxHashSet::default();
    }

    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };

    // FRESH-local candidate dsts: a heap string literal OR a fresh collection
    // Construct/Reuse/CollectionReuse, RcPtr/FatVal repr.
    let mut candidates: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let dst = match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Literal(LitValue::String(_)),
                    ..
                }
                | ArcInstr::Construct { dst, .. }
                | ArcInstr::Reuse { dst, .. }
                | ArcInstr::CollectionReuse { dst, .. } => *dst,
                _ => continue,
            };
            if is_rcptr(dst) {
                candidates.insert(dst);
            }
        }
    }
    if candidates.is_empty() {
        return FxHashSet::default();
    }

    // Per-candidate: total use count across the whole function + whether the
    // SOLE use is a `Binary(Add)` concat operand. A use is ANY read position
    // (body instr used_vars / terminator used_vars). RC ops carry no used_vars
    // for the operand here (BurdenInc/BurdenDec are not yet emitted at scan
    // time), so this counts genuine value uses.
    let mut use_count: rustc_hash::FxHashMap<ArcVarId, u32> = rustc_hash::FxHashMap::default();
    let mut sole_use_is_concat: rustc_hash::FxHashMap<ArcVarId, bool> =
        rustc_hash::FxHashMap::default();

    for block in &func.blocks {
        for instr in &block.body {
            let is_concat = matches!(
                instr,
                ArcInstr::Let {
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                        ..
                    },
                    ..
                }
            );
            for &v in &instr.used_vars() {
                if candidates.contains(&v) {
                    *use_count.entry(v).or_insert(0) += 1;
                    sole_use_is_concat.insert(v, is_concat);
                }
            }
        }
        for v in block.terminator.used_vars() {
            if candidates.contains(&v) {
                *use_count.entry(v).or_insert(0) += 1;
                sole_use_is_concat.insert(v, false);
            }
        }
    }

    candidates
        .into_iter()
        .filter(|dst| {
            // EXACTLY ONE use, and that use is a `Binary(Add)` concat operand.
            use_count.get(dst).copied() == Some(1)
                && sole_use_is_concat.get(dst).copied() == Some(true)
        })
        .collect()
}
