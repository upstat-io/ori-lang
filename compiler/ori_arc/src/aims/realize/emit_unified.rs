//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Phase 1 sub-step B of [`super::realize_rc_reuse()`].

#[cfg(test)]
mod burden_lowering_tests;

use std::sync::LazyLock;

use ori_ir::Name;
use ori_types::{Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, RcAtomicity, RcStrategy,
};
use crate::lower::type_has_user_drop;

use super::metrics;

/// Per-phase RC-op snapshot for post-walk pass debugging.
///
/// Emits one `tracing::trace!` per block summarising every `RcInc`/`RcDec` by
/// `ArcVarId`. Gated behind `tracing::enabled!` — zero overhead when the
/// `ori_arc::aims::realize` target is below trace level.
///
/// `ORI_LOG=ori_arc::aims::realize=trace` activates it; bisects which post-walk
/// pass (burden-op elimination, burden edge-cleanup, `coalesce_block_rc`)
/// rewrote a block's RC ops.
fn trace_phase_snapshot(
    phase: &'static str,
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) {
    if !tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        return;
    }
    let fn_name = interner.lookup(func.name);
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let mut incs: Vec<u32> = Vec::new();
        let mut decs: Vec<u32> = Vec::new();
        let mut binc: Vec<u32> = Vec::new();
        let mut bdec: Vec<u32> = Vec::new();
        for instr in &block.body {
            match instr {
                ArcInstr::RcInc { var, .. } => incs.push(var.raw()),
                ArcInstr::RcDec { var, .. } => decs.push(var.raw()),
                ArcInstr::BurdenInc { var } => binc.push(var.raw()),
                ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => bdec.push(var.raw()),
                _ => {}
            }
        }
        if incs.is_empty() && decs.is_empty() && binc.is_empty() && bdec.is_empty() {
            continue;
        }
        tracing::trace!(
            target: "ori_arc::aims::realize",
            phase = phase,
            fn_name = fn_name,
            block = block_idx,
            inc = ?incs,
            dec = ?decs,
            binc = ?binc,
            bdec = ?bdec,
            "post-walk RC snapshot"
        );
    }
}

/// RC emission for a class-ledger-replaced function.
///
/// Every production shape is class-ledger-replaced (the Step-4b fail-loud
/// gate admits nothing else), so the burden ops in the instruction stream
/// ARE the verified plan: this lowers them mechanically to `RcInc`/`RcDec`
/// (`lower_burden_ops_to_rc`) and finalizes emission. A non-replaced
/// function reaching this point is an internal error (`unreachable!`).
pub(super) fn emit_rc_unified(
    func: &mut ArcFunction,
    _state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    _contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &TypeRegistry,
) -> (
    usize,
    Vec<DeathEvent>,
    Vec<AllocEvent>,
    metrics::SynergyMetrics,
) {
    assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    // The burden ops in the stream ARE the per-class-verified plan. Lower the
    // plan mechanically in Phase 7 and coalesce the resulting RC ops.
    if func.class_ledger_emission {
        lower_burden_ops_to_rc(func, pool, type_registry, &FxHashSet::default());
        trace_phase_snapshot("after_phase_7_burden_lowering", func, interner);
        finalize_rc_emission(func, interner);
        return (
            count_rc_ops(func),
            Vec::new(),
            Vec::new(),
            metrics::SynergyMetrics::default(),
        );
    }

    // The class-ledger plan is the SOLE RC emitter. On the normal
    // (burden-ops-enabled) path the Step-4b `assert!` already ICEs before a
    // non-replaced function reaches here. Under `ORI_DISABLE_BURDEN_OPS=1`
    // the Step-4b assert is vacuously satisfied (its condition is
    // `!burden_ops_enabled || replaced`), so every function declines
    // replacement without tripping it — THIS `unreachable!()` is itself the
    // fail-loud gate for that ablation path, not a redundant backstop.
    unreachable!(
        "realize reached a non-class-ledger function `{}` — the class-ledger \
         plan admits only replaced functions",
        interner.lookup(func.name)
    );
}

/// Shared RC-emission tail: Phase 3 coalescing peephole (merge adjacent RC
/// ops per block) followed by RL-2 scope-exit drop-order correction on
/// `Return` blocks ([`order_return_block_scope_exit_decs`]).
///
/// BOTH `emit_rc_unified` exit paths — the class-ledger replacement early
/// return and the default burden-path walk — emit real `RcInc`/`RcDec`
/// instructions subject to the SAME user-`@drop`-observable ordering hazard
/// on `Return` blocks (Spec: Annex E §AIMS RL-2 + RL-DROP); routing both
/// through one finalize step keeps them from drifting out of sync the way a
/// duplicated inline tail would.
fn finalize_rc_emission(func: &mut ArcFunction, interner: &ori_ir::StringInterner) {
    use crate::aims::emit_rc::coalesce_block_rc;

    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }
    trace_phase_snapshot("after_phase_3_coalesce", func, interner);

    order_return_block_scope_exit_decs(func);
}

/// The released var of a scope-exit release op (whole-var or field-grain),
/// `None` for every non-release instruction.
fn release_var(instr: &ArcInstr) -> Option<ArcVarId> {
    match instr {
        ArcInstr::RcDec { var, .. }
        | ArcInstr::BurdenDec { var }
        | ArcInstr::RcDecPartial { var, .. }
        | ArcInstr::BurdenDecPartial { var, .. }
        | ArcInstr::RcDecVariant { var }
        | ArcInstr::BurdenDecVariant { var } => Some(*var),
        ArcInstr::RcDecField { base, .. } | ArcInstr::BurdenDecField { base, .. } => Some(*base),
        _ => None,
    }
}

/// RL-2 scope-exit drop ordering on `Return` blocks: sort each Return block's
/// trailing release run into REVERSE DECLARATION ORDER (descending `ArcVarId`),
/// the value-semantics teardown order (a later-declared container drops before
/// the earlier locals its teardown may observe — the two-channel map teardown
/// fires before the caller's own key/value copies release). Releases within
/// one trailing run are a per-path permutation (RC-net neutral); only the
/// user-`@drop`-observable order changes. Spec: Annex E §AIMS RL-2 + RL-DROP.
fn order_return_block_scope_exit_decs(func: &mut ArcFunction) {
    for block_idx in 0..func.blocks.len() {
        if !matches!(
            func.blocks[block_idx].terminator,
            crate::ir::ArcTerminator::Return { .. }
        ) {
            continue;
        }
        let body_len = func.blocks[block_idx].body.len();
        // The maximal trailing run of release ops — whole-var AND field-grain
        // (a partial/field/variant dec walks field payloads whose drop glue
        // may fire transitively, so it is order-bearing and must not truncate
        // the run).
        let mut start = body_len;
        while start > 0 && release_var(&func.blocks[block_idx].body[start - 1]).is_some() {
            start -= 1;
        }
        if body_len - start < 2 {
            continue;
        }
        // One unit per release op; the sort is stable, so same-var release
        // sequences keep their relative order. `func.spans` is indexed
        // `[block_index][instr_index]` in lockstep with `body` (per every
        // other body-reordering pass in this crate — `block_merge::select`,
        // `aims::emit_reuse::dynamic`, `tail_call::rewrite`); split + resort
        // the span tail alongside the instruction tail so a reordered
        // release's provenance stays attached to the reordered instruction
        // instead of silently describing whichever instruction ends up at
        // its old position.
        let tail: Vec<ArcInstr> = func.blocks[block_idx].body.split_off(start);
        let span_tail: Vec<Option<ori_ir::Span>> = func
            .spans
            .get_mut(block_idx)
            .map(|spans| {
                let at = start.min(spans.len());
                spans.split_off(at)
            })
            .unwrap_or_default();
        let mut units: Vec<(usize, ArcInstr, Option<ori_ir::Span>)> = tail
            .into_iter()
            .enumerate()
            .map(|(i, instr)| {
                let Some(var) = release_var(&instr) else {
                    unreachable!("trailing run contains only release ops")
                };
                let span = span_tail.get(i).copied().flatten();
                (var.index(), instr, span)
            })
            .collect();
        units.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, instr, span) in units {
            func.blocks[block_idx].body.push(instr);
            if let Some(spans) = func.spans.get_mut(block_idx) {
                spans.push(span);
            }
        }
    }
}

/// Whether `recv`'s jump-threaded collection lineage is RETURNED from `func` —
/// the SAME discriminator the Phase-6.68b element-escape keep-alive gates on
/// ([`collection_receiver_returned`]). Consumed by the LLVM emitter's paired
/// elem-header store on push results: the keep-alive inc's balancing release is
/// the receiving collection's `elem_dec_fn`, which exists only when the result
/// buffer's header is populated; an in-scope (non-returned) receiver holds
/// UNFUNDED element views and must NOT dec them at free.
/// Spec: Annex E §AIMS RL-1 + RL-2.
pub fn push_receiver_lineage_returned(func: &ArcFunction, recv: ArcVarId) -> bool {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    collection_receiver_returned(rep_of(recv), func, &rep_of)
}

/// Whether collection-receiver lineage `recv_rep` (a jump-threaded rep of a
/// `@push`/`ori_list_push` arg-0 collection) is RETURNED from the function — its
/// lineage flows to a `Return` value.
///
/// This is the precise over-fire boundary for the element-escape keep-alive
/// ([`emit_iter_element_pushed_into_returned_collection_keepalive_inc`]): the
/// keep-alive is needed ONLY when the receiving collection survives the SOURCE
/// collection's in-function `ori_iter_drop`. A RETURNED collection's drop runs in
/// the CALLER — a SECOND `elem_dec_fn` over the borrowed element backing, distinct
/// from the in-callee `ori_iter_drop`, so the rc-1 backing is decced twice and
/// needs the keep-alive (the receiving collection's `elem_dec_fn` is the inc's
/// balancing release).
///
/// An IN-SCOPE receiver — iterated (`for w in r` lowers to `@iter(r [own])` ->
/// `ori_iter_drop`, an in-scope consume, NOT an escape), or dropped at scope exit —
/// is sequenced with the source iter-drop WITHIN one function, where the base
/// accounting already balances them; a keep-alive there orphans a +1 -> leak. The
/// in-scope `@iter [own]` consume is exactly why this gate is RETURN-reachability,
/// NOT a generic owned-position transfer-out test (which would mis-classify the
/// in-scope iterate as an escape).
///
/// `rep_of` is the jump-threaded rep map: the COW append (`%out = push(r [own],
/// ..)`) re-binds the loop-carried collection to its result `dst`, and the
/// loop-back `Jump` merges `%out` into the receiver's rep, so the finally-returned
/// value shares `recv_rep`. Spec: Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec`).
fn collection_receiver_returned(
    recv_rep: ArcVarId,
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> bool {
    func.blocks.iter().any(|block| {
        matches!(&block.terminator, ArcTerminator::Return { value } if rep_of(*value) == recv_rep)
    })
}

/// LOCAL jump-threaded same-allocation reps: `Let{Var}` aliases PLUS the
/// Jump-arg→block-param POSITIONAL SSA-rename edges `compute_same_alloc_reps`
/// excludes BY DESIGN. When `apply_direct_seed` is `Some`, the union-find is ALSO
/// seeded with that map's equivalences (the apply-Direct/Conditional transfer
/// edges from `compute_same_alloc_reps`), so a forwarder result (`%dst = Apply
/// @id(%arg [own])` where the callee param `transfers_through_return ∧
/// ReturnAliasShape::Direct`) merges with its transferred owned arg — the result
/// IS the same allocation. With `None` the result is byte-identical to the
/// Let+Jump-only threading.
///
/// Kept local: threading the phi globally would widen `same_alloc_reps`, changing
/// the fresh-inc-elision and alloc-aware-net verdicts that depend on the
/// unthreaded reps. The apply-Direct seed is wired ONLY for the dead-owned-
/// collection pass (the forwarder-RESULT leak), where the threaded rep attributes
/// the result's borrowed-read scope-exit sink to its allocation source.
pub(super) fn compute_jump_threaded_reps(
    func: &ArcFunction,
    apply_direct_seed: Option<&FxHashMap<ArcVarId, ArcVarId>>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    fn find(parent: &mut FxHashMap<ArcVarId, ArcVarId>, v: ArcVarId) -> ArcVarId {
        let p = *parent.get(&v).unwrap_or(&v);
        if p == v {
            return v;
        }
        let r = find(parent, p);
        parent.insert(v, r);
        r
    }
    fn union(parent: &mut FxHashMap<ArcVarId, ArcVarId>, a: ArcVarId, b: ArcVarId) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    // Apply-Direct/Conditional transfer-edge seed (forwarder result ↔ owned arg).
    // Reconstructs the `compute_same_alloc_reps` equivalences (member → rep) so a
    // `transfers_through_return ∧ Direct` forwarder result shares its arg's
    // allocation rep. `None` for every caller except the dead-owned-collection
    // pass — see the doc comment. Iterate in SORTED key order: `apply_direct_seed`
    // is an `FxHashMap`, and a HashMap-order union sequence yields a
    // nondeterministic union-find rep (the downstream net + sink would flip across
    // runs — codegen must be deterministic: same inputs, byte-identical output).
    // Sorting fixes the rep.
    if let Some(seed) = apply_direct_seed {
        let mut edges: Vec<(ArcVarId, ArcVarId)> = seed.iter().map(|(&m, &r)| (m, r)).collect();
        edges.sort_unstable();
        for (member, rep) in edges {
            union(&mut parent, member, rep);
        }
    }
    // Edge type 1: Let{Var} aliases.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Var(src),
                ..
            } = instr
            {
                union(&mut parent, *dst, *src);
            }
        }
    }
    // Edge type 2: Jump-arg → successor block-param POSITIONAL rename. This is
    // the edge `compute_same_alloc_reps` omits; threading it locally lets the
    // dead-block-param at the loop-exit trace back to its allocation source.
    for block in &func.blocks {
        let (target, args) = match &block.terminator {
            ArcTerminator::Jump { target, args } => (*target, args),
            _ => continue,
        };
        let Some(succ) = func.blocks.get(target.index()) else {
            continue;
        };
        for (pos, &arg) in args.iter().enumerate() {
            if let Some(&(param, _)) = succ.params.get(pos) {
                union(&mut parent, param, arg);
            }
        }
    }
    let keys: Vec<ArcVarId> = parent.keys().copied().collect();
    let mut reps = FxHashMap::default();
    for v in keys {
        let r = find(&mut parent, v);
        reps.insert(v, r);
    }
    reps
}

/// Phase 7 (probe): mechanically lower surviving whole-var burden ops to real
/// RC instructions.
///
/// `BurdenInc { var }` → `RcInc { var, count: 1, strategy, atomicity }` and
/// whole-var `BurdenDec { var }` → `RcDec { var, strategy, atomicity }`, with
/// the canonical `RcStrategy::from_repr` (same strategy the predicate-stack
/// emitter embeds) and `atomicity = Atomic` (RL-19/20/21 thread-local dispatch
/// pending).
///
/// Field-grain `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` are
/// rewritten to their REALIZED spellings (`RcDecPartial` / `RcDecField` /
/// `RcDecVariant`) — same codegen drop glue (`skip_fields`-aware partial drop,
/// `SetTag` pre-drop variant walk, `Set` old-value field drop), never a
/// whole-var `RcDec` (would double-drop). The re-spelling takes the lowered op
/// OUT of the Step-11 burden census: the VF-1 whole-var ledger counts SURVIVING
/// burden ops, and a mechanically-lowered op must leave the burden stream with
/// its pair partner (the whole-var acquire inc lowers to `RcInc` in the same
/// pass) — a half-pair surviving in burden spelling nets `-1` at every exit
/// through its path and aborts gated runs (Spec: Annex E §AIMS RL-comp
/// net-preservation). `ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1` restores the
/// legacy burden-spelled survival for bisection.
///
/// `Scalar` reprs cannot reach here from class-ledger emission: state-map
/// exclusion and class admission consult this same `var_reprs` source (Spec:
/// Annex E §AIMS RE-2 / DP-1 / L-9). A `Scalar` or out-of-range `var_repr`
/// leaves the burden op in place rather than synthesizing an unsound `RcDec`.
///
/// `elidable_fresh_incs` (per `compute_elidable_fresh_self_alloc_incs`): FRESH
/// self-allocation `BurdenInc` def-sites whose paired fresh inc is REDUNDANT
/// under lowering — the allocation already supplies the lineage's `+1`. The
/// FIRST `BurdenInc` encountered for such a var is REMOVED (an elided op is
/// gone from the op stream, keeping the VF-1 whole-var ledger net-0; a
/// surviving no-op marker would count `+1` at function exit and abort gated
/// runs). Subsequent `BurdenInc`s for the same var (genuine dup-alias
/// acquires) still lower — only the ONE redundant fresh-site inc per var is
/// elided. `ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL=1` restores the legacy
/// no-op-marker form (codegen no-ops it) for bisection.
///
/// Spec: Annex E §AIMS RL-comp (lowered `BurdenInc`/`BurdenDec` net-preservation).
fn lower_burden_ops_to_rc(
    func: &mut ArcFunction,
    pool: &Pool,
    type_registry: &TypeRegistry,
    elidable_fresh_incs: &FxHashSet<ArcVarId>,
) {
    let mut fresh_inc_elided: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut elided_sites: Vec<(usize, usize)> = Vec::new();
    let lower_field_grain = !*FIELD_GRAIN_DEC_LOWERING_DISABLED;
    for block_idx in 0..func.blocks.len() {
        let body_len = func.blocks[block_idx].body.len();
        for instr_idx in 0..body_len {
            if lower_field_grain && respell_field_grain_dec(func, block_idx, instr_idx) {
                continue;
            }
            let (ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var }) =
                func.blocks[block_idx].body[instr_idx]
            else {
                continue;
            };
            // Elide the ONE redundant fresh-site inc: the first `BurdenInc` for
            // an elidable FRESH self-alloc var (allocation already supplies the
            // `+1`). Later incs for the var are genuine dup-alias acquires and
            // lower normally.
            if matches!(
                func.blocks[block_idx].body[instr_idx],
                ArcInstr::BurdenInc { .. }
            ) && elidable_fresh_incs.contains(&var)
                && fresh_inc_elided.insert(var)
            {
                elided_sites.push((block_idx, instr_idx));
                continue;
            }
            // RE-2 backstop: class-ledger emission never emits whole-var burden
            // ops on a repr-less var. An absent repr here is a contract violation;
            // leave the burden op in place rather than emit unsound RC.
            let Some(repr) = func.var_repr(var) else {
                continue;
            };
            let ty = func.var_type(var);
            let has_user_drop = type_has_user_drop(ty, type_registry);
            // Why: a Scalar repr carries no RC header — skip it (no RC op) UNLESS
            // its type has a user `@drop`, which falls through to the `UserDrop`
            // strategy in the match that follows (the `@drop` call alone).
            // Spec: Annex E §AIMS RL-DROP.
            if matches!(repr, crate::ir::ValueRepr::Scalar) && !has_user_drop {
                continue;
            }
            // Why: scalar+`@drop` has no RC fields → `UserDrop` (the `@drop` call
            // alone, balance-neutral); heap-field+`@drop` → `AggregateFields` (run
            // `@drop` THEN walk RC fields). Spec: Annex E §AIMS RL-DROP.
            let strategy = if has_user_drop && matches!(repr, crate::ir::ValueRepr::Scalar) {
                RcStrategy::UserDrop
            } else if has_user_drop {
                RcStrategy::AggregateFields
            } else {
                RcStrategy::from_repr(repr, pool, ty)
            };
            let atomicity = RcAtomicity::default_atomic();
            let lowered = match func.blocks[block_idx].body[instr_idx] {
                ArcInstr::BurdenInc { var } => ArcInstr::RcInc {
                    var,
                    count: 1,
                    strategy,
                    atomicity,
                },
                ArcInstr::BurdenDec { var } => ArcInstr::RcDec {
                    var,
                    strategy,
                    atomicity,
                },
                _ => unreachable!("filtered to whole-var burden ops above"),
            };
            func.blocks[block_idx].body[instr_idx] = lowered;
        }
    }
    if !*ELIDED_FRESH_INC_REMOVAL_DISABLED {
        remove_elided_fresh_inc_sites(func, &elided_sites);
    }
}

/// `ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL=1` keeps each elided fresh-site
/// `BurdenInc` as a codegen-no-op marker instead of removing it. Bisection
/// surface: isolates a behavior change to the marker removal vs the elision
/// verdict. Default (unset): elided incs are removed.
static ELIDED_FRESH_INC_REMOVAL_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_ELIDED_FRESH_INC_REMOVAL").as_deref() == Ok("1"));

/// `ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING=1` keeps `BurdenDecPartial` /
/// `BurdenDecField` / `BurdenDecVariant` in burden spelling through Phase 7
/// (the legacy half-pair shape the Step-11 VF-1 ledger nets `-1`). Bisection
/// surface: isolates a gated-verification change to the field-grain
/// re-spelling vs the rest of the lowering. Default (unset): field-grain decs
/// lower to `RcDecPartial` / `RcDecField` / `RcDecVariant`.
static FIELD_GRAIN_DEC_LOWERING_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_FIELD_GRAIN_DEC_LOWERING").as_deref() == Ok("1"));

/// RE-2 backstop for the field-grain re-spelling: a field-grain dec on a
/// provably-Scalar (or repr-unpopulated) subject is an upstream admission
/// contract violation — leave it burden-spelled so the Step-11 census surfaces
/// it instead of emitting drop glue against a header-less value.
fn field_grain_repr_lowerable(func: &ArcFunction, var: ArcVarId) -> bool {
    match func.var_repr(var) {
        Some(crate::ir::ValueRepr::Scalar) | None => false,
        Some(_) => true,
    }
}

/// Phase-7 field-grain dec re-spelling at one instruction slot:
/// `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` become their
/// realized `RcDecPartial` / `RcDecField` / `RcDecVariant` forms (same
/// drop-glue codegen arm; out of the Step-11 burden census). Returns `true`
/// when the slot held a field-grain dec (re-spelled, or left in place by the
/// [`field_grain_repr_lowerable`] RE-2 backstop so the census surfaces it).
fn respell_field_grain_dec(func: &mut ArcFunction, block_idx: usize, instr_idx: usize) -> bool {
    let realized = match &func.blocks[block_idx].body[instr_idx] {
        ArcInstr::BurdenDecPartial { var, skip_fields }
            if field_grain_repr_lowerable(func, *var) =>
        {
            ArcInstr::RcDecPartial {
                var: *var,
                skip_fields: skip_fields.clone(),
            }
        }
        ArcInstr::BurdenDecField { base, field } if field_grain_repr_lowerable(func, *base) => {
            ArcInstr::RcDecField {
                base: *base,
                field: *field,
            }
        }
        ArcInstr::BurdenDecVariant { var } if field_grain_repr_lowerable(func, *var) => {
            ArcInstr::RcDecVariant { var: *var }
        }
        ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. } => return true,
        _ => return false,
    };
    func.blocks[block_idx].body[instr_idx] = realized;
    true
}

/// Remove the elided fresh-site `BurdenInc` instructions at `sites`
/// (`(block_idx, instr_idx)` pairs recorded by [`lower_burden_ops_to_rc`]).
/// An elided op is GONE from the op stream — the VF-1 whole-var ledger
/// (`verify_burden_balance`) counts surviving burden ops, so a retained no-op
/// marker would net `+1` at every function exit through its definition.
fn remove_elided_fresh_inc_sites(func: &mut ArcFunction, sites: &[(usize, usize)]) {
    if sites.is_empty() {
        return;
    }
    let mut by_block: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    for &(b, i) in sites {
        by_block.entry(b).or_default().insert(i);
    }
    for (block_idx, remove) in by_block {
        let body = &mut func.blocks[block_idx].body;
        let mut idx = 0usize;
        body.retain(|_| {
            let keep = !remove.contains(&idx);
            idx += 1;
            keep
        });
    }
}

/// Count RC operations (`RcInc` + `RcDec`) in a function.
fn count_rc_ops(func: &ArcFunction) -> usize {
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}
