//! RL-1 surplus-inc suppression for a FRESH self-allocating `Invoke`-terminator
//! call result whose SOLE function-wide use is a single BORROWED BODY-`Apply`
//! call arg.
//!
//! Shape: `print(msg: xs.len().to_str())` — `%7 = Invoke @to_str(..)` is a fresh
//! rc=1 str whose only use is the borrowed body `@ori_print(%7 [borrow])` arg.
//! The fresh result is borrowed by exactly one call arg and released by its
//! single last-use dec, so the keep-alive FRESH-site `BurdenInc` is surplus
//! (alloc rc=1 + inc rc=2 - one dec rc=1 -> net +1 LEAK). The value is
//! move-once-linear (`incElidable`, DP-3 / RL-1 Once AND Affine): suppress ONLY
//! the fresh-site inc; the paired single dec frees the rc=1 alloc.
//!
//! SRP boundary: the SIBLING shape — a fresh builtin `Invoke`-result consumed at
//! a TERMINATOR-`Invoke` borrowed arg — is owned by
//! `borrowed_invoke_lineage::collect_fresh_builtin_invoke_result_roots` (the
//! `ORI_DISABLE_BUILTIN_INVOKE_RESULT_LINEAGE` family, a death-point lineage
//! treatment that suppresses the inline dec AND places one carrier-succ
//! release). This scan handles ONLY the BODY-`Apply` consume the lineage
//! treatment does not reach; gating on body calls keeps the two disjoint.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcVarId};

/// Fresh self-allocating `Invoke`-terminator call-result dsts whose SOLE
/// function-wide use is a single BORROWED (non-owned) body-`Apply` /
/// `ApplyIndirect` call-arg position.
///
/// Discriminator (Once AND Affine, RL-1 `incElidable`):
/// - FRESH: the dst is a self-allocating-builtin `Invoke`-terminator result
///   (`fresh_rc_alloc_dst_terminator`) — a fresh rc=1 alloc, NOT a borrow-view /
///   shared-buffer result (whose inc stays load-bearing).
/// - ONCE: total function-wide use count is exactly 1 (a second use is a genuine
///   duplication whose inc is load-bearing).
/// - AFFINE: the sole use is at a BORROWED (non-owned) arg position of a BODY
///   `Apply` / `ApplyIndirect`. An owned-position transfer (`Construct` / owned
///   call arg / `Set.value`), a `Return` escape, a non-call read, or a
///   TERMINATOR consume (owned by `borrowed_invoke_lineage`) keeps the inc.
///
/// Empty when `ORI_DISABLE_FRESH_CALL_RESULT_BORROWED_ARG_INC_ELISION=1`.
/// Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`, `incElidable`) + DP-3.
pub(in crate::lower::burden_lower) fn compute_fresh_call_result_borrowed_arg_inc_dsts(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    if std::env::var("ORI_DISABLE_FRESH_CALL_RESULT_BORROWED_ARG_INC_ELISION").as_deref() == Ok("1")
    {
        return FxHashSet::default();
    }

    // 1. FRESH: self-allocating `Invoke`-terminator call results.
    let mut fresh: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        if let Some((dst, _normal)) =
            crate::aims::realize::fresh_rc_alloc_dst_terminator(&block.terminator, func, interner)
        {
            fresh.insert(dst);
        }
    }
    if fresh.is_empty() {
        return FxHashSet::default();
    }

    // 2. ONCE: total function-wide use count per fresh dst (body + terminator).
    let mut use_count: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            for &v in &instr.used_vars() {
                if fresh.contains(&v) {
                    *use_count.entry(v).or_default() += 1;
                }
            }
        }
        for v in block.terminator.used_vars() {
            if fresh.contains(&v) {
                *use_count.entry(v).or_default() += 1;
            }
        }
    }

    // 3. AFFINE: the SOLE use is a BORROWED arg position of a BODY `Apply` /
    //    `ApplyIndirect`. Owned transfer / Return escape / non-call read /
    //    terminator consume keep the inc.
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let is_call = matches!(
                instr,
                ArcInstr::Apply { .. } | ArcInstr::ApplyIndirect { .. }
            );
            if !is_call {
                continue;
            }
            for (pos, &v) in instr.used_vars().iter().enumerate() {
                if fresh.contains(&v)
                    && use_count.get(&v).copied() == Some(1)
                    && !instr.is_owned_position(pos)
                {
                    out.insert(v);
                }
            }
        }
    }
    out
}
