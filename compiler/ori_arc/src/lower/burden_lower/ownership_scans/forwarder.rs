//! Transfer-through-return forwarder scans: ttr results / param vars,
//! forwarder-identity transparent aliases, ttr param-alias chains, and the
//! forwarder call/consumer classification helpers. Spec: Annex E §AIMS RL-1 +
//! RL-2 + RL-34 + IC-1.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::dimensions::AccessClass;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};
use crate::ownership::Ownership;

use super::instr_owned_position_transfer_vars;
use super::union_find::ForwarderUnionFind;

/// Apply / Invoke result dsts whose callee transfers an owned argument THROUGH
/// the return (the callee's `MemoryContract` has a param with
/// `transfers_through_return == true`, i.e. `return_alias == Some(Direct)`).
///
/// The result `dst` of such a call is NOT a fresh allocation: it IS the same
/// allocation the caller transferred IN at the owned arg position (the callee
/// returns its owned param unchanged — `@id<T>(x: T) -> T = x`). The owned arg's
/// own definition (`Construct` / upstream alloc) already supplied the lifecycle
/// `+1`; the callee strips its scope-exit dec on the param
/// (`transfers_through_return`), handing the SAME reference back to the caller.
///
/// Per AIMS RL-1 (`AimsProof.Realization::RL1_emit_iff_not_elidable`): the result
/// is a move-once-linear forward of the caller's already-owned reference, NOT a
/// duplicating use, so its inc is ELIDABLE. The `fresh_site_burden_inc_dst`
/// Apply arm + `compute_invoke_result_incs` treat every `MaybeShared` / no-contract
/// result as FRESH (`*dst`) and emit a spurious result-`BurdenInc`; under
/// sole-emitter Phase-7 lowering that `RcInc` double-counts the transferred-in
/// allocation (`rcBalance` alloc-aware: alloc(+1) + spurious RcInc(+1) − path
/// decs = net +1 LEAK). Excluding these dsts from the result-inc restores
/// conformance to the proven `rcBalance` (the transferred arg's alloc is the
/// sole `+1`; the lineage's per-path decs release it).
///
/// Scope: ONLY the result-INC is suppressed. The dup-alias / borrow-read
/// machinery downstream is unchanged — those aliases own their paired inc/dec.
///
/// Repr gate (over-fire boundary): the result is admitted ONLY when its
/// `ValueRepr` is `RcPointer` or `FatValue` — a single directly-RC-managed
/// reference whose forwarded lineage is read via borrows and released by its own
/// per-path decs. An `Aggregate` result (a struct / sum the forwarder returns,
/// e.g. `Box<[int]>`) is EXCLUDED: its inner heap FIELDS are projected
/// (`Project dst.k`) and each projection lineage carries its own `RcDec`, so the
/// result-inc keeps the inner buffer's RC ≥ the projection-dec count — eliding
/// it double-frees the inner field across the projection paths. Per AIMS RL-1 +
/// `rcBalance`: the elision is sound only when the result IS the single
/// transferred-in allocation, not a wrapper whose fields are independently
/// projected and dec'd.
pub(in crate::lower::burden_lower) fn compute_transfer_through_return_results(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    let mut results: FxHashSet<ArcVarId> = FxHashSet::default();
    let callee_transfers_through_return = |callee: &Name| -> bool {
        contracts
            .get(callee)
            .is_some_and(|c| c.params.iter().any(|p| p.transfers_through_return))
    };
    let result_repr_admits = |dst: ArcVarId| -> bool {
        matches!(
            func.var_repr(dst),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if callee_transfers_through_return(callee) && result_repr_admits(*dst) {
                    results.insert(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if callee_transfers_through_return(callee) && result_repr_admits(*dst) {
                results.insert(*dst);
            }
        }
    }
    results
}

/// The function's OWN parameters whose `MemoryContract` records
/// `transfers_through_return == true` — the param flows to a `Return { value }`
/// terminator (directly or through a `Let { Var }` move-alias chain), so its
/// ownership transfers back out to the caller.
///
/// Per AIMS RL-2 (`AimsProof.Realization::RL2_dec_at_last_use` +
/// `RL2_transfer_kinds_no_dec` for the `Return` `TerminalUse`): a `Return` is an
/// ownership-transferring terminal use, so the callee MUST NOT emit a scope-exit
/// `BurdenDec` on the transferred param — the caller decs the bound result
/// variable when ITS scope exits (`ParamContract.transfers_through_return` doc).
/// Emitting the callee dec double-releases the allocation handed back through
/// the return (SIGSEGV / double-free under sole-emitter Phase-7 lowering).
///
/// The structural move-alias scan (`compute_transfer_via_move_alias`) covers the
/// pure single-block move case but conservatively keeps the dec when the param is
/// used across MULTIPLE blocks (its terminal move is not statically pin-pointable
/// by the global-last-use heuristic). The interprocedural contract carries the
/// proven Return-flow fact precisely (`facts.return_flow` in
/// `interprocedural/extract`), so consult it directly for the param case — the
/// contract IS the SSOT for "this param transfers through return", not a fragile
/// per-block structural re-derivation.
///
/// Params have no FRESH-site `BurdenInc` (only definitions allocate), so only
/// the last-use dec is suppressed; no symmetric inc-suppression is needed.
pub(in crate::lower::burden_lower) fn compute_transfer_through_return_param_vars(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    let Some(contract) = contracts.get(&func.name) else {
        return FxHashSet::default();
    };
    func.params
        .iter()
        .zip(contract.params.iter())
        .filter_map(|(param, pc)| pc.transfers_through_return.then_some(param.var))
        .collect()
}

/// Forwarder-identity transparent aliases per AIMS RL-1 + RL-34: `Let { Var(src) }`
/// dsts where `src` is an Owned param of THIS function whose `MemoryContract`
/// proves `transfers_through_return`, and the whole lineage (`src` plus every
/// alias of it) is read-only-or-move-out. Such an alias is a same-allocation
/// view of a value moving through to the `Return` — NOT an RL-1 duplication
/// (no new retained reference exists: the lineage's single transferred-in
/// reference stays live through the Return flow). Classifying it as a
/// duplication emits a paired alias inc/dec that the per-var DP-2/DP-3
/// elimination can split (DP-3 elides the `Once ∧ Affine` inc; DP-2 keeps the
/// `Once` dec) — an over-release double-free on the moved-through allocation.
///
/// EXTREME-CONSERVATIVE vetting — ALL must hold, else the `src` and every one
/// of its aliases keep the duplication classification (a genuine duplication
/// NEEDS its paired inc/dec):
/// - `src` is an `Ownership::Owned` param with `transfers_through_return` in
///   this function's own contract (`Spec: Annex E §AIMS RL-34` — the
///   structural Return-flow fact; the caller releases the bound result).
/// - Every body use of `src` and of each alias is a non-owned position
///   (borrow read: `Project` source, borrowed call arg) — never `Set` /
///   `SetTag` base, never `Set.value`, never an owned-position consume
///   (`Construct` / `Reuse` / `CollectionReuse` / `PartialApply` / owned
///   `Apply` arg), never a nested `Let { Var }` re-alias (single-hop only).
/// - Every terminator use is a `Return` value or a `Jump` arg feeding a LIVE
///   successor block-param (the move-out hop); `Invoke` / `InvokeIndirect`
///   args admit only borrowed positions. A Jump into a DEAD param declines
///   (the RL-5 dead-param release machinery owns that shape).
///
/// All-or-nothing per `src`: one non-vetted use anywhere in the lineage keeps
/// EVERY alias classified, so per-path nets are never half-changed.
///
/// Consumers: the duplication-classification skip in
/// [`compute_use_counts_and_dup_aliases`] and the `owned_vars_needing_rc`
/// exclusion in `emit_burden_ops` (the alias carries NO burden ops; the
/// lineage's release accounting stays with `src`).
pub(in crate::lower::burden_lower) fn compute_forwarder_identity_transparent_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    let Some((aliases_of, alias_to_src)) = collect_ttr_param_aliases(func, contracts) else {
        return FxHashSet::default();
    };

    let mut declined_srcs: FxHashSet<ArcVarId> = FxHashSet::default();
    let lineage_src_of = |v: ArcVarId| -> Option<ArcVarId> {
        if aliases_of.contains_key(&v) {
            Some(v)
        } else {
            alias_to_src.get(&v).copied()
        }
    };
    let decline = |v: ArcVarId, declined: &mut FxHashSet<ArcVarId>| {
        if let Some(src) = lineage_src_of(v) {
            declined.insert(src);
        }
    };

    for block in &func.blocks {
        for instr in &block.body {
            // A `Let { Var(src) }` whose `src` AND `dst` are both lineage members
            // (the ttr root, or a transitive alias of it) is a vetted in-lineage
            // move-hop — `collect_ttr_param_aliases` already folded the whole
            // multi-hop read-only chain into one root, so the hop reads `src` at a
            // move position (not a duplicating use) and re-binds it as another
            // same-allocation view. Skip it (do NOT decline on the `src` read).
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if lineage_src_of(*src).is_some() && alias_to_src.contains_key(dst) {
                    continue;
                }
            }
            // Mutation through the lineage declines it: a `Set` / `SetTag`
            // base write invalidates the read-only-view premise.
            match instr {
                ArcInstr::Set { base, value, .. } => {
                    decline(*base, &mut declined_srcs);
                    decline(*value, &mut declined_srcs);
                }
                ArcInstr::SetTag { base, .. } => {
                    decline(*base, &mut declined_srcs);
                }
                _ => {}
            }
            for (pos, &used) in instr.used_vars().iter().enumerate() {
                if lineage_src_of(used).is_none() {
                    continue;
                }
                if instr.is_owned_position(pos) {
                    decline(used, &mut declined_srcs);
                }
            }
        }
        for (pos, &used) in block.terminator.used_vars().iter().enumerate() {
            if lineage_src_of(used).is_none() {
                continue;
            }
            match &block.terminator {
                ArcTerminator::Jump { target, args } => {
                    // The Jump hop is the move-out only when the receiving
                    // block-param is itself LIVE (used somewhere); a dead
                    // param's release belongs to the RL-5 dead-param
                    // machinery — decline to keep the status quo there.
                    if !jump_arg_feeds_live_param(func, *target, args, used) {
                        decline(used, &mut declined_srcs);
                    }
                }
                ArcTerminator::Invoke { .. } | ArcTerminator::InvokeIndirect { .. } => {
                    if block.terminator.is_owned_position(pos) {
                        decline(used, &mut declined_srcs);
                    }
                }
                // `Return` is the move-out itself; `Branch` / `Switch` read a
                // scalar; `Resume` / `Unreachable` use nothing.
                ArcTerminator::Return { .. }
                | ArcTerminator::Branch { .. }
                | ArcTerminator::Switch { .. }
                | ArcTerminator::Resume
                | ArcTerminator::Unreachable => {}
            }
        }
    }

    aliases_of
        .iter()
        .filter(|(src, _)| !declined_srcs.contains(src))
        .flat_map(|(_, dsts)| dsts.iter().copied())
        .collect()
}

/// Collect the single-hop `Let { Var }` aliases of this function's Owned
/// `transfers_through_return` params (per its own contract). Returns
/// `(src -> [alias dsts], alias dst -> src)`; `None` when the function has no
/// such param or no alias of one.
#[expect(
    clippy::type_complexity,
    reason = "paired forward/reverse alias maps returned to one caller"
)]
pub(super) fn collect_ttr_param_aliases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Option<(
    FxHashMap<ArcVarId, Vec<ArcVarId>>,
    FxHashMap<ArcVarId, ArcVarId>,
)> {
    let contract = contracts.get(&func.name)?;
    let ttr_params: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .zip(contract.params.iter())
        .filter(|(param, pc)| {
            matches!(param.ownership, Ownership::Owned) && pc.transfers_through_return
        })
        .map(|(param, _)| param.var)
        .collect();
    if ttr_params.is_empty() {
        return None;
    }
    // Transitive `Let { Var }` alias closure rooted at each ttr param: a nested
    // re-alias of an alias (`%nested = %alias` where `%alias` already aliases the
    // root) is the SAME allocation, so it joins the root's lineage. The loop-body
    // shape `%0 -> %loop_alias = %0 (re-read per iteration) -> %nested = %loop_alias`
    // is a >1-hop chain of one allocation; declining it on the nested hop leaves
    // the deepest alias mis-classified (a spurious last-use dec on the returned
    // param -> double-free). Fixpoint over `Let { Var }` edges keeps the whole
    // chain mapped to its ttr root. Membership precedence: a var already an alias
    // is not re-rooted; only NEW dsts off a lineage member are added.
    let mut aliases_of: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut alias_to_src: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let in_lineage = |v: ArcVarId,
                      ttr: &FxHashSet<ArcVarId>,
                      a2s: &FxHashMap<ArcVarId, ArcVarId>|
     -> Option<ArcVarId> {
        if ttr.contains(&v) {
            Some(v)
        } else {
            a2s.get(&v).copied()
        }
    };
    // Bounded fixpoint: each pass adds at most the next alias hop, so the var
    // count caps the iteration count.
    for _ in 0..func.var_types.len() {
        let mut changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if alias_to_src.contains_key(dst) {
                        continue;
                    }
                    if let Some(root) = in_lineage(*src, &ttr_params, &alias_to_src) {
                        aliases_of.entry(root).or_default().push(*dst);
                        alias_to_src.insert(*dst, root);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    if aliases_of.is_empty() {
        return None;
    }
    Some((aliases_of, alias_to_src))
}

/// True iff `used`, passed as a `Jump` arg to `target`, lands in a block-param
/// that is itself used somewhere in the function (body or terminator).
fn jump_arg_feeds_live_param(
    func: &ArcFunction,
    target: crate::ir::ArcBlockId,
    args: &[ArcVarId],
    used: ArcVarId,
) -> bool {
    args.iter()
        .position(|&a| a == used)
        .and_then(|i| {
            func.blocks
                .get(target.index())
                .and_then(|tb| tb.params.get(i))
        })
        .is_some_and(|&(param_var, _)| {
            func.blocks.iter().any(|b| {
                b.body.iter().any(|i| i.used_vars().contains(&param_var))
                    || b.terminator.uses_var(param_var)
            })
        })
}

/// Number of distinct CALL SITES (`Apply` / `ApplyIndirect` instruction or
/// `Invoke` / `InvokeIndirect` terminator) that consume any member of `rep`'s
/// union-find class as an argument. A site is counted ONCE regardless of how
/// many of its arg positions name the rep. Structural call-site multiplicity —
/// the over-fire discriminator for the construct-fed collection dead-param arm:
/// a collection consumed at >1 call site is live-across (used after the catch),
/// so a dead-param release would free it before a later borrowed use. Distinct
/// from `compute_alt_consumer_reps` (owned-transfer positions only) — this
/// counts BORROWED arg consumption too, which a second borrowed call is.
pub(super) fn rep_call_site_count(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    rep: ArcVarId,
) -> usize {
    let mut count = 0;
    for block in &func.blocks {
        for instr in &block.body {
            let args: &[ArcVarId] = match instr {
                ArcInstr::Apply { args, .. } | ArcInstr::ApplyIndirect { args, .. } => args,
                _ => continue,
            };
            if args.iter().any(|&a| uf.find(a) == rep) {
                count += 1;
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke { args, .. } | ArcTerminator::InvokeIndirect { args, .. } => {
                if args.iter().any(|&a| uf.find(a) == rep) {
                    count += 1;
                }
            }
            _ => {}
        }
    }
    count
}

/// Forwarder reps with an ALTERNATE consumer — a rep member used at a NON-forwarder
/// owned transfer position (a second owned call/Construct/Set arg, or a non-forwarder
/// Invoke owned-arg). When an alternate consumer owns the release, the per-var path
/// supplies it and the dead-param block-entry dec must NOT double it (the edge-release
/// gate; bounds the over-fire surface).
pub(super) fn compute_alt_consumer_reps(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    uf: &mut ForwarderUnionFind,
) -> FxHashSet<ArcVarId> {
    let mut alt: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if instr_is_forwarder_call(instr, contracts) {
                continue;
            }
            for v in instr_owned_position_transfer_vars(instr) {
                let r = uf.find(v);
                alt.insert(r);
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if !args
                .iter()
                .any(|&a| arg_owned_transfers_through_return(contracts, *callee, a, args))
            {
                for (pos, &v) in block.terminator.used_vars().iter().enumerate() {
                    if block.terminator.is_owned_position(pos) {
                        let r = uf.find(v);
                        alt.insert(r);
                    }
                }
            }
        }
    }
    alt
}

/// True iff `instr` is an `Apply` to a callee that owns-transfers an arg through its
/// return (the forwarder edge itself — excluded from the alternate-consumer scan).
fn instr_is_forwarder_call(instr: &ArcInstr, contracts: &FxHashMap<Name, MemoryContract>) -> bool {
    matches!(
        instr,
        ArcInstr::Apply { func: callee, args, .. }
            if args
                .iter()
                .any(|&a| arg_owned_transfers_through_return(contracts, *callee, a, args))
    )
}

/// True iff `callee`'s contract owns-transfers `arg` (at its position in `args`) through
/// the return (`transfers_through_return ∧ access == Owned`). The forwarder-identity
/// edge predicate; mirrors `compute_transfer_forwarder_anchors`'s over-fire boundary.
pub(super) fn arg_owned_transfers_through_return(
    contracts: &FxHashMap<Name, MemoryContract>,
    callee: Name,
    arg: ArcVarId,
    args: &[ArcVarId],
) -> bool {
    contracts.get(&callee).is_some_and(|c| {
        args.iter()
            .zip(c.params.iter())
            .any(|(&a, p)| a == arg && p.transfers_through_return && p.access == AccessClass::Owned)
    })
}
