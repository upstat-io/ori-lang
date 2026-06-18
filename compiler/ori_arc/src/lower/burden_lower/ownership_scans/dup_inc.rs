//! Duplication-inc scans: use counts + RL-1 dup-alias classification, the
//! genuine-duplication store-out aliases, the move-edge / store-consume SSOT,
//! and the ttr-iter-consume duplication aliases. Spec: Annex E §AIMS RL-1 +
//! RL-2.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;
use ori_types::TypeRegistry;

use crate::aims::contract::MemoryContract;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

use super::forwarder::collect_ttr_param_aliases;
use super::live_extract::{collect_fresh_sum_roots, is_niche_family_sum};
use super::successor_reachable_blocks;

/// Compute function-wide use counts and the duplication-alias dst set, and
/// extend `inc_suppressed_vars` with dead FRESH values per AIMS RL-2.
///
/// - `use_counts`: total used-var occurrences per `ArcVarId` (body + terminator).
/// - dead-FRESH inc-suppression: a var never used receives a FRESH-site
///   `BurdenInc` but no last-use `BurdenDec`; the predicate-stack emits its
///   dead-value cleanup `RcDec` (RL-2 unused-owned dec), so it carries no
///   burden ops — suppress the orphaned inc (else net +1).
/// - `dup_alias_dsts`: `Let { Var(src) }` dsts whose `src` stays live
///   (`use_counts` ≥ 2) — a duplication alias (RL-1). The burden path emits the
///   alias's own paired `BurdenInc dst` (alias site) + `BurdenDec dst`
///   (last-use); the alias owns its release, not the predicate stack.
/// - `forwarder_transparent_aliases` are NOT duplication aliases: a same-alloc
///   view of a moved-through allocation (RL-34 forwarder identity) needs no
///   paired RC of its own — see
///   [`compute_forwarder_identity_transparent_aliases`].
pub(in crate::lower::burden_lower) fn compute_use_counts_and_dup_aliases(
    func: &ArcFunction,
    inc_suppressed_vars: &mut FxHashSet<ArcVarId>,
    forwarder_transparent_aliases: &FxHashSet<ArcVarId>,
) -> (FxHashMap<ArcVarId, u32>, FxHashSet<ArcVarId>) {
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
    for raw in 0..func.var_types.len() {
        let var = ArcVarId::new(
            u32::try_from(raw).unwrap_or_else(|_| panic!("var index {raw} fits in u32")),
        );
        if !use_counts.contains_key(&var) {
            inc_suppressed_vars.insert(var);
        }
    }
    let mut dup_alias_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if forwarder_transparent_aliases.contains(dst) {
                    continue;
                }
                if use_counts.get(src).copied().unwrap_or(0) >= 2 {
                    dup_alias_dsts.insert(*dst);
                }
            }
        }
    }
    (use_counts, dup_alias_dsts)
}

/// Genuine-duplication STORE-out aliases per AIMS RL-1: `Let { Var(src) }` dsts
/// in `dup_alias_dsts` whose move chain terminates at an AGGREGATE-STORE
/// consume (`Construct` / `Reuse` / `CollectionReuse` arg or `Set.value`) while
/// a use of `src` is forward-REACHABLE from the alias site — `src` stays live
/// PAST the alias on that path, so the store creates a real SECOND reference to
/// `src`'s allocation while the first stays live. The alias-site `BurdenInc` is
/// LOAD-BEARING (`AimsProof.Realization::RL1_duplication_balanced`: the
/// duplication's inc is matched by exactly one release — the container's drop);
/// the RL-2 inc-suppression symmetry MUST NOT fire on it (suppressing nets -1:
/// the container releases a reference no inc supplied — double-free).
///
/// TWO exclusions bound the family:
///
/// - Terminal moves: when NO use of `src` is reachable past the alias, the
///   store consumes `src`'s ORIGINAL reference (RL-2 transfer, no duplication)
///   and the inc-suppression stays correct (keeping the inc nets +1).
///   Reachability is the discriminator — NOT `last_use_points` entry counts:
///   branch-EXCLUSIVE aliases (`bb3: %a = %s; bb4: %b = %s` under a `Branch`)
///   give `src` two last-use entries, yet each path's alias is the terminal
///   move of `src`'s one reference (neither use reaches the other). "Use after
///   the alias" = later-in-block use (body or terminator), OR a use in any
///   block forward-reachable from the alias block's successors (covers the
///   loop back-edge: an earlier-in-block use re-reached on the next iteration
///   IS a use after).
/// - Call-arg consumers: an alias moved into an `Apply` / `Invoke` owned arg
///   (user callee OR protocol builtin — `iter`'s `[own]` collection consume,
///   `__iter_next`'s iterator) is OUT of the family. Those consumes have their
///   own interprocedural / protocol ownership accounting (RL-2 iter-consume
///   transfer, `MemoryContract` transfer-through-return, the loop-threading
///   keep-alive machinery); injecting an alias inc there over-counts (+1 per
///   loop on the iterator corpus). Only the aggregate-STORE family — the
///   `Holder { kept: a }` duplication-by-storage shape — is classified.
///
/// `full_move_vars` members are excluded: their whole-var ops are owned by the
/// field-projection full-move suppression, not the alias pair.
pub(in crate::lower::burden_lower) fn compute_genuine_dup_move_aliases(
    func: &ArcFunction,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
    full_move_vars: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    if super::super::genuine_dup_pair_coupling_disabled() {
        return FxHashSet::default();
    }
    let use_sites = collect_use_sites(func);
    let (alias_next, store_consumed) = collect_move_edges_and_store_consumes(func);
    // Walk the single-use move chain from `dst`; true iff it ends at an
    // aggregate-store consume. A var used >1 time is not a pure move hop, so
    // the chain only follows `use_sites.len() == 1` links.
    let chain_ends_in_store = |start: ArcVarId| -> bool {
        let mut cur = start;
        for _ in 0..func.var_types.len() {
            if use_sites.get(&cur).map(Vec::len) != Some(1) {
                return false;
            }
            if store_consumed.contains(&cur) {
                return true;
            }
            match alias_next.get(&cur) {
                Some(&next) => cur = next,
                None => return false,
            }
        }
        false
    };
    // Forward-reachable block set from `start`'s SUCCESSORS (start itself is
    // included only when a cycle re-reaches it). Memoized per alias block.
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut genuine: FxHashSet<ArcVarId> = FxHashSet::default();
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
            if !dup_alias_dsts.contains(dst)
                || full_move_vars.contains(dst)
                || !chain_ends_in_store(*dst)
            {
                continue;
            }
            let Some(sites) = use_sites.get(src) else {
                continue;
            };
            let reachable = reachable_cache
                .entry(block_idx)
                .or_insert_with(|| successor_reachable_blocks(func, block_idx));
            let src_used_after = sites
                .iter()
                .any(|&(ub, ui)| (ub == block_idx && ui > instr_idx) || reachable.contains(&ub));
            if src_used_after {
                genuine.insert(*dst);
            }
        }
    }
    genuine
}

/// Whether `instr` is a burden-op accounting marker (TF-N/A metadata, not a
/// real value use). The duplication scans run BOTH pre-emission (Phase 5 —
/// no burden ops exist) and post-emission (the Phase-6 pair-atomic / Phase-7
/// lineage-net consumers recompute the SAME set on the op-carrying IR), so
/// use counting MUST skip them to stay in lock-step across phases.
pub(super) fn is_burden_op(instr: &ArcInstr) -> bool {
    matches!(
        instr,
        ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::BurdenDecPartial { .. }
            | ArcInstr::BurdenDecVariant { .. }
            | ArcInstr::BurdenDecField { .. }
    )
}

/// Per-var use sites: body uses as `(block, instr_idx)`; terminator uses as
/// `(block, usize::MAX)` so any body index in the block precedes them.
/// Burden-op markers are skipped (TF-N/A metadata, not uses).
pub(super) fn collect_use_sites(func: &ArcFunction) -> FxHashMap<ArcVarId, Vec<(usize, usize)>> {
    let mut use_sites: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if is_burden_op(instr) {
                continue;
            }
            for &v in &instr.used_vars() {
                use_sites.entry(v).or_default().push((block_idx, instr_idx));
            }
        }
        for v in block.terminator.used_vars() {
            use_sites
                .entry(v)
                .or_default()
                .push((block_idx, usize::MAX));
        }
    }
    use_sites
}

/// Single-use move-alias edges (`src -> dst`) + aggregate-store consume
/// membership (`Construct` / `Reuse` / `CollectionReuse` args, `Set.value`),
/// for the forward chain walk to an alias's terminal consumer. `pub(crate)`:
/// the Phase-6 pair-atomic collector
/// (`aims::realize::burden_elim::collect_pair_atomic_alias_dsts`) consumes the
/// same store-consume membership for its Construct-rooted admission gate (one
/// SSOT for the aggregate-STORE family).
pub(crate) fn collect_move_edges_and_store_consumes(
    func: &ArcFunction,
) -> (FxHashMap<ArcVarId, ArcVarId>, FxHashSet<ArcVarId>) {
    let mut alias_next: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let mut store_consumed: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    alias_next.insert(*src, *dst);
                }
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => {
                    store_consumed.extend(args.iter().copied());
                }
                ArcInstr::Set { value, .. } => {
                    store_consumed.insert(*value);
                }
                _ => {}
            }
        }
    }
    (alias_next, store_consumed)
}

/// RL-1 + RL-2 iter-consume duplication: `Let { Var(src) }` aliases of an Owned
/// `transfers_through_return` param whose lineage is consumed at an
/// ITER-CONSUMING OWNED position — the `@iter` PROTOCOL builtin (`for x in xs`
/// lowers to `Apply @iter(xs [own])` -> `ori_iter_drop` frees the buffer INSIDE
/// the function) OR a user callee with `ParamContract.iter_consumes`. The
/// iter-consume FREES the same allocation the param ALSO transfers out via the
/// `Return` — one reference, two transfer consumes. Per AIMS RL-1
/// (`RL1_duplication_balanced`) this is a DUPLICATION: the iter-consumed alias
/// owes one `BurdenInc` (the duplicate the iterator frees via `ori_iter_drop`,
/// the RL-2 `ApplyToIterConsumingParam` transfer release), leaving the param's
/// original reference for the Return transfer. Without the kept inc the single
/// allocation is freed once by the iterator and transferred out via Return =
/// double-free (the `borrow_*_for_yield_then_return` family).
///
/// The returned set joins `genuine_dup_move_aliases` in the inc-suppression skip
/// (`emit_burden_ops`): the alias keeps its FRESH-site inc while its last-use
/// dec stays transfer-suppressed (the `@iter [own]` consume is an owned-position
/// transfer — `instr_transfer_vars` already suppresses the dec). Net 0.
///
/// STRUCTURAL discriminators (the over-fire boundary — a missing transfer is a
/// LEAK, an extra inc is also a leak per the no-return negative):
/// - `src` (the alias source) is in a ttr Owned-param lineage (the param flows
///   to a `Return` per THIS function's contract). An iter-consumed alias of a
///   NON-returned collection dies at the iter-consume (single transfer) — NOT
///   admitted (the `consume_only_no_return` over-fire negative: an extra inc
///   leaves the buffer leaked, `ori_iter_drop` is the sole release).
/// - The alias `dst` (directly, or via a single-use move chain) reaches an
///   iter-consuming owned position. Detection mirrors the Phase-6.66c
///   `record_iter_consume_uses` SSOT: `callee == iter_name` with the arg `[own]`
///   (inline for-loop), OR a user contract param with `iter_consumes`.
pub(in crate::lower::burden_lower) fn compute_ttr_iter_consume_dup_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let Some((_, alias_to_src)) = collect_ttr_param_aliases(func, contracts) else {
        return FxHashSet::default();
    };
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    // The set of vars consumed at an iter-consuming OWNED arg position. A
    // user-callee arg at an `iter_consumes` param is recorded; an `@iter [own]`
    // protocol-builtin arg is recorded (the inline for-loop's iterator owns +
    // frees the buffer via `ori_iter_drop`). Mirrors the Phase-6.66c
    // `record_iter_consume_uses` SSOT.
    let mut iter_consumed: FxHashSet<ArcVarId> = FxHashSet::default();
    let user_iter_consume_args =
        |callee: Name, args: &[ArcVarId], out: &mut FxHashSet<ArcVarId>| {
            let Some(contract) = contracts.get(&callee) else {
                return;
            };
            for (pos, &arg) in args.iter().enumerate() {
                if contract.params.get(pos).is_some_and(|p| p.iter_consumes) {
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
                            iter_consumed.insert(arg);
                        }
                    }
                } else {
                    user_iter_consume_args(*callee, args, &mut iter_consumed);
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            user_iter_consume_args(*callee, args, &mut iter_consumed);
        }
    }
    if iter_consumed.is_empty() {
        return FxHashSet::default();
    }
    // Admit a ttr-lineage alias whose own chain reaches an iter-consumed var.
    // The alias `dst` and the iter-consumed var coincide for the `%alias =
    // %param; @iter(%alias [own])` shape (the for-yield root); a single-hop
    // intermediate move (`%a = %param; %b = %a; @iter(%b)`) is folded into the
    // ttr lineage by `collect_ttr_param_aliases`, so any lineage member that is
    // also iter-consumed admits its lineage's alias dsts.
    alias_to_src
        .keys()
        .copied()
        .filter(|dst| iter_consumed.contains(dst))
        .collect()
}

/// The set of vars consumed at an iter-consuming OWNED position — `@iter [own]`
/// (the inline for-loop's iterator owns + frees the buffer via `ori_iter_drop`)
/// OR a user-callee arg at an `iter_consumes` contract param. Mirrors the
/// `record_iter_consume_uses` SSOT + the [`compute_ttr_iter_consume_dup_aliases`]
/// detection above.
fn iter_consumed_vars(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
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
                if contract.params.get(pos).is_some_and(|p| p.iter_consumes) {
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
    let mut reaches_iter_consume = iter_consumed.clone();
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
