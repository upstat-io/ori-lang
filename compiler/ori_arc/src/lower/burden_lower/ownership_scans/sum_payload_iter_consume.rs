//! Iter-consumed-position computation + the sum-payload surplus dup-inc
//! suppression it feeds. Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::live_extract::{collect_fresh_sum_roots, is_niche_family_sum};

/// Which iter-consume predicate [`collect_iter_consumed_positions`] applies to
/// a user-callee param — see the two variant docs for the concrete callers
/// each mode serves.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum IterConsumeScope {
    /// The canonical RL-2 CONSUME predicate (`ParamContract::is_rl2_consume`):
    /// `iter_consumes ∧ ¬transfers_through_return`. A ttr+iter-consume callee
    /// hands the arg back through its own return, so that overlap keeps its
    /// own accounting (`dup_inc::compute_ttr_iter_consume_dup_aliases`) and
    /// must not double-count as a plain consume here. Used by
    /// [`iter_consumed_vars`].
    Rl2ConsumeOnly,
    /// The bare `iter_consumes` signal, ADMITTING the ttr-overlap itself —
    /// `compute_ttr_iter_consume_dup_aliases` is specifically looking for
    /// THIS function's own ttr-param aliases that are ALSO iter-consumed, so
    /// excluding the overlap would empty out the exact case it targets.
    IncludingTtrOverlap,
}

/// The set of vars consumed at an iter-consuming OWNED position — `@iter [own]`
/// (the inline for-loop's iterator owns + frees the buffer via `ori_iter_drop`)
/// OR a user-callee arg at an `iter_consumes` contract param — the shared
/// skeleton behind both [`iter_consumed_vars`] and
/// `dup_inc::compute_ttr_iter_consume_dup_aliases`'s own scan. Mirrors
/// the Phase-6.66c `record_iter_consume_uses` SSOT.
pub(super) fn collect_iter_consumed_positions(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    scope: IterConsumeScope,
) -> FxHashSet<ArcVarId> {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    let user_iter_consume_args =
        |callee: Name, args: &[ArcVarId], out: &mut FxHashSet<ArcVarId>| {
            let Some(contract) = contracts.get(&callee) else {
                return;
            };
            for (pos, &arg) in args.iter().enumerate() {
                let consumes = contract.params.get(pos).is_some_and(|p| match scope {
                    IterConsumeScope::Rl2ConsumeOnly => p.is_rl2_consume(),
                    IterConsumeScope::IncludingTtrOverlap => p.iter_consumes,
                });
                if consumes {
                    out.insert(arg);
                }
            }
        };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                if *callee == iter_name {
                    for (pos, &arg) in args.iter().enumerate() {
                        if arg_ownership
                            .get(pos)
                            .is_none_or(|o| *o == crate::ir::ArgOwnership::Owned)
                        {
                            out.insert(arg);
                        }
                    }
                } else {
                    user_iter_consume_args(*callee, args, &mut out);
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            user_iter_consume_args(*callee, args, &mut out);
        }
    }
    out
}

/// The set of vars consumed at an iter-consuming OWNED position, EXCLUDING the
/// ttr+iter-consume overlap (see [`collect_iter_consumed_positions`]). Mirrors
/// the `record_iter_consume_uses` SSOT + `dup_inc::compute_ttr_iter_consume_dup_aliases`.
fn iter_consumed_vars(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    collect_iter_consumed_positions(func, contracts, interner, IterConsumeScope::Rl2ConsumeOnly)
}

/// RL-1 + RL-2 surplus dup-alias-inc suppression for a fresh niche-family
/// sum-aggregate whose extracted payload is ITER-CONSUMED on the live-extract
/// path (`let m: Option<[str]> = Some([..]); match m { Some(words) -> { for w
/// in words do .. }, None -> .. }`).
///
/// The buffer is born ONCE (the `Construct List` then MOVED by-value into
/// `Construct Variant(Option.0)` — the niche-family sum carries no allocation of
/// its own; its inline payload IS the buffer). A `Let { Var }` alias of the
/// by-value aggregate (`%5 = %3`) is a MOVE-ONCE alias and adds NO owner, so
/// per RL-1 (`AimsProof.Realization::RL1_duplication_balanced`, `incElidable =
/// true` → no inc) its duplication `BurdenInc` is ELIDABLE. The base walk's
/// `use_counts >= 2` proxy mis-classes the alias as a duplication and emits the
/// surplus inc. On the live-extract path the payload (`Project alias.1`) is
/// iter-consumed via `@iter [own]` whose `ori_iter_drop` releases the buffer
/// (RL-2 `ApplyToIterConsumingParam` transfer, no caller dec —
/// `RL2_iter_consuming_no_caller_dec`); the aggregate's kept construct-site inc
/// is released by the Some-arm scope-exit dec, and the dead None / Unreachable
/// arms release the birth + construct-site inc. The surplus dup-alias inc has no
/// matching dec on the iter-consume path → +1 leak (the buffer ends at rc=1; its
/// heap str elements leak with it since `ori_iter_drop` never re-runs the
/// `elem_dec_fn`).
///
/// Cure: suppress the SURPLUS Let-Var dup-alias `BurdenInc`. CP-1 case (a) —
/// the impl over-emits a move-once alias inc the proven calculus elides; no Lean
/// change.
///
/// Gates (the over-fire boundary — proven same-allocation identity, NOT a
/// use-count / type proxy):
///   (a) root is a fresh niche-family sum-aggregate ([`collect_fresh_sum_roots`]
///       + [`is_niche_family_sum`]) with `Aggregate` repr, in
///       `owned_vars_needing_rc`.
///   (b) the alias is a `Let { Var(root) }` dup-alias (in `dup_alias_dsts`) of
///       that root.
///   (c) the alias's NICHE-PAYLOAD `Project` extract is iter-consumed at an
///       `@iter [own]` / `iter_consumes`-param owned position (the live-extract
///       iter-consume — the transfer that leaves the dup-alias inc unmatched).
///
/// Spec: Annex E §AIMS RL-1 + RL-2 + TF-4.
pub(in crate::lower::burden_lower) fn compute_sum_payload_iter_consume_dup_inc_suppression(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
    type_registry: &TypeRegistry,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let iter_consumed = iter_consumed_vars(func, contracts, interner);
    if iter_consumed.is_empty() {
        return FxHashSet::default();
    }
    // A var transitively reaches an iter-consume through `Let { Var }` move
    // aliases (`%11 = %8; @iter(%11 [own])` folds the intermediate move).
    let mut reaches_iter_consume = iter_consumed;
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
                    if reaches_iter_consume.contains(dst) && reaches_iter_consume.insert(*src) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }

    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    let roots: FxHashSet<ArcVarId> = collect_fresh_sum_roots(func, contracts)
        .into_iter()
        .filter(|r| {
            owned_vars_needing_rc.contains(r)
                && matches!(func.var_repr(*r), Some(ValueRepr::Aggregate))
                && is_niche_family_sum(func, *r, type_registry)
        })
        .collect();
    if roots.is_empty() {
        return out;
    }
    for block in &func.blocks {
        for instr in &block.body {
            // The alias: `Let { Var(root) }` dup-alias of a fresh niche-sum root.
            let ArcInstr::Let {
                dst: alias,
                value: ArcValue::Var(src),
                ..
            } = instr
            else {
                continue;
            };
            if !roots.contains(src) || !dup_alias_dsts.contains(alias) {
                continue;
            }
            // The alias's niche-payload `Project` extract reaches an iter-consume.
            let extracts_iter_consumed = func.blocks.iter().any(|b| {
                b.body.iter().any(|i| {
                    matches!(
                        i,
                        ArcInstr::Project { dst, value, .. }
                            if *value == *alias && reaches_iter_consume.contains(dst)
                    )
                })
            });
            if extracts_iter_consumed {
                tracing::trace!(
                    target: "ori_arc::aims::realize",
                    fn_name = ?func.name,
                    alias = alias.index(),
                    root = src.index(),
                    "sum-payload iter-consume surplus dup-alias inc suppressed (RL-1 move-once)"
                );
                out.insert(*alias);
            }
        }
    }
    out
}
