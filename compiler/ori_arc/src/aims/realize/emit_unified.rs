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
use crate::aims::lattice::AccessClass;
use crate::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, PrimOp, RcAtomicity, RcStrategy,
    ValueRepr,
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

/// Unified RC emission: per-block walk with inline death/alloc event collection.
///
/// Forward walk routing every decision through `decide()`, collecting reuse
/// events inline (no separate death/alloc scans).
///
/// # Phases
///
/// 1. Per-block: dead-at-entry → unified body walk → terminator RC → deferred
/// 2. Dead Invoke cleanup (orphaned Invoke result variables)
/// 3. Inter-block edge cleanup (with deferred parent decs)
/// 4. RC coalescing peephole per block
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
    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    // Class-ledger replacement (`ArcFunction::class_ledger_emission`): the
    // burden ops in the stream ARE the per-class-verified plan. Phase-6
    // elimination and the Phase-6.5..6.99 repair passes optimize over the
    // LEGACY Phase-5 baseline (a per-var DP-3 verdict would strip a planned
    // funding inc; the repairs compensate legacy-specific shapes), so the
    // plan lowers mechanically (Phase 7, no fresh-site inc elision — the
    // plan emits none) and coalesces.
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

    // The class-ledger plan is the SOLE RC emitter: a non-replaced function
    // cannot reach realization (the Step-4b fail-loud gate ICEs first).
    unreachable!(
        "realize reached a non-class-ledger function `{}` — the Step-4b \
         fail-loud gate admits only replaced functions",
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

/// True iff `instr` is an `Apply` using the rep at a BORROWED arg position (the
/// survivor read) — owned positions, Projects, and RC bookkeeping are NOT reads.
pub(super) fn instr_borrow_reads_rep(
    instr: &ArcInstr,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
) -> bool {
    let ArcInstr::Apply { arg_ownership, .. } = instr else {
        return false;
    };
    instr.used_vars().iter().enumerate().any(|(pos, &v)| {
        rep_of(v) == rep
            && !instr.is_owned_position(pos)
            && arg_ownership.get(pos) == Some(&crate::ir::ArgOwnership::Borrowed)
    })
}

/// Terminator analogue of [`instr_borrow_reads_rep`]: an `Invoke` terminator
/// borrow-reading the rep (`Invoke @len(coll [borrow])`). A `Jump`/`Branch`/`Return`
/// is a forward/transfer, never a survivor read.
pub(super) fn terminator_borrow_reads_rep(
    term: &ArcTerminator,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
) -> bool {
    let (ArcTerminator::Invoke {
        args,
        arg_ownership,
        ..
    }
    | ArcTerminator::InvokeIndirect {
        args,
        arg_ownership,
        ..
    }) = term
    else {
        return false;
    };
    args.iter().enumerate().any(|(pos, &v)| {
        rep_of(v) == rep && arg_ownership.get(pos) == Some(&crate::ir::ArgOwnership::Borrowed)
    })
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

/// Lineage reps of the COLLECTION-CONVERSION SOURCE: a value passed at a
/// BORROWED arg position (arg 0, the receiver) to a collection-from-collection
/// conversion builtin (`@keys` / `@values` / `@split` / `@to_list`). The
/// conversion borrows its source, returns a fresh owned collection, and the
/// source survives — then dies at the post-loop block where this pass frees it.
///
/// Restricting the leaked-source set to exactly this shape (vs any owned
/// collection source) is the precise scope: it excludes loop-carried
/// invariants (`root = [1]`) and COW-reassigned lists (`xs = xs.push(i)`) whose
/// release the loop edge cleanup already owns. The receiver must be a borrowed
/// `RcPtr`/`FatVal` collection (`is_owned_collection_dst` confirms the tag).
/// The collection-CONVERSION builtin name set (`keys` / `values` / `split` /
/// `to_list`): a borrowed-receiver builtin producing a FRESH owned collection
/// RESULT from its receiver's contents. SSOT shared by the conversion-source pass
/// (which frees the borrowed SOURCE) and the dead-owned-collection candidate set
/// (which frees the conversion RESULT — `let vals = m.values(); vals.len()` leaks
/// the result list at scope exit). `iter` is NOT a conversion (its result is an
/// iterator handle freed by `ori_iter_drop`, not a collection).
fn collection_conversion_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ["keys", "values", "split", "to_list"]
        .iter()
        .map(|n| interner.intern(n))
        .collect()
}

/// Iterator-CONSUMER builtins whose result is a FRESH owned collection the runtime
/// allocates: `.iter().map(..).collect()` lowers to `@collect` (`ori_iter_collect`,
/// a `ori_rc_alloc`'d `[T]` distinct from the iterator/source — never aliasing) and
/// `.collect()` into a `Set<T>` lowers to `@collect_set` (`ori_iter_collect_set`).
/// The consumed iterator (and its source buffer) is freed by the iterator's own
/// `ori_iter_drop` machinery; the collect RESULT is a distinct fresh allocation
/// borrowed-read then dead at scope exit (`let doubled = xs.iter().map(f).collect();
/// doubled.length()` leaks the result list under the flag — the burden walk emits
/// ZERO ops on it). Freed by the dead-owned-collection pass (RL-2 scope-exit dec,
/// lowering to `RcDec [HeapPtr]` which walks `elem_dec_fn` for heap elements). The
/// result NEVER aliases the source (`ori_iter_collect` copies + `elem_inc_fn`s each
/// element into a fresh buffer), so a scope-exit dec cannot free a value the source
/// holds. Spec: Annex E §AIMS RL-2.
fn iterator_consumer_collection_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ["collect", "collect_set"]
        .iter()
        .map(|n| interner.intern(n))
        .collect()
}

/// Set-ALGEBRA builtins whose result is a FRESH owned `{T}` Set the runtime
/// allocates from the two operands' contents: `a.union(b)` / `a.difference(b)` /
/// `a.intersection(b)` (`Set` registry methods returning `SELF`). The runtime
/// allocates a distinct result Set — neither operand aliases it. A
/// `let s = a.union(b); s.len()` borrowed-reads the result then drops it dead at
/// scope exit; the burden walk emits ZERO ops on it, leaking the result buffer.
/// Freed by the dead-owned-collection pass (RL-2 scope-exit dec). Shape-identical
/// to the collection-conversion producers (`@keys`/`@values`): a fresh owned
/// collection result whose lineage is borrowed-read then dead. Spec: Annex E
/// §AIMS RL-2.
fn collection_set_algebra_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ["union", "difference", "intersection"]
        .iter()
        .map(|n| interner.intern(n))
        .collect()
}

/// Builtin / derived methods that SYNTHESISE a FRESH owned `str` from the receiver
/// (a pure allocation, distinct from the receiver's buffer): `debug` (the derived
/// `#[derive(Debug)]` / builtin `@debug()` quoting path) and `to_str` (the
/// `Printable` conversion). The result fat-pointer buffer is a fresh allocation the
/// callee built; a multi-read-then-dead result (`let s = p.debug(); s.contains(..)
/// && s.contains(..)`) carries a dup-use keep-alive `BurdenInc` netting the explicit
/// ops to 0, leaving the result's alloc `+1` unreleased -> leak. The str analogue of
/// [`collection_conversion_names`]: the alloc-aware net frees the result at its
/// borrowed-read scope-exit sink (RL-2).
///
/// EXCLUDES sharing-view str methods (`slice` / `substring`, in
/// [`sharing_view_relocation_names`]) whose result SHARES the receiver's buffer (a
/// freeing dec would double-free the shared backing), and `str + str` concat (a
/// `PrimOp Binary(Add)`, NOT an `Apply` to a named method — structurally outside
/// this recognizer). Spec: Annex E §AIMS RL-2.
fn fresh_str_producing_method_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ["debug", "to_str"]
        .iter()
        .map(|n| interner.intern(n))
        .collect()
}

/// Phi-aware allocation-equivalence reps: the `same_alloc_reps` classes
/// (`Let{Var}` + apply-result Direct/Conditional edges) UNIONED with the
/// FORWARD Jump-arg → successor block-param POSITIONAL rename edges that
/// `compute_same_alloc_reps` excludes BY DESIGN.
///
/// Consumed ONLY by [`compute_lineage_alloc_aware_net`], and there ONLY for an
/// `ori_list_take` for_yield-result lineage, to ATTRIBUTE a jump-threaded
/// downstream `BurdenDec` to its allocation rep — never to widen
/// `same_alloc_reps` itself (which still drives the fresh-inc-elision CANDIDATE
/// set + every other alloc-aware verdict on the unthreaded reps, per the
/// local-only discipline of [`compute_jump_threaded_reps`]).
///
/// FORWARD-ONLY: a loop BACK-EDGE (`target` dominates the jumping block) is
/// SKIPPED so a self-looping consumer block-param does not fold its own
/// back-edge into the alloc class. The `for_yield` result is allocated in a
/// POST-loop block and threads FORWARD into its consumers — an acyclic chain
/// whose lone downstream release IS the phi-target's dec.
fn compute_phi_threaded_alloc_reps(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
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
    let dom = crate::graph::DominatorTree::build(func);
    let mut parent: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    // Seed with the committed `same_alloc_reps` equivalence (member → rep).
    for (&member, &rep) in same_alloc_reps {
        union(&mut parent, member, rep);
    }
    // Add the FORWARD Jump-arg → successor block-param POSITIONAL rename edge —
    // the one `compute_same_alloc_reps` omits. Loop back-edges (`target`
    // dominates the jumping block) are skipped.
    for (b, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Jump { target, args } = &block.terminator else {
            continue;
        };
        let block_id = crate::ir::ArcBlockId::new(u32::try_from(b).unwrap_or(u32::MAX));
        if dom.dominates(*target, block_id) {
            continue;
        }
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

/// The interned `ori_list_take` finalizer name — the `for x in coll yield expr`
/// comprehension's result builder. The loop lowering (`lower/control_flow/
/// for_yield.rs`, `for_yield_option.rs`) allocates a growable scratch via
/// `ori_list_new`, pushes each body result, then `ori_list_take`s the scratch:
/// the runtime MOVES the data buffer out of the scratch `OriList` (freeing only
/// the struct, not the buffer — `ori_rt`'s `ori_list_take`), yielding a FRESH
/// owned `[T]` at RC = 1. For alloc-aware-net accounting this result IS a fresh
/// self-allocation (the buffer's own `+1`), so [`fresh_self_alloc_dst`] treats it
/// like a `Construct`.
pub(crate) fn for_yield_result_finalizer_name(interner: &ori_ir::StringInterner) -> ori_ir::Name {
    interner.intern("ori_list_take")
}

/// The `dst` of a FRESH self-allocation instruction (created at runtime RC = 1),
/// or `None`. Mirrors the FRESH-site set `fresh_site_burden_inc_dst`
/// (`burden_lower/emit.rs`) treats as self-allocating: `Construct` / `Reuse` /
/// `CollectionReuse` / `PartialApply` / `Let { String }`, PLUS the `for_yield`
/// `ori_list_take` finalizer (the comprehension result builder — moves a fresh
/// buffer out of the scratch, RC = 1 like a `Construct`). General `Apply` /
/// `Invoke` results are NOT self-allocs — they inherit an owned reference from
/// the callee (the callee's allocation, not one created here), so their fresh
/// inc is the caller's genuine acquire, never the redundant alloc double-count;
/// `ori_list_take` is the exception because the buffer it returns was just
/// allocated by the `for_yield` scratch in THIS function.
#[expect(
    clippy::match_same_arms,
    reason = "the heap-ctor arm, the Let{String} arm, and the list_take arm are \
              distinct instruction shapes that share a `Some(*dst)` body; merging \
              would obscure the FRESH-site categories the burden walk distinguishes"
)]
fn fresh_self_alloc_dst(instr: &ArcInstr, list_take_name: ori_ir::Name) -> Option<ArcVarId> {
    match instr {
        ArcInstr::Construct { dst, .. }
        | ArcInstr::Reuse { dst, .. }
        | ArcInstr::CollectionReuse { dst, .. }
        | ArcInstr::PartialApply { dst, .. } => Some(*dst),
        ArcInstr::Let {
            dst,
            value: ArcValue::Literal(crate::ir::LitValue::String(_)),
            ..
        } => Some(*dst),
        ArcInstr::Apply { dst, func, .. } if *func == list_take_name => Some(*dst),
        _ => None,
    }
}

/// The `dst` of a SELF-ALLOCATING builtin collection-source `Apply`/`Invoke`
/// result (created at runtime RC = 1 from a fresh `ori_rc_alloc`'d buffer), or
/// `None`. These are the `@collect` / `@collect_set` iterator-consumer results,
/// the `@union` / `@difference` / `@intersection` set-algebra results, the
/// `@keys` / `@values` / `@split` / `@to_list` conversion results, the COW-method
/// results, and the fresh-str-producing method results — the same name sets the
/// `allocates` predicate in [`compute_owned_collection_delta`] recognizes as a
/// fresh-buffer-allocating callee.
///
/// Distinct from [`fresh_self_alloc_dst`] (`Construct` / literal / `ori_list_take`
/// only): Phase-5 `fresh_site_burden_inc_dst` emits a fresh-site `BurdenInc` on
/// EVERY `Apply`/`Invoke` result with a Unique/MaybeShared return contract,
/// treating the result as a caller-acquires-owned-reference. For a SELF-allocating
/// builtin (rc = 1 fresh buffer, distinct from any operand) that inc is the M1
/// over-count under Phase-7 sole-emitter lowering (`alloc(+1) + RcInc − RcDec =
/// +1` -> leak). The M1 alloc-aware-net elision drops it once the result is
/// recognized as a fresh self-alloc here. Restricted to BUILTIN callees: a
/// user-function collection result needs the `transfers_through_return ∧ Direct`
/// forwarder discriminator (a forwarder returns an EXISTING allocation, NOT a
/// fresh one — eliding its inc over-frees the shared buffer), which the
/// `owned_consumed` / forwarder machinery owns; keeping this builtin-only is the
/// conservative scope. Spec: Annex E §AIMS RL-1 + RL-2.
///
/// LIMITATION: recognizes `Apply` INSTRUCTION results only. An `Invoke`
/// TERMINATOR result (whose fresh-site `BurdenInc` is prepended to the
/// normal-successor block) is NOT recognized, so the M1 elision does not
/// fire for self-allocating builtins called via `Invoke`; terminator-result
/// recognition lands with the Phase-5 broad-shape emission completion.
fn fresh_collection_source_apply_dst(
    instr: &ArcInstr,
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> Option<ArcVarId> {
    let (dst, callee) = match instr {
        ArcInstr::Apply {
            dst, func: callee, ..
        } => (*dst, *callee),
        _ => return None,
    };
    if !matches!(
        func.var_repr(dst),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) {
        return None;
    }
    is_self_allocating_builtin_callee(callee, interner).then_some(dst)
}

/// SSOT callee predicate: `true` iff `callee` is a builtin that SELF-ALLOCATES a
/// fresh RC = 1 collection/string buffer (distinct from any operand). The same
/// name sets the `allocates` predicate in [`compute_owned_collection_delta`]
/// recognizes as a fresh-buffer-allocating callee. Consumed by BOTH the
/// `Apply`-instruction recognizer ([`fresh_collection_source_apply_dst`]) and the
/// `Invoke`-terminator recognizer ([`fresh_rc_alloc_dst_terminator`]) — one
/// callee-classification home for both call forms.
///
/// `.collect()` into a `Set` lowers to the `__collect_set` PROTOCOL builtin
/// (`@__collect_set`, prefixed), distinct from the `@collect` / `@collect_set`
/// method names in `iterator_consumer_collection_names`. The protocol builtin
/// self-allocates a fresh Set buffer (rc=1) and has no contract, so Phase-5
/// emits the conservative fresh-site inc — the M1 over-count under sole-emitter
/// lowering. Spec: Annex E §AIMS RL-1.
fn is_self_allocating_builtin_callee(callee: Name, interner: &ori_ir::StringInterner) -> bool {
    let collect_set_protocol =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet.name());
    callee == collect_set_protocol
        || crate::borrow::all_cow_method_names(interner).contains(&callee)
        || collection_conversion_names(interner).contains(&callee)
        || iterator_consumer_collection_names(interner).contains(&callee)
        || collection_set_algebra_names(interner).contains(&callee)
        || fresh_str_producing_method_names(interner).contains(&callee)
}

/// The `(dst, normal-successor block)` of a SELF-ALLOCATING builtin collection-
/// source `Invoke` TERMINATOR result, or `None`. The `Invoke`-terminator
/// counterpart to [`fresh_collection_source_apply_dst`]'s `Apply`-instruction
/// recognition: a value-mutation COW builtin (`insert` / `push` / `remove` / …)
/// or set-algebra / conversion builtin called via the may-unwind `Invoke`
/// terminator self-allocates a fresh rc=1 result buffer just as its `Apply`
/// sibling does. The result `dst` first LIVES at the `normal` successor block,
/// where Phase-5 prepends its fresh-site `BurdenInc`; the returned block is the
/// alloc-attribution site so the M1 alloc-aware net counts the `+1` on the path
/// the result is defined.
///
/// `InvokeIndirect` (closure call, no callee `Name`) is NOT recognized — an
/// unknown closure callee has no self-allocating-builtin identity (conservative;
/// matches `fresh_collection_source_apply_dst` recognizing only direct `Apply`).
/// Spec: Annex E §AIMS RL-1.
pub(crate) fn fresh_rc_alloc_dst_terminator(
    term: &ArcTerminator,
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
) -> Option<(ArcVarId, crate::ir::ArcBlockId)> {
    let (dst, callee, normal) = match term {
        ArcTerminator::Invoke {
            dst,
            func: callee,
            normal,
            ..
        } => (*dst, *callee, *normal),
        _ => return None,
    };
    if !matches!(
        func.var_repr(dst),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) {
        return None;
    }
    is_self_allocating_builtin_callee(callee, interner).then_some((dst, normal))
}

/// The `dst` of ANY fresh RcPtr/FatValue self-allocation defined by an
/// `ArcInstr` (block body), or `None`: the union of [`fresh_self_alloc_dst`]
/// (`Construct`/literal/`ori_list_take`, repr-gated to RcPtr/FatValue) and
/// [`fresh_collection_source_apply_dst`] (self-allocating builtin collection-
/// source `Apply` result). The instruction-form recognizer the M1 alloc-aware-net
/// and fresh-inc elision both query for "is this a fresh rc=1 buffer whose
/// Phase-5 fresh-site inc the alloc supplies."
///
/// `Invoke`-TERMINATOR fresh self-alloc results are recognized by the sibling
/// [`fresh_rc_alloc_dst_terminator`]; every fresh-alloc enumeration loop scans
/// BOTH (block body via this fn + terminator via the sibling) so an
/// `Invoke`-form self-allocating builtin (`s.insert(..)` etc.) is not
/// mis-treated as a non-fresh acquire (its surplus fresh-site inc would
/// otherwise survive → leak). Spec: Annex E §AIMS RL-1.
pub(super) fn fresh_rc_alloc_dst(
    instr: &ArcInstr,
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    list_take_name: ori_ir::Name,
) -> Option<ArcVarId> {
    fresh_self_alloc_dst(instr, list_take_name)
        .filter(|&d| {
            matches!(
                func.var_repr(d),
                Some(ValueRepr::RcPointer | ValueRepr::FatValue)
            )
        })
        .or_else(|| fresh_collection_source_apply_dst(instr, func, interner))
}

/// USER-callee call results certified fresh by contract — every dst of a body
/// `Apply` / terminator `Invoke` whose callee's `ReturnContract` proves
/// `returns_fresh_self_alloc ∧ uniqueness == Unique` (every return path a
/// fresh rc=1 self-alloc; a param-returning / view-returning callee never
/// qualifies) on an RC-carrying repr. The fresh-alloc classification the M1
/// alloc-aware-net + fresh-inc elision extend to user callees
/// (`@build_bundle (seed) = Bundle { .. }` — the callee-returns-unique
/// multi-read shape whose surplus fresh-site inc otherwise survives → leak).
/// EXCLUDES dsts the instruction-form classifiers already recognize
/// ([`fresh_rc_alloc_dst`] / [`fresh_rc_alloc_dst_terminator`]) so a delta
/// tally consuming both never double-counts an alloc. Empty under
/// `ORI_DISABLE_CERTIFIED_FRESH_USER_RESULT_INC_ELISION=1`.
/// Spec: Annex E §AIMS RL-1.
/// Contract-level fresh certification for a named callee: every return path a
/// fresh rc=1 self-alloc (`returns_fresh_self_alloc`) at `Unique` uniqueness —
/// the discriminator shared by the fresh-inc elision root set and the Phase-6
/// pair-atomic Pass 3c admission. Spec: Annex E §AIMS RL-1 + §1.9.
pub(super) fn callee_certified_fresh(
    contracts: &FxHashMap<Name, MemoryContract>,
    callee: Name,
) -> bool {
    contracts.get(&callee).is_some_and(|c| {
        c.return_info.returns_fresh_self_alloc
            && c.return_info.uniqueness == crate::aims::lattice::Uniqueness::Unique
    })
}

/// Per-position may-COW verdict for a named-callee call arg — the SSOT
/// predicate shared by [`compute_cow_mutated_lineage_reps`] and
/// [`collect_rc_reading_consume_sites`]. `true` iff the callee MAY COW/share
/// the arg at `pos` (the caller's fresh inc is load-bearing per RL-1
/// `!incElidable`). NOT may-COW:
/// - `ori_list_take` (the `for_yield` finalizer — moves the buffer out, never
///   reads its rc);
/// - `__`-prefixed protocol builtins (compiler-interceptable, known read-only
///   ownership per Spec: Annex E §AIMS Protocol Builtins);
/// - KNOWN builtins (compiler-known ownership — COW-mutators take the
///   receiver `[own]`, caught by the owned-position check);
/// - a user callee whose contract proves the param a pure borrow-read
///   (`access == Borrowed && borrowed_read_only` — the param's lineage never
///   reaches an owned/COW/iter-consume position on any path, transitively
///   forward-safe per the IC-3 AND-join), so no COW point inside the callee
///   reads the caller's runtime refcount.
///
/// Everything else — unknown contract, missing param entry, unresolvable
/// `Name` — stays conservatively may-COW (when in doubt, fund). Spec: Annex E
/// §AIMS RL-1 + RL-2.
fn callee_may_cow_arg(
    contracts: &FxHashMap<Name, MemoryContract>,
    builtins: &crate::borrow::BuiltinOwnershipSets,
    interner: &ori_ir::StringInterner,
    list_take_name: Name,
    callee: Name,
    pos: usize,
) -> bool {
    let is_protocol_builtin = interner
        .try_lookup(callee)
        .is_some_and(|n| n.starts_with("__"));
    if callee == list_take_name || is_protocol_builtin || builtins.is_builtin(callee) {
        return false;
    }
    let proven_borrow_read_only = contracts
        .get(&callee)
        .and_then(|c| c.params.get(pos))
        .is_some_and(|p| p.access == AccessClass::Borrowed && p.borrowed_read_only);
    !proven_borrow_read_only
}

/// Same-alloc lineage reps whose value is consumed at a COW-mutation operand —
/// a runtime site that reads the operand's refcount to choose copy-vs-mutate.
/// Three site kinds:
/// - an owned `Apply` / `Invoke` / `CollectionReuse` arg position whose argument
///   is an `RcPtr` collection (the value-mutation builtins `push` / `set` /
///   `insert` / `remove` / `sort` / `reverse` consume the receiver at an owned
///   position and COW it);
/// - a `Let { PrimOp { Binary, args } }` with an `RcPtr` operand (collection
///   `+`/concat — `ori_list_concat_cow` / `ori_map_*_cow` read `ori_rc_is_unique`
///   on the operand to choose buffer-takeover vs copy);
/// - an `RcPtr` arg passed to a may-COW NON-protocol-builtin `Apply` / `Invoke`
///   position (a user function / stdlib method `Invoke @<name>` whose `<name>`
///   is not `__`-prefixed), per [`callee_may_cow_arg`]. Such a callee may
///   COW-mutate a collection param INTERPROCEDURALLY (e.g. `@check` doing
///   `list.push(...)` on a borrowed param), reading the CALLER's runtime
///   refcount through the call boundary; eliding the caller's fresh inc drops
///   the value to RC = 1 at the callee's COW point → mutate-in-place corrupts
///   the caller's still-live holder (the `arc_borrowed_param_cow_push_use_after`
///   shape). The `access` dimension alone cannot prove non-COW (a COW-mutated
///   receiver param stays `Borrowed` at the ABI boundary yet the callee still
///   reads rc); the affirmative `ParamContract.borrowed_read_only` fact DOES —
///   the param's lineage never reaches an owned/COW/iter-consume position on
///   any path (IC-3 AND-join, transitively forward-safe), so such a position is
///   NOT may-COW. Unknown contracts stay conservatively may-COW (when in doubt,
///   fund). Protocol builtins (`__`-prefixed `Apply` — `__index` etc.) are
///   compiler-interceptable with known read-only ownership (Spec: Annex E §AIMS
///   Protocol Builtins) and stay elidable.
///
/// SSOT COW-awareness signal: the fresh keep-alive inc on a rep in this set is
/// load-bearing (RL-1 `!incElidable` on a duplicating/COW use — it raises the
/// runtime refcount to ≥ 2 so the COW protocol COPIES instead of mutating the
/// shared buffer in place). Consumed by [`compute_elidable_fresh_self_alloc_incs`]
/// and by the burden-elim lineage re-balance candidate gate
/// (`burden_elim::mark_lineage_rebalance_removals`); both defer such a rep to the
/// COW-aware per-var elision path. Spec: Annex E §AIMS RL-1.
pub(super) fn compute_cow_mutated_lineage_reps(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    // Per-position may-COW verdict (the [`callee_may_cow_arg`] SSOT): a callee
    // that internally COW-mutates a collection param reads the caller's runtime
    // refcount across the call boundary, so the caller's fresh inc is
    // load-bearing. A user-callee position is COW-risk UNLESS its contract
    // proves the param a pure borrow-read (`borrowed_read_only` — never an
    // owned/COW/iter-consume position on any path); unknown contracts stay
    // conservatively may-COW. `ori_list_take` (the for_yield finalizer) is
    // exempt: it MOVES the scratch buffer out (consume-and-finalize), never
    // reads the buffer's rc to COW it (`ori_rt`'s `ori_list_take`).
    let list_take_name = for_yield_result_finalizer_name(interner);
    // A KNOWN builtin method has compiler-known ownership: its COW-mutators
    // (`push`/`set`/`insert`/`remove`/`sort`/`reverse`) take the receiver at an
    // OWNED position (already caught by the `consumed_owned` owned-position check),
    // while its read-only methods (`contains`/`len`/`get`/`first`/`last`) take it
    // BORROWED and never COW it. Only NON-builtin (user-function) callees are the
    // interprocedural-COW-through-borrowed-param risk (`@check` doing
    // `list.push(..)` on a borrowed param). Flagging a borrowed-position
    // read-only builtin arg as may-COW spuriously keeps a fresh inc and leaks
    // (`.collect()` borrow-read by `@contains`/`@len` then dead). Spec: Annex E
    // §AIMS RL-1.
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let callee_may_cow = |callee: Name, pos: usize| -> bool {
        callee_may_cow_arg(contracts, &builtins, interner, list_take_name, callee, pos)
    };
    // Two consume classes, distinguished by RC certainty:
    // - `consumed_owned`: a GENUINE in-loop CONSUME — an owned-position
    //   value-mutation arg (`push`/`set`/`insert`/`remove`/`sort`/`reverse`) or
    //   an `Add`-concat operand. The callee MOVES/COW-reads the value's buffer;
    //   this IS the source's release across a Jump-arg → block-param rename. These
    //   are phi-propagated so a loop-carried fresh source consumed at a back-edge
    //   alias (`rc_matrix` `xs = xs.push(i)`: the push consumes the loop-body
    //   rename `%19`, not the bb0 construct `%3`) flags the SOURCE's same-alloc
    //   rep — RL-1 duplication-balanced (the fresh inc is kept; bb0 lowers to a
    //   harmless `[RcInc, RcDec]` pair, never a stranded `[dec]`).
    // - `consumed_maycow`: the CONSERVATIVE may-COW-user-call over-approximation —
    //   an RcPtr arg (any position) to a non-builtin call whose callee MIGHT COW a
    //   collection param internally. This stays REP-LOCAL (no phi-propagation): a
    //   closure passed `[borrow]` to a user fn (`apply(scale [borrow])`) is NOT an
    //   owned consume of the loop-carried value, so phi-propagating it would
    //   spuriously keep a loop-INVARIANT borrow's fresh inc and leak it.
    let mut consumed_owned: Vec<ArcVarId> = Vec::new();
    let mut consumed_maycow: Vec<ArcVarId> = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            // Owned-position consume of an RcPtr collection (value-mutation
            // builtin). `is_owned_position` covers Construct/PartialApply/
            // CollectionReuse/Apply/ApplyIndirect; the value-mutation builtins
            // (`push`/`set`/...) lower to `Apply`/`Invoke @<method>` with the
            // receiver at an owned arg position.
            let used = instr.used_vars();
            for (pos, &arg) in used.iter().enumerate() {
                if instr.is_owned_position(pos) && is_rcptr(arg) {
                    consumed_owned.push(arg);
                }
            }
            // Interprocedural COW: an RcPtr arg to a may-COW user `Apply`.
            // `Apply.args` are positional user args (no leading closure), so the
            // used-var index IS the param index.
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                for (pos, &arg) in args.iter().enumerate() {
                    if is_rcptr(arg) && callee_may_cow(*callee, pos) {
                        consumed_maycow.push(arg);
                    }
                }
            }
            // Collection `+`/concat: a `Binary(Add)` PrimOp with an RcPtr operand.
            // ONLY `Add` COW-consumes/shares an RcPtr operand: `ori_str_concat`
            // (`emit_runtime_binary_op` Add arm, consume=true) + `ori_list_concat_cow`
            // both read/consume the operand buffer's runtime refcount, so the fresh
            // inc is load-bearing (RL-1 duplicating use). Comparison (`Eq`/`NotEq`/
            // `Lt`/`Le`/`Gt`/`Ge` → `ori_str_eq`/`ori_str_ne`/`emit_str_cmp_predicate`,
            // consume=false), logical (bool operands, scalar), and bitwise
            // (byte/int operands) ops BORROW-READ their operands — an RcPtr literal
            // flowing into a comparison is NOT a duplicating use (RL-1 `!incElidable`),
            // so its fresh inc is elidable. Over-flagging every `Binary(_)` keeps a
            // spurious keep-alive inc on a comparison literal → leak.
            if let ArcInstr::Let {
                value: ArcValue::PrimOp { op, args },
                ..
            } = instr
            {
                if matches!(op, PrimOp::Binary(ori_ir::BinaryOp::Add)) {
                    for &arg in args {
                        if is_rcptr(arg) {
                            consumed_owned.push(arg);
                        }
                    }
                }
            }
        }
        // Terminator-position owned consume (Invoke/InvokeIndirect owned args).
        let term_used = block.terminator.used_vars();
        for (pos, &arg) in term_used.iter().enumerate() {
            if block.terminator.is_owned_position(pos) && is_rcptr(arg) {
                consumed_owned.push(arg);
            }
        }
        // Interprocedural COW at an `Invoke` terminator: RcPtr args to a may-COW
        // callee. `Invoke.args` are positional user args (param index == arg idx).
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            for (pos, &arg) in args.iter().enumerate() {
                if is_rcptr(arg) && callee_may_cow(*callee, pos) {
                    consumed_maycow.push(arg);
                }
            }
        }
    }
    // Resolve OWNED consumes to same-alloc reps THROUGH the forward phi-threaded
    // attribution. A consumed var's phi class unions the FORWARD Jump-arg →
    // block-param rename edges `same_alloc_reps` excludes by design, so a
    // loop-body owned-consume of a back-edge-renamed alias propagates the cow-flag
    // to the SOURCE's same-alloc rep. Spec: Annex E §AIMS RL-1 (`!incElidable` on
    // a duplicating / COW use).
    let phi_reps = compute_phi_threaded_alloc_reps(func, same_alloc_reps);
    let phi_rep_of = |v: ArcVarId| phi_reps.get(&v).copied().unwrap_or(v);
    // Phi-rep → {same-alloc reps of every var in the phi class}. Iterate the
    // phi-union's full key set (NOT just `same_alloc_reps`) so a bb0 construct
    // source merged into the class ONLY by a forward Jump-arg edge (a var absent
    // from `same_alloc_reps`, e.g. the `rc_matrix` `%3` jumped into the loop) is
    // reachable. The phi-rep ROOT is itself a same-alloc rep but is NOT a
    // `phi_reps` KEY (keys are only the unioned children), so map each phi-rep to
    // ITS OWN same-alloc rep too.
    let mut phi_class_members: FxHashMap<ArcVarId, FxHashSet<ArcVarId>> = FxHashMap::default();
    for &member in phi_reps.keys() {
        let root = phi_rep_of(member);
        let class = phi_class_members.entry(root).or_default();
        class.insert(rep_of(member));
        class.insert(rep_of(root));
    }
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for arg in consumed_owned {
        let root = phi_rep_of(arg);
        reps.insert(rep_of(arg));
        reps.insert(rep_of(root));
        if let Some(members) = phi_class_members.get(&root) {
            reps.extend(members.iter().copied());
        }
    }
    // May-COW user-call args stay REP-LOCAL (no phi-propagation) — the prior
    // conservative behavior (`arc_borrowed_param_cow_push_use_after` shape).
    for arg in consumed_maycow {
        reps.insert(rep_of(arg));
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
/// `Scalar` reprs cannot reach here — `emit_burden_ops` filters them at
/// admission via the type-level `burden_carries_rc` filter PLUS the per-var
/// repr-aware gate consulting this same `var_reprs` source (Spec: Annex E
/// §AIMS RE-2 / DP-1 / L-9). A `Scalar` or out-of-range `var_repr` leaves the
/// burden op in place rather than synthesizing an unsound `RcDec`.
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
            // RE-2 backstop: an ABSENT repr is unpopulated — emit_burden_ops never
            // emits whole-var burden ops on a repr-less var, so an absent repr at
            // this point is a contract violation — leave the burden op in place
            // (codegen no-ops it) rather than emit unsound RC.
            let Some(repr) = func.var_repr(var) else {
                continue;
            };
            let ty = func.var_type(var);
            let has_user_drop = type_has_user_drop(ty, type_registry);
            // Why: a Scalar repr carries no RC header — skip it (no RC op) UNLESS
            // its type has a user `@drop`, which falls through to the `UserDrop`
            // branch below (the `@drop` call alone). Spec: Annex E §AIMS RL-DROP.
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
