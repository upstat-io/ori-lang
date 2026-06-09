//! Ownership / transfer scans feeding `emit_burden_ops`: owned-burden
//! collection, transfer-point and last-use detection, move-alias chains,
//! use counts, and the live-out / gen-kill dataflow inputs.

mod live_out;
mod move_alias;

pub(super) use live_out::compute_live_out_owned;
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
pub(super) fn compute_use_counts_and_dup_aliases(
    func: &ArcFunction,
    inc_suppressed_vars: &mut FxHashSet<ArcVarId>,
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
                if use_counts.get(src).copied().unwrap_or(0) >= 2 {
                    dup_alias_dsts.insert(*dst);
                }
            }
        }
    }
    (use_counts, dup_alias_dsts)
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
        for (param_var, rep) in dead_params {
            if alt_consumer_reps.contains(&rep) {
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
            }
        }
    }
    ConstructFedDeadParamLineage {
        releases,
        suppressed_lineage_vars,
    }
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
