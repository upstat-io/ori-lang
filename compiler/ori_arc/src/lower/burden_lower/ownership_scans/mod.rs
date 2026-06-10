//! Ownership / transfer scans feeding `emit_burden_ops`: owned-burden
//! collection, transfer-point and last-use detection, move-alias chains,
//! use counts, and the live-out / gen-kill dataflow inputs.

mod live_extract;
mod live_extract_site;
mod live_out;
mod move_alias;

pub(super) use live_extract::compute_fresh_sum_live_extract_lineage;
pub(super) use live_out::{compute_dead_owned_param_branch_releases, compute_live_out_owned};
pub(super) use move_alias::compute_transfer_via_move_alias;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::dimensions::AccessClass;
use crate::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, PrimOp, ValueRepr,
};
use crate::ownership::Ownership;
use ori_types::TypeRegistry;

use super::super::burden::TypeRef;
use super::super::burden_lookup::{idx_to_type_ref, lookup_burden};
use super::burden_carries_rc;
use super::BurdenLowerCtx;

/// Phase 1 — per-`ArcVarId` ownership-filtered burden lookup walk.
///
/// Build `ArcVarId -> Ownership` map from `func.params`. Locals (vars not in
/// params) lack `ArcParam.ownership` and are collected unconditionally; params
/// with `Borrowed` ownership are skipped (only owned `ArcVarId`s carry RC).
pub(super) fn collect_owned_burdens<'a>(
    ctx: &mut BurdenLowerCtx<'a>,
    func: &ArcFunction,
    type_registry: &'a TypeRegistry,
) {
    let param_ownership: FxHashMap<ArcVarId, Ownership> =
        func.params.iter().map(|p| (p.var, p.ownership)).collect();
    for (raw, &idx) in func.var_types.iter().enumerate() {
        let var = ArcVarId::new(
            u32::try_from(raw).unwrap_or_else(|_| panic!("var index {raw} fits in u32")),
        );
        if matches!(param_ownership.get(&var), Some(Ownership::Borrowed)) {
            continue;
        }
        let ty: TypeRef = idx_to_type_ref(idx, type_registry);
        let burden = lookup_burden(ty, type_registry);
        ctx.collected.push((var, burden));
    }
}

/// Phase 2 — transfer-point detection via the canonical helpers
/// `ArcInstr::used_vars()` and `ArcInstr::is_owned_position(pos)`. Covers
/// `Construct`, `PartialApply`, `CollectionReuse` (positions 1..=args.len),
/// `ApplyIndirect` (positions 1..= for Owned args), and `Apply` (positions
/// 0..args.len with `arg_ownership` filter) through the one canonical helper.
/// `Set`/`SetTag` use the IA-5 alias-transfer model (NOT covered by
/// `is_owned_position`'s `_ => false` catch-all per AIMS TF-15); `Set`'s
/// `value` is handled explicitly. Terminator transfer points land in
/// `compute_terminator_transfer_per_block`.
pub(super) fn detect_transfer_points<'a>(
    ctx: &mut BurdenLowerCtx<'a>,
    func: &ArcFunction,
    type_registry: &'a TypeRegistry,
) {
    for block in &func.blocks {
        for instr in &block.body {
            for (pos, &arg) in instr.used_vars().iter().enumerate() {
                if instr.is_owned_position(pos) {
                    let arg_idx = func.var_types[arg.index()];
                    let ty: TypeRef = idx_to_type_ref(arg_idx, type_registry);
                    let burden = lookup_burden(ty, type_registry);
                    ctx.transfer_points.push((arg, burden));
                }
            }
            if let ArcInstr::Set { value, .. } = instr {
                let value_idx = func.var_types[value.index()];
                let ty: TypeRef = idx_to_type_ref(value_idx, type_registry);
                let burden = lookup_burden(ty, type_registry);
                ctx.transfer_points.push((*value, burden));
            }
        }
    }
}

/// Phase 3 — per-block backward last-use detection: `BurdenDec(v)` emits
/// immediately following EVERY last-use of `v` along EVERY reachable CFG path.
/// Per-block linear scan, no global flow analysis / fixpoint / lattice
/// consultation. Terminator last-uses register at sentinel idx = `body.len()`
/// so terminator-ordering rules can distinguish them.
pub(super) fn detect_last_uses(ctx: &mut BurdenLowerCtx<'_>, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
        let terminator_idx = block.body.len();
        for arg in block.terminator.used_vars() {
            if seen.insert(arg) {
                ctx.last_use_points.push((arg, block_idx, terminator_idx));
            }
        }
        for (instr_idx, instr) in block.body.iter().enumerate().rev() {
            for &arg in &instr.used_vars() {
                if seen.insert(arg) {
                    ctx.last_use_points.push((arg, block_idx, instr_idx));
                }
            }
        }
    }
}

/// Filter `ctx.collected` to vars whose burden carries any RC-tracked
/// dimension. `lookup_burden(Idx::INT, ...)` returns `Some(BurdenRef)`
/// carrying the empty builtin burden; the filter MUST reject EMPTY specs via
/// `burden_carries_rc` vs naively admitting any `Some(_)`.
pub(super) fn compute_owned_vars_needing_rc(ctx: &BurdenLowerCtx<'_>) -> FxHashSet<ArcVarId> {
    ctx.collected
        .iter()
        .filter_map(|(var, burden)| {
            burden
                .as_ref()
                .filter(|b| burden_carries_rc(b))
                .map(|_| *var)
        })
        .collect()
}

/// Group `ctx.last_use_points` by `(block_idx, instr_idx)`, retaining only
/// vars that need RC. Output is consumed by the emission loop to position
/// `BurdenDec` ops at last-use sites.
pub(super) fn group_last_uses_filtered(
    ctx: &BurdenLowerCtx<'_>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<(usize, usize), Vec<ArcVarId>> {
    let mut last_uses_at: FxHashMap<(usize, usize), Vec<ArcVarId>> = FxHashMap::default();
    for &(var, b, i) in &ctx.last_use_points {
        if !owned_vars_needing_rc.contains(&var) {
            continue;
        }
        last_uses_at.entry((b, i)).or_default().push(var);
    }
    last_uses_at
}

/// Vars consumed at a BORROWED arg position of any `Invoke` / `InvokeIndirect`
/// terminator. Mirrors the per-block `invoke_terminator_borrowed_args` in
/// `emit.rs` (which gates the terminator-last-use `BurdenDec` suppression) but
/// computed function-wide for the FRESH-site `BurdenInc` suppression. A FRESH
/// value passed at a borrowed Invoke arg position has its terminator dec
/// suppressed there; suppressing its FRESH inc here keeps the per-value burden
/// ledger empty so the predicate-stack edge cleanup fully owns its release.
pub(super) fn compute_borrowed_terminator_invoke_args(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        let term = &block.terminator;
        if !matches!(
            term,
            crate::ir::ArcTerminator::Invoke { .. }
                | crate::ir::ArcTerminator::InvokeIndirect { .. }
        ) {
            continue;
        }
        for (pos, &var) in term.used_vars().iter().enumerate() {
            if !term.is_owned_position(pos) {
                borrowed.insert(var);
            }
        }
    }
    borrowed
}

/// `Let { Var(src) }` aliases whose SOLE use is a BORROWED terminator-Invoke arg
/// position. Such an alias is a borrow-view of `src`, NOT a new owned reference:
/// per RL-1 (`08-realization/RL-1.proof` + `AimsProof.Realization`
/// `RL1_emit_iff_not_elidable`) a duplication inc is emitted ONLY when a value is
/// passed to an OWNED param while still live, so `f(x, x)` over two Borrowed
/// params creates no reference at either arg — the owned SOURCE `x` carries the
/// sole inc + release. Without excluding these aliases the dup-alias FRESH-site
/// `BurdenInc` + borrowed-arg scope-exit `BurdenDec` pair on each alias leaves the
/// source's FRESH inc orphaned (VF-1 net=+1 leak). Excluded from
/// `owned_vars_needing_rc` (neither inc nor dec) per the LEDGER §06.1
/// "borrowed aliases get neither" principle. Use-count gated to 1 so a value also
/// used at an OWNED position keeps its burden ops.
pub(super) fn compute_borrowed_arg_let_aliases(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let borrowed_args = compute_borrowed_terminator_invoke_args(func);
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
    let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                // dst is the sole-use borrow-view; src must STAY LIVE (used >= 2,
                // a genuine duplication) so the OWNED source carries the
                // release. A move source (src used once) makes the alias the
                // sole carrier — excluding it would drop the release (leak).
                if borrowed_args.contains(dst)
                    && use_counts.get(dst).copied().unwrap_or(0) == 1
                    && use_counts.get(src).copied().unwrap_or(0) >= 2
                {
                    out.insert(*dst);
                }
            }
        }
    }
    out
}

/// `Project` dsts whose EVERY use is at a BORROWED (non-owned) position — a pure
/// borrow-view of an owned aggregate's field per `Spec: Annex E §AIMS TF-4`
/// (Project produces `Borrowed`). The PARENT aggregate owns the projected field
/// and releases it via its whole-var scope-exit drop-glue, so the borrowed
/// projection gets NO dec. An RcPtr-typed Project dst (e.g. `Project box.0 :
/// [int]`) enters `owned_vars_needing_rc` by TYPE alone; without this exclusion
/// a borrowed field-read (`@len [borrow]` / `@__index [borrow]`) gets a spurious
/// last-use `BurdenDec`, over-releasing the field the parent drop already frees.
///
/// Over-fire gate (RL-15a project-escape boundary): excluded ONLY when never used
/// at an owned arg position (`instr_transfer_vars` honors `is_owned_position`)
/// AND never returned — an escaping / owned-position-transferred Project IS an
/// owned reference and KEEPS its burden RC.
pub(super) fn compute_borrowed_projection_dsts(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut project_dsts: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let crate::ir::ArcInstr::Project { dst, .. } = instr {
                project_dsts.insert(*dst);
            }
        }
    }
    if project_dsts.is_empty() {
        return FxHashSet::default();
    }
    let mut transferred_or_escaped: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            for v in super::instr_transfer_vars(instr, func) {
                transferred_or_escaped.insert(v);
            }
        }
        let term = &block.terminator;
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if term.is_owned_position(pos) {
                transferred_or_escaped.insert(v);
            }
        }
        if let crate::ir::ArcTerminator::Return { value } = term {
            transferred_or_escaped.insert(*value);
        }
    }
    project_dsts.retain(|d| !transferred_or_escaped.contains(d));
    project_dsts
}

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
pub(super) fn compute_transfer_through_return_results(
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
pub(super) fn compute_transfer_through_return_param_vars(
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
pub(super) fn compute_use_counts_and_dup_aliases(
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
pub(super) fn compute_genuine_dup_move_aliases(
    func: &ArcFunction,
    dup_alias_dsts: &FxHashSet<ArcVarId>,
    full_move_vars: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    if super::genuine_dup_pair_coupling_disabled() {
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

/// Per-var use sites: body uses as `(block, instr_idx)`; terminator uses as
/// `(block, usize::MAX)` so any body index in the block precedes them.
fn collect_use_sites(func: &ArcFunction) -> FxHashMap<ArcVarId, Vec<(usize, usize)>> {
    let mut use_sites: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
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

/// Borrowed-store duplication args per AIMS RL-1: vars whose `Let { Var }`
/// alias-chain ROOT is a BORROWED param and that are consumed at an
/// aggregate-STORE position (`Construct` / `Reuse` / `CollectionReuse` arg or
/// `Set.value`).
///
/// A borrow proves the CALLER retains a live reference across the whole call
/// (RL-2: the caller emits the borrowed param's release —
/// `AimsProof.Realization::RL2_borrowed_param_emits_caller_dec`), so storing
/// the borrowed value into an aggregate ALWAYS creates a real SECOND
/// reference: the caller's reference persists and the aggregate takes a new
/// one. The store-site `BurdenInc` is LOAD-BEARING
/// (`AimsProof.Realization::RL1_duplication_balanced` — the duplication's inc
/// is matched by exactly one release: the container's drop); without it the
/// container's drop releases a reference no inc supplied (the caller's
/// still-live value frees at live=1 — use-after-free).
///
/// THREE exclusions bound the family:
///
/// - Call-arg consumers: a borrowed value passed at an `Apply` / `Invoke` arg
///   is OUT — call consumes have their own interprocedural / protocol
///   ownership accounting; injecting an inc there over-counts. Only the
///   aggregate-STORE family is classified.
/// - Non-param roots: only a chain rooted at a BORROWED param carries the
///   borrow proof. Owned roots keep their existing treatment (terminal move /
///   genuine-dup per [`compute_genuine_dup_move_aliases`]); over-adding on an
///   owned source leaks.
/// - RC-free types: a stored value whose burden carries no RC dimension needs
///   no inc (nothing to release).
///
/// Membership is per-VAR; the emission walk emits one inc per store SITE the
/// member is consumed at (each store duplicates). No paired dec is emitted in
/// this function — the container's drop is the matched release wherever the
/// container dies.
pub(super) fn compute_borrowed_store_dup_args(
    func: &ArcFunction,
    type_registry: &TypeRegistry,
) -> FxHashSet<ArcVarId> {
    if super::borrowed_store_dup_inc_disabled() {
        return FxHashSet::default();
    }
    let mut borrowed_rooted: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| p.ownership == Ownership::Borrowed)
        .map(|p| p.var)
        .collect();
    if borrowed_rooted.is_empty() {
        return FxHashSet::default();
    }
    // Vars are defined before use within the function walk, so a single
    // forward pass resolves each `Let { Var }` chain to its root (same
    // discipline as `collect_use_sites`' single pass).
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if borrowed_rooted.contains(src) {
                    borrowed_rooted.insert(*dst);
                }
            }
        }
    }
    let mut dup_args: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut admit = |var: ArcVarId| {
        if !borrowed_rooted.contains(&var) || dup_args.contains(&var) {
            return;
        }
        // L-9 repr-aware admission gate (same SSOT as `owned_vars_needing_rc`):
        // a provably-Scalar-repr stored value is a niche-packed copy — no
        // second reference exists, and a store-site inc on it can never lower.
        if super::is_provably_scalar_repr(func, var) {
            return;
        }
        let ty: TypeRef = idx_to_type_ref(func.var_types[var.index()], type_registry);
        if lookup_burden(ty, type_registry)
            .as_ref()
            .is_some_and(burden_carries_rc)
        {
            dup_args.insert(var);
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => {
                    for &arg in args {
                        admit(arg);
                    }
                }
                ArcInstr::Set { value, .. } => admit(*value),
                _ => {}
            }
        }
    }
    dup_args
}

/// Forward-reachable block set from `start`'s SUCCESSORS (`start` itself is
/// included only when a cycle re-reaches it).
fn successor_reachable_blocks(func: &ArcFunction, start: usize) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect()
        })
        .unwrap_or_default();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        if let Some(block) = func.blocks.get(b) {
            for s in crate::graph::successor_block_ids(&block.terminator) {
                stack.push(s.index());
            }
        }
    }
    visited
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
pub(super) fn compute_forwarder_identity_transparent_aliases(
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
            // A nested re-alias of an alias keeps the whole lineage classified
            // (single-hop only). The alias-def of a ttr param itself is the
            // vetted hop — skip it (its `src` use is the move into the alias).
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if alias_to_src.contains_key(src) {
                    decline(*src, &mut declined_srcs);
                }
                if aliases_of.contains_key(src) && alias_to_src.contains_key(dst) {
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
fn collect_ttr_param_aliases(
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
    let mut aliases_of: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    let mut alias_to_src: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if ttr_params.contains(src) {
                    aliases_of.entry(*src).or_default().push(*dst);
                    alias_to_src.insert(*dst, *src);
                }
            }
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

/// Snapshot vars consumed at an owned position by `instr`, used to suppress
/// `BurdenDec` at transfer points per AIMS RL-2. `Set.value` is added
/// explicitly per AIMS TF-15 (`is_owned_position`'s `_ => false` catch-all
/// excludes it). A list-concat `PrimOp Binary(Add)` `RcPointer` operand is added
/// per the dual-consuming `ori_list_concat_cow` runtime contract (the helper
/// dec/frees BOTH input buffers;
/// `is_owned_position`'s `_ => false` excludes `Let { PrimOp }`). Shared by the
/// driver, `moved_fields`, and `emit` submodules.
pub(super) fn instr_transfer_vars(instr: &ArcInstr, func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut transfer_vars = instr_owned_position_transfer_vars(instr);
    transfer_vars.extend(list_concat_consumed_operands(instr, func));
    transfer_vars
}

/// The `func`-free subset of `instr_transfer_vars`: vars consumed at an
/// `is_owned_position` arg plus the `Set.value` TF-15 carve-out. Used at the
/// `&mut`-walk emission site where `func` cannot be re-borrowed for `var_repr`;
/// the list-concat consume set is consulted there via the precomputed
/// `BurdenAnalysisCtx::list_concat_transfer_vars` instead.
pub(super) fn instr_owned_position_transfer_vars(instr: &ArcInstr) -> FxHashSet<ArcVarId> {
    let mut transfer_vars: FxHashSet<ArcVarId> = instr
        .used_vars()
        .iter()
        .enumerate()
        .filter_map(|(pos, &arg)| instr.is_owned_position(pos).then_some(arg))
        .collect();
    if let ArcInstr::Set { value, .. } = instr {
        transfer_vars.insert(*value);
    }
    transfer_vars
}

/// Per-block representative block-param vars needing exactly ONE RL-5 dead-at-entry
/// release for a forwarder-identity allocation.
///
/// Shape: an Owned non-scalar allocation `A` forwarded through a transfer-through-return
/// callee (`@id<T>(x: T) -> T = x`, `ParamContract.transfers_through_return ∧
/// access == Owned`) reaches a merge/return block's DEAD block-params (`Cardinality =
/// Absent` — the param is bound by the block but used nowhere on any forward path) via
/// `Jump` args on every predecessor edge. The Jump-arg → Owned-param handoff suppresses
/// the source's last-use dec (RL-4 Jump-arg exemption: ownership transfers to the
/// successor block-param), and that successor param is dead, so RL-5
/// (`AimsProof.Realization::RL5_dead_at_entry_cleanup`: an Owned non-scalar `Absent`
/// param gets ONE immediate dec) is the lineage's sole release point. The Phase-5 walk
/// emits no RL-5 dec → the allocation leaks (`RL5_cleanup_balanced` is violated: the
/// param entered with a live RC=1 reference that is never released).
///
/// One allocation reaching N dead params is the dedup case: the forwarder identity (the
/// Invoke/Apply result aliases its `[own]` arg, `id(x)=x`) makes `%4` (the source) and
/// `%7` (the forwarder result) the SAME RC=1 allocation, passed as TWO Jump args (`Jump
/// bb(.., %4, %7)`) to two dead params. RL-5's balance proof is PER-ALLOCATION (one
/// `[inc, dec]` per allocation, not per param): emitting a dec on BOTH params is
/// `[inc, dec, dec]` = net −1 = double-free. The cure emits EXACTLY ONE dec per distinct
/// source allocation reaching the block (dedup by the forwarder-identity rep), returning
/// one representative dead-param var per `(block, rep)`.
///
/// Forwarder-identity gate (the over-fire boundary): the dead-param's source allocation
/// MUST be reached through a forwarder edge (an Invoke/Apply transfer-through-return
/// result aliasing its owned arg). A plain `let d = Numbers(..); match d` dead param
/// (`compute_*` no Invoke-forwarder) is NOT admitted — its lineage is a distinct
/// (non-forwarder) dead-block-param sub-root; admitting it here would emit a release the
/// per-var path does not expect on the borrow-view sum-payload shapes (double-free).
///
/// Edge-release gate (the unwind subtlety): the rep is admitted ONLY when NO predecessor
/// edge into the block already releases the allocation — when an arm consumed the lineage
/// (e.g. an unwind `Resume` edge whose Phase-8a `unwind_cleanup` decs the value, or a
/// match arm that decs the inner payload) the block-entry dec would double the release.
/// The Phase-5 walk's union-find identity is conservative: a rep is admitted only when
/// the lineage's sole transfer is the forwarder + the dead-param handoff, with no
/// alternate release on any incoming edge.
pub(super) fn compute_dead_forwarder_block_param_releases(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashMap<usize, Vec<ArcVarId>> {
    let mut uf = ForwarderUnionFind::build(func, contracts);
    let used = function_used_vars(func);
    let alt_consumer_reps = compute_alt_consumer_reps(func, contracts, &mut uf);

    let mut out: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
        for &(param_var, _) in &block.params {
            if used.contains(&param_var) {
                continue; // not dead
            }
            // L-9 repr-aware admission gate (same SSOT as
            // `owned_vars_needing_rc`): a provably-Scalar-repr dead param
            // carries no allocation to release; an RL-5 dec on it can never
            // lower and survives as VF-1 ledger residue.
            if super::is_provably_scalar_repr(func, param_var) {
                continue;
            }
            let Some(rep) = dead_param_forwarder_rep(func, &mut uf, block_idx, param_var) else {
                continue;
            };
            if alt_consumer_reps.contains(&rep) {
                continue;
            }
            if seen_reps.insert(rep) {
                out.entry(block_idx).or_default().push(param_var);
            }
        }
    }
    out
}

/// Resolve a DEAD block-param's source allocation to a forwarder-identity rep, or
/// `None` when it is not a single forwarder lineage. The param carries no union edge
/// (it is a phi sink), so its allocation is the rep of the args feeding its position
/// across every `Jump` predecessor; admitted only when every feeding edge resolves to
/// ONE rep AND that rep is a forwarder identity.
fn dead_param_forwarder_rep(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    block_idx: usize,
    param_var: ArcVarId,
) -> Option<ArcVarId> {
    let rep = dead_param_single_feeding_rep(func, uf, block_idx, param_var)?;
    uf.is_forwarder_rep.contains(&rep).then_some(rep)
}

/// Resolve a DEAD block-param to the SINGLE union-find rep feeding its position
/// across every `Jump` predecessor, or `None` when the feeding edges resolve to
/// zero or more than one rep (a genuine phi over distinct allocations). This is
/// the gate-free core: callers apply their own admission gate (forwarder-identity
/// for [`dead_param_forwarder_rep`]; sum-aggregate-Construct for
/// [`compute_construct_fed_dead_param_lineage`]).
fn dead_param_single_feeding_rep(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    block_idx: usize,
    param_var: ArcVarId,
) -> Option<ArcVarId> {
    let pos = func.blocks[block_idx]
        .params
        .iter()
        .position(|&(p, _)| p == param_var)?;
    let mut feeding_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut any_feed = false;
    for pred in &func.blocks {
        if let ArcTerminator::Jump { target, args } = &pred.terminator {
            if target.index() == block_idx {
                if let Some(&arg) = args.get(pos) {
                    any_feed = true;
                    feeding_reps.insert(uf.find(arg));
                }
            }
        }
    }
    if !any_feed || feeding_reps.len() != 1 {
        return None;
    }
    feeding_reps.iter().next().copied()
}

/// Result of [`compute_construct_fed_dead_param_lineage`]: the dead-block-param
/// releases (Part A) + the lineage vars to suppress (Part B).
pub(super) struct ConstructFedDeadParamLineage {
    /// `block_idx → [dead block-param var]` — one representative dead-param var
    /// per `(block, rep)` needing exactly ONE RL-5 dead-at-entry `BurdenDec`.
    pub releases: FxHashMap<usize, Vec<ArcVarId>>,
    /// Every var in an admitted construct-fed dead-param lineage class. These
    /// carry spurious keep-alive incs (FRESH-Construct + dup-alias) and a
    /// misplaced release that must be suppressed (removed from
    /// `owned_vars_needing_rc`) so the sole release is the dead-param dec.
    pub suppressed_lineage_vars: FxHashSet<ArcVarId>,
}

/// RL-5 dead-at-entry release for a SUM-AGGREGATE-Construct-fed allocation
/// reaching a merge/return block's DEAD block-params, PLUS the spurious-op
/// suppression that the over-emitting lineage requires.
///
/// Shape (the `for x in Some(str) yield { break }` lineage): a sum-aggregate
/// `Construct` (`%1 = Construct Variant(Option.0)(%0)`, an `Option<str>` owning
/// the heap str `%0`) is threaded — possibly through `Let { Var }` aliases
/// (`%3 = %1`) — as a `Jump` arg to a merge/return block's DEAD block-param
/// (`Jump bb3(%1)` → `%7: str?`, `Cardinality = Absent`). The Jump-arg → Owned-
/// param handoff (RL-4 exemption) defers `%1`'s release to the dead successor
/// param `%7`, which the Phase-5 walk never released → the str backing leaks
/// (`RL5_cleanup_balanced` violated).
///
/// Distinct from [`compute_dead_forwarder_block_param_releases`] in TWO ways:
///   1. The feeding allocation is a `Construct` (not an Invoke/Apply forwarder
///      identity), so the rep is gated by `is_sum_aggregate_construct_rep`, NOT
///      `is_forwarder_rep`.
///   2. The lineage OVER-emits — the Construct gets a FRESH-site `BurdenInc`
///      (TF-3) AND its Let-Var alias gets a dup-alias `BurdenInc` (the
///      `use_counts >= 2` cardinality proxy mis-classes the same-alloc alias
///      `%3 = %1` as a duplication: `%1` is "live" only because it ALSO feeds the
///      `Jump bb3(%1)` handoff), plus a misplaced alias `BurdenDec` in the Some
///      arm. RL-2 (`RL2_release_exactly_once`) requires ONE allocation released
///      EXACTLY once with ZERO keep-alive incs; the lineage's two incs + one
///      misplaced dec net +1 (leak). The cure removes the whole lineage from
///      `owned_vars_needing_rc` (suppressing both incs + the misplaced dec) and
///      supplies the sole release at the dead param `%7`.
///
/// Gates (the over-fire boundary — a double-free is FAR worse than the leak):
///   (a) FRESH heap allocation: the rep's allocation root is a sum-aggregate
///       `Construct` (`is_sum_aggregate_construct_rep`). A non-Construct lineage
///       (forwarder, plain param) is NOT admitted here.
///   (b) Heap element: the Construct dst is in `owned_vars_needing_rc` (the
///       burden machinery proved the sum payload carries RC). An `int?`
///       (`[Scalar]` repr) Construct is absent from `owned_vars_needing_rc` → not
///       admitted (the int variant's burden ops are codegen no-ops anyway, but
///       gating here keeps the suppression scoped to genuine heap lineages).
///   (c) Dead merge/return param: the param is `Cardinality = Absent` (used
///       nowhere) and every feeding `Jump` edge resolves to the ONE rep
///       (`dead_param_single_feeding_rep`).
///   (d) No alternate release: the rep has no member used at a NON-forwarder
///       owned transfer position (`compute_alt_consumer_reps`). When an arm
///       consumed the lineage at an owned call/Construct/Set the per-var path
///       owns that release and the dead-param dec would double it.
///
/// SAFE for the both-paths-fail shape (verified `ORI_DISABLE_BURDEN_OPS=1`
/// emits zero Option release): the predicate stack emits no normal-path release
/// for this lineage, so suppressing the burden ops + supplying the dead-param
/// release does not race a predicate-stack release. Spec: Annex E §AIMS RL-5 +
/// RL-4 + RL-2.
pub(super) fn compute_construct_fed_dead_param_lineage(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> ConstructFedDeadParamLineage {
    let mut uf = ForwarderUnionFind::build(func, contracts);
    let used = function_used_vars(func);
    let alt_consumer_reps = compute_alt_consumer_reps(func, contracts, &mut uf);

    let mut releases: FxHashMap<usize, Vec<ArcVarId>> = FxHashMap::default();
    let mut suppressed_lineage_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
        // Collect the dead params + their feeding reps first, then apply gates,
        // to avoid borrow conflicts on `uf` inside the loop.
        let dead_params: Vec<(ArcVarId, ArcVarId)> = block
            .params
            .iter()
            .filter(|(p, _)| !used.contains(p))
            .filter_map(|&(param_var, _)| {
                dead_param_single_feeding_rep(func, &mut uf, block_idx, param_var)
                    .map(|rep| (param_var, rep))
            })
            .collect();
        // The set of ALL reps feeding a DEAD block-param in THIS block. A nested
        // heap-aggregate Construct whose rep is in this set has its own copy dead
        // (used nowhere but its dead-param handoff) — the gate for nested-lineage
        // suppression below (Part B (cont. 2)).
        let block_dead_reps: FxHashSet<ArcVarId> =
            dead_params.iter().map(|&(_, rep)| rep).collect();
        for (param_var, rep) in dead_params {
            if alt_consumer_reps.contains(&rep) {
                continue;
            }
            // Disjointness gate: a FORWARDER-identity rep is owned by
            // `compute_dead_forwarder_block_param_releases` (which KEEPS the
            // keep-alive inc + adds the dead-param dec — net 0 for a transferred-in
            // allocation whose `+1` came from the forwarded arg). This pass's Part-B
            // suppression would strip that keep-alive inc and double-free. The two
            // passes target DISJOINT reps: forwarder-fed (`@id(x)=x` result) vs
            // construct-fed (a local `Construct` not forwarded through a call). A
            // lineage that is BOTH (a forwarded Construct) stays with the forwarder
            // pass — its `+1` is the forwarded arg's alloc, not a spurious keep-alive.
            if uf.is_forwarder_rep.contains(&rep) {
                continue;
            }
            // Gate (a): rep's allocation root is a sum-aggregate Construct.
            if !uf.is_sum_aggregate_construct_rep(rep) {
                continue;
            }
            // Gate (b): heap element — at least one class member carries RC.
            let members = uf.class_members(rep);
            if !members.iter().any(|m| owned_vars_needing_rc.contains(m)) {
                continue;
            }
            if seen_reps.insert(rep) {
                releases.entry(block_idx).or_default().push(param_var);
                // Part B: suppress the spurious keep-alive incs + misplaced
                // release on the whole lineage class. The dead param itself is
                // NOT suppressed — it carries the sole RL-5 release.
                for m in &members {
                    if *m != param_var {
                        suppressed_lineage_vars.insert(*m);
                    }
                }
                // Part B (cont.): the heap ELEMENT borrow-views projected out of
                // the lineage (`%11 = Project %3.1` extracting the `str` payload,
                // plus their `Let { Var }` alias closure `%12 = %11`) are BORROWS
                // of the lineage's heap element (TF-4 `Project` is Borrowed). The
                // lineage's sole release at the dead param frees that element, so a
                // borrow-view release double-frees it. A `Project`-dst escapes
                // `compute_borrowed_projection_dsts` once it is re-aliased by a
                // Let-Var hop. Suppress the projected-element borrow-view closure.
                for view in collect_lineage_element_borrow_views(func, &members) {
                    suppressed_lineage_vars.insert(view);
                }
                // Part B (cont. 2): NESTED heap-aggregate Construct lineages
                // transitively OWNED by this released sum-aggregate. The matched
                // sum payload may itself be a heap Construct (`Some(Node{..})`)
                // that wraps deeper Constructs (`Node{ next: Some(Node{..}) }`).
                // Each such nested Construct is moved into its parent via an OWNING
                // `Construct`-arg (RL-2 transfer), so this sum-aggregate's
                // dead-param release transitively frees it. The base walk
                // nonetheless gives each nested Construct a spurious FRESH-site
                // keep-alive `BurdenInc` (its rep ALSO feeds a Jump-arg to its own
                // dead block-param, so `use_counts >= 2` mis-classes it as a
                // duplication) → +1 leak per nested level. Suppress those whole
                // lineage classes too. NO separate release: this aggregate's dec
                // already owns them (a second dec would double-free — the over-fire
                // boundary). Gate: the nested Construct's rep ALSO feeds a dead
                // block-param in THIS block (`block_dead_reps`), so its own copy is
                // dead (sole escapes = into the released parent + the dead param).
                let nested = collect_nested_construct_owned_dead_lineages(
                    func,
                    &mut uf,
                    &members,
                    &block_dead_reps,
                    rep,
                    owned_vars_needing_rc,
                );
                for v in nested {
                    suppressed_lineage_vars.insert(v);
                }
            }
        }
    }
    ConstructFedDeadParamLineage {
        releases,
        suppressed_lineage_vars,
    }
}

/// The lineage members of every NESTED heap-aggregate `Construct` transitively
/// owned (via owning `Construct`-args + `Let { Var }` aliases) by the admitted
/// sum-aggregate `released_rep`, whose own rep ALSO feeds a dead block-param in
/// the same block (`block_dead_reps`).
///
/// Shape (the `Node { value, next: Option<Node> }` recursion matched out of an
/// `Option`/`Result`): the matched aggregate `%10 = Construct Variant(Option.0)(%9)`
/// (the released `released_rep`) owns `%9 = %7`, where `%7 = Construct Struct(Node)
/// (%4, %6)` is a nested heap Construct that ITSELF feeds a dead block-param
/// (`Jump bb1(.., %7, ..)` → dead `%15`). `%7` in turn owns `%6 = Construct
/// Variant(Option.0)(%5)` owning `%5 = %2 = Construct Struct(Node)(%0, %1)`, also
/// dead-param-fed. The base walk emits a spurious FRESH-site `BurdenInc` on each
/// of `%2`/`%7` (their rep feeds a Jump-arg to a dead param → `use_counts >= 2`
/// dup-proxy) → +1 leak per nested level (`RL2_release_exactly_once` violated:
/// the parent's single release already frees the whole tree).
///
/// Traversal: BFS from `released_rep`'s class members, following OWNING
/// `Construct`-arg edges (`Construct { dst, args }` → each `arg` whose repr is
/// heap and whose rep is in `block_dead_reps`) and `Let { Var }` aliases. Each
/// discovered nested rep contributes its whole class to the suppression set. The
/// `released_rep` itself is excluded (handled by the caller's Part-B). NO release
/// is emitted for the nested reps — the parent's dead-param dec owns them.
///
/// Over-fire boundary (a double-free is FAR worse than the leak):
///   - PAYLOAD-EXTRACTED-LIVE precondition (`released_payload_escapes_live`): if
///     the released aggregate's heap payload is PROJECTED out to a LIVE block-param
///     destination (`Some(node) -> node` extracting + returning the inner heap
///     Node, vs `Some(node) -> node.value` reading only a scalar field), the
///     extracted copy holds a live reference to the same allocation as the nested
///     Construct. The parent's dead-param dec frees the tree AND the live extract's
///     own release frees its copy → double-free. The WHOLE nested suppression is
///     aborted in that case (the parent release + the live extract's release are
///     both correct as-is; the nested keep-alive incs are NOT spurious then).
///   - `block_dead_reps` membership: a nested Construct whose own copy is LIVE
///     (used outside the dead-param handoff) is NOT suppressed — it has a genuine
///     independent reference needing its own release. Only a dead-copy nested
///     Construct (sole escapes = into the released parent + its own dead param) is
///     transitively freed by the parent's dec.
///   - heap repr (`owned_vars_needing_rc` membership of a class member): a scalar
///     nested aggregate carries no RC, nothing to suppress.
///   - owning-`Construct`-arg edges only: a `Project` (Borrowed, TF-4) or a
///     non-owning position does NOT establish transitive ownership; the parent's
///     release does not reach a borrowed view (handled separately by
///     `collect_lineage_element_borrow_views`).
fn collect_nested_construct_owned_dead_lineages(
    func: &ArcFunction,
    uf: &mut ForwarderUnionFind,
    released_members: &FxHashSet<ArcVarId>,
    block_dead_reps: &FxHashSet<ArcVarId>,
    released_rep: ArcVarId,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    // PAYLOAD-EXTRACTED-LIVE precondition: abort the whole nested suppression when
    // the released aggregate's heap payload escapes live (its `Project` view reaches
    // a live block-param). See the over-fire boundary in the doc comment.
    if released_payload_escapes_live(func, released_members, owned_vars_needing_rc) {
        return FxHashSet::default();
    }
    // Map each var defined by a `Construct` to its owning args (heap-aggregate
    // construction transfers each arg inward). Built once per call.
    let mut construct_owned_args: FxHashMap<ArcVarId, Vec<ArcVarId>> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, args, .. } = instr {
                construct_owned_args.insert(*dst, args.clone());
            }
        }
    }
    let mut suppressed: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut visited_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    visited_reps.insert(released_rep);
    // BFS frontier of vars whose owning-Construct args we still expand. Seed with
    // every member of the released aggregate's class (the Construct dst + its
    // Let-Var aliases).
    let mut frontier: Vec<ArcVarId> = released_members.iter().copied().collect();
    while let Some(var) = frontier.pop() {
        let Some(args) = construct_owned_args.get(&var).cloned() else {
            continue;
        };
        for arg in args {
            let arg_rep = uf.find(arg);
            if visited_reps.contains(&arg_rep) {
                continue;
            }
            // Gate: the nested arg's rep must ALSO feed a dead block-param in this
            // block (its own copy is dead) AND carry RC (heap aggregate). A live or
            // scalar arg is NOT transitively suppressible.
            let nested_members = uf.class_members(arg_rep);
            let is_dead = block_dead_reps.contains(&arg_rep);
            let is_heap = nested_members
                .iter()
                .any(|m| owned_vars_needing_rc.contains(m));
            if is_dead && is_heap {
                visited_reps.insert(arg_rep);
                for m in &nested_members {
                    suppressed.insert(*m);
                }
                // Recurse into the nested Construct's class to reach deeper levels.
                frontier.extend(nested_members.iter().copied());
            }
        }
    }
    suppressed
}

/// True iff the released aggregate's heap payload is PROJECTED out to a LIVE
/// block-param destination — the `Some(node) -> node` extract-and-return shape
/// (vs `Some(node) -> node.value` reading only a scalar field).
///
/// When the heap payload escapes live, the extracted copy holds a live reference
/// to the SAME allocation as the nested Construct, with its OWN release. Suppressing
/// the nested Construct's keep-alive inc then double-frees (the parent's dead-param
/// dec frees the tree AND the live extract's release frees its copy). This is the
/// over-fire boundary that aborts the whole nested-lineage suppression.
///
/// Detection: a `Project { dst, value }` whose `value` is a released-lineage member
/// AND whose `dst` is a HEAP aggregate (`owned_vars_needing_rc`) AND whose
/// `Let { Var }` / `Jump`-arg closure reaches a block-param that is USED (live). A
/// scalar projection (`Project %node.0` → an `int .value`) is NOT heap, so it does
/// not trip this gate — that is the `node.value` case that stays suppressible.
fn released_payload_escapes_live(
    func: &ArcFunction,
    released_members: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    let used = function_used_vars(func);
    // ALL Project views of the released lineage's payload (owned or borrowed — the
    // extracted Node flows out as a borrowed `Project` view re-bound to an OWNED
    // live param). The discriminator is whether the closure reaches a USED param
    // that is ALSO owned (a live heap reference), NOT whether the Project dst
    // itself is owned.
    let mut payload_views: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if released_members.contains(value) {
                    payload_views.insert(*dst);
                }
            }
        }
    }
    if payload_views.is_empty() {
        return false;
    }
    // Forward `Let { Var }` alias + nested-`Project` + `Jump`-arg → block-param
    // closure of the payload views: if any reaches a USED block-param that is ALSO
    // an owned heap var (a live heap extract — `Some(node) -> node`), the payload
    // escapes live. A scalar `.value` projection reaches a USED but NON-owned int
    // param, which does NOT abort (the suppressible `node.value` case).
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if payload_views.contains(src) => {
                        if payload_views.insert(*dst) {
                            grew = true;
                        }
                    }
                    // A `Project` of a payload view that is ITSELF a heap aggregate
                    // stays in the closure (the extracted Node's own field, still the
                    // released allocation's identity); a scalar field projection is a
                    // distinct scalar value and drops out of the closure.
                    ArcInstr::Project { dst, value, .. }
                        if payload_views.contains(value) && owned_vars_needing_rc.contains(dst) =>
                    {
                        if payload_views.insert(*dst) {
                            grew = true;
                        }
                    }
                    _ => {}
                }
            }
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                for (pos, &arg) in args.iter().enumerate() {
                    if payload_views.contains(&arg) {
                        if let Some(&(param, _)) = func.blocks[target.index()].params.get(pos) {
                            // A USED (live) destination param that is ALSO an owned
                            // heap var means the heap payload escapes live → abort.
                            if used.contains(&param) && owned_vars_needing_rc.contains(&param) {
                                return true;
                            }
                            if payload_views.insert(param) {
                                grew = true;
                            }
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    false
}

/// The heap-element borrow-view closure of a suppressed construct-fed lineage:
/// every `Project { value, .. }` dst whose `value` is a lineage member (the
/// element extracted out of the Option / sum payload), PLUS the `Let { Var }`
/// alias closure of those projection dsts. These are BORROWS of the lineage's
/// heap element (TF-4), so they carry no release — the lineage's sole dead-param
/// release frees the element. Excluded from `owned_vars_needing_rc` alongside the
/// lineage class itself.
fn collect_lineage_element_borrow_views(
    func: &ArcFunction,
    lineage_members: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut views: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: Project dsts whose projected value is a lineage member.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if lineage_members.contains(value) {
                    views.insert(*dst);
                }
            }
        }
    }
    if views.is_empty() {
        return views;
    }
    // Fixpoint: a `Let { Var(src) }` whose `src` is already a view makes `dst` a
    // view too (the alias of a borrow is a borrow).
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
                    if views.contains(src) && views.insert(*dst) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    views
}

/// Forwarder reps with an ALTERNATE consumer — a rep member used at a NON-forwarder
/// owned transfer position (a second owned call/Construct/Set arg, or a non-forwarder
/// Invoke owned-arg). When an alternate consumer owns the release, the per-var path
/// supplies it and the dead-param block-entry dec must NOT double it (the edge-release
/// gate; bounds the over-fire surface).
fn compute_alt_consumer_reps(
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
fn arg_owned_transfers_through_return(
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

/// Function-wide used-var set: a var appearing in ANY instr / terminator operand
/// position. A block-param NOT in this set is dead (`Cardinality = Absent`); its only
/// appearance is its own param-binding slot.
fn function_used_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut used: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            used.extend(instr.used_vars());
        }
        used.extend(block.terminator.used_vars());
    }
    used
}

/// RL-2 scope-exit release for a transfer-through-return forwarder RESULT whose
/// monomorphized result-type burden is EMPTY (`burden_carries_rc == false`), so the
/// result lineage was never collected into `owned_vars_needing_rc` and gets neither
/// a FRESH inc nor any scope-exit dec — leaking its transferred-in allocation when
/// the lineage is consumed only by a borrow-projection / borrow-read then dies.
///
/// Returns `(block_idx, instr_idx) -> [result_var]`: a single whole-var `BurdenDec`
/// emitted AFTER the lineage's last-use instruction. The whole-var dec lowers
/// (Phase 7) to a `RcDec` whose drop-glue recursively frees the result's owned
/// fields (`result_list`'s `[int]` Ok-payload) OR the result pointer itself
/// (`set_int`'s `{int}` buffer) — `RL2_release_exactly_once` holds: the transferred-in
/// allocation's single lifecycle `+1` is matched by this single `-1`.
///
/// THE ROOT (ground-truthed 2026-06-09): `@id<T>(x: T) -> T` instantiated returns a
/// distinct monomorphized result type `Idx` whose `UserBurdenSpec` is empty, so
/// `compute_owned_vars_needing_rc` skips the result lineage. RL-34 makes the caller
/// own the returned allocation; `RL2_borrowed_param_emits_caller_dec` mandates a dec
/// at the borrow-read last use. The dec is MISSING. This restores it (the impl
/// under-emits; the calculus is correct — case-(a) per `arc.md §CP-1`).
///
/// CONSERVATIVE GATES (the over-emission / double-free boundary — add ONLY when ALL hold):
///  (a) the var is an Apply/Invoke result whose callee `transfers_through_return ∧ Owned`
///      (the forwarder identity — RL-34) AND its forwarder-rep lineage exists;
///  (b) NO lineage class member is in `owned_vars_needing_rc` — there is NO existing
///      release (distinguishes from the `inherent` over-emission, where the result
///      `carries_rc` is true, IS in `owned_vars`, and already over-decs, so a release
///      here would deepen the double-free);
///  (c) the lineage rep is NOT in `alt_consumer_reps` — never consumed at an owned arg
///      position / returned / stored (a moved/transferred result's downstream consumer
///      decs it; adding a dec here double-frees);
///  (d) the lineage IS used (a genuinely-dead-immediately result is the dead-block-param
///      shape, owned by `compute_dead_forwarder_block_param_releases`).
/// When ANY gate is uncertain the release is NOT added — under-emission is the current
/// leak (no regression); over-emission is a DOUBLE-FREE (catastrophic).
pub(super) fn compute_forwarder_result_under_release(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> {
    let mut uf = ForwarderUnionFind::build(func, contracts);
    let alt_consumer_reps = compute_alt_consumer_reps(func, contracts, &mut uf);

    // Collect the forwarder-result vars: Apply/Invoke `dst` whose callee owns-transfers
    // an arg through its return. Repr-admit RcPointer / FatValue / Aggregate (the
    // forwarded heap value or sum/struct wrapper); scalars carry no RC.
    let mut result_vars: Vec<ArcVarId> = Vec::new();
    let result_repr_admits = |dst: ArcVarId| -> bool {
        matches!(
            func.var_repr(dst),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue | ValueRepr::Aggregate)
        )
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
                if args
                    .iter()
                    .any(|&a| arg_owned_transfers_through_return(contracts, *callee, a, args))
                    && result_repr_admits(*dst)
                {
                    result_vars.push(*dst);
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            if args
                .iter()
                .any(|&a| arg_owned_transfers_through_return(contracts, *callee, a, args))
                && result_repr_admits(*dst)
            {
                result_vars.push(*dst);
            }
        }
    }

    let mut out: FxHashMap<(usize, ForwarderReleasePos), Vec<ArcVarId>> = FxHashMap::default();
    let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for result_var in result_vars {
        let rep = uf.find(result_var);
        // Gate (a): forwarder-identity rep.
        if !uf.is_forwarder_rep.contains(&rep) {
            continue;
        }
        // Gate (c): not owned-transferred / returned / stored downstream.
        if alt_consumer_reps.contains(&rep) {
            continue;
        }
        // Gate (e) — the FRESH-Construct-owner boundary (the over-emission distinguisher).
        // Decline when the forwarder's FULL class (arg + result, via `class_members`)
        // contains a FRESH-allocating definition (`Construct`/`Reuse`/`CollectionReuse`/
        // `PartialApply`) whose dst is owned-RC-tracked: that fresh member OWNS the shared
        // allocation and retains its own scope-exit release, so a whole-var release here
        // double-frees it (the `host`/imported-generic shape: `%3 = Construct List` flows
        // `%5 = %3` → `identity(%5)` → result, and `%3`'s native release covers the alloc).
        // FIRE only when the allocation entered the class via a CALL RESULT (`@__collect_set`)
        // with no in-class FRESH-Construct owner — then no retained release exists (the
        // `set_int` shape). The class members are resolved BEFORE the result-side narrowing
        // because the over-fire owner is UPSTREAM of the result (the transferred-in arg).
        if class_has_fresh_construct_owner(func, &uf.class_members(rep), owned_vars_needing_rc) {
            continue;
        }
        // RESULT-SIDE lineage: the result var + its FORWARD Let-Var alias closure (the
        // `%10 = %8` continuation). DISTINCT from the full forwarder-rep class (which
        // ALSO unions the transferred-in ARG `%7` — whose own release is correctly
        // transfer-suppressed at the forwarder call). Gate (b) + the release-site walk
        // operate on the result-side lineage so the upstream arg's owned-RC membership
        // does NOT mask the genuinely-unreleased result.
        let result_lineage = result_side_lineage(func, result_var);
        // Gate (b): NO existing release on the RESULT-SIDE lineage (the carries_rc=false
        // bug). If any result-side member is owned-RC-tracked, the existing last-use
        // machinery already releases it — adding a dec here double-frees (the `inherent`
        // shape: its result `%7` carries_rc=true, IS in owned_vars, already over-decs).
        if result_lineage
            .iter()
            .any(|m| owned_vars_needing_rc.contains(m))
        {
            continue;
        }
        // Gate (c, cont.): the result lineage's payload may be PROJECTED to an
        // owned-RC var that the existing machinery releases (e.g. a `Box<[int]>`
        // forwarder whose `[int]` field is projected and owns its own dec). If any
        // payload-projection of a result-side member carries RC and is released, the
        // whole-var dec here would double-free that field. Decline.
        if lineage_has_released_projection(func, &result_lineage, owned_vars_needing_rc) {
            continue;
        }
        // Dedup: one release per distinct forwarder allocation (multiple result vars
        // can alias one rep through the union-find; a second dec double-frees).
        if !seen_reps.insert(rep) {
            continue;
        }
        // Find the result-side lineage's death point (just past its final borrow-read).
        // Gate (d): a lineage with NO use is the dead-block-param shape (owned elsewhere)
        // — skip.
        let Some(site) = lineage_release_site(func, &result_lineage) else {
            continue;
        };
        out.entry(site).or_default().push(result_var);
    }
    out
}

/// The RESULT-SIDE lineage of a forwarder result: the result var plus the FORWARD
/// closure over `Let { Var }` aliases (`%10 = %8`) AND borrow `Project` dsts
/// (`%14 = Project %9.1` — the Ok-payload of an Aggregate result). EXCLUDES the
/// transferred-in arg (upstream of the forwarder call) — that arg's own release is
/// transfer-suppressed at the call site and must NOT mask the result's missing release.
///
/// Forward-only fixpoint: seed with `result_var`; repeatedly add any `Let { dst = Var(m) }`
/// dst or `Project { dst, value: m }` dst whose source `m` is already a member, until no
/// growth. Gives the set of vars that share the result's allocation downstream of the
/// forwarder — the lineage that needs the single whole-var release + whose final use is
/// the death point.
fn result_side_lineage(func: &ArcFunction, result_var: ArcVarId) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    members.insert(result_var);
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } if members.contains(src) && members.insert(*dst) => grew = true,
                    ArcInstr::Project { dst, value, .. }
                        if members.contains(value) && members.insert(*dst) =>
                    {
                        grew = true;
                    }
                    _ => {}
                }
            }
        }
        if !grew {
            break;
        }
    }
    members
}

/// True iff any member of the forwarder class is defined by a FRESH-allocating
/// instruction (`Construct` / `Reuse` / `CollectionReuse` / `PartialApply`) AND is in
/// `owned_vars_needing_rc`. Such a member OWNS the class's shared allocation and retains
/// its own scope-exit release — a whole-var forwarder-result release would double-free it.
///
/// The over-emission distinguisher between the genuinely-unreleased forwarder result
/// (allocation enters via a CALL RESULT — `@__collect_set` — with NO in-class FRESH-Construct
/// owner) and the already-released one (`host`: `%3 = Construct List` flows into the
/// forwarder, retains its native release). Resolving the FULL class (not result-side) is
/// load-bearing — the FRESH owner is UPSTREAM of the result (the transferred-in arg).
fn class_has_fresh_construct_owner(
    func: &ArcFunction,
    class_members: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            let fresh_dst = match instr {
                ArcInstr::Construct { dst, .. }
                | ArcInstr::Reuse { dst, .. }
                | ArcInstr::CollectionReuse { dst, .. }
                | ArcInstr::PartialApply { dst, .. } => Some(*dst),
                _ => None,
            };
            if let Some(dst) = fresh_dst {
                if class_members.contains(&dst) && owned_vars_needing_rc.contains(&dst) {
                    return true;
                }
            }
        }
    }
    false
}

/// Where a forwarder-result whole-var `BurdenDec` lands relative to one block. The
/// release follows the lineage's FINAL borrow-read so the read completes before the
/// allocation is freed (no UAF).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ForwarderReleasePos {
    /// At the START of the block's body (the normal-successor of a borrowed
    /// terminator-`Invoke` whose arg was the lineage's final use — the borrowed call
    /// completed on the predecessor's terminator, the result is dead at this entry).
    BlockEntry,
    /// Immediately AFTER the body instruction at `instr_idx` (the lineage's final use
    /// was a body `Apply`/etc. borrow at that position).
    AfterInstr(usize),
}

/// True iff any `Project` of a lineage member produces an owned-RC dst that the
/// existing last-use machinery already releases (in `owned_vars_needing_rc`). Such a
/// projection's field is freed by its own dec; a whole-var release on the parent
/// would double-free it. Over-fire boundary for `compute_forwarder_result_under_release`.
fn lineage_has_released_projection(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
    owned_vars_needing_rc: &FxHashSet<ArcVarId>,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                if members.contains(value) && owned_vars_needing_rc.contains(dst) {
                    return true;
                }
            }
        }
    }
    false
}

/// Resolve the lineage's death point — the `(block_idx, pos)` just past its FINAL
/// borrow-read, where the whole-var `BurdenDec` lands. Walks every block in forward
/// CFG order; the last block carrying a member use wins (the lineage is straight-line
/// forwarder-then-borrow, so the textually-last use is the death point).
///
/// Two terminal shapes:
///  - the final use is a body instruction (e.g. `Apply @__index(.. [borrow])`) →
///    `AfterInstr(instr_idx)`;
///  - the final use is a BORROWED terminator-`Invoke` arg (e.g. `Invoke @len(.. [borrow])
///    normal bbN`) → the value survives the borrowed call and dies at the normal
///    successor's entry → `(normal_succ, BlockEntry)`.
///
/// An OWNED terminator-position use (Return / Jump-arg / owned Invoke-arg) is a
/// transfer, NOT a borrow-read — `compute_alt_consumer_reps` already excluded such a
/// lineage (gate c), so it cannot reach here; return `None` defensively if it does.
fn lineage_release_site(
    func: &ArcFunction,
    members: &FxHashSet<ArcVarId>,
) -> Option<(usize, ForwarderReleasePos)> {
    let mut site: Option<(usize, ForwarderReleasePos)> = None;
    for (block_idx, block) in func.blocks.iter().enumerate() {
        // Body-instruction uses (skip Let-Var alias re-binds: their dst is itself a
        // member, not a terminal read).
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Let {
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                if members.contains(src) {
                    continue;
                }
            }
            if instr.used_vars().iter().any(|v| members.contains(v)) {
                site = Some((block_idx, ForwarderReleasePos::AfterInstr(instr_idx)));
            }
        }
        // Terminator borrow-use: a member at a BORROWED position of an `Invoke` /
        // `InvokeIndirect` (the result survives, dies at the normal successor's entry).
        let term = &block.terminator;
        if let ArcTerminator::Invoke { normal, .. } | ArcTerminator::InvokeIndirect { normal, .. } =
            term
        {
            for (pos, &v) in term.used_vars().iter().enumerate() {
                if members.contains(&v) && !term.is_owned_position(pos) {
                    site = Some((normal.index(), ForwarderReleasePos::BlockEntry));
                }
            }
        }
    }
    site
}

/// Structural forwarder-identity union-find over Let{Var} aliases (`%6 = %4`) +
/// forwarder result→arg edges (Invoke/Apply `transfers_through_return ∧ Owned`,
/// `%7 = id(%6 [own])`). Mirrors `compute_same_alloc_reps` (`edge_cleanup.rs`) EXCEPT it
/// is structural (no `state_map.apply_result_aliases()` — unavailable at Phase 5) and
/// intentionally EXCLUDES the Jump-arg → block-param edge (a phi merge over DISTINCT
/// allocations is NOT a same-allocation relation). `is_forwarder_rep` records reps that
/// participate in a forwarder edge (the over-fire gate).
struct ForwarderUnionFind {
    parent: FxHashMap<ArcVarId, ArcVarId>,
    is_forwarder_rep: FxHashSet<ArcVarId>,
    /// Vars defined by a sum-aggregate `Construct` (`EnumVariant`) — the FRESH
    /// heap allocation root of an `Option<T>` / `Result<T, E>` / user sum lineage.
    /// Tracked per-var (resolved to rep via `find`) so the construct-fed
    /// dead-block-param scan can gate on "this rep's allocation root is a sum
    /// Construct" without re-walking the body.
    sum_aggregate_construct_dsts: FxHashSet<ArcVarId>,
}

impl ForwarderUnionFind {
    fn build(func: &ArcFunction, contracts: &FxHashMap<Name, MemoryContract>) -> Self {
        let mut uf = ForwarderUnionFind {
            parent: FxHashMap::default(),
            is_forwarder_rep: FxHashSet::default(),
            sum_aggregate_construct_dsts: FxHashSet::default(),
        };
        // Edge type 1: Let{Var} aliases.
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    uf.union(*dst, *src);
                }
                // Record sum-aggregate `Construct` dsts (the FRESH allocation
                // root of an Option / Result / user sum lineage). Recorded on
                // the raw dst; resolved to rep at query time via `find`.
                if let ArcInstr::Construct {
                    dst,
                    ctor: CtorKind::EnumVariant { .. },
                    ..
                } = instr
                {
                    uf.sum_aggregate_construct_dsts.insert(*dst);
                }
            }
        }
        // Edge type 4 (forwarder only): Invoke/Apply result `dst` ← every owned
        // transfer-through-return arg. The result IS the forwarded arg's allocation.
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } = instr
                {
                    uf.record_forwarder(contracts, *dst, *callee, args);
                }
            }
            if let ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                ..
            } = &block.terminator
            {
                uf.record_forwarder(contracts, *dst, *callee, args);
            }
        }
        uf
    }

    /// True iff `rep` (a union-find representative) has a member defined by a
    /// sum-aggregate `Construct` — the construct-fed lineage gate. Resolves every
    /// recorded sum-Construct dst to its rep and checks for a match.
    fn is_sum_aggregate_construct_rep(&mut self, rep: ArcVarId) -> bool {
        let dsts: Vec<ArcVarId> = self.sum_aggregate_construct_dsts.iter().copied().collect();
        dsts.into_iter().any(|d| self.find(d) == rep)
    }

    /// Every member var of `rep`'s class, drawn from the recorded edge endpoints
    /// (`parent` keys + values). Used to suppress the spurious keep-alive incs +
    /// misplaced release on the construct-fed dead-param lineage.
    fn class_members(&mut self, rep: ArcVarId) -> FxHashSet<ArcVarId> {
        let endpoints: Vec<ArcVarId> = self
            .parent
            .keys()
            .chain(self.parent.values())
            .copied()
            .collect();
        let mut out: FxHashSet<ArcVarId> = FxHashSet::default();
        for v in endpoints {
            if self.find(v) == rep {
                out.insert(v);
            }
        }
        out
    }

    fn record_forwarder(
        &mut self,
        contracts: &FxHashMap<Name, MemoryContract>,
        dst: ArcVarId,
        callee: Name,
        args: &[ArcVarId],
    ) {
        for &arg in args {
            if arg_owned_transfers_through_return(contracts, callee, arg, args) {
                self.union(dst, arg);
                let rep = self.find(dst);
                self.is_forwarder_rep.insert(rep);
            }
        }
    }

    fn find(&mut self, v: ArcVarId) -> ArcVarId {
        let p = *self.parent.get(&v).unwrap_or(&v);
        if p == v {
            return v;
        }
        let r = self.find(p);
        self.parent.insert(v, r);
        r
    }

    fn union(&mut self, a: ArcVarId, b: ArcVarId) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

/// Operands consumed by the dual-consuming `ori_list_concat_cow` runtime helper:
/// the `RcPointer` operands of a `Let { PrimOp Binary(Add) }`. SSOT for the
/// list-concat consume set — consulted by `instr_transfer_vars` (last-use dec
/// suppression) AND precomputed function-wide in `emit_burden_ops_for_blocks`
/// (the `&mut` walk cannot re-borrow `func` for `var_repr`).
///
/// `RcPointer` is the list discriminator: `str`/closure operands are `FatValue`
/// and BORROWED by `ori_str_concat`'s `*const OriStr` contract (not consumed);
/// scalar `Add` operands carry no RC (filtered by `owned_vars_needing_rc`).
/// User `Add` impls dispatch via trait `Apply`/`Invoke`, never `PrimOp Binary`.
/// List + list → COW concat: both operands are consumed by the runtime.
pub(crate) fn list_concat_consumed_operands(
    instr: &ArcInstr,
    func: &ArcFunction,
) -> FxHashSet<ArcVarId> {
    let mut consumed = FxHashSet::default();
    if let ArcInstr::Let {
        value: ArcValue::PrimOp { op, args },
        ..
    } = instr
    {
        if matches!(op, PrimOp::Binary(ori_ir::BinaryOp::Add)) {
            for &arg in args {
                if matches!(func.var_repr(arg), Some(ValueRepr::RcPointer)) {
                    consumed.insert(arg);
                }
            }
        }
    }
    consumed
}
