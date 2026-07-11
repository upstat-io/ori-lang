//! SOLE-CARRIER borrowed-`Invoke` alias claim:
//! [`compute_sole_carrier_borrowed_invoke_aliases`]. Spec: Annex E §AIMS
//! RL-2 + RL-4.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::compute_borrowed_terminator_invoke_args;

/// `ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM=1` restores the inline
/// last-use dec for a SOLE-CARRIER borrowed-`Invoke` alias
/// ([`compute_sole_carrier_borrowed_invoke_aliases`] returns empty): the
/// lineage's single release lands BEFORE the borrowed terminator that reads it
/// (use-after-free / double-free when the callee aliases the value into its
/// result — the `wrap_ok(m: m)` mint shape). Bisection surface: isolates a
/// borrowed-call-arg early-release to the edge-claim vs the rest of the
/// Phase-5 walk. Spec: Annex E §AIMS RL-2 + RL-4.
static SOLE_CARRIER_BORROWED_INVOKE_CLAIM_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM").as_deref() == Ok("1")
});

/// SOLE-CARRIER borrowed-`Invoke` aliases (RL-2 + RL-4): `Let { Var(src) }`
/// dsts that carry their lineage's ONLY release while their single use is a
/// BORROWED arg of a may-unwind `Invoke` terminator. The base walk places the
/// dst's last-use `BurdenDec` INLINE before that terminator — releasing the
/// allocation BEFORE the borrowed call reads it (use-after-free; a double-free
/// when the callee aliases the value into its result). The cure removes the
/// dst from `owned_vars_needing_rc` (suppressing the early inline dec) and
/// CLAIMS it for the class-ledger per-edge `deadAtSucc` release (via
/// `claimed_no_sink_vars` / `burden_emitted`): the dst is dead at EVERY
/// successor of its carrier `Invoke`, so each executing path releases exactly
/// once AFTER the call completes.
///
/// Gates (ALL hold; a declined dst keeps the inline dec — status quo):
/// - dst is in `owned_vars_needing_rc` (it carries the lineage's release) and
///   its SOLE use is a borrowed `Invoke` / `InvokeIndirect` terminator arg;
/// - src is used EXACTLY once (the alias) — the lineage genuinely dies at the
///   carrier call; a forward-live src owns later reads (and the funded
///   duplication families require `src >= 2` uses — disjoint by construction);
/// - src is NOT a function param (param scope-exit accounting is its own
///   surface);
/// - src is DIRECTLY the dst of a USER-call `Invoke` / `InvokeIndirect`
///   terminator AND is itself still in `owned_vars_needing_rc`. Only that
///   shape guarantees src's own burden ops are a self-canceling
///   fresh-inc/last-use-dec pair at the alias site (net 0), leaving the
///   claimed dst the lineage's single effective release. Every other src
///   carries the lineage's release ITSELF — a `Project` src is a borrow-view
///   whose alias dec is the element's designated release (RL-2 final-read
///   release); a block-param src inherited its RC obligation through the
///   Jump transfer (RL-4 exemption); a builtin `Apply` / `Invoke` result
///   (`@__index` element copy, `@concat` buffer) and a `Let { Var }`
///   chain hop carry a bare last-use dec with no funding inc. Claiming an
///   alias of those relocates the bare dec INLINE before the borrowed
///   terminator (use-after-free) or drops the only release (leak).
pub(in crate::lower::burden_lower) fn compute_sole_carrier_borrowed_invoke_aliases(
    func: &ArcFunction,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    contracts: &FxHashMap<ori_ir::Name, crate::aims::contract::MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    if *SOLE_CARRIER_BORROWED_INVOKE_CLAIM_DISABLED {
        return FxHashSet::default();
    }
    // BORROWED at the carrier = the call-site annotation says Borrowed, OR a
    // USER callee's interprocedural contract proves the param a pure borrow
    // (the call-site `arg_ownership` of a named user call is unpopulated at
    // Phase 5 and `ArcTerminator::is_owned_position` defaults missing entries
    // to Owned; the contract is the proven signal — the `wrap_ok(m: m)`
    // borrowed-store-mint shape). The contract branch mirrors
    // `contract_consuming_arg_position`'s fences: KNOWN builtins keep their
    // precise structural annotations (a COW receiver's seeded contract would
    // over-admit); an iter-consuming / ttr / COW-consumed borrowed param is a
    // transfer or consume with its own release accounting — never claimed.
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let mut borrowed_args = compute_borrowed_terminator_invoke_args(func);
    for block in &func.blocks {
        if let crate::ir::ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if builtins.is_builtin(*callee) {
                continue;
            }
            if let Some(contract) = contracts.get(callee) {
                for (pos, &arg) in args.iter().enumerate() {
                    let pure_borrow = contract.params.get(pos).is_some_and(|p| {
                        p.access == crate::aims::lattice::dimensions::AccessClass::Borrowed
                            && !p.iter_consumes
                            && !p.transfers_through_return
                            && !p.borrowed_cow_consumed
                    });
                    if pure_borrow {
                        borrowed_args.insert(arg);
                    }
                }
            }
        }
    }
    if borrowed_args.is_empty() {
        return FxHashSet::default();
    }
    let mut use_counts: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            for &v in &instr.used_vars() {
                *use_counts.entry(v).or_default() += 1;
            }
        }
        for v in block.terminator.used_vars() {
            *use_counts.entry(v).or_default() += 1;
        }
    }
    let params: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let user_invoke_results = collect_user_invoke_result_dsts(func, &builtins);
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                let gates = [
                    ("owned-dst", owned_vars_needing_rc.contains(dst)),
                    ("borrowed-arg", borrowed_args.contains(dst)),
                    (
                        "dst-sole-use",
                        use_counts.get(dst).copied().unwrap_or(0) == 1,
                    ),
                    (
                        "src-sole-use",
                        use_counts.get(src).copied().unwrap_or(0) == 1,
                    ),
                    ("src-not-param", !params.contains(src)),
                    (
                        "src-user-invoke-result",
                        user_invoke_results.contains(src) && owned_vars_needing_rc.contains(src),
                    ),
                ];
                if let Some((gate, _)) = gates.iter().find(|(_, ok)| !ok) {
                    if owned_vars_needing_rc.contains(dst) && borrowed_args.contains(dst) {
                        tracing::trace!(
                            target: "ori_arc::aims::realize",
                            fn_name = ?func.name,
                            dst = dst.index(),
                            gate,
                            "sole-carrier borrowed-invoke claim declined"
                        );
                    }
                } else {
                    tracing::trace!(
                        target: "ori_arc::aims::realize",
                        fn_name = ?func.name,
                        dst = dst.index(),
                        "sole-carrier borrowed-invoke claim admitted"
                    );
                    out.insert(*dst);
                }
            }
        }
    }
    out
}

/// USER-call Invoke / `InvokeIndirect` terminator dsts — the sole src shape
/// whose burden ops self-cancel (the sole-carrier gate table's
/// src-user-invoke-result row). Builtin Invoke results are excluded: their
/// single dec is the lineage's release. Spec: Annex E §AIMS RL-2 + RL-4.
fn collect_user_invoke_result_dsts(
    func: &ArcFunction,
    builtins: &crate::borrow::BuiltinOwnershipSets,
) -> FxHashSet<ArcVarId> {
    let mut user_invoke_results: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        match &block.terminator {
            crate::ir::ArcTerminator::Invoke {
                dst, func: callee, ..
            } => {
                if !builtins.is_builtin(*callee) {
                    user_invoke_results.insert(*dst);
                }
            }
            crate::ir::ArcTerminator::InvokeIndirect { dst, .. } => {
                user_invoke_results.insert(*dst);
            }
            _ => {}
        }
    }
    user_invoke_results
}
