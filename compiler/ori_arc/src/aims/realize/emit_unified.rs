//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Phase 1 sub-step B of [`super::realize_rc_reuse()`].

#[cfg(test)]
mod burden_lowering_tests;

use std::sync::LazyLock;

use crate::lower::type_has_user_drop;
use ori_ir::Name;
use ori_types::{Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::MemoryContract;
use crate::aims::emit_rc::DeferredDec;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::apply_aliases::build_let_alias_map;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::AccessClass;
use crate::ir::{
    ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, PrimOp,
    RcAtomicity, RcStrategy, ValueRepr,
};

use super::metrics;
use super::transfer_anchor_net;

/// `ORI_DISABLE_BURDEN_ELIM=1` bypasses Phase 2.5 burden-op elimination, read
/// once at first access. Isolates Phase 5 emission from elimination for
/// diagnostic bisection.
static BURDEN_ELIM_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_BURDEN_ELIM").as_deref() == Ok("1"));

/// `ORI_DISABLE_BORROWED_ITER_CONSUME_KEEPALIVE_DECLINE=1` restores the Phase-6.66b
/// single-iter-consume keep-alive's paired `BurdenDec` landing on the BORROWED PARAM
/// itself (when the lineage's last non-iter use names the param). The keep-alive
/// `[inc, dec]` pair bridges the buffer's life across `ori_iter_drop` for the
/// reuse-after-iter shape and both halves reference the iter source alias; the dec
/// on the borrowed param is an `RcDec on borrowed param` VF-1 ICE (the caller owns
/// the borrowed param's release — `@iter` is an `ApplyToIterConsumingParam` transfer
/// per `RL2_iter_consuming_no_caller_dec`). Default (unset): the paired dec is
/// retargeted onto the non-param keep-alive alias `inc_arg` (balanced pair, same
/// site, no borrowed-param dec).
static BORROWED_ITER_CONSUME_KEEPALIVE_DECLINE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_BORROWED_ITER_CONSUME_KEEPALIVE_DECLINE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_TAKE_PROJECT_BYPASS_ENTRY_RELEASE=1` declines the
/// dead-at-bypass-entry fallback in `compute_take_project_source_plan`: an
/// iterator-bearing take-project source enum dead-at-entry on the BYPASS edge of an
/// OUTER runtime gate (`if flag then <match consumes> else 0`) reverts to the
/// pre-cure under-release (the bypass path's `+1` iterator leak). The entry-states
/// loop's emission (source live at the bypass-safe entry) is byte-identical
/// regardless. Bisection surface: isolates an outer-gated take-project bypass-path
/// leak to this fallback vs the rest of the take-project source-dec pass. Default
/// (unset): the dominance-safe dead-at-bypass-entry release is emitted. Spec: Annex
/// E §AIMS RL-4 + RL-2.
static TAKE_PROJECT_BYPASS_ENTRY_RELEASE_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_TAKE_PROJECT_BYPASS_ENTRY_RELEASE").as_deref() == Ok("1")
});

/// `ORI_DISABLE_COMPARISON_FORWARDER_SAME_ROOT_EXEMPT=1` keeps the comparison-operand
/// same-root guard purely structural (the pre-cure behavior): a `==`/`!=` whose two
/// operands share one `same_alloc` rep declines the M3/M4 strip unconditionally.
/// Default (unset): a forwarder-transfer pair (one operand a `transfers_through_return
/// ∧ Direct` forwarder RESULT sharing the other operand's allocation) is EXEMPTED from
/// the guard — the two operands are genuinely distinct co-references (the duplication
/// funds the transfer, rc 1 -> 2), so the strip fires (RL-1 + RL-2).
static COMPARISON_FORWARDER_SAME_ROOT_EXEMPT_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ORI_DISABLE_COMPARISON_FORWARDER_SAME_ROOT_EXEMPT").as_deref() == Ok("1")
});

fn comparison_forwarder_same_root_exempt_disabled() -> bool {
    *COMPARISON_FORWARDER_SAME_ROOT_EXEMPT_DISABLED
}

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
    state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    type_registry: &TypeRegistry,
) -> (
    usize,
    Vec<DeathEvent>,
    Vec<AllocEvent>,
    metrics::SynergyMetrics,
) {
    use crate::aims::emit_rc::{coalesce_block_rc, collect_all_borrowed_defs, DeferredDec};

    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    let all_borrowed_defs = collect_all_borrowed_defs(func, pool);

    // Per-class take-project facts (union-find + CFG reachability), computed
    // once per function. Each source seeds a connected-component class over
    // Let-alias + Jump-arg→block-param edges, with its own bypass-safe blocks
    // and entries. `dead_cleanup` (sources 1, 2) and `edge_cleanup` query
    // `is_in_class` / `class_of` / `is_bypass_safe_entry_for_var` to drop
    // exactly once per CFG path — no double-free, no leak.
    let take_move_facts = crate::aims::emit_rc::take_project::analyze(func, pool);

    // Same-allocation union-find reps (`Let{Var}` + apply Direct/Conditional;
    // EXCLUDES Jump-arg phi), computed once per function for the burden tail's
    // same-alloc gates.
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(func, state_map.apply_result_aliases());

    // Deferred decs routed to edge cleanup. The burden path populates none here;
    // `emit_edge_cleanup` (Phase 6.5) restores deferred dying-edge releases from
    // its own SSOT, so this stays empty.
    let block_deferred: FxHashMap<usize, Vec<DeferredDec>> = FxHashMap::default();

    // The burden path (Phase 2.5 elimination + Phase 7 lowering below) is the
    // sole RC emitter.

    // Phase 2.5: DP-2/DP-3 burden-op elimination.
    eliminate_burden_ops_phase(func, state_map, contracts, interner, &same_alloc_reps);

    emit_burden_path_probe_tail(
        func,
        state_map,
        pool,
        interner,
        contracts,
        &all_borrowed_defs,
        &take_move_facts,
        &block_deferred,
        &same_alloc_reps,
        type_registry,
    );

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }
    trace_phase_snapshot("after_phase_3_coalesce", func, interner);

    let rc_count = count_rc_ops(func);
    (
        rc_count,
        Vec::new(),
        Vec::new(),
        metrics::SynergyMetrics::default(),
    )
}

/// Phase 2.5: DP-2/DP-3 burden-op elimination. Consumes post-emission IR with
/// full burden ops present; removes redundant `BurdenInc` / `BurdenDec*` sites
/// whose lattice state satisfies `is_rc_inc_elidable` / `is_rc_dec_unnecessary`.
/// Runs BEFORE Phase 3 coalesce so coalesce operates on the post-elimination IR.
///
/// The burden path is the sole RC emitter: DP-2 dec-elision must NOT elide a
/// sole-emitter release (it would leak), so `eliminate_whole_function` gates DP-2
/// internally; DP-3 inc-elision is co-emitter-independent (`RL1_duplication_balanced`).
fn eliminate_burden_ops_phase(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if !*BURDEN_ELIM_DISABLED {
        super::eliminate_burden_ops(func, state_map, same_alloc_reps, contracts, interner);
    }
    trace_phase_snapshot("after_phase_2_5_burden_elim", func, interner);
}

/// Probe-path realization tail (`ORI_DISABLE_PREDICATE_STACK_RC=1`): the
/// burden path is the sole RC emitter, so the predicate-stack Phases 1/1.5/2/2.1
/// are skipped above. Two steps run here:
///
/// - Phase 6.5 — burden-path dying-edge cleanup. The Phase-5 burden walk
///   (`burden_lower`) defers the dec for vars live-out of a block to the
///   predicate-stack `emit_edge_cleanup` (RL-4), which is off under the probe.
///   `emit_edge_cleanup(..., burden_only=true)` restores that deferred release,
///   emitting a `BurdenDec` on each dying CFG edge for the SAME
///   `compute_branch_edge_dead_set` / `compute_invoke_edge_dead_set` SSOT the
///   default-path emitter consumes. Without it, a value discarded on a branch
///   edge (e.g. the Err variant of a `Result` discarded by `??`) leaks its heap
///   payload.
/// - Phase 7 — mechanical burden lowering (`lower_burden_ops_to_rc`): the
///   Phase-6.5 + surviving whole-var `BurdenInc`/`BurdenDec` become real
///   `RcInc`/`RcDec`.
///
/// Spec: Annex E §AIMS RL-4 (edge-specific dec) + RL-comp (lowered net-balance).
#[expect(
    clippy::too_many_arguments,
    reason = "probe-path realization tail threads the function-wide realization \
              context (state map, pool, interner, callee contracts, borrowed defs, \
              take-move facts, deferred decs, same-alloc reps) the phases below \
              consume; bundling into a struct fragments the single probe-tail \
              orchestration"
)]
#[expect(
    clippy::too_many_lines,
    reason = "single phase-ordered burden-strip pipeline — the 6.5 -> 6.6 -> 6.65 \
              -> 6.66 -> 6.66c -> 6.66b -> 6.67 -> 6.68 -> 6.68b -> 6.68c -> 6.69 \
              -> ... sequence is one cohesive orchestration; splitting mid-sequence \
              fragments the load-bearing RL-1/RL-2/RL-4 phase order and hides the \
              probe-tail shape"
)]
fn emit_burden_path_probe_tail(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    take_move_facts: &crate::aims::emit_rc::take_project::TakeMoveFacts,
    block_deferred: &FxHashMap<usize, Vec<DeferredDec>>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    type_registry: &TypeRegistry,
) {
    crate::aims::emit_rc::emit_edge_cleanup(
        func,
        state_map,
        pool,
        all_borrowed_defs,
        take_move_facts,
        block_deferred,
        true,
    );
    trace_phase_snapshot("after_phase_6_5_burden_edge_cleanup", func, interner);

    // Step-2 RL-4 release of the step-1 COW-inc emitted at an `Invoke` TERMINATOR
    // owned position (a may-unwind COW/iter call — `list.push(v)` lowers to
    // `Invoke @push(recv [own], ...) normal/unwind`). The borrowed-alias receiver
    // got a `BurdenInc` (step 1); the COW helper COPIES (rc ≥ 2), so the original
    // SURVIVES the call into both successors and is dead at each — release it on
    // the normal AND unwind successor edges (RL-4 `RL4_edge_release_balanced`,
    // released exactly once per path). The probe-tail `emit_edge_cleanup`
    // SUPPRESSES this via `is_owned_transfer_arg_at_terminator` (it treats the
    // owned Invoke position as an ownership transfer), so the dedicated pass below
    // is the freeing-dec emitter for the COW-inc'd lineage.
    emit_cow_inc_terminator_edge_release(func, interner, same_alloc_reps);
    trace_phase_snapshot("after_phase_6_6_cow_inc_edge_release", func, interner);

    // Phase 6.65 — RL-2/RL-4 relocation of a borrowed-terminator-Invoke arg's
    // scope-exit BurdenDec that the Phase-5 walk placed INLINE in the block whose
    // TERMINATOR is `Invoke @callee(recv [borrow]) normal N unwind U`. Inline, the
    // dec runs BEFORE the call reads `recv` → the source collection (and its heap
    // element strings, via the runtime val_inc / ori_iter_drop) is freed before
    // the callee borrows it → use-after-free / double-free.
    //
    // Two escape-safe callee classes (per the callee `MemoryContract`):
    //  - collection-conversion builtins (`@keys`/`@values`/`@split`/`@to_list`):
    //    structurally COPY elements into a fresh result (the result never aliases
    //    the source's payload). Source survives the borrowed call, dead on each
    //    successor → relocate the release to BOTH normal AND unwind edges.
    //  - non-conversion callees whose `ParamContract` for `recv` proves NO
    //    return-view aliasing (`return_alias == None && !return_payload_contains_param`):
    //    a borrow-and-return-fresh/scalar callee (`@len`, `@union`) behaves like a
    //    conversion → BOTH edges; a transfer/iter-consume callee (`ParamContract.access
    //    == Owned`, e.g. a user fn whose body `@iter`-consumes the arg, freeing it
    //    via `ori_iter_drop` on the NORMAL path) → relocate to the UNWIND edge ONLY
    //    (the callee owns/frees it on normal return; the caller releases only on the
    //    unwind path where the callee may not have consumed it yet).
    //
    // Escape-over-fire guard: a callee that BORROWS-AND-RETURNS A VIEW into `recv`
    // (`return_alias` = Direct/Project, or `return_payload_contains_param`, or a
    // non-scalar result that may alias) needs the caller dec KEPT — relocating frees
    // the returned view's backing. No contract for the callee → leave inline
    // (conservative). RL-4 release exactly once per concrete path:
    // `AimsProof.Realization::RL4_edge_release_balanced`; the non-transfer caller dec
    // for a borrowed last use is `RL2_nontransfer_kinds_dec`, the transfer-kind no-dec
    // is `RL2_transfer_kinds_no_dec`. Spec: Annex E §AIMS RL-2 + RL-4.
    relocate_borrowed_terminator_arg_dec_to_edges(func, interner, contracts);
    trace_phase_snapshot(
        "after_phase_6_65_borrowed_terminator_arg_edge",
        func,
        interner,
    );

    // Phase 6.66 — MULTI-BORROW iter-consume source accounting (RL-1 keep-alive
    // inc + RL-2 single release). The 6.65 relocation handles the SINGLE-borrow
    // iter-consume source (its `lineage_live_out` guard declines a live-out
    // source). This pass handles the live-out / multi-borrow case it declines: a
    // source `coll` (RcPtr collection) flowing to N >= 2 iter-consuming `[own]`
    // call positions, where the source SURVIVES the earlier calls. Two iter-consume
    // kinds count: a USER callee whose `ParamContract.iter_consumes` is true, AND
    // the INLINE for-loop `@iter(coll [own])` protocol builtin (`for x in coll`
    // lowers to `@iter [own]` -> `ori_iter_drop`, which has no user contract). Each
    // consuming position's `@iter [own]` -> `ori_iter_drop` frees the collection on
    // every exit, so:
    //  - the first N-1 uses are DUPLICATING uses (the value is duplicated into a
    //    callee that will drop it) -> RL-1 emits a keep-alive `BurdenInc` per
    //    non-last use (`AimsProof.Realization::RL1_emit_iff_not_elidable`);
    //  - the caller emits NO `BurdenDec` on the source lineage (the callees free;
    //    the Nth call's iter-drop IS the single release per
    //    `RL2_iter_consuming_no_caller_dec` + `RL2_release_exactly_once`).
    // The Phase-5 walk diverges: the multi-use source is not a single-use move
    // (`compute_transfer_via_move_alias` requires use-count == 1) so it emits a
    // spurious source `BurdenDec` at its multi-use move-alias point, which
    // double-frees against the callee iter-drops. This pass rewrites the source
    // lineage's normal-path burden ops to the proven oracle ledger: (N-1)
    // keep-alive incs before the non-last consuming calls, zero source decs on
    // the normal paths (unwind-edge decs are panic cleanup, left intact).
    // Spec: Annex E §AIMS RL-1 + RL-2.
    suppress_multi_borrow_iter_consume_source_decs(func, pool, interner, contracts);
    trace_phase_snapshot("after_phase_6_66_multi_borrow_iter_consume", func, interner);

    // Phase 6.66c — SINGLE borrowed-Invoke-arg iter-consume of an owned FRESH
    // collection source, DEAD after the call (RL-2 iter-consume transfer). A
    // freshly-constructed collection passed at a BORROWED terminator-`Invoke` arg
    // to a USER callee whose `ParamContract.iter_consumes` is true (`@get_lengths(words)`
    // whose body `@iter(words [own])` -> `ori_iter_drop` frees the buffer) is a FULL
    // inward ownership transfer: the callee's iterator machinery is the single
    // release (`RL2_iter_consuming_no_caller_dec`). The Phase-5 walk does NOT model
    // the user-callee iter-consume as a transfer, so it emits a spurious FRESH-site
    // `BurdenInc` (RL-1 duplication) on the source plus a misplaced scope-exit
    // `BurdenDec` reaching only the unwind edges -> the source buffer (and its owned
    // element strings) is incd once but never released on the normal path -> leak.
    // This is the SINGLE-borrow complement of Phase 6.66 (which handles N >= 2
    // live-out borrowed-Invoke iter-consume uses): for the N == 1 dead-after case the
    // caller emits NO inc and NO dec, so strip every normal-path burden op on the
    // source lineage. The scalar-vs-collection callee return type is irrelevant —
    // `iter_consumes` PROVES the callee freed the source, so the callee's result
    // cannot alias it (the `dst_scalar` gate Phase 6.65 needs for non-iter-consume
    // borrow reads does not bound this transfer). Probe-gated -> default codegen
    // byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
    suppress_single_borrowed_invoke_iter_consume_source(
        func,
        pool,
        interner,
        contracts,
        same_alloc_reps,
    );
    trace_phase_snapshot(
        "after_phase_6_66c_single_borrowed_invoke_iter_consume",
        func,
        interner,
    );

    // Phase 6.66d — ITER-CONSUME + TRANSFER-THROUGH-RETURN source-dec suppression
    // (RL-1 keep-alive inc + RL-2 iter-consume transfer + the proven overlap
    // balance). An owned param both iter-consumed via `@iter [own]` AND
    // transferred through the function's own `Return` has its premature
    // normal-path source `BurdenDec` (emitted before the iter-consume) freeing
    // the param before the Return -> caller UAF. Strip that normal-path source
    // dec, keeping the keep-alive inc so the kept-from-arrival reference survives
    // as the live Return value. Probe-gated -> default codegen byte-identical when
    // no param matches the overlap contract. Spec: Annex E §AIMS RL-1 + RL-2.
    suppress_iter_consume_transferred_return_source_dec(
        func,
        pool,
        interner,
        contracts,
        same_alloc_reps,
    );
    trace_phase_snapshot(
        "after_phase_6_66d_iter_consume_transferred_return",
        func,
        interner,
    );

    // Phase 6.66g — AGGREGATE-FIELD iter-consume partial-dec (RL-2 field-grained
    // iter-consume inward transfer). A fresh/dead OWNED aggregate passed at a
    // BORROWED `Invoke` arg to a callee whose `ParamContract.iter_consumes_projected_field`
    // is `Some(field)` (the callee iter-consumes `Project param.field` — e.g.
    // `for item in c.items` over a borrowed struct `c`) has that field's
    // ownership transferred inward: the callee's `ori_iter_drop` is the single
    // release of the projected collection. The caller's whole-aggregate scope-exit
    // `BurdenDec` would recursively re-free that consumed field (double-free), so
    // rewrite it to a `BurdenDecPartial skip_fields=[field]` — releasing the
    // aggregate shell + its OTHER owned fields but NOT the iter-consumed field.
    // Probe-gated (`ORI_DISABLE_AGG_FIELD_ITER_CONSUME_PARTIAL`) -> default codegen
    // byte-identical when no aggregate matches. Spec: Annex E §AIMS RL-2.
    rewrite_aggregate_iter_consume_field_decs(func, contracts, same_alloc_reps);
    trace_phase_snapshot(
        "after_phase_6_66g_aggregate_field_iter_consume",
        func,
        interner,
    );

    // Phase 6.66b — SINGLE iter-consume + non-iter REUSE keep-alive (RL-1
    // duplication + RL-2 iter-consume transfer). A borrowed collection iterated
    // ONCE (`@iter [own]` -> `ori_iter_drop`) then REUSED at a non-iter position
    // (`words[0]`, an `@__index`/`Apply`-arg/`Return`) would have its buffer freed
    // by the iter-drop out from under the reuse. Emit a keep-alive `BurdenInc`
    // before the `@iter` (the iter-drop decs the keep-alive copy, the original
    // borrow survives the reuse) + a paired `BurdenDec` at the lineage's LAST
    // non-iter use (after the reuse) that releases the duplicate. The `[inc, dec]`
    // pair nets 0 (`RL1_duplication_balanced`); both lower normally in Phase 7
    // (`BurdenInc` -> `RcInc`, `BurdenDec` -> `RcDec`) and mark `arg` in
    // `func.burden_emitted` so VF-1's per-var balance check is satisfied. The
    // `ori_iter_drop` still consumes the caller-transferred ref exactly as in the
    // no-reuse case — the pair only bridges the buffer's life across the iter-drop.
    // Spec: Annex E §AIMS RL-1 + RL-2.
    emit_single_iter_consume_reuse_keepalive(func, pool, interner, contracts);
    trace_phase_snapshot(
        "after_phase_6_66b_single_iter_consume_reuse_keepalive",
        func,
        interner,
    );

    // Phase 6.66e — LOOP-INVARIANT iter-consumed SURVIVOR surplus suppression
    // (RL-1 duplication + RL-2 iter-consume transfer + RL-2 release-exactly-once).
    // A loop-INVARIANT collection (`Construct` OUTSIDE the loop) iter-consumed via
    // the inline for-loop `@iter(arg [own])` and READ AFTER the loop via a borrow
    // (`words.len()`) is the survivor shape. The base walk over-emits across the
    // loop-carried lineage (`same_alloc_reps` drops the Jump-phi back-edge): a
    // surplus FRESH-site `BurdenInc` at the `Construct` + a surplus pre-survivor-read
    // `BurdenDec`, beyond the genuine keep-alive inc + the one post-read survivor
    // release → net -1 double-free. This pass rewrites the survivor rep's burden ops
    // to the proven oracle ledger (keep ONE keep-alive inc the `@iter`/`ori_iter_drop`
    // pair balances + the LAST dec, the post-read survivor release; strip the
    // surplus). Four discriminators bound the over-fire to EXACTLY the survivor
    // shape: BACK-EDGE-EXCLUDED forward-threaded reps (no loop-carried-accumulator
    // merge), a BORROWED-position post-loop COLLECTION read (vs str_split's no-read /
    // a Project element-view / a forwarded dec), a genuine-value-COW-mutator decline
    // (@iter excluded — keeps map-cow keep-alive incs), and a collection-conversion
    // decline (@keys/@values/@split/@to_list). Lean: `RL1_duplication_balanced` +
    // `RL2_iter_consuming_no_caller_dec` + `RL2_release_exactly_once`.
    // Spec: Annex E §AIMS RL-1 + RL-2.
    suppress_loop_invariant_iter_survivor_surplus(func, pool, interner, contracts);
    trace_phase_snapshot(
        "after_phase_6_66e_loop_invariant_iter_survivor",
        func,
        interner,
    );

    // Phase 6.66f — SHARING-VIEW slice + iter-consume surplus-inc suppression
    // (RL-1 duplication + RL-2 iter-consume transfer + RL-2 release-exactly-once).
    // A FRESH owned collection (`let words = [..]`) BORROWED into a seamless-slice
    // producer (`words.take(2)` / `.slice(..)` / `.substring(..)` / `.drop(..)` —
    // `sharing_view_relocation_names`) AND iter-consumed by the inline `@iter [own]`
    // is the slice+iter-interaction shape. The sharing-view producer ITSELF
    // rc-INCs the shared backing buffer (rc 1 -> 2; the surviving slice's own
    // scope-exit dec is the balancing release), and the `@iter [own]` -> `ori_iter_drop`
    // is the alloc's single transfer release (`RL2_iter_consuming_no_caller_dec`).
    // So the source's correct burden ledger is ZERO incs: the slice-share is funded
    // by the producer's codegen inc, the iter-consume is a transfer. The base walk
    // diverges — it emits keep-alive `BurdenInc`s on the source lineage (treating the
    // live-across iter-consume as a duplication that needs a caller keep-alive),
    // beyond the producer's own inc — so the rc-1 buffer never reaches 0 (the buffer
    // plus its owned element strings leak via the never-run `elem_dec_fn`). This pass
    // strips every NORMAL-path source `BurdenInc` on the iter-consumed sharing-view
    // source lineage (unwind-edge ops are panic cleanup, left intact). Lean:
    // `RL1_duplication_balanced` (the single slice-share dup is funded by the producer
    // inc) + `RL2_iter_consuming_no_caller_dec` + `RL2_release_exactly_once`.
    // Spec: Annex E §AIMS RL-1 + RL-2.
    suppress_sharing_view_iter_consume_surplus_inc(
        func,
        pool,
        interner,
        contracts,
        same_alloc_reps,
    );
    trace_phase_snapshot(
        "after_phase_6_66f_sharing_view_iter_consume_surplus",
        func,
        interner,
    );

    // Phase 6.67 — NESTED-loop iter-element-view keep-alive inc (RL-1). A nested
    // `for inner in outer do { for x in inner do .. }` projects the inner
    // collection `inner` out of the OUTER source's iter-element-view
    // (`Project @__iter_next.1`) and consumes it `[own]` at the INNER `@iter`.
    // The inner element view OWNS NO allocation — the buffer belongs to the
    // outer collection, freed by the outer's `elem_dec_fn` when the outer
    // `ori_iter_drop` runs. But the inner `@iter [own]` -> `ori_iter_drop` ALSO
    // frees that buffer (the inner iterator took it owned), so the buffer is
    // released TWICE -> double-free. The oracle emits one keep-alive `RcInc` on
    // the inner element view immediately before the inner `@iter [own]`: the
    // inner `ori_iter_drop` then drops rc 2->1 (its single release, RL-2) and
    // the outer `elem_dec_fn` drops rc 1->0 (the buffer's true free). RL-1: the
    // inner `@iter [own]` is a DUPLICATING consume of a borrowed element view.
    // The recursion is automatic — the inner-of-inner element view is itself an
    // iter-element-view of the inner `@__iter_next`, so each nesting level's
    // `@iter [own]` arg lands in `collect_iter_element_defs` and gets its own
    // keep-alive. Probe-gated -> default codegen byte-identical. Spec: Annex E
    // §AIMS RL-1 (`RL1_emits_inc = !incElidable`) + RL-2 (`RL2_release_exactly_once`).
    emit_iter_element_view_iter_consume_keepalive_inc(func, interner, contracts, pool);
    trace_phase_snapshot(
        "after_phase_6_67_nested_iter_element_keepalive",
        func,
        interner,
    );

    // Phase 6.68 — iter-element-view stored into an AGGREGATE field keep-alive inc
    // (RL-1). A loop element `p` from `coll.split(..)` is a seamless-slice
    // (`SLICE_FLAG` cap) sharing the source backing buffer; it is a Borrowed
    // Project-view (`collect_iter_element_defs`), excluded from
    // `owned_vars_needing_rc`, so the base burden walk emits NO Construct-arg inc
    // on it. But when it is stored as a field of a burden-carrying aggregate
    // (`let w = Wrapper { s: p, .. }`), the aggregate's scope-exit `RcDec
    // [AggFields]`/`[InlineEnum]` drop-glue WALKS that field and decs the shared
    // backing once per iteration -> the source's single allocation reaches rc 0
    // early -> double-free. The oracle emits one keep-alive `RcInc <slice>` before
    // the `Construct`, balanced by the aggregate field-drop (the slice survives as
    // a duplicating use: the aggregate takes one ref + the element view is read
    // again, e.g. `p.len()`). RL-1 (`RL1_emit_iff_not_elidable`): storing a
    // still-live borrowed element view into an owned aggregate field is a
    // duplicating, non-move-once use -> emit the inc; the aggregate field-drop is
    // the balancing release (`RL2_release_exactly_once`). Probe-gated -> default
    // codegen byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
    emit_slice_element_aggregate_field_keepalive_inc(func, interner, pool);
    trace_phase_snapshot(
        "after_phase_6_68_slice_element_aggregate_field_keepalive",
        func,
        interner,
    );

    // Phase 6.68b — iter-element-view PUSHED into a RETURNED collection keep-alive
    // inc (RL-1, the element-escape shape). `for w in coll do { r = r.push(w) }; r`
    // pushes a borrowed `Project @__iter_next.1` element view `[own]` into a second
    // `r` collection that is RETURNED. Two distinct `elem_dec_fn` paths reach the
    // one element backing — the source's in-callee `ori_iter_drop` AND the caller's
    // drop of the returned `r` — so the rc-1 backing is freed then double-freed.
    // The oracle emits one keep-alive `RcInc <view>` before the push; the receiving
    // collection's `elem_dec_fn` is the balancing release (`RL1_duplication_balanced`
    // / `RL2_release_exactly_once`). The discriminator gates on the receiver being a
    // RETURNED collection (`collection_receiver_returned`),
    // which is the precise boundary distinguishing the returned-result double-free
    // from the benign in-scope-only push (where the source iter-drop and the in-scope
    // `r` drop are sequenced within one function and the base accounting balances —
    // a keep-alive there would orphan a +1 -> leak). Distinct from Phase 6.68 (which
    // handles a `Construct`/`Reuse` aggregate field DROPPED in-scope) and Phase 6.95
    // (the `for...yield` `ori_list_take` finalizer's internal `ori_list_push`).
    // Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
    emit_iter_element_pushed_into_returned_collection_keepalive_inc(func, interner, pool);
    trace_phase_snapshot(
        "after_phase_6_68b_iter_element_pushed_into_returned_collection_keepalive",
        func,
        interner,
    );

    // Phase 6.68c — N>=2 callee-returned SCALAR-list cross-call surplus keep-alive-
    // inc strip (RL-1, the multi-call RETURNED-collection accounting). `let a =
    // clone_list(words); let b = clone_list(words)` (each callee `for w in words
    // yield w`): the FIRST returned `[int]` `a` is live across the second call, so
    // the base walk emits a spurious live-across `BurdenInc a` in the block carrying
    // that call (`a` is NOT its argument). `a` is iter-consumed (`for w in a`), whose
    // `ori_iter_drop` is the acquired ref's release, so `a`'s explicit ops must net 0
    // — the spurious inc leaves +1 -> the returned buffer leaks. This pass strips the
    // ONE surplus cross-call inc; the genuine keep-alive (before `@iter`) is kept. The
    // full discriminator + over-fire guards (scalar-element gate, net-+1, non-loop)
    // live on the helper doc. Distinct from 6.66 (iter-consume SOURCE), 6.66c (single
    // borrowed-Invoke source strip), 6.68b (iter-ELEMENT into a returned collection).
    // Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-1
    // (`RL1_duplication_balanced`) + RL-2 (`RL2_release_exactly_once`).
    strip_returned_collection_multi_call_surplus_inc(func, pool, interner, same_alloc_reps);
    trace_phase_snapshot(
        "after_phase_6_68c_returned_collection_multi_call_surplus_inc",
        func,
        interner,
    );

    // Phase 6.69 — owned closure-VALUE scope-exit release (RL-2). An owned closure
    // value (`ValueRepr::FatValue` + `Tag::Function` -> `RcStrategy::Closure`) whose
    // env carries the captured allocations' RC must be released at its non-transfer
    // last use. The base burden walk emits this dec for a `let`-bound closure but
    // NOT for an ANONYMOUS chained intermediate (`fst("hello")(0)` -> the
    // PartialApply result + each closure-returning `ApplyIndirect` result are
    // intermediates, never bound, so they fall outside `owned_vars_needing_rc` /
    // the move-alias transfer set) NOR for a closure-returning `Apply`/`ApplyIndirect`
    // result that is not let-bound -> the closure env leaks under the flag. The
    // oracle emits one `RcDec <closure> [Closure]` at each closure value's last
    // (invoking) read. RL-2: invoking a closure is a `.LastReadBeforeScopeExit`
    // (non-transfer) -> the owned closure value releases its env there; a closure
    // TRANSFERRED out (stored in an aggregate, returned, passed `[own]`) has its
    // env freed by the consumer's drop -> NO dec here (over-fire boundary). Probe-
    // gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-2
    // (`RL2_dec_at_last_use` / `rl2_emits_dec(.LastReadBeforeScopeExit) = true`).
    emit_owned_closure_scope_exit_dec(func, pool, interner, all_borrowed_defs);
    trace_phase_snapshot(
        "after_phase_6_69_owned_closure_scope_exit_dec",
        func,
        interner,
    );

    // Step-B' RL-5 release of a genuinely-leaked OWNED collection-source whose
    // lineage flows (as a Jump-arg) into a block param dead at a normal terminal
    // (loop-exit / Return) and is never freed there (e.g. `m.keys()` /
    // `s.split()` / `set.to_list()`: the source map/set/str is borrowed by the
    // conversion builtin, survives, then dies at the post-loop block without a
    // dec). The whole-var `BurdenDec` lowers (Phase 7) to a `RcDec` whose
    // `RcStrategy::HeapPointer` routes a `Tag::Map`/`Set`/`List` value through
    // `ori_buffer_rc_dec`, which reads the V5 header's `elem_dec_fn`/`elem_count`
    // and walks the element-drop glue — so freeing the buffer ALSO frees its
    // owned element strings (the 2 key strings of `map_keys_str`), composing
    // with `elem_dec_fn` instead of leaking them. SEED-not-reuse: emit ONLY for
    // the jump-threaded-leaked owned collection-source lineage, excluding every
    // iterator-element / iterator-handle / borrowed / already-decced / transfer
    // var so it cannot double-free a for-loop iterator cluster (a naive
    // dead-block-param dec there double-frees the iterator-owned buffer).
    // Spec: Annex E §AIMS RL-5 (dead-block-param cleanup).
    emit_burden_dead_collection_source_decs(func, pool, interner);
    trace_phase_snapshot("after_phase_6_7_dead_collection_source", func, interner);

    // Phase 6.8 — dead OWNED-COLLECTION / mutation-result freeing (RL-2 ScopeExit
    // / ApplyToBorrowedParam). A FRESH owned collection bound as a body-local (a
    // mutation RESULT `let ys = xs.sort()` / `xs.insert(..)` / `a.union(..)`, or a
    // read-only `let m = {..}; m.contains_key(..)`), last-used at a BORROWED
    // position then dead at function scope exit, leaks its allocation under
    // sole-emitter lowering: the duplicating-use fresh-site `BurdenInc` (RL-1) +
    // per-path scope-exit decs net the EXPLICIT ops to 0, leaving the alloc `+1`
    // unreleased. The compiled-Lean `rcBalance` mandates the lifecycle-excluding-
    // alloc net `-1`; this pass restores conformance by emitting ONE additional
    // whole-var `BurdenDec` at each alloc-aware-net-positive last-use sink. The
    // `RcDec { HeapPointer }` it lowers to routes a Map/Set/List through
    // `ori_buffer_rc_dec` (the V5 `elem_dec_fn` walk) so heap element strings free
    // too. Net-gated (net 0 = already freed; `let xs = [1,2,3]; xs.length()` and
    // branch-merge phi never fire) + SEED-not-reuse (transferred / iterator-managed
    // / conversion-source lineages excluded) so it cannot double-free.
    // Spec: Annex E §AIMS RL-2.
    emit_burden_dead_owned_collection_decs(func, pool, interner, contracts, same_alloc_reps);
    trace_phase_snapshot("after_phase_6_8_dead_owned_collection", func, interner);

    // Phase 6.85 — dead-no-use INLINE-AGGREGATE freeing (RL-2 ScopeExit). A bare
    // `let a = Doc { field: <heap> }` / `let c = Link(..)` / `let t = (.., ..)`
    // binds an inline struct / enum / tuple (`ValueRepr::Aggregate`) with a
    // heap-bearing field, dead with ZERO uses. The Phase-5 walk emits ZERO burden
    // ops on a no-use aggregate (no duplicating use -> no inc, no last-use sink ->
    // no dec), so the heap field is never freed (the user `@drop` silently does
    // not run). The oracle emits one scope-exit `RcDec [AggFields]`/`[InlineEnum]`
    // walking the field drop-glue; this pass restores the RL-2 single-release the
    // compiled-Lean `rcBalance` mandates by emitting ONE whole-var `BurdenDec` at
    // the defining block's scope exit. Distinct from Phase 6.8 (dead-owned
    // COLLECTION `RcPointer` buffers): these are BARE inline aggregates with no
    // self-buffer `+1` — the dec balances the heap FIELD's implicit `+1` owned by
    // the AggFields/InlineEnum drop-glue. SEED-not-reuse (owned-consumed /
    // returned / iterator-managed lineages excluded) so it never double-frees a
    // nested node or a transferred value. Spec: Annex E §AIMS RL-2.
    emit_burden_dead_no_use_aggregate_decs(func, pool, interner, contracts);
    trace_phase_snapshot("after_phase_6_85_dead_no_use_aggregate", func, interner);

    // Phase 6.86 — branch-dead fresh-value freeing (RL-4 edge cleanup). A FRESH
    // owned non-scalar value (heap `str` FatValue / collection RcPointer /
    // heap-bearing inline aggregate) used + released on one branch but DEAD on a
    // sibling early-exit branch (`?`-None return, an `if/else` arm that never reads
    // it) leaks on the dead branch: the Phase-5 walk emits the lineage's single
    // release on the value-survives branch only, leaving the early-exit edge with
    // no release. This pass emits ONE `BurdenDec` at the FRONT of the dead
    // single-predecessor successor (RL-4: Owned, non-scalar, live at the split
    // block's exit, dead at the successor's entry, not a Jump arg). SEED-not-reuse
    // (returned / owned-consumed / PrimOp-operand / user-call-arg / owned-moved /
    // iterator-managed / borrowed lineages excluded) so it never double-frees a
    // transferred or already-released value. Distinct from Phase 6.85 (ZERO-use
    // aggregates): the branch-dead value HAS a use (its release on the surviving
    // branch). Spec: Annex E §AIMS RL-4.
    emit_burden_branch_dead_value_decs(func, pool, interner, contracts);
    trace_phase_snapshot("after_phase_6_86_branch_dead_value", func, interner);

    // Phase 6.87 — fresh INLINE-AGGREGATE-into-borrowed-call scope-exit release
    // (RL-2 ApplyToBorrowedParam + RL-4 edge cleanup). A heap value (`str` / `[T]`)
    // moved into an INLINE SUM VARIANT or struct (`ConstructArg` transfer INTO the
    // aggregate), where the fresh aggregate is then passed BORROWED to a callee that
    // borrow-reads it and is DEAD afterward, leaks the moved-in heap field. The
    // Phase-5 walk emits a matched `BurdenInc v; BurdenDec v` pair on the aggregate
    // BEFORE the borrowed call; the Phase-3 coalesce peephole cancels the adjacent
    // pair to net-0, so no scope-exit `RcDec [InlineEnum]`/`[AggFields]` survives —
    // the aggregate drop-glue (which walks the heap field) never runs. The moved-in
    // field is an `RL2_transfer_kinds_no_dec` `ConstructArg` transfer INTO the
    // aggregate, so the aggregate's own scope-exit drop is the field's sole release
    // (`RL2_release_exactly_once`); the aggregate's last use is a borrowed call, an
    // `rl2_emits_dec(.LastReadBeforeScopeExit)` non-transfer use whose release
    // relocates to the dead successor edges (`RL4_edge_release_balanced`, released
    // once per concrete path). This pass strips the coalesce-doomed inc/dec pair and
    // emits ONE `BurdenDec` at the FRONT of both successor edges. SEED-not-reuse
    // (returned / owned-consumed / owned-moved / iter-managed / Value-variant /
    // return-view-aliasing-callee lineages excluded) so it never double-frees a
    // transferred or already-balanced value. Probe-gated -> default codegen
    // byte-identical. Spec: Annex E §AIMS RL-2 + RL-4.
    relocate_borrowed_terminator_aggregate_dec(func, pool, interner, contracts);
    trace_phase_snapshot(
        "after_phase_6_87_borrowed_terminator_aggregate",
        func,
        interner,
    );

    // Phase 6.9 — dead in-function ITERATOR-HANDLE freeing (RL-2 ScopeExit). An
    // `@iter`-family result is a FRESH owned `Tag::Iterator` / `DoubleEndedIterator`
    // handle (no RC header; the source buffer is MOVED into the iterator state).
    // Iterator handles carry no `BURDEN_TABLE` / `TypeRegistry::burden` burden
    // (`UnmanagedPtr`, no refcount), so the Phase-5 walk emits ZERO ops on the
    // handle / its containing aggregate — under sole-emitter lowering the value is
    // never freed (the freeing is a destructor call `ori_iter_drop`, not a
    // refcount dec). This pass emits ONE whole-var `BurdenDec` on the bare handle
    // (lowered `RcStrategy::Iterator` → `ori_iter_drop`) OR the iterator-bearing
    // aggregate (lowered `RcStrategy::AggregateFields`/`InlineEnum`, whose drop
    // walks the iterator field) at its dead-at-scope-exit sink, restoring the RL-2
    // single-release the compiled-Lean `rcBalance` mandates. SEED-not-reuse:
    // for-loop-managed handles (`@ori_iter_drop` already emitted) + returned +
    // owned-transferred lineages are excluded so it cannot double-free.
    // Spec: Annex E §AIMS RL-2.
    emit_burden_dead_iterator_handle_decs(func, pool, interner);
    trace_phase_snapshot("after_phase_6_9_dead_iterator_handle", func, interner);

    // Phase 6.10 — take-project iterator-handle SOURCE freeing (RL-2 ScopeExit +
    // bypass-safe per-class drop). An `Iterator<int>` payload inside an enum
    // (`MaybeIter = Empty | Holds(it: Iterator<int>)`) is PROJECTED OUT on a match
    // arm and consumed at an owned position (`@count(it [own])`); on every
    // NON-projecting path the source enum (holding the iterator handle) is
    // dead-at-scope-exit and must be freed (`RcDec [InlineEnum]` -> the InlineEnum
    // drop walks the iterator field -> `ori_iter_drop`). The Phase-5 walk mis-models
    // the take-project source: it emits a spurious last-use `BurdenDec` on the
    // consuming arm (-> use-after-free, the source frees the iterator before `@count`
    // reads it) and OMITS the dec on the bypass / Empty paths (-> leak). This pass
    // mirrors the predicate-stack `dead_cleanup` bypass-safe-entry emission via the
    // shared `TakeMoveFacts` SSOT: strip the spurious in-class source ops, then emit
    // ONE `BurdenDec` per let-alias rep at its `is_bypass_safe_entry_for_var` block.
    // Probe-gated -> default codegen byte-identical (the predicate stack co-emits the
    // bypass-safe scope-exit drop on the default path; this pass never runs there).
    // Spec: Annex E §AIMS RL-2.
    emit_burden_take_project_source_decs(func, state_map, pool, take_move_facts);
    trace_phase_snapshot("after_phase_6_10_take_project_source", func, interner);

    // Phase 6.95 — for_yield INDEX-consumed element RC (the joint yield-element-inc
    // + index-result-element-dec). `for x in coll yield expr` moves each borrowed
    // `Project @__iter_next.1` element view into `ori_list_push(scratch, w [own])`;
    // the iter-element-view exclusion drops `w`'s `BurdenInc`. When the result is
    // INDEX-consumed (`result[i]` -> `@__index`) the result owns its own element
    // COPIES (the source's `IterState::Drop` frees the source copies), so the
    // result needs a yield-element `BurdenInc` (RL-1: the push DUPLICATES the
    // element into the result buffer) AND each `@__index` view needs a release
    // (RL-2). An ITER-consumed result (a second `@iter [own]` -> `ori_iter_drop`)
    // frees its own elements -> NO inc/dec (the move-vs-borrow discriminator; the
    // `yield_identity_str_list` canary). Probe-gated -> default codegen
    // byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
    emit_for_yield_index_consumed_element_rc(func, pool, interner, same_alloc_reps);
    trace_phase_snapshot("after_phase_6_95_for_yield_index_element", func, interner);

    // Phase 6.95b — relocate a PREMATURE normal-path release of an eligible
    // `for_yield` `ori_list_take` result list whose allocation is read again in a
    // LATER block via a same-allocation sibling alias. The base walk places the
    // list's single normal-path dec at an early sibling's SSA last-use (the
    // per-SSA-var `live_out` suppressor misses the sibling-alias allocation
    // liveness), freeing the list before a later block re-reads it (-134 UAF). This
    // relocates the single dec to AFTER the lineage's execution-final read — one
    // release, moved later (RL2_release_exactly_once preserved; net unchanged).
    // Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-2 + RL-4.
    relocate_for_yield_result_premature_release(func, pool, interner, same_alloc_reps);
    trace_phase_snapshot(
        "after_phase_6_95b_for_yield_result_premature_release",
        func,
        interner,
    );

    // Phase 6.96 — strip the spurious project-borrowed-view `BurdenDec` whose
    // source aggregate's `[AggFields]`/`[InlineEnum]` drop already frees the
    // projected heap field (RL-4 borrowed view emits no release; RL-2 the
    // aggregate drop is the field's single release). A `let v = w.field`
    // borrow-view of a LOCAL owned aggregate gets a spurious last-use `BurdenDec`
    // from the Phase-5 walk; the aggregate's own scope-exit `RcDec [AggFields]`
    // ALSO frees that field -> double-free. The alloc-aware net (the source
    // aggregate is single-ref + its drop fires) distinguishes the redundant
    // surplus dec (strip) from a paired-inc shared-aggregate projection dec that
    // releases an extra ref (keep). NOT a `collect_project_borrowed_defs`
    // membership strip (too broad -> orphans last-owner collection-field views).
    // Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-2 + RL-4.
    let project_borrowed_defs =
        crate::aims::emit_rc::borrowed_defs::collect_project_borrowed_defs(func, pool);
    strip_redundant_project_borrowed_view_decs(func, pool, &project_borrowed_defs);
    trace_phase_snapshot(
        "after_phase_6_96_project_borrowed_view_strip",
        func,
        interner,
    );

    // Phase 6.97 — strip the spurious comparison-operand keep-alive `BurdenInc`
    // (M3) + the misplaced branch whole-var `BurdenDec` (M4) for the
    // USED-and-compared aggregate-with-heap-field derived-`Eq` / derived-`Clone`
    // `a == b` / `a != c` leak. A multi-use compared aggregate `%src` gets ONE
    // construct keep-alive `BurdenInc`; each comparison move-alias `%op = %src`
    // (whose sole non-RC use is a `Binary(Eq|NotEq)` operand) is wrongly classed a
    // `dup_alias_dst` and gets a SPURIOUS keep-alive inc, even though a `==` / `!=`
    // operand is an RL-1 BORROW-READ (`incElidable`). The spurious operand incs net
    // the `%src` allocation +1 on every path -> the heap field leaks. M3 strips the
    // operand inc (its paired dec balances the construct keep-alive); M4 strips the
    // whole-var `BurdenDec %src` on the branch the surviving operand dec already
    // releases (the oracle emits `%src`'s whole-var release only on the complement
    // branch). The strip is gated on the comparison-operand alias membership, NEVER
    // use_counts / aggregate membership -> a `Config { settings, name }` whose
    // fields are PROJECTED + independently freed has no operand alias and never
    // fires. Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS
    // RL-1 (`RL1_emit_iff_not_elidable`) + RL-2 (`RL2_release_exactly_once`).
    strip_comparison_operand_keepalive(func, pool, same_alloc_reps);
    trace_phase_snapshot(
        "after_phase_6_97_comparison_operand_keepalive",
        func,
        interner,
    );

    // Phase 6.98 — RL-4 release on dying Invoke UNWIND edges whose
    // predecessor carries a self-canceling whole-var burden pair (net 0 —
    // the walk's terminator-arg bookkeeping). Without it, a panic landing
    // in a LIVE unwind successor (`catch(expr: opt.expect(msg:))` via the
    // armed intercepted-unwind route) leaks the value on the caught path.
    // Runs LAST so every normal-path repair pass above computes against
    // the unchanged baseline; touches only unwind-successor block fronts.
    // Spec: Annex E §AIMS RL-4.
    crate::aims::emit_rc::emit_invoke_unwind_pair_net_releases(
        func,
        state_map,
        pool,
        all_borrowed_defs,
        take_move_facts,
    );
    trace_phase_snapshot(
        "after_phase_6_98_invoke_unwind_pair_release",
        func,
        interner,
    );

    // Phase 6.99 — transfer-anchor credit net (RL-34 + RL-2 + RL-1). Per-rep
    // alloc-aware net over the result-side lineage of a PROVEN
    // `transfers_through_return ∧ Owned ∧ Direct` forwarder call whose arg-side
    // lineage is CALLER-FRESH: models the transferred-in CREDIT (+1 at the
    // anchor's normal successor — the caller re-acquires the SAME allocation,
    // RL-34) + the `[own]` hand-offs (-1, RL-2) + fresh births (+1) + surviving
    // whole-var ops (±1), then repairs the lineage to net EXACTLY 0 on every
    // Return path (Resume/Unreachable >= 0): removing the single spurious
    // fresh-site keep-alive inc, OR placing ONE whole-var release after the
    // execution-final value-read (RC ops are NOT value-reads per TF-11). When
    // the net cannot be proven, changes NOTHING. Runs LAST so every repair pass
    // above computes against a byte-identical baseline. Spec: Annex E §AIMS
    // RL-34 + RL-2 + RL-1 + RL-comp.
    transfer_anchor_net::apply_transfer_anchor_credit_net(
        func,
        pool,
        interner,
        contracts,
        same_alloc_reps,
    );
    trace_phase_snapshot(
        "after_phase_6_99_transfer_anchor_credit_net",
        func,
        interner,
    );

    lower_and_diagnose_burden_path(
        func,
        pool,
        type_registry,
        interner,
        contracts,
        same_alloc_reps,
    );
}

/// Probe-tail Phase 7: mechanically lower the surviving burden ops to real RC
/// (`BurdenInc → RcInc` / `BurdenDec → RcDec`, eliding the M1 fresh-surplus incs)
/// then emit the per-same-alloc-rep alias-lineage RC-net diagnostic — the
/// "lower then verify" stage of [`emit_burden_path_probe_tail`]. Spec: Annex E §AIMS
/// RL-comp (lowered net-balance), RL-2 (per-lineage release-once diagnostic).
fn lower_and_diagnose_burden_path(
    func: &mut ArcFunction,
    pool: &Pool,
    type_registry: &TypeRegistry,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    // Cure B (Decision 10): the conversion-source lineages (`m.keys()` /
    // `s.split()` / `@to_list` SOURCES borrowed by the conversion, dying at a
    // post-loop dead-block-param sink) — computed here where `pool` is available,
    // passed into the fresh-inc-elision so they take the phi-threaded per-path
    // alloc-aware net (their surplus fresh-site inc is mis-attributed across the
    // Jump-arg→block-param SSA rename when unthreaded → left unbalanced → leak).
    let conversion_source_reps =
        compute_conversion_source_reps(func, pool, same_alloc_reps, interner);
    let elidable_fresh_incs = compute_elidable_fresh_self_alloc_incs(
        func,
        same_alloc_reps,
        interner,
        contracts,
        &conversion_source_reps,
    );
    lower_burden_ops_to_rc(func, pool, type_registry, &elidable_fresh_incs);
    trace_phase_snapshot("after_phase_7_burden_lowering", func, interner);

    // Phase-7 alias-lineage RC-net diagnostic (ORI_LOG=ori_arc::aims::realize):
    // per same-alloc rep, `fresh-alloc(+1) + RcInc − RcDec`. A non-zero net is a
    // leak (+N) / double-free (−N) the per-VAR elim cannot see across distinct
    // SSA vars of one allocation. Spec: Annex E §AIMS RL-2 (release once/lineage).
    if tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
        let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
        let list_take_name = for_yield_result_finalizer_name(interner);
        let mut rep_net: FxHashMap<ArcVarId, i64> = FxHashMap::default();
        for block in &func.blocks {
            for instr in &block.body {
                if let Some(dst) = fresh_rc_alloc_dst(instr, func, interner, list_take_name) {
                    *rep_net.entry(rep_of(dst)).or_default() += 1;
                }
                match instr {
                    ArcInstr::RcInc { var, .. } => {
                        *rep_net.entry(rep_of(*var)).or_default() += 1;
                    }
                    ArcInstr::RcDec { var, .. } => {
                        *rep_net.entry(rep_of(*var)).or_default() -= 1;
                    }
                    _ => {}
                }
            }
            // `Invoke`-terminator fresh self-alloc result also supplies a lineage
            // alloc(+1) — count it so the imbalance probe sees the full lineage.
            if let Some((dst, _)) = fresh_rc_alloc_dst_terminator(&block.terminator, func, interner)
            {
                *rep_net.entry(rep_of(dst)).or_default() += 1;
            }
        }
        let fn_name = interner.lookup(func.name);
        for (rep, net) in &rep_net {
            if *net != 0 {
                tracing::trace!(
                    target: "ori_arc::aims::realize",
                    fn_name = fn_name,
                    rep = rep.raw(),
                    net = *net,
                    "alias-lineage RC-net imbalance post-burden-lowering (+N leak / -N double-free)"
                );
            }
        }
    }
}

/// Step-2 RL-4 edge release for step-1 COW-inc'd borrowed-alias receivers
/// consumed at an `Invoke` / `InvokeIndirect` TERMINATOR owned position.
///
/// A may-unwind COW/iter call (`Invoke @push(recv [own], ...) normal N unwind U`)
/// where `recv` is a borrowed-param alias got a step-1 `BurdenInc` (the COW
/// helper reads `recv`'s refcount; the inc raises it ≥ 2 so the helper COPIES,
/// leaving the caller's value intact / the iterator's drop non-freeing). The
/// inc'd reference then SURVIVES the call into both the normal and unwind
/// successors, where it is dead — RL-4 mandates a freeing `BurdenDec recv` on
/// EACH dying edge (the value is released exactly once per concrete path:
/// `AimsProof.Realization::RL4_edge_release_balanced`).
///
/// Escape gate: relocate the freeing dec to
/// the successor edge ONLY when the call result does NOT alias `recv`'s payload
/// — i.e. `recv` is NOT also a borrowed arg of a SHARING call (`slice` /
/// `substring`) whose result would carry the freed buffer. Step 1's set already
/// excludes sharing methods (they take the receiver BORROWED, never owned), so
/// the COW set is escape-safe by construction; the gate is the structural
/// guarantee, not a runtime filter. The dec is emitted at the TOP of each
/// successor block, before any other instruction, so a successor that re-reads
/// `recv` (e.g. `@main`'s `if r1 == ...` after `Invoke @push`) is unaffected —
/// `recv` is dead at the successor by the edge-dead precondition.
fn emit_cow_inc_terminator_edge_release(
    func: &mut ArcFunction,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    let borrowed_aliases = crate::lower::burden_lower::compute_borrowed_alias_vars(func);
    if borrowed_aliases.is_empty() {
        return;
    }
    let cow_inc = crate::lower::burden_lower::compute_cow_inc_borrowed_aliases(
        func,
        &borrowed_aliases,
        interner,
    );
    if cow_inc.is_empty() {
        return;
    }
    // COW-MUTATOR names (release gate) = COW set MINUS `iter`. Only a COW-mutator
    // `Invoke` releases its receiver on the successor edges — the result is fresh
    // and nothing else holds the original. An `iter` Invoke's receiver inc is
    // balanced by the runtime `ori_iter_drop` (the iterator state owns the held
    // buffer), so it gets NO edge dec here.
    let mut cow_mutators = crate::borrow::all_cow_method_names(interner);
    cow_mutators.remove(&interner.intern("iter"));
    // Same-allocation reps over the receiver's `Let { Var }` alias chain: the
    // borrowed-param root (`%0`), its aliases (`%1 = %0`), and a later re-read
    // (`%9 = %0`) all name ONE allocation. The live-out check operates on the rep
    // so a use through ANY alias keeps the lineage live.
    let recv_reps = same_alloc_reps;
    let rep_of = |v: ArcVarId| recv_reps.get(&v).copied().unwrap_or(v);
    // Reps whose same-alloc root is a BORROWED parameter: the caller retains and
    // drops the allocation. A COW-mutator inc on such a receiver is balanced by
    // the COW helper's own slow-path dec of the copied-from buffer (rc 2 -> 1),
    // leaving the caller's borrowed reference for the caller to drop — the callee
    // emits NO release on a live-across lineage (a callee dec double-frees against
    // the caller's drop).
    let borrowed_param_reps: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| matches!(p.ownership, crate::ownership::Ownership::Borrowed))
        .map(|p| recv_reps.get(&p.var).copied().unwrap_or(p.var))
        .collect();
    let mut edge_releases: Vec<(usize, ArcVarId)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        use crate::ir::ArcTerminator;
        let (normal, unwind, used, callee): (
            crate::ir::ArcBlockId,
            crate::ir::ArcBlockId,
            &ArcTerminator,
            Option<&Name>,
        ) = match &block.terminator {
            ArcTerminator::Invoke {
                normal,
                unwind,
                func: callee,
                ..
            } => (*normal, *unwind, &block.terminator, Some(callee)),
            // InvokeIndirect (closure call) is never a builtin COW mutator —
            // closures dispatch through user code, not `all_cow_method_names`.
            _ => continue,
        };
        let Some(callee) = callee else { continue };
        if !cow_mutators.contains(callee) {
            continue;
        }
        for (pos, &var) in used.used_vars().iter().enumerate() {
            if !(used.is_owned_position(pos) && cow_inc.contains(&var)) {
                continue;
            }
            let rep = rep_of(var);
            // RL-4 `deadAtSucc` precondition: the inc'd receiver is released on a
            // dying successor edge ONLY when it is genuinely dead at that successor.
            // A BORROWED-param-rooted receiver live PAST the call through a
            // same-alloc alias (`@check(list)`'s `list.push(..); list.len()`) is NOT
            // dead at the normal successor — and the CALLER owns + drops the
            // allocation. The step-1 COW-inc is balanced by the helper's own
            // slow-path dec of the copied-from buffer (rc 2 -> 1) —
            // `RL1_duplication_balanced`, the inc and copy-source-dec are the
            // matched pair. The callee emits NO release on ANY edge: a callee
            // edge-dec frees the caller's still-owned reference before the later
            // same-alloc read (UAF) and double-frees against the caller's drop.
            // All other receivers (owned-local, or borrowed-param dead-after-call)
            // keep the unconditional both-edge release.
            if borrowed_param_reps.contains(&rep)
                && lineage_live_out(func, recv_reps, rep, block_idx)
            {
                continue;
            }
            edge_releases.push((normal.index(), var));
            edge_releases.push((unwind.index(), var));
        }
    }
    for (succ_idx, var) in edge_releases {
        if let Some(block) = func.blocks.get_mut(succ_idx) {
            block.body.insert(0, ArcInstr::BurdenDec { var });
        }
    }
}

/// RL-4 relocation: move a conversion-source's scope-exit `BurdenDec`, placed
/// INLINE by the Phase-5 walk in the block whose TERMINATOR is the borrowed
/// conversion `Invoke`, to the normal AND unwind successor edges.
///
/// Shape: `bbK: ... ; burden_dec %m ; Invoke @values(%m [borrow]) normal N
/// unwind U`. The inline `burden_dec %m` lowers (Phase 7) to a `RcDec %m` that
/// runs BEFORE `@values` reads `%m` → the source map/set/str (and its heap
/// element strings, freed via the runtime's `val_inc` reading a now-freed header)
/// is destroyed before the conversion borrows it → use-after-free. The release
/// belongs on each dying successor edge (RL-4 `RL4_edge_release_balanced`): `%m`
/// is dead on both the normal and unwind successors after the borrowed call
/// returns, released exactly once per concrete path.
///
/// Single-borrow scope: relocates ONLY when the call is the arg's SOLE borrow
/// (the value is not live past the call — `lineage_live_out` false). A source
/// borrowed by SEVERAL calls (`m.values(); m.keys()`) requires joint inc/dec
/// accounting across the move-aliases (the Phase-5 walk emits a duplication inc
/// per alias) — out of scope for this dec-placement pass; the `live-out` guard
/// skips it rather than emit a premature free.
///
/// Escape gate (per the callee `MemoryContract` — see [`borrowed_arg_release_verdict`]):
///  - Conversion builtins (`@keys`/`@values`/`@split`/`@to_list`) are escape-safe
///    by construction (the result is a fresh allocation that never aliases the
///    source's payload) and borrow the receiver → `EdgeRelease::Both`.
///  - Non-conversion callees relocate only when `ParamContract` for the receiver
///    proves NO return-view aliasing (`return_alias == None &&
///    !return_payload_contains_param`) — a borrow-and-return-view callee
///    (`unwrap`/`slice`/`take`/`match`-alias) needs the caller dec KEPT, else the
///    relocation frees the returned view's backing. A transfer/iter-consume callee
///    (`access == Owned`, the callee frees on the normal path via `ori_iter_drop`)
///    → `EdgeRelease::UnwindOnly`; a genuine borrow → `EdgeRelease::Both`.
///  - No contract for the callee → keep the inline dec (conservative).
///
/// Per-block gather decision for [`relocate_borrowed_terminator_arg_dec_to_edges`].
/// Returns `(recv, normal_idx, unwind_idx, release)` when `block`'s terminator is
/// an `Invoke` with a borrowed receiver carrying a misplaced inline scope-exit dec
/// the verdict says to relocate; `None` when no relocation applies (every gate
/// that declines maps to `None`). Extracted to keep the driver loop's complexity
/// bounded. Spec: Annex E §AIMS RL-2 + RL-4.
#[allow(
    clippy::too_many_arguments,
    reason = "gather gates read several name sets"
)]
fn borrowed_terminator_arg_relocation_for_block(
    func: &ArcFunction,
    block: &ArcBlock,
    escape_safe_names: &EscapeSafeBorrowedNames,
    sharing_view_names: &FxHashSet<Name>,
    accessor_retain_names: &FxHashSet<Name>,
    contracts: &FxHashMap<Name, MemoryContract>,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    b: usize,
) -> Option<(ArcVarId, usize, usize, EdgeRelease)> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    // The terminator must be an `Invoke` with a BORROWED receiver (arg 0 at a
    // non-owned position) — the value read but not (yet) consumed at the call.
    let ArcTerminator::Invoke {
        dst,
        func: callee,
        args,
        arg_ownership,
        normal,
        unwind,
        ..
    } = &block.terminator
    else {
        return None;
    };
    let &recv = args.first()?;
    let recv_borrowed = arg_ownership
        .first()
        .is_none_or(|o| *o == crate::ir::ArgOwnership::Borrowed);
    if !recv_borrowed {
        return None;
    }
    // Escape-safety + placement gate. `None` keeps the inline dec (return-view-risk
    // callee or no contract).
    let dst_scalar = matches!(func.var_repr(*dst), Some(ValueRepr::Scalar));
    let release =
        borrowed_arg_release_verdict(*callee, 0, dst_scalar, escape_safe_names, contracts)?;
    // The receiver must carry a freeable scope-exit dec the Phase-5 walk misplaced.
    // A heap-pointer collection (`@keys`/`@values`/`@first`/`@get` receivers) is
    // `RcPointer`. An accessor-retain wrapper (`@unwrap`/`@expect` on
    // `Option`/`Result`) is an `Aggregate` / `InlineEnum` whose RC-bearing payload
    // its drop glue releases; a seamless-slice receiver can be a `str` FatValue
    // (`substring`) whose backing buffer carries a freeable dec — widen the repr
    // gate for those classes only. A Scalar receiver has no freeable dec.
    let recv_repr = func.var_repr(recv);
    let recv_nonscalar = !matches!(recv_repr, Some(ValueRepr::Scalar) | None);
    let recv_has_freeable_dec = matches!(recv_repr, Some(ValueRepr::RcPointer))
        || (accessor_retain_names.contains(callee) && recv_nonscalar)
        || (sharing_view_names.contains(callee) && recv_nonscalar)
        // A `survives_transform` clone with a non-scalar AGGREGATE receiver (a
        // derived `@clone` on `Node { value, next: Option<Node> }`): the receiver
        // is an `Aggregate`/`InlineEnum` whose drop glue releases its RC-bearing
        // fields (the boxed recursive `next`). That freeable dec the Phase-5 walk
        // misplaced INLINE before the borrowed clone read — relocating it to the
        // successor edges keeps the boxed field alive across the clone's recursive
        // read (else the inline dec frees the box the clone then re-reads ->
        // use-after-free / double-free). Same non-scalar widening as the
        // accessor-retain / sharing-view classes. Spec: Annex E §AIMS RL-2 + RL-4.
        || (escape_safe_names.survives_transform.contains(callee) && recv_nonscalar);
    if !recv_has_freeable_dec {
        return None;
    }
    // Single-borrow guard: relocate only when the source genuinely dies after this
    // call (not live-out). A source still live past the call needs joint multi-alias
    // accounting — leave its decs as-is rather than free it prematurely.
    if lineage_live_out(func, jt_reps, rep_of(recv), b) {
        return None;
    }
    // Sharing-view scope guards: the receiver-dec relocation balances only the
    // single-read non-branchy / single-read take/drop/substring subset. A RESULT
    // read across MULTIPLE branch blocks (`&&`-branchy `slice_basic`) OR a chained
    // slice (`xs.take(4).drop(1)`) carries its own keep-alive accounting the
    // relocation does NOT model — those stay inline (their own follow-up leaf owns
    // the result keep-alive). Spec: Annex E §AIMS RL-1 + RL-2.
    if release == EdgeRelease::PostDominator {
        if lineage_reader_block_count(func, jt_reps, rep_of(*dst)) > 1 {
            return None;
        }
        if sharing_view_result_feeds_sharing_view(func, jt_reps, sharing_view_names, rep_of(*dst)) {
            return None;
        }
    }
    // Find the INLINE scope-exit `BurdenDec recv` in this block's body — the
    // misplaced release the Phase-5 walk emitted before the terminator.
    if !block
        .body
        .iter()
        .any(|instr| matches!(instr, ArcInstr::BurdenDec { var } if *var == recv))
    {
        return None;
    }
    Some((recv, normal.index(), unwind.index(), release))
}

/// The arg-1 sibling of [`borrowed_terminator_arg_relocation_for_block`] for the
/// 3 named set-algebra ops (`union`/`intersection`/`difference`): the receiver
/// (arg 0) is OWNED/consumed, but `other` (arg 1) is BORROWED and has its
/// surviving elements rc-inc'd into the FRESH result set by the runtime
/// (`inc_copied_set_elements`). The Phase-5 walk misplaces `other`'s container
/// dec INLINE before the may-unwind `@union` terminator that still READS `other`
/// — the inline dec cascade-frees `other`'s elements, the call then re-incs the
/// freed elements into the result (UAF), and the result drop frees them again
/// (double-free). Relocating `other`'s dec to BOTH successor edges releases it
/// AFTER the read on each concrete path (`RL2_release_exactly_once` +
/// `RL4_edge_release_balanced`). Returns `(other, normal, unwind)`; `None`
/// declines. TIGHT: fires only for the 3 named ops, only on a borrowed arg 1
/// that genuinely dies after the call and carries an inline dec. Spec: Annex E
/// §AIMS RL-1 + RL-2 + RL-4.
fn set_algebra_other_arg_relocation_for_block(
    func: &ArcFunction,
    block: &ArcBlock,
    set_algebra_names: &FxHashSet<Name>,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    b: usize,
) -> Option<(ArcVarId, usize, usize)> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let ArcTerminator::Invoke {
        func: callee,
        args,
        arg_ownership,
        normal,
        unwind,
        ..
    } = &block.terminator
    else {
        return None;
    };
    if !set_algebra_names.contains(callee) {
        return None;
    }
    // arg 0 = receiver (consumed/COW); arg 1 = `other` (borrowed, element-retained).
    let &other = args.get(1)?;
    let other_borrowed = arg_ownership
        .get(1)
        .is_none_or(|o| *o == crate::ir::ArgOwnership::Borrowed);
    if !other_borrowed {
        return None;
    }
    // `other` must carry a freeable container dec (an RcPointer Set) — a scalar
    // has none.
    if !matches!(func.var_repr(other), Some(ValueRepr::RcPointer)) {
        return None;
    }
    // Single-borrow guard: relocate only when `other` genuinely dies after this
    // call. A source still live past the call needs joint multi-alias accounting.
    if lineage_live_out(func, jt_reps, rep_of(other), b) {
        return None;
    }
    // The INLINE scope-exit `BurdenDec other` the Phase-5 walk emitted before the
    // terminator must exist (else nothing to relocate).
    if !block
        .body
        .iter()
        .any(|instr| matches!(instr, ArcInstr::BurdenDec { var } if *var == other))
    {
        return None;
    }
    Some((other, normal.index(), unwind.index()))
}

/// Probe-only effect: the relocation runs in the predicate-stack-disabled probe
/// tail. On the default path the predicate stack co-emits the arg's release on
/// the successor edge already, and this pass never runs — default codegen stays
/// byte-identical. Spec: Annex E §AIMS RL-2 + RL-4.
fn relocate_borrowed_terminator_arg_dec_to_edges(
    func: &mut ArcFunction,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    let conversion_names = collection_conversion_names(interner);
    let survives_transform_names = borrow_survives_transform_names(interner);
    let accessor_retain_names = crate::borrow::accessor_retain_builtin_names(interner);
    let sharing_view_names = sharing_view_relocation_names(interner);
    let fresh_str_names = fresh_str_producing_method_names(interner);
    let set_algebra_names = set_algebra_relocation_names(interner);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let escape_safe_names = EscapeSafeBorrowedNames {
        conversion: &conversion_names,
        survives_transform: &survives_transform_names,
        accessor_retain: &accessor_retain_names,
        sharing_view: &sharing_view_names,
        fresh_str: &fresh_str_names,
        set_algebra: &set_algebra_names,
        builtins: &builtins,
    };
    let post_doms = crate::graph::PostDominatorTree::build(func);
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // (block_idx, var, normal, unwind, release) per relocation. Gather first
    // (immutable borrow), then mutate.
    let mut relocations: Vec<(usize, ArcVarId, usize, usize, EdgeRelease)> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        if let Some((recv, normal, unwind, release)) = borrowed_terminator_arg_relocation_for_block(
            func,
            block,
            &escape_safe_names,
            &sharing_view_names,
            &accessor_retain_names,
            contracts,
            &jt_reps,
            b,
        ) {
            relocations.push((b, recv, normal, unwind, release));
        }
        // Set-algebra `other` (arg 1) sibling: a borrowed non-receiver arg whose
        // surviving elements are rc-inc'd into the fresh result; its premature
        // inline dec relocates to BOTH edges (the receiver relocation's arg-1
        // analogue, scoped to the 3 named set-algebra ops). Distinct from the
        // arg-0 case above — `union`'s arg 0 is OWNED, so the arg-0 path declines.
        if let Some((other, normal, unwind)) =
            set_algebra_other_arg_relocation_for_block(func, block, &set_algebra_names, &jt_reps, b)
        {
            relocations.push((b, other, normal, unwind, EdgeRelease::Both));
        }
    }
    // Remove the inline dec from each call block, then prepend the release to the
    // successor edge(s) the verdict selected (RL-4: released exactly once per
    // concrete path; the value is dead on each selected edge after the call). For a
    // `Suppress` (iter-consume full transfer) ALSO remove a paired inline `BurdenInc`
    // of the same var in the call block — a net-0 merge-carry inc/dec pair left with
    // only its inc would net +1 on the normal path (leak); the callee frees on both
    // exits, so the var carries NO caller burden ops.
    for &(b, var, _, _, release) in &relocations {
        if let Some(block) = func.blocks.get_mut(b) {
            if let Some(dec_idx) = block
                .body
                .iter()
                .position(|instr| matches!(instr, ArcInstr::BurdenDec { var: v } if *v == var))
            {
                block.body.remove(dec_idx);
            }
            if release == EdgeRelease::Suppress {
                if let Some(inc_idx) = block
                    .body
                    .iter()
                    .position(|instr| matches!(instr, ArcInstr::BurdenInc { var: v } if *v == var))
                {
                    block.body.remove(inc_idx);
                }
            }
        }
    }
    for &(_, var, normal, unwind, release) in &relocations {
        match release {
            // Callee borrows-and-returns → release on both successor edges.
            EdgeRelease::Both => {
                if let Some(succ) = func.blocks.get_mut(normal) {
                    succ.body.insert(0, ArcInstr::BurdenDec { var });
                }
                if normal != unwind {
                    if let Some(succ) = func.blocks.get_mut(unwind) {
                        succ.body.insert(0, ArcInstr::BurdenDec { var });
                    }
                }
            }
            // Owned-transfer callee frees on normal → caller releases on unwind only.
            EdgeRelease::UnwindOnly => {
                if normal != unwind {
                    if let Some(succ) = func.blocks.get_mut(unwind) {
                        succ.body.insert(0, ArcInstr::BurdenDec { var });
                    }
                }
            }
            // Iter-consume callee frees on EVERY exit → caller emits no dec on either
            // edge (inline dec + paired inc already removed above).
            EdgeRelease::Suppress => {}
            // Seamless-slice receiver: ONE dec at the post-dominating dead block of
            // the normal successor (where the receiver is dead on all forward paths
            // and the shared buffer is still live for result reads), PLUS the unwind
            // edge (the slice did not inc on the panic path, so the receiver still
            // holds rc 1 there). The normal-side placement is post-dominating
            // (single dec per path) vs `Both`'s normal-successor-front (which
            // double-counts when the result is read across branches downstream).
            EdgeRelease::PostDominator => {
                let pd =
                    post_dominating_dead_block(func, &post_doms, &jt_reps, rep_of(var), normal);
                if let Some(succ) = func.blocks.get_mut(pd) {
                    succ.body.insert(0, ArcInstr::BurdenDec { var });
                }
                if normal != unwind && pd != unwind {
                    if let Some(succ) = func.blocks.get_mut(unwind) {
                        succ.body.insert(0, ArcInstr::BurdenDec { var });
                    }
                }
            }
        }
    }
}

/// Count the DISTINCT blocks that reference lineage `rep` (as a block param, an
/// instruction operand, or a terminator arg). Used by the sharing-view
/// relocation to distinguish a single-read slice result (`slice_then_length`:
/// read in one block) from a branchy-multi-read result (`slice_basic`: the
/// `&&`-short-circuit reads the result across several branch blocks). A
/// `BurdenInc`/`BurdenDec`/`RcInc`/`RcDec` of the lineage does NOT count as a read
/// (it is RC bookkeeping, not a use). Spec: Annex E §AIMS RL-1.
fn lineage_reader_block_count(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
) -> usize {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let mut count = 0;
    for block in &func.blocks {
        let param_ref = block.params.iter().any(|&(p, _)| rep_of(p) == rep);
        let body_ref = block.body.iter().any(|i| {
            // Exclude pure RC bookkeeping ops — they are not reads of the value.
            if matches!(
                i,
                ArcInstr::BurdenInc { .. }
                    | ArcInstr::BurdenDec { .. }
                    | ArcInstr::RcInc { .. }
                    | ArcInstr::RcDec { .. }
            ) {
                return false;
            }
            i.used_vars().iter().any(|&v| rep_of(v) == rep)
        });
        let term_ref = block
            .terminator
            .used_vars()
            .iter()
            .any(|&v| rep_of(v) == rep);
        if param_ref || body_ref || term_ref {
            count += 1;
        }
    }
    count
}

/// Whether lineage `rep` (a slice RESULT) is consumed as the RECEIVER (arg 0) of
/// another sharing-view producer anywhere in the function — the chained-slice
/// shape (`xs.take(4).drop(1)`: the `take` result feeds `drop`'s receiver). The
/// sharing-view single-relocation declines such chains; the intermediate slice result is
/// both a result and a receiver, needing joint chain accounting. Spec: Annex E
/// §AIMS RL-2.
fn sharing_view_result_feeds_sharing_view(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    sharing_names: &FxHashSet<Name>,
    rep: ArcVarId,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let feeds = |callee: &Name, args: &[ArcVarId]| -> bool {
        sharing_names.contains(callee) && args.first().is_some_and(|&a| rep_of(a) == rep)
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if feeds(callee, args) {
                    return true;
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if feeds(callee, args) {
                return true;
            }
        }
    }
    false
}

/// The post-dominating block of `start` (the slice's normal successor) where the
/// receiver lineage `rep` is DEAD on entry — the single block every forward path
/// from `start` passes through where the receiver no longer lives. For the
/// receiver-dead-after-slice subset the receiver is already dead at `start`'s
/// entry, so this returns `start`; the post-dominator walk generalizes the search
/// for shapes where `start` itself still references the receiver (none in the
/// covered subset, but the walk keeps the placement provably post-dominating).
/// Used by the `EdgeRelease::PostDominator` sharing-view placement. Spec: Annex E §AIMS RL-4.
fn post_dominating_dead_block(
    func: &ArcFunction,
    post_doms: &crate::graph::PostDominatorTree,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    start: usize,
) -> usize {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let lineage_referenced_in = |b: usize| -> bool {
        let Some(block) = func.blocks.get(b) else {
            return false;
        };
        block.params.iter().any(|&(p, _)| rep_of(p) == rep)
            || block
                .body
                .iter()
                .any(|i| i.used_vars().iter().any(|&v| rep_of(v) == rep))
            || block
                .terminator
                .used_vars()
                .iter()
                .any(|&v| rep_of(v) == rep)
    };
    // Walk the post-dominator chain from `start` upward until the block does not
    // reference the receiver lineage (dead there) AND `start` is dead-out-of it.
    // The dead-after-slice guard (`lineage_live_out` at the call site) already
    // proved `start` is dead on every forward path, so `start` itself qualifies
    // unless it directly references the receiver (a re-borrow inside the same
    // block) — then advance to its immediate post-dominator.
    let mut current = start;
    let mut steps = 0;
    while lineage_referenced_in(current) && steps < func.blocks.len() {
        let cur_id = crate::ir::ArcBlockId::new(u32::try_from(current).unwrap_or(u32::MAX));
        match post_doms.immediate_post_dominator(cur_id) {
            Some(n) => current = n.index(),
            None => break,
        }
        steps += 1;
    }
    current
}

/// Edge-placement verdict for a relocated borrowed-terminator-Invoke arg dec.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EdgeRelease {
    /// Callee borrows-and-returns (conversion / builtin borrowing read like `@len`):
    /// the arg survives the normal return and is dead on each successor → release on
    /// BOTH the normal and unwind edges.
    Both,
    /// Callee TRANSFERS the arg on the NORMAL path only (an `ownParamsUsingArgs`
    /// transfer — `ParamContract.access == Owned`): the callee owns/frees it on
    /// normal return, but on the UNWIND path may not have reached the consume yet →
    /// the caller releases only on the UNWIND edge.
    UnwindOnly,
    /// Callee ITER-CONSUMES the arg, freeing it on EVERY exit — normal AND unwind
    /// (`for x in coll` → `@iter [own]` → `ori_iter_drop` on both the callee's
    /// normal-return and its panic-unwind cleanup pad). The full inward transfer
    /// (RL2 `RL2_iter_consuming_no_caller_dec`) means the caller emits NO dec on
    /// EITHER edge: SUPPRESS the inline dec AND its paired inline inc (a net-0
    /// merge-carry pair left intact would otherwise net +1 on the normal path →
    /// leak), inserting on no edge. A unwind-edge dec would double-free against the
    /// callee's own unwind iter-drop.
    Suppress,
    /// Seamless-slice producer (`slice`/`substring`/`take`/`drop`) whose result
    /// SHARES the receiver buffer: the receiver survives the borrowed read but is
    /// dead on every forward path AFTER the slice. Place ONE `BurdenDec recv` at
    /// the FRONT of the post-dominating dead block of the slice's NORMAL successor
    /// — the single block every forward path from the slice passes through where
    /// the receiver is dead. Distinct from `Both` (which decs on the normal AND
    /// unwind edges): when the shared-buffer result is read across `&&` branches,
    /// `Both` double/under-counts against the result's per-branch decs (the
    /// over-fire). One post-dominating dec is released exactly once per path (RL-2
    /// `RL2_release_exactly_once`) and never frees the buffer before a result read
    /// (the post-dominator follows all reads). RL-4 `RL4_edge_release_balanced`.
    PostDominator,
}

/// The escape-safe-by-name builtin-method sets a borrowed-terminator-arg
/// relocation consults. Bundled into one parameter object because every consumer
/// constructs + reads all four together (they co-vary at every site). Each set
/// names callees whose result holds its own buffer ref, so a borrowed receiver
/// survives the call and its dec relocates to the successor edges.
struct EscapeSafeBorrowedNames<'a> {
    /// Collection-conversion builtins (`keys`/`values`/`split`/`to_list`) — fresh
    /// result, map -> list shape change.
    conversion: &'a FxHashSet<Name>,
    /// Borrow-survives transforms (`filter`/`map`/`clone`) — fresh `[T]` or rc-inc
    /// alias, result holds its own ref (see `borrow_survives_transform_names`).
    survives_transform: &'a FxHashSet<Name>,
    /// Accessor-retaining builtins (`unwrap`/`first`/`last`/`get`/`expect`) — the
    /// codegen retains the extracted payload (fresh owned ref, never an aliasing
    /// view).
    accessor_retain: &'a FxHashSet<Name>,
    /// Seamless-slice producers (`slice`/`substring`/`take`/`drop`) — the result
    /// SHARES the receiver's backing buffer (`SLICE_FLAG` cap) and rc-incs it. The
    /// receiver survives the call but its dec needs SINGLE post-dominating-edge
    /// placement (`PostDominator`), not `Both`, because the shared-buffer result
    /// is read across branches (see `sharing_view_relocation_names`).
    sharing_view: &'a FxHashSet<Name>,
    /// Fresh-str producers (`debug`/`to_str`) — the result is a FRESH owned `str`
    /// the callee synthesised, NEVER a view aliasing the receiver's buffer (the
    /// str analogue of `conversion`). The receiver survives the borrowed read and
    /// is released on BOTH successor edges (see `fresh_str_producing_method_names`).
    fresh_str: &'a FxHashSet<Name>,
    /// Set-algebra ops (`union`/`intersection`/`difference`) — the borrowed
    /// `other` arg (arg 1; the receiver is OWNED/consumed) has its surviving
    /// elements rc-inc'd into a FRESH result set (`inc_copied_set_elements`),
    /// never aliased uninc'd. `other` survives the borrowed read, so its dec
    /// relocates to BOTH successor edges (see `set_algebra_relocation_names`).
    set_algebra: &'a FxHashSet<Name>,
    /// All builtin methods (for the scalar-result borrowing-read fallback).
    builtins: &'a crate::borrow::BuiltinOwnershipSets,
}

/// Escape-safety + placement verdict for `recv` (an RC collection) passed at a
/// borrowed terminator-Invoke `arg_index` position to `callee`, whose result is
/// `dst_scalar`. `None` = NOT escape-safe (keep the inline dec): the call may
/// return a view aliasing `recv`'s payload.
///
/// Escape-safety is established by EITHER: (a) `dst_scalar` — a scalar result
/// (`int`/`bool`) cannot hold a heap reference into `recv` (covers iter-consume
/// user fns `@sum_values`/`@count_items` returning `int`, AND builtin reads `@len`
/// returning `int` — the `match coll.first() { Some(s) -> s }` element-view case
/// the contract's `return_alias` MISSES is excluded here because its result is a
/// heap `str`, NOT scalar); OR (b) a collection-conversion builtin (structurally
/// copies elements into a fresh result). A non-scalar-result non-conversion callee
/// is conservatively NOT relocated (the result may alias `recv`).
///
/// Placement: a builtin borrowing read (`@len`, conversions) lets `recv` survive
/// the call → `Both`; a user-fn transfer (`recv` lowers to `[own]` at Phase 7, OR
/// the contract upgraded access to Owned) frees on normal → `UnwindOnly`.
fn borrowed_arg_release_verdict(
    callee: Name,
    arg_index: usize,
    dst_scalar: bool,
    names: &EscapeSafeBorrowedNames,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Option<EdgeRelease> {
    // Collection-conversion builtins are escape-safe by construction (the result
    // is a fresh allocation that never aliases the source's payload) and borrow
    // (not consume) the receiver → survives the call → Both edges.
    if names.conversion.contains(&callee) {
        return Some(EdgeRelease::Both);
    }
    // Borrow-survives transforms (`filter`/`map` fresh result + `clone` rc-inc
    // alias): borrow the receiver; the result holds its OWN buffer ref, so the
    // receiver survives the borrowed call, dead on each successor → Both edges.
    // Same escape-safe class as conversions; checked BEFORE the scalar gate since
    // the result is a non-scalar `[T]` whose own ref keeps the buffer alive past
    // the relocated source dec (Spec: Annex E §AIMS RL-2 + RL-4).
    if names.survives_transform.contains(&callee) {
        return Some(EdgeRelease::Both);
    }
    // Accessor-retaining builtins (`@unwrap`/`@first`/`@last`/`@get`/`@expect`):
    // the codegen RETAINS the extracted payload (`inc_value_rc`), so the result is
    // a FRESH owned reference, never a view aliasing `recv` (contrast `slice`/
    // `substring`, which share backing — NOT in this set). The retained payload
    // raises its own rc, so freeing `recv` on the successor edge cannot destroy the
    // escaped payload — the borrowed receiver survives the call and is released on
    // BOTH edges. Checked BEFORE the scalar gate: the accessor result is non-scalar
    // (`str`/`[T]`/`Option<T>`) yet provably non-aliasing via the retain
    // (Spec: Annex E §AIMS RL-2 / RL-4).
    if names.accessor_retain.contains(&callee) {
        return Some(EdgeRelease::Both);
    }
    // Seamless-slice producers (`slice`/`substring`/`take`/`drop`): the result
    // SHARES the receiver's backing buffer and rc-INCs it (rc 1 -> 2). The
    // receiver survives the borrowed read (the slice incs before the receiver
    // dies), so its dec relocates off the inline pre-read site — but to ONE
    // post-dominating edge, NOT both. The `Both`-edge placement double/under-counts
    // against the shared-buffer result's per-branch decs when the result is read
    // across `&&` branches (the sharing-view over-fire). `PostDominator` places one dec at
    // the post-dominating dead block of the slice's normal successor, where the
    // receiver is dead on every forward path AND the shared buffer is still live
    // for the result reads. Checked BEFORE the scalar gate: the slice result is a
    // non-scalar buffer-sharing view (Spec: Annex E §AIMS RL-2 / RL-4).
    if names.sharing_view.contains(&callee) {
        return Some(EdgeRelease::PostDominator);
    }
    // Fresh-str producers (`debug`/`to_str`): the result is a FRESH owned `str`
    // the callee synthesised, NEVER a view aliasing `recv`'s payload (contrast
    // `slice`/`substring`, which share backing — in `sharing_view`). The receiver
    // is borrow-read (debug/to_str copies, never consumes), so it survives the
    // call and is dead on each successor → Both edges. Checked BEFORE the scalar
    // gate: the result is a non-scalar `str` yet provably non-aliasing (the str
    // analogue of the conversion class). Spec: Annex E §AIMS RL-2 + RL-4.
    if names.fresh_str.contains(&callee) {
        return Some(EdgeRelease::Both);
    }
    // Set-algebra ops (`union`/`intersection`/`difference`): the borrowed `other`
    // arg's surviving elements are rc-inc'd into a FRESH result set by the runtime
    // (`inc_copied_set_elements`), so `other` survives the borrowed read and is
    // dead on each successor → Both edges. Checked BEFORE the scalar gate: the
    // result is a non-scalar `{T}` Set yet provably non-aliasing via the
    // element-retain inc (the Set analogue of the conversion class). Spec: Annex E
    // §AIMS RL-1 + RL-2 + RL-4.
    if names.set_algebra.contains(&callee) {
        return Some(EdgeRelease::Both);
    }
    // Escape-safety: a non-scalar result MAY alias `recv` (slice/substring return a
    // view). Only a scalar result is provably non-aliasing here.
    if !dst_scalar {
        return None;
    }
    // A BUILTIN borrowing read with a scalar result (`@len`): `recv` survives the
    // call and is dead on each successor → Both edges. No contract needed.
    if names.builtins.is_builtin(callee) {
        return Some(EdgeRelease::Both);
    }
    // A user fn requires a contract for the arg.
    let param = contracts
        .get(&callee)
        .and_then(|c| c.params.get(arg_index))?;
    // Return-view aliasing → the caller dec is load-bearing (the result threads the
    // arg). Keep inline (defensive — should not co-occur with a scalar result).
    if param.return_alias.is_some() || param.return_payload_contains_param {
        return None;
    }
    // CASE (b): the iter-consume callee (`@sum_values`: `for x in coll do` →
    // `@iter [own]` → `ori_iter_drop` frees the collection INSIDE the callee on
    // EVERY exit, normal AND unwind) is a FULL inward transfer → the caller emits
    // NO dec on EITHER edge (`Suppress`). An iter-consumer's `ori_iter_drop` runs
    // on its unwind cleanup pad too, so a caller unwind-edge dec would double-free
    // — `Suppress` (not `UnwindOnly`) is the iter-consume verdict. The borrow-read
    // case (`@sum_list`: `xs.fold(..)` BORROWS, does NOT free) presents an IDENTICAL
    // contract on every other dimension; `ParamContract.iter_consumes` is the sole
    // discriminator (`AimsProof.Realization::RL2_iter_consuming_caller_dec_splits`).
    // Checked BEFORE the Owned-transfer branch: an iter-consuming callee freeing on
    // both exits takes the both-exits-free `Suppress`, never the normal-only
    // `UnwindOnly`.
    if param.iter_consumes {
        return Some(EdgeRelease::Suppress);
    }
    // A contract that upgraded the arg to Owned (a true consume at an owned
    // position — the Lean `ownParamsUsingArgs` transfer) frees on the normal path
    // → UnwindOnly.
    if param.access == AccessClass::Owned {
        return Some(EdgeRelease::UnwindOnly);
    }
    // A Borrowed-access non-iter-consuming scalar-returning user fn is NOT
    // relocated (keep the inline dec — the borrow-read case).
    None
}

/// An iter-consuming use of a collection lineage at a call site: `(block, instr)`
/// — `instr = Some(i)` for a body `Apply` at instruction `i`; `instr = None` for
/// a terminator `Invoke`/`InvokeIndirect`.
#[derive(Clone, Copy)]
struct IterConsumeUse {
    block: usize,
    instr: Option<usize>,
}

/// Phase 6.66: rewrite a MULTI-BORROW iter-consume source lineage's normal-path
/// burden ops to the proven oracle ledger — (N-1) keep-alive `BurdenInc` + zero
/// normal-path source `BurdenDec` (per `RL1_emit_iff_not_elidable` +
/// `RL2_iter_consuming_no_caller_dec` + `RL2_release_exactly_once`).
///
/// A source lineage qualifies when ALL hold:
///  - its representative is an `RcPtr` collection (the source buffer);
///  - it flows to N >= 2 iter-consuming call positions on distinct call sites —
///    the multi-borrow case. Two iter-consume kinds count (see
///    [`record_iter_consume_uses`]): a USER callee whose `ParamContract.iter_consumes`
///    is true, AND the INLINE for-loop `@iter(arg [own])` protocol builtin (`for x
///    in coll` -> `@iter [own]` -> `ori_iter_drop`);
///  - it is live-out at the first such call (the 6.65 single-borrow relocation
///    already covers the dead-after-first-call case via its `Suppress` verdict;
///    this pass is its live-out complement).
///
/// SCOPE GUARD (the over-fire boundary): a use is counted as iter-consuming ONLY
/// via the two directly-detected signals — `ParamContract.iter_consumes` (user
/// callee) or `@iter` with an `[own]` arg (inline for-loop). A borrow-read callee
/// (`xs.fold(..)`, `iter_consumes == false`) and a non-`[own]` `@iter` arg are
/// NOT counted, so their caller dec is NEVER suppressed (the borrow-read over-fire
/// trap). Forwarding wrappers whose `iter_consumes` is not directly detected are
/// out of scope here (left to the base path) rather than risk an `access == Owned`
/// over-fire.
///
/// UNWIND SCOPE: only NORMAL-path lineage decs are removed; a dec in an
/// unwind/`Resume` cleanup block is the panic-path release of the keep-alive inc
/// and is left intact (matching the default-path oracle's unwind-edge decs).
/// Bucket every iter-consuming use of an `RcPtr`-collection lineage by its
/// jump-threaded rep (the scan half of
/// [`suppress_multi_borrow_iter_consume_source_decs`]). Two iter-consume kinds
/// count per [`record_iter_consume_uses`]: a USER callee whose
/// `ParamContract.iter_consumes` is true, AND the inline for-loop `@iter(coll
/// [own])` protocol builtin. Body `Apply` and terminator `Invoke` positions are
/// scanned; the returned positions are valid against the UN-mutated body (the
/// caller resolves keep-alive args before any `retain`).
fn collect_iter_consume_uses_per_rep(
    func: &ArcFunction,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
    iter_name: Name,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> FxHashMap<ArcVarId, Vec<IterConsumeUse>> {
    let mut uses_per_rep: FxHashMap<ArcVarId, Vec<IterConsumeUse>> = FxHashMap::default();
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                record_iter_consume_uses(
                    *callee,
                    args,
                    arg_ownership,
                    iter_name,
                    b,
                    Some(i),
                    contracts,
                    func,
                    pool,
                    rep_of,
                    &mut uses_per_rep,
                );
            }
        }
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            record_iter_consume_uses(
                *callee,
                args,
                arg_ownership,
                iter_name,
                b,
                None,
                contracts,
                func,
                pool,
                rep_of,
                &mut uses_per_rep,
            );
        }
    }
    uses_per_rep
}

fn suppress_multi_borrow_iter_consume_source_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let iter_name = interner.intern("iter");

    let uses_per_rep = collect_iter_consume_uses_per_rep(func, pool, contracts, iter_name, &rep_of);

    // Determine which blocks are unwind/Resume cleanup (excluded from dec removal).
    let unwind_blocks = compute_unwind_reachable_blocks(func);

    // Two-phase to keep instruction indices valid: the `retain` below removes
    // `BurdenInc`/`BurdenDec` (shifting later indices), so the keep-alive ARG is
    // resolved here against the UN-mutated body, and the keep-alive INSERTION
    // re-locates each consuming call by its arg AFTER the retain (re-location, not
    // a stale index). Resolving args + reps to remove BEFORE any mutation also
    // fixes the cross-rep hazard where rep A's retain invalidated rep B's indices.
    let mut reps_to_strip: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut keepalives: Vec<KeepAlive> = Vec::new();
    for (rep, mut uses) in uses_per_rep {
        if uses.len() < 2 {
            continue;
        }
        // Multi-borrow case only: the source must SURVIVE the first call (the
        // single-borrow dead-after-call case is the 6.65 relocation's job).
        // Order uses by (block, instr) to find the "first" call.
        uses.sort_by_key(|u| (u.block, u.instr.unwrap_or(usize::MAX)));
        let first = uses[0];
        if !lineage_live_out_after_use(func, &jt_reps, rep, first.block, first.instr) {
            continue;
        }
        reps_to_strip.insert(rep);
        // (N-1) keep-alive incs: one before each non-last iter-consuming call's
        // lineage value. The Nth (last) call's iter-drop is the single release
        // (RL-2); the source allocation rc=1 covers it, so no inc before the last
        // call. Resolve the arg now (pre-retain), re-locate the call post-retain.
        let n = uses.len();
        for u in uses.iter().take(n - 1) {
            if let Some(arg) = lineage_arg_at_use(func, &jt_reps, rep, *u) {
                keepalives.push(KeepAlive {
                    block: u.block,
                    is_terminator: u.instr.is_none(),
                    arg,
                });
            }
        }
    }
    if reps_to_strip.is_empty() {
        return;
    }
    // Remove every NORMAL-path `BurdenInc` / `BurdenDec` on every stripped
    // lineage's SSA-alias members. The keep-alive incs are re-emitted below from
    // the call structure; normal-path source decs are dropped (the callees free).
    for (b, block) in func.blocks.iter_mut().enumerate() {
        if unwind_blocks.contains(&b) {
            continue;
        }
        block.body.retain(|instr| match instr {
            ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } => {
                !reps_to_strip.contains(&rep_of(*var))
            }
            _ => true,
        });
    }
    // Insert each keep-alive immediately before the consuming call of its arg,
    // re-located in the post-retain body (the iter-consume calls are never
    // removed by the retain — only burden ops are). A terminator-position
    // iter-consume appends the inc at end of body (before the terminator reads it).
    for ka in keepalives {
        let Some(block) = func.blocks.get_mut(ka.block) else {
            continue;
        };
        let inc = ArcInstr::BurdenInc { var: ka.arg };
        if ka.is_terminator {
            block.body.push(inc);
            continue;
        }
        let pos = block
            .body
            .iter()
            .position(|instr| iter_consume_call_uses_arg(instr, ka.arg, iter_name, contracts));
        match pos {
            Some(i) => block.body.insert(i, inc),
            None => block.body.push(inc),
        }
    }
}

/// A resolved keep-alive `BurdenInc` for
/// [`suppress_multi_borrow_iter_consume_source_decs`]: the lineage `arg` the
/// consuming call receives, the `block` carrying that call, and whether the call
/// is the block's terminator (`is_terminator` → append at end of body).
struct KeepAlive {
    block: usize,
    is_terminator: bool,
    arg: ArcVarId,
}

/// True iff `instr` is a body iter-consume call (`@iter(arg [own])` or a USER
/// callee whose `ParamContract.iter_consumes` is true) that consumes `arg`.
/// Re-location predicate for [`suppress_multi_borrow_iter_consume_source_decs`]:
/// the consuming calls survive the burden-op `retain`, so the keep-alive is
/// re-found by its arg rather than by a pre-retain index. Mirrors the SSOT
/// iter-consume detection in [`record_iter_consume_uses`] / [`arg_pos_iter_consumes`]
/// (AIMS Invariant 5 — no parallel iter-consume tracker).
fn iter_consume_call_uses_arg(
    instr: &ArcInstr,
    arg: ArcVarId,
    iter_name: Name,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    let ArcInstr::Apply {
        func: callee,
        args,
        arg_ownership,
        ..
    } = instr
    else {
        return false;
    };
    args.iter().enumerate().any(|(pos, &a)| {
        a == arg && arg_pos_iter_consumes(*callee, arg_ownership, pos, iter_name, contracts)
    })
}

/// Phase 6.66b — SINGLE iter-consume + non-iter REUSE keep-alive (RL-1
/// duplication + RL-2 iter-consume transfer).
///
/// The complement of Phase 6.66 (`suppress_multi_borrow_iter_consume_source_decs`,
/// which handles N >= 2 iter-consume uses) and Phase 6.65
/// (`relocate_borrowed_terminator_arg_dec_to_edges`, which handles the
/// dead-after-the-single-call case). THIS pass handles a lineage with EXACTLY
/// ONE iter-consume use whose lineage is live-out at a NON-iter-consume position
/// AFTER that use — the iter-consume-of-a-reused-borrow shape:
///
/// ```text
/// %3 = %0                       // borrowed [str] param, RcPtr
/// %4 = @iter(%3 [own])          // iterator [own]-takes %3's ref
/// ...                           // for-loop body
/// @ori_iter_drop(%4 [own])      // runtime decs the iter-source buffer
/// %23 = %0                      // REUSE — a NON-iter use of the same lineage
/// %26 = @__index(%23 [borrow])  // reads element 0 of the (now-freed) buffer
/// ```
///
/// Without a keep-alive the iter's `ori_iter_drop` decs the (caller-transferred)
/// buffer ref while the reuse still needs it, and the reuse reads / decs a freed
/// buffer (double-free / UAF). The `@iter` use is a DUPLICATING use (RL-1): emit a
/// keep-alive `BurdenInc(arg)` before the `@iter` so `ori_iter_drop` decs the
/// keep-alive copy, leaving the caller-transferred ref alive for the reuse, then a
/// paired `BurdenDec(arg)` at the lineage's LAST non-iter use (after the reuse)
/// releases it. The `[inc, dec]` pair nets 0 (`RL1_duplication_balanced`) — the
/// caller's owned-arg accounting is UNCHANGED (the `ori_iter_drop` still consumes
/// the transferred ref exactly as in the no-reuse case); the pair only bridges the
/// buffer's life across the iter-drop so the reuse reads a live buffer.
///
/// Both ops lower normally in Phase 7 (`BurdenInc -> RcInc`, `BurdenDec -> RcDec`):
/// the keep-alive `RcInc` gives the iterator its own ref; the last-use `RcDec`
/// releases the duplicate after the reuse. They mark `arg` in `func.burden_emitted`
/// so VF-1's per-var balance check (`Σ BurdenInc(arg) - Σ BurdenDec(arg) == 0`)
/// sees the paired ops and is satisfied.
///
/// SCOPE GUARD (over-fire boundary): fires ONLY when the lineage rep has EXACTLY
/// ONE iter-consume use AND is live-out at a non-iter position after it. A
/// no-reuse single-call canary (`for w in words; total` — no reuse) declines via
/// `lineage_live_out_after_use == false`. A multi-iter-consume canary
/// (`borrowed_str_list_two_calls`) has 2 uses and is handled by Phase 6.66 (this
/// pass requires == 1). The keep-alive arg lineage MUST be a genuine borrow-view
/// (no real source dec exists), so a fresh-owned lineage with its own decs is out
/// of scope. Spec: Annex E §AIMS RL-1 + RL-2.
/// One resolved Phase-6.66b keep-alive + paired-dec site: the keep-alive
/// `BurdenInc` at `(inc_block, inc_at)` (before the `@iter`) and the paired
/// `BurdenDec` at `(dec_block, dec_at)` (after the lineage's last non-iter use).
/// Resolved BEFORE any block mutation (insertions shift later indices).
struct KeepaliveSite {
    inc_arg: ArcVarId,
    inc_block: usize,
    inc_at: usize,
    dec_arg: ArcVarId,
    dec_block: usize,
    dec_at: usize,
}

/// Retarget the Phase-6.66b keep-alive paired dec onto the non-param `inc_arg`
/// alias when `dec_arg` is a borrowed param — avoids `RcDec on borrowed param`
/// (VF-1 `check_no_dec_on_borrowed`) while keeping the `[inc, dec]` pair balanced
/// on the same allocation at the same site. Spec: Annex E §AIMS RL-2.
pub(super) fn retarget_borrowed_keepalive_dec(
    dec_arg: ArcVarId,
    inc_arg: ArcVarId,
    borrowed_param_vars: &FxHashSet<ArcVarId>,
) -> ArcVarId {
    if !*BORROWED_ITER_CONSUME_KEEPALIVE_DECLINE_DISABLED
        && borrowed_param_vars.contains(&dec_arg)
        && !borrowed_param_vars.contains(&inc_arg)
    {
        inc_arg
    } else {
        dec_arg
    }
}

/// Resolve the keep-alive + paired-dec [`KeepaliveSite`] for one iter-consume
/// `rep`, or `None` when the shape does not qualify. Spec: Annex E §AIMS RL-1 + RL-2.
#[expect(
    clippy::too_many_arguments,
    reason = "threads the resolved func/jt_reps/contracts context for one site"
)]
fn resolve_keepalive_site(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    uses: &[IterConsumeUse],
    iter_name: Name,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    borrowed_param_vars: &FxHashSet<ArcVarId>,
) -> Option<KeepaliveSite> {
    // Exactly ONE iter-consume use — the multi-use case is Phase 6.66's.
    if uses.len() != 1 {
        return None;
    }
    let u = uses[0];
    // The single iter-consume MUST be the inline for-loop `@iter(arg [own])` body
    // call (not a terminator-position user-callee iter-consume): the keep-alive
    // pairs with the SAME-function `ori_iter_drop`. A terminator user-callee
    // iter-consume frees inside the callee — Phase 6.65 owns that.
    let iter_idx = u.instr?;
    // Lineage must be live-out at a NON-iter position after the `@iter` — the
    // reuse that the iter-drop would otherwise free out from under.
    if !lineage_live_out_after_use(func, jt_reps, rep, u.block, u.instr) {
        return None;
    }
    // The keep-alive names the value the `@iter` receives (the lineage arg at this
    // use). The paired dec lands AFTER the lineage's LAST non-iter use.
    let inc_arg = lineage_arg_at_use(func, jt_reps, rep, u)?;
    // The `ori_iter_drop` paired with this `@iter` bounds where the reuse can begin
    // — the last non-iter use (the reuse) must live AT OR AFTER it.
    let (drop_block, drop_at, _) = paired_iter_drop_site(func, u.block, iter_idx, interner)?;
    // The paired dec goes immediately after the lineage's last non-iter use in the
    // iter-drop's block (the merged loop-exit / reuse block). The reuse
    // (`@__index(%0 [borrow])`, a `Project`, a `Return`-feeding read) is a
    // borrowed/last use of the lineage AFTER the drop; releasing the keep-alive
    // duplicate there leaves no dangling ref.
    let (dec_arg, dec_at) = lineage_last_noniter_use_after(
        func, jt_reps, rep, drop_block, drop_at, iter_name, contracts,
    )?;
    let dec_arg = retarget_borrowed_keepalive_dec(dec_arg, inc_arg, borrowed_param_vars);
    Some(KeepaliveSite {
        inc_arg,
        inc_block: u.block,
        inc_at: iter_idx,
        dec_arg,
        dec_block: drop_block,
        dec_at,
    })
}

fn emit_single_iter_consume_reuse_keepalive(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    let mut keepalive_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let iter_name = interner.intern("iter");

    // RL-2 borrowed-param paired-dec retarget set: when the lineage's last non-iter
    // use names the BORROWED PARAM itself, the paired dec is retargeted onto the
    // non-param `inc_arg` alias (`retarget_borrowed_keepalive_dec`) so it never
    // decrements a borrowed param — VF-1 `check_no_dec_on_borrowed` (`@iter` is
    // `ApplyToIterConsumingParam`: `AimsProof.Realization::RL2_iter_consuming_no_caller_dec`).
    let borrowed_param_vars: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| matches!(p.ownership, crate::ownership::Ownership::Borrowed))
        .map(|p| p.var)
        .collect();

    let uses_per_rep = collect_iter_consume_uses_per_rep(func, pool, contracts, iter_name, &rep_of);

    let sites: Vec<KeepaliveSite> = uses_per_rep
        .iter()
        .filter_map(|(rep, uses)| {
            resolve_keepalive_site(
                func,
                &jt_reps,
                *rep,
                uses,
                iter_name,
                interner,
                contracts,
                &borrowed_param_vars,
            )
        })
        .collect();
    if sites.is_empty() {
        return;
    }

    // Group inserts by block; apply each block's inserts in DESCENDING effective
    // index so an earlier insertion never invalidates a later one.
    let mut inserts_by_block: FxHashMap<usize, Vec<(usize, ArcInstr)>> = FxHashMap::default();
    for s in &sites {
        keepalive_vars.insert(s.inc_arg);
        keepalive_vars.insert(s.dec_arg);
        // keep-alive inc lands immediately BEFORE its `@iter`
        inserts_by_block
            .entry(s.inc_block)
            .or_default()
            .push((s.inc_at, ArcInstr::BurdenInc { var: s.inc_arg }));
        // paired dec lands immediately AFTER the lineage's last non-iter use
        inserts_by_block.entry(s.dec_block).or_default().push((
            s.dec_at.saturating_add(1),
            ArcInstr::BurdenDec { var: s.dec_arg },
        ));
    }
    for (block_idx, mut inserts) in inserts_by_block {
        let Some(block) = func.blocks.get_mut(block_idx) else {
            continue;
        };
        inserts.sort_by(|a, b| b.0.cmp(&a.0));
        for (at, instr) in inserts {
            block.body.insert(at.min(block.body.len()), instr);
        }
    }
    // The keep-alive inc + paired dec mark the lineage in burden_emitted (VF-1
    // checks it); re-populate so the balance verifier sees the new pair.
    populate_burden_emitted_from_iter_keepalive(func, &keepalive_vars);
}

/// Mark every var in `vars` in `func.burden_emitted` (the keep-alive `BurdenInc` +
/// paired `BurdenDec` emitted by [`emit_single_iter_consume_reuse_keepalive`]
/// introduce burden ops on a previously-unmarked borrowed lineage; VF-1 keys its
/// per-var balance check on `burden_emitted`).
fn populate_burden_emitted_from_iter_keepalive(func: &mut ArcFunction, vars: &FxHashSet<ArcVarId>) {
    if func.burden_emitted.len() != func.var_types.len() {
        func.burden_emitted = vec![false; func.var_types.len()];
    }
    for &v in vars {
        if let Some(slot) = func.burden_emitted.get_mut(v.index()) {
            *slot = true;
        }
    }
}

/// True iff `ORI_DISABLE_LOOP_INVARIANT_ITER_SURVIVOR_SURPLUS=1` declines the
/// Phase-6.66e loop-invariant iter-consumed survivor surplus suppression.
fn loop_invariant_iter_survivor_disabled() -> bool {
    static DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DISABLED.get_or_init(|| {
        std::env::var("ORI_DISABLE_LOOP_INVARIANT_ITER_SURVIVOR_SURPLUS").as_deref() == Ok("1")
    })
}

/// Same-allocation reps via Let-Var aliases + FORWARD Jump-arg→block-param renames
/// (the Jump-phi BACK-edge EXCLUDED). Connects a survivor's alias chain to its
/// post-loop read AND threads a loop-INVARIANT value unchanged through the loop
/// header, but NEVER unions a rebuilt loop accumulator's distinct per-iteration
/// allocation (`result = result.push(w)` / `for...yield` scratch — only the back-edge
/// would). Used by the loop-invariant-survivor pass to avoid the jump-threaded
/// over-merge of `words` with an accumulator. Spec: Annex E §AIMS — merge-point
/// filtering (back-edge declines).
fn compute_forward_threaded_reps(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
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
    let param_edges = crate::aims::intraprocedural::project_aliases::compute_param_edge_args(func);
    for (&param, edges) in &param_edges {
        for e in edges {
            if !e.is_back_edge {
                union(&mut parent, param, e.arg);
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

/// The resolved burden-op rewrite for ONE loop-invariant iter-consumed survivor
/// rep: the keep-alive `BurdenInc` site to KEEP (the dup the `@iter`/`ori_iter_drop`
/// pair balances) + the survivor-release `BurdenDec` site to KEEP (the LAST dec,
/// after the post-loop borrow-read). Every OTHER normal-path `BurdenInc`/`BurdenDec`
/// on the rep is the base-walk surplus and is removed.
struct SurvivorRewrite {
    keep_inc: (usize, usize),
    keep_dec: (usize, usize),
}

/// Phase 6.66e — suppress the base-walk surplus burden ops on a loop-INVARIANT
/// collection iter-consumed via the inline for-loop `@iter [own]` and read AFTER
/// the loop via a borrow (the survivor shape — `str_list_explicit_last_owner`).
///
/// A rep (computed via BACK-EDGE-EXCLUDED [`compute_forward_threaded_reps`] so a
/// loop-carried accumulator's allocation is NOT merged in) qualifies when ALL hold:
///  - exactly ONE inline `@iter(arg [own])` iter-consume, on a loop path, with a
///    paired `ori_iter_drop`;
///  - a fresh `Construct`/collection-source alloc OUTSIDE the loop (loop-invariant);
///  - a post-`ori_iter_drop` BORROWED-position read of the COLLECTION rep (the
///    survivor read — NOT a Project element-view, NOT a Jump-arg forward, NOT a
///    scope-exit dec; the discriminator vs `str_split`/`set_to_list`/`derive_clone`);
///  - it does NOT flow into a genuine value-COW-mutator (`@iter` excluded) NOR a
///    collection-conversion builtin (`@keys`/`@values`/`@split`/`@to_list`);
///  - the base walk over-emitted (>= 2 incs OR >= 2 decs on the rep).
///
/// The rewrite KEEPS the keep-alive inc + the LAST dec and REMOVES every other
/// normal-path burden op on the rep. Unwind/`Resume` decs are left intact.
fn suppress_loop_invariant_iter_survivor_surplus(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    if loop_invariant_iter_survivor_disabled() {
        return;
    }
    let jt_reps = compute_forward_threaded_reps(func);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let iter_name = interner.intern("iter");
    let preds = crate::graph::compute_predecessors(func);
    let dom = crate::graph::DominatorTree::build(func);
    let loop_blocks = compute_loop_blocks_local(func, &preds, &dom);
    let unwind_blocks = compute_unwind_reachable_blocks(func);
    // Genuine value-COW-mutator names (`@iter` EXCLUDED — it is an iter-consume
    // transfer balanced by `ori_iter_drop`, not a value mutation). A rep flowing into
    // one of these at an owned position needs its keep-alive inc KEPT (RL-1 COW copy).
    let mut cow_mutators = crate::borrow::all_cow_method_names(interner);
    cow_mutators.remove(&iter_name);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let list_take_name = for_yield_result_finalizer_name(interner);

    let uses_per_rep = collect_iter_consume_uses_per_rep(func, pool, contracts, iter_name, &rep_of);

    let mut rewrites: Vec<(ArcVarId, SurvivorRewrite)> = Vec::new();
    for (rep, uses) in &uses_per_rep {
        // Decline a rep consumed at a SECOND owned position beyond the single `@iter`
        // (a `result = result.push(w)` accumulator / `for...yield` scratch / concat):
        // the single-survivor-release oracle cannot collapse its branch-distributed
        // releases. A true survivor's collection is consumed ONCE + otherwise read.
        if rep_has_second_owned_consume(func, &rep_of, *rep, iter_name) {
            continue;
        }
        // Decline a rep flowing into a genuine value-COW-mutation (keep-alive inc is
        // load-bearing for the COW copy verdict; the map-cow / map_keys families).
        if rep_lineage_is_cow_tainted(
            func,
            &rep_of,
            &cow_mutators,
            &builtins,
            contracts,
            interner,
            list_take_name,
            *rep,
        ) {
            continue;
        }
        let Some(rw) = resolve_loop_invariant_iter_survivor(
            func,
            &jt_reps,
            *rep,
            uses,
            &loop_blocks,
            &unwind_blocks,
            interner,
        ) else {
            continue;
        };
        rewrites.push((*rep, rw));
    }
    if rewrites.is_empty() {
        return;
    }

    // Resolve the remove-set per rep against the un-mutated body, then remove in
    // descending (block, idx) order so earlier removals never invalidate later.
    let mut to_remove: FxHashSet<(usize, usize)> = FxHashSet::default();
    let mut burden_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for (rep, rw) in &rewrites {
        for (b, block) in func.blocks.iter().enumerate() {
            if unwind_blocks.contains(&b) {
                continue;
            }
            for (i, instr) in block.body.iter().enumerate() {
                let var = match instr {
                    ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } => *var,
                    _ => continue,
                };
                if rep_of(var) != *rep {
                    continue;
                }
                burden_vars.insert(var);
                if (b, i) != rw.keep_inc && (b, i) != rw.keep_dec {
                    to_remove.insert((b, i));
                }
            }
        }
    }
    let mut by_block: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    for (b, i) in to_remove {
        by_block.entry(b).or_default().push(i);
    }
    for (b, mut idxs) in by_block {
        let Some(block) = func.blocks.get_mut(b) else {
            continue;
        };
        idxs.sort_unstable_by(|a, c| c.cmp(a));
        for i in idxs {
            if i < block.body.len() {
                block.body.remove(i);
            }
        }
    }
    populate_burden_emitted_from_iter_keepalive(func, &burden_vars);
}

/// True iff the rep is consumed at a NON-`@iter` owned position anywhere — a SECOND
/// owned consume beyond the single survivor `@iter` (a `@push` accumulator, a
/// `for...yield` scratch finalizer, a concat). A true survivor's collection is
/// consumed ONCE (the `@iter`).
fn rep_has_second_owned_consume(
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
    iter_name: Name,
) -> bool {
    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    for block in &func.blocks {
        for instr in &block.body {
            let callee = match instr {
                ArcInstr::Apply { func: c, .. } => Some(*c),
                _ => None,
            };
            let used = instr.used_vars();
            for (pos, &arg) in used.iter().enumerate() {
                if instr.is_owned_position(pos)
                    && is_rcptr(arg)
                    && rep_of(arg) == rep
                    && callee != Some(iter_name)
                {
                    return true;
                }
            }
        }
        let tused = block.terminator.used_vars();
        for (pos, &arg) in tused.iter().enumerate() {
            if block.terminator.is_owned_position(pos) && is_rcptr(arg) && rep_of(arg) == rep {
                return true;
            }
        }
    }
    false
}

/// Resolve the [`SurvivorRewrite`] for `rep` iff it is a loop-invariant
/// iter-consumed survivor (see [`suppress_loop_invariant_iter_survivor_surplus`]).
fn resolve_loop_invariant_iter_survivor(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    uses: &[IterConsumeUse],
    loop_blocks: &FxHashSet<usize>,
    unwind_blocks: &FxHashSet<usize>,
    interner: &ori_ir::StringInterner,
) -> Option<SurvivorRewrite> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    // (1) Exactly ONE inline `@iter [own]` consume with a paired `ori_iter_drop`.
    if uses.len() != 1 {
        return None;
    }
    let u = uses[0];
    let iter_idx = u.instr?;
    let (drop_block, drop_at, _) = paired_iter_drop_site(func, u.block, iter_idx, interner)?;
    // The iteration is a loop: a loop block lies ON the path between the `@iter` and
    // the paired `ori_iter_drop`. A straight-line single iter-consume is not the
    // back-edge over-emission this pass cures.
    let from_iter = forward_reachable_from(func, &[u.block]);
    let on_loop_path = loop_blocks.contains(&u.block)
        || loop_blocks.contains(&drop_block)
        || loop_blocks.iter().any(|lb| from_iter.contains(lb));
    if loop_blocks.is_empty() || !on_loop_path {
        return None;
    }
    // (2) Fresh loop-invariant alloc OUTSIDE the loop.
    if !rep_has_loop_invariant_fresh_alloc(func, &rep_of, rep, loop_blocks, interner) {
        return None;
    }
    // (3) A post-`ori_iter_drop` BORROWED-position read of the COLLECTION rep.
    if !rep_has_post_drop_collection_borrow_read(func, &rep_of, rep, drop_block, drop_at) {
        return None;
    }
    // (3b) Decline a rep flowing into a collection-CONVERSION builtin (the
    // derived-second-collection `map_keys_then_use_map` shape).
    if rep_flows_into_collection_conversion(func, &rep_of, rep, interner) {
        return None;
    }
    // (4) The keep-alive inc + the survivor release + the over-emission signature.
    let mut incs: Vec<(usize, usize)> = Vec::new();
    let mut decs: Vec<(usize, usize)> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        if unwind_blocks.contains(&b) {
            continue;
        }
        for (i, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::BurdenInc { var } if rep_of(*var) == rep => incs.push((b, i)),
                ArcInstr::BurdenDec { var } if rep_of(*var) == rep => decs.push((b, i)),
                _ => {}
            }
        }
    }
    if incs.len() < 2 && decs.len() < 2 {
        return None;
    }
    let keep_inc = incs
        .iter()
        .rfind(|(b, i)| *b == u.block && *i < iter_idx)
        .or_else(|| incs.last())
        .copied()?;
    let keep_dec = *decs.last()?;
    if keep_inc == keep_dec {
        return None;
    }
    Some(SurvivorRewrite { keep_inc, keep_dec })
}

/// True iff `rep` has a fresh RcPtr/FatValue alloc (the canonical `fresh_rc_alloc_dst`
/// birth `+1`) in a body block OUTSIDE every loop block — the loop-invariant birth.
fn rep_has_loop_invariant_fresh_alloc(
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
    loop_blocks: &FxHashSet<usize>,
    interner: &ori_ir::StringInterner,
) -> bool {
    let list_take_name = for_yield_result_finalizer_name(interner);
    for (b, block) in func.blocks.iter().enumerate() {
        if loop_blocks.contains(&b) {
            continue;
        }
        for instr in &block.body {
            if let Some(dst) = fresh_rc_alloc_dst(instr, func, interner, list_take_name) {
                if rep_of(dst) == rep {
                    return true;
                }
            }
        }
    }
    false
}

/// True iff `rep` has a genuine BORROWED-position read of the COLLECTION at or after
/// the paired `ori_iter_drop` `(drop_block, drop_at)` — the survivor read. NOT a read:
/// a `Project` element-view, a scope-exit `BurdenDec`, a Jump-arg forward, an owned
/// consume. This is the discriminator vs `str_split`/`set_to_list`/`derive_clone` (whose
/// fresh `@iter` source has NO post-loop COLLECTION borrow-read).
fn rep_has_post_drop_collection_borrow_read(
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
    drop_block: usize,
    drop_at: usize,
) -> bool {
    let reachable = compute_successor_reachable(func, drop_block);
    for (b, block) in func.blocks.iter().enumerate() {
        let after_drop = b == drop_block;
        if !after_drop && !reachable.contains(&b) {
            continue;
        }
        for (i, instr) in block.body.iter().enumerate() {
            if after_drop && i <= drop_at {
                continue;
            }
            if instr_borrow_reads_rep(instr, rep_of, rep) {
                return true;
            }
        }
        if terminator_borrow_reads_rep(&block.terminator, rep_of, rep) {
            return true;
        }
    }
    false
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

/// True iff the rep's lineage flows into a collection-CONVERSION builtin
/// (`@keys`/`@values`/`@split`/`@to_list`) at any `Apply`/`Invoke` position — the
/// result is a SECOND derived collection whose branch-distributed releases the
/// single-survivor-release oracle cannot collapse. The simple survivor (`words.len()`)
/// never flows into a conversion.
fn rep_flows_into_collection_conversion(
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    rep: ArcVarId,
    interner: &ori_ir::StringInterner,
) -> bool {
    let conversions = collection_conversion_names(interner);
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if conversions.contains(callee) && args.iter().any(|&a| rep_of(a) == rep) {
                    return true;
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            if conversions.contains(callee) && args.iter().any(|&a| rep_of(a) == rep) {
                return true;
            }
        }
    }
    false
}

/// True iff the rep's lineage flows into a GENUINE value-COW-mutation — an
/// owned-position consume at a genuine COW-mutator (`cow_mutators`, with `@iter`
/// already excluded by the caller), a concat `Add` `PrimOp` operand, or a may-COW
/// user-call arg. The keep-alive inc is then load-bearing (RL-1 COW copy verdict).
#[expect(
    clippy::too_many_arguments,
    reason = "the COW-taint predicate reads the function, the rep resolver, the \
              COW-mutator set, the builtin ownership sets, the contracts, the \
              interner, the list-take name, and the rep — each a distinct SSOT input"
)]
fn rep_lineage_is_cow_tainted(
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    cow_mutators: &FxHashSet<Name>,
    builtins: &crate::borrow::BuiltinOwnershipSets,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    list_take_name: Name,
    rep: ArcVarId,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if cow_mutators.contains(callee) {
                    let used = instr.used_vars();
                    for (pos, &arg) in used.iter().enumerate() {
                        if instr.is_owned_position(pos) && rep_of(arg) == rep {
                            return true;
                        }
                    }
                }
                for (pos, &arg) in args.iter().enumerate() {
                    if rep_of(arg) == rep
                        && callee_may_cow_arg(
                            contracts,
                            builtins,
                            interner,
                            list_take_name,
                            *callee,
                            pos,
                        )
                    {
                        return true;
                    }
                }
            }
            if let ArcInstr::Let {
                value: ArcValue::PrimOp { op, args },
                ..
            } = instr
            {
                if matches!(op, PrimOp::Binary(ori_ir::BinaryOp::Add))
                    && args.iter().any(|&a| rep_of(a) == rep)
                {
                    return true;
                }
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            let used = block.terminator.used_vars();
            if cow_mutators.contains(callee) {
                for (pos, &arg) in used.iter().enumerate() {
                    if block.terminator.is_owned_position(pos) && rep_of(arg) == rep {
                        return true;
                    }
                }
            }
            for (pos, &arg) in args.iter().enumerate() {
                if rep_of(arg) == rep
                    && callee_may_cow_arg(
                        contracts,
                        builtins,
                        interner,
                        list_take_name,
                        *callee,
                        pos,
                    )
                {
                    return true;
                }
            }
        }
    }
    false
}

/// The `(arg, instr_idx)` of the lineage `rep`'s LAST non-iter-consume use at or
/// after `(block, at)` in `block`'s body — the reuse the keep-alive must outlive.
/// "Non-iter-consume" excludes the `@iter(arg [own])` / iter-consuming-callee
/// positions (those are the consumption the keep-alive duplicates for, not the
/// reuse). Returns the var actually used (so the paired dec names a value whose
/// `var_repr` carries the buffer's `RcStrategy`) and its instruction index.
fn lineage_last_noniter_use_after(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block_idx: usize,
    at: usize,
    iter_name: Name,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Option<(ArcVarId, usize)> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let block = func.blocks.get(block_idx)?;
    let mut found: Option<(ArcVarId, usize)> = None;
    for (i, instr) in block.body.iter().enumerate() {
        if i < at {
            continue;
        }
        for (pos, &arg) in instr.used_vars().iter().enumerate() {
            if rep_of(arg) != rep {
                continue;
            }
            // Skip the iter-consume position itself (the `@iter [own]` / user
            // iter-consuming arg) — it is the consumption, not the reuse. Only an
            // `Apply` at an owned position can be iter-consuming; a `Project` /
            // `Let` / borrowed use of the lineage IS the reuse and is kept.
            if let ArcInstr::Apply {
                func: callee,
                arg_ownership,
                ..
            } = instr
            {
                if instr.is_owned_position(pos)
                    && arg_pos_iter_consumes(*callee, arg_ownership, pos, iter_name, contracts)
                {
                    continue;
                }
            }
            found = Some((arg, i));
        }
    }
    found
}

/// The `(block, instr_idx, handle)` of the `ori_iter_drop(handle [own])` paired
/// with the `@iter` at `(iter_block, iter_idx)`: the `@iter`'s result var (the
/// iterator handle) is the `ori_iter_drop` arg. Returns the FIRST such drop in CFG
/// order (the for-loop's normal-exit `ori_iter_drop`); unwind-edge drops are panic
/// cleanup and are NOT the keep-alive's site.
fn paired_iter_drop_site(
    func: &ArcFunction,
    iter_block: usize,
    iter_idx: usize,
    interner: &ori_ir::StringInterner,
) -> Option<(usize, usize, ArcVarId)> {
    let iter_drop_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterDrop.name());
    let handle = match func.blocks.get(iter_block)?.body.get(iter_idx)? {
        ArcInstr::Apply { dst, .. } => *dst,
        _ => return None,
    };
    let unwind_blocks = compute_unwind_reachable_blocks(func);
    for (b, block) in func.blocks.iter().enumerate() {
        if unwind_blocks.contains(&b) {
            continue;
        }
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                if *callee == iter_drop_name && args.first() == Some(&handle) {
                    return Some((b, i, handle));
                }
            }
        }
    }
    None
}

/// A pending keep-alive `BurdenInc` for
/// [`emit_iter_element_view_iter_consume_keepalive_inc`]: insert `BurdenInc var`
/// at `block` index `at` (`None` = append at end of body, for a terminator-
/// position iter-consume).
struct NestedIterKeepAlive {
    block: usize,
    at: Option<usize>,
    var: ArcVarId,
}

/// True iff `callee(args, arg_ownership)` iter-consumes `args[pos]` — either the
/// INLINE for-loop `@iter(arg [own])` protocol builtin or a USER callee whose
/// `ParamContract.iter_consumes` is true for that parameter. Mirrors the SSOT
/// iter-consume detection in [`record_iter_consume_uses`] (AIMS Invariant 5 — no
/// parallel iter-consume tracker; this is a position-level predicate over the
/// same two signals).
fn arg_pos_iter_consumes(
    callee: Name,
    arg_ownership: &[ArgOwnership],
    pos: usize,
    iter_name: Name,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    if callee == iter_name {
        return arg_ownership
            .get(pos)
            .is_some_and(|o| *o == ArgOwnership::Owned);
    }
    contracts
        .get(&callee)
        .and_then(|c| c.params.get(pos))
        .is_some_and(|p| p.iter_consumes)
}

/// Phase 6.67 (probe): emit a keep-alive `BurdenInc` on every iter-element-view
/// passed to an iter-consuming position.
///
/// For a nested `for inner in outer do { for x in inner do .. }` (or
/// `for l in lists yield sum_list(l)`) the inner collection flows from
/// `Project @__iter_next.1` of the outer source (an iter-element-view per
/// [`collect_iter_element_defs`]) into an iter-consuming position — the INLINE
/// inner `@iter(inner [own])`, OR a USER callee `@sum_list(inner)` whose
/// `ParamContract.iter_consumes` is true (its body `@iter [own]` ->
/// `ori_iter_drop`s the arg). The element view owns no allocation (the buffer is
/// the outer collection's, freed by the outer `elem_dec_fn`), yet the inner
/// iter-consume ALSO frees it -> double-free. The keep-alive inc raises the rc
/// so the inner consume (RL-2 single release) and the outer `elem_dec_fn` each
/// release exactly once. Nesting recurses automatically: an inner-of-inner view
/// is an iter-element-view of the inner `@__iter_next`, so its consume position
/// is also covered.
///
/// Gate (the over-fire boundary): the inc fires ONLY when the consumed arg is in
/// `collect_iter_element_defs` (a borrow-view of an enclosing collection). A
/// top-level iter-consume of the genuinely-owned source (NOT an element view) is
/// freed solely by its own `ori_iter_drop` -> NO keep-alive (it would orphan a
/// +1 -> leak). Probe-gated -> default codegen byte-identical. Spec: Annex E
/// §AIMS RL-1 + RL-2.
fn emit_iter_element_view_iter_consume_keepalive_inc(
    func: &mut ArcFunction,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    pool: &Pool,
) {
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    if iter_element_defs.is_empty() {
        return;
    }

    // A keep-alive on `arg` is warranted iff `arg` is an iter-element-view of an
    // ENCLOSING collection: it is in `iter_element_defs` AND it is a heap
    // collection (`is_collection_dst` excludes scalar/closure element views that
    // carry no buffer the outer `elem_dec_fn` frees — a closure element view's
    // env is freed differently). The `RcPtr`/`FatVal` repr gate keeps scalar
    // views out.
    let warrants_keepalive = |arg: ArcVarId| -> bool {
        iter_element_defs.contains(&arg)
            && matches!(
                func.var_repr(arg),
                Some(ValueRepr::RcPointer | ValueRepr::FatValue)
            )
    };

    let mut inserts: Vec<NestedIterKeepAlive> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                for (pos, &arg) in args.iter().enumerate() {
                    if arg_pos_iter_consumes(*callee, arg_ownership, pos, iter_name, contracts)
                        && warrants_keepalive(arg)
                    {
                        inserts.push(NestedIterKeepAlive {
                            block: b,
                            at: Some(i),
                            var: arg,
                        });
                    }
                }
            }
        }
        // Terminator-position iter-consume: `Invoke @callee(view) normal/unwind`
        // where the callee iter-consumes the view (`for l in lists yield
        // sum_list(l)` lowers `sum_list` to a may-unwind `Invoke`). The keep-alive
        // inc appends at the end of the body, before the terminator reads the arg.
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            for (pos, &arg) in args.iter().enumerate() {
                if arg_pos_iter_consumes(*callee, arg_ownership, pos, iter_name, contracts)
                    && warrants_keepalive(arg)
                {
                    inserts.push(NestedIterKeepAlive {
                        block: b,
                        at: None,
                        var: arg,
                    });
                }
            }
        }
    }

    let _ = pool;
    // Apply back-to-front per block so earlier indices stay valid.
    inserts.sort_by(|a, b| {
        b.block
            .cmp(&a.block)
            .then(b.at.unwrap_or(usize::MAX).cmp(&a.at.unwrap_or(usize::MAX)))
    });
    for ins in inserts {
        if let Some(block) = func.blocks.get_mut(ins.block) {
            let inc = ArcInstr::BurdenInc { var: ins.var };
            match ins.at {
                Some(at) => block.body.insert(at.min(block.body.len()), inc),
                None => block.body.push(inc),
            }
        }
    }
}

/// Whether `agg_rep` (a jump-threaded rep of a `Construct`/`Reuse` aggregate dst)
/// is TRANSFERRED OUT of the current function rather than dropped in-scope.
///
/// Genuine transfer-out (the field-drop runs ELSEWHERE -> keep-alive would orphan
/// a `+1`): the aggregate's lineage flows to a `Return` value, an owned
/// `Invoke`/`InvokeIndirect` terminator arg, an owned `Apply`/`ApplyIndirect`/
/// `CollectionReuse` arg (a collection push `@ori_list_push(list, agg [own])`, a
/// user-call owned arg), i.e. anything whose in-scope `RcDec [AggFields]`/
/// `[InlineEnum]` never runs here (`for p in parts yield Some(p)` pushes into a
/// collected list freed later by its `elem_dec_fn`; a returned aggregate freed by
/// the caller).
///
/// NOT transfer-out (FIRE the keep-alive): being consumed as an owned arg of a
/// PARENT `Construct`/`Reuse` — a NESTED aggregate (`MaybeNamed { name: Some(p) }`
/// nests `Some(p)` into `MaybeNamed`). The inner `Some(p)` is consumed into the
/// outer aggregate, whose field-drop RECURSIVELY walks the inner payload (the
/// slice) in-scope IF the OUTERMOST aggregate is itself dropped in-scope. This
/// fn FOLLOWS each parent-Construct/Reuse consumption to the outermost enclosing
/// aggregate and tests THAT for genuine transfer-out — matching the oracle, which
/// emits the keep-alive on the slice at the inner `Construct` whenever the
/// outermost aggregate is dropped in-scope. `rep_of` threads Let/Jump aliases.
/// Spec: Annex E §AIMS RL-1 + RL-2.
fn aggregate_transferred_out(
    agg_rep: ArcVarId,
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> bool {
    let mut frontier = vec![agg_rep];
    let mut seen: FxHashSet<ArcVarId> = FxHashSet::default();
    while let Some(cur) = frontier.pop() {
        if !seen.insert(cur) {
            continue;
        }
        for block in &func.blocks {
            for instr in &block.body {
                // Consumed at an owned arg of a PARENT Construct/Reuse -> follow to
                // that parent (the nested-aggregate case); the parent's field-drop
                // walks `cur`'s payload in-scope if the parent is dropped in-scope.
                if let ArcInstr::Construct { dst, args, .. } | ArcInstr::Reuse { dst, args, .. } =
                    instr
                {
                    if args.iter().any(|&a| rep_of(a) == cur) {
                        frontier.push(rep_of(*dst));
                        continue;
                    }
                }
                // Consumed at any OTHER owned position (Apply/ApplyIndirect/
                // CollectionReuse owned arg — collection push, user call) -> genuine
                // transfer-out.
                for (pos, &used) in instr.used_vars().iter().enumerate() {
                    if rep_of(used) == cur && instr.is_owned_position(pos) {
                        return true;
                    }
                }
            }
            match &block.terminator {
                ArcTerminator::Return { value } => {
                    if rep_of(*value) == cur {
                        return true;
                    }
                }
                ArcTerminator::Invoke {
                    args,
                    arg_ownership,
                    ..
                }
                | ArcTerminator::InvokeIndirect {
                    args,
                    arg_ownership,
                    ..
                } => {
                    for (pos, &arg) in args.iter().enumerate() {
                        if rep_of(arg) == cur
                            && arg_ownership.get(pos) == Some(&ArgOwnership::Owned)
                        {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// True iff the burden-carrying aggregate `agg_rep` is moved at an OWNED position
/// into a collection-append (`@push` / `ori_list_push`) whose RECEIVER collection
/// is ITER-CONSUMED IN-SCOPE and NOT returned — the precise complement of
/// [`collection_receiver_returned`] for the nested-aggregate shape.
///
/// `for p in parts yield Some(p); for opt in opts do ..` pushes the `Some(p)`
/// aggregate `[own]` into `opts`, which is iter-consumed locally (`@iter [own]` ->
/// `ori_iter_drop`) and never returned. The aggregate's field-drop therefore runs
/// IN-SCOPE (via `opts`'s `elem_dec_fn` at the iter-drop), decing the stored
/// slice-element view's shared backing a SECOND time (the source `parts` buffer's
/// iter-drop is the first) -> double-free without a keep-alive. This is the
/// transfer-out subcase where [`aggregate_transferred_out`] is true yet the
/// field-drop is still in-scope, so the RL-1 keep-alive (`RL1_duplication_balanced`)
/// is load-bearing. Reuses [`for_yield_result_iter_consumed_not_returned`] for the
/// receiver's iter-consumed-not-returned proof (AIMS Invariant 5 — no parallel
/// iter-consume tracker). Returned receivers stay with Phase 6.68b
/// (`collection_receiver_returned`); genuinely-escaping transfers (user-call
/// `[own]`, returned aggregate) decline. Spec: Annex E §AIMS RL-1 + RL-2.
fn aggregate_pushed_into_in_scope_consumed_collection(
    agg_rep: ArcVarId,
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    interner: &ori_ir::StringInterner,
) -> bool {
    // `ORI_DISABLE_NESTED_AGG_INSCOPE_KEEPALIVE=1` reverts the Phase 6.68
    // nested-aggregate-into-in-scope-consumed-collection admission (this fn
    // returns false): the `for p in parts yield Some(p); for opt in opts do ..`
    // shape reverts to the under-funded double-free (the stored slice view's
    // backing is released by BOTH the source `parts` iter-drop and the in-scope
    // `opts` `elem_dec_fn`). Bisects the nested-aggregate arm vs the rest of
    // Phase 6.68. Default (unset) keeps the keep-alive. Spec: Annex E §AIMS RL-1.
    if std::env::var("ORI_DISABLE_NESTED_AGG_INSCOPE_KEEPALIVE").as_deref() == Ok("1") {
        return false;
    }
    let push_name = interner.intern("push");
    let list_push_name = interner.intern("ori_list_push");
    let list_take_name = for_yield_result_finalizer_name(interner);
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());

    // The `for...yield` finalizer `ori_list_take(scratch)` moves the scratch
    // buffer into the result `[T]`; the push RECEIVER (the scratch handle) and
    // the iter-consumed RESULT are linked by this take, not by a jump/Let rep.
    // Map each take-receiver rep to its take-result rep so the iter-consume proof
    // runs on the result lineage.
    let take_result_of = |recv_rep: ArcVarId| -> Option<ArcVarId> {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } = instr
                {
                    if *callee == list_take_name
                        && args.first().is_some_and(|&a| rep_of(a) == recv_rep)
                    {
                        return Some(rep_of(*dst));
                    }
                }
            }
        }
        None
    };

    for block in &func.blocks {
        for instr in &block.body {
            let (callee, args, arg_ownership) = match instr {
                ArcInstr::Apply {
                    func: callee,
                    args,
                    arg_ownership,
                    ..
                } => (*callee, args, arg_ownership),
                _ => continue,
            };
            if callee != push_name && callee != list_push_name {
                continue;
            }
            // The aggregate must be a NON-receiver owned arg (the pushed element
            // payload, position >= 1; position 0 is the receiver collection).
            let pushed_as_element = args.iter().enumerate().skip(1).any(|(pos, &a)| {
                rep_of(a) == agg_rep && arg_ownership.get(pos) == Some(&ArgOwnership::Owned)
            });
            if !pushed_as_element {
                continue;
            }
            let Some(&recv) = args.first() else {
                continue;
            };
            // The consumed lineage is the take-finalizer RESULT (the moved-out
            // buffer), falling back to the receiver itself when no take links it.
            let consumed_rep = take_result_of(rep_of(recv)).unwrap_or_else(|| rep_of(recv));
            // Iter-consumed in-scope and NOT returned -> the field-drop runs
            // locally, so the keep-alive is needed. A returned result is Phase
            // 6.68b's domain (declined here to avoid double-emit).
            if for_yield_result_iter_consumed_not_returned(func, consumed_rep, rep_of, iter_name) {
                return true;
            }
        }
    }
    false
}

/// Phase 6.68 (probe): emit a keep-alive `BurdenInc` on every iter-element-view
/// stored as a field arg of a burden-carrying aggregate `Construct` / `Reuse`
/// that is DROPPED IN-SCOPE.
///
/// A loop element `p` from `coll.split(..)` is a seamless-slice (`SLICE_FLAG`
/// cap) sharing the source backing buffer — a Borrowed Project-view in
/// [`collect_iter_element_defs`], excluded from `owned_vars_needing_rc`, so the
/// base burden walk emits NO Construct-arg inc on it. When it is stored as a
/// field of an aggregate whose [`is_burden_carrying_aggregate`] drop-glue walks
/// heap fields (`let w = Wrapper { s: p, .. }`, `MaybeNamed { name: Some(p), .. }`,
/// `Pair { data: (p, ..) }`, `Holder { payload: Ok(p), .. }`), the aggregate's
/// scope-exit `RcDec [AggFields]`/`[InlineEnum]` decs the shared backing once per
/// iteration -> double-free. The oracle balances this with one `RcInc <view>`
/// before the `Construct`; this pass restores that RL-1 keep-alive.
///
/// Gate (the over-fire boundary): the inc fires ONLY when (a) the field arg is in
/// `collect_iter_element_defs` (a borrowed element view of an enclosing
/// collection — an OWNED, freshly-built collection/str field is NOT in this set
/// and already carries its own Construct-arg inc, so it is left untouched), (b)
/// the arg's repr is a heap value (`RcPointer | FatValue` — scalar element views
/// carry no backing the aggregate drop frees), (c) the `Construct`/`Reuse` dst is
/// `is_burden_carrying_aggregate` (its field-drop actually walks the heap field —
/// a non-burden-carrying aggregate `{ x: int, y: int }` has no field drop-glue, so
/// no inc is needed), AND (d) the aggregate is DROPPED IN-SCOPE, NOT transferred
/// out (`!aggregate_transferred_out`). The transferred-out case
/// (`for p in parts yield Some(p)` pushes the `Some(p)` into a collected list,
/// `Return Some(p)`, or `Construct Outer(.. Some(p) ..)`) has its field-drop run
/// LATER (the collected list's `elem_dec_fn` / the caller / the outer aggregate),
/// so a keep-alive here orphans a `+1` -> leak; that moved-out element accounting
/// is a SEPARATE under-emission, not this leaf. A slice element used scalar-only
/// (`p.len()` with no aggregate store) never reaches a Construct field position.
/// Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
fn emit_slice_element_aggregate_field_keepalive_inc(
    func: &mut ArcFunction,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
) {
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    if iter_element_defs.is_empty() {
        return;
    }
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    let warrants_keepalive = |arg: ArcVarId| -> bool {
        iter_element_defs.contains(&arg)
            && matches!(
                func.var_repr(arg),
                Some(ValueRepr::RcPointer | ValueRepr::FatValue)
            )
    };

    let mut inserts: Vec<NestedIterKeepAlive> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            let (dst, args) = match instr {
                ArcInstr::Construct { dst, args, .. } | ArcInstr::Reuse { dst, args, .. } => {
                    (*dst, args.as_slice())
                }
                _ => continue,
            };
            // The aggregate must carry an RC burden whose drop-glue walks heap
            // fields — otherwise no field-drop ever decs the slice and a
            // keep-alive would orphan a +1 (leak).
            if !is_burden_carrying_aggregate(dst, func, pool) {
                continue;
            }
            // The aggregate's field-drop must run IN-SCOPE. Two admitting shapes:
            // (1) dropped directly in-scope (`!aggregate_transferred_out`); OR
            // (2) pushed `[own]` into a collection that is iter-consumed in-scope
            //     and NOT returned — the field-drop still runs locally (via the
            //     receiver collection's `elem_dec_fn` at its iter-drop), so the
            //     RL-1 keep-alive on the stored slice view is load-bearing
            //     (`for p in parts yield Some(p); for opt in opts do ..`). A
            //     returned receiver is Phase 6.68b's domain; a genuinely-escaping
            //     transfer (returned aggregate, user-call `[own]`) has its
            //     field-drop run elsewhere, so a keep-alive there orphans a +1 ->
            //     leak (the over-fire boundary). Spec: Annex E §AIMS RL-1 + RL-2.
            if aggregate_transferred_out(rep_of(dst), func, &rep_of)
                && !aggregate_pushed_into_in_scope_consumed_collection(
                    rep_of(dst),
                    func,
                    &rep_of,
                    interner,
                )
            {
                continue;
            }
            // De-duplicate: a slice value passed in two field positions of one
            // aggregate (e.g. `Pair { a: p, b: p }`) is decced once per occurrence
            // by the field-drop, so one keep-alive per occurrence is correct;
            // collect each occurrence.
            for &arg in args {
                if warrants_keepalive(arg) {
                    inserts.push(NestedIterKeepAlive {
                        block: b,
                        at: Some(i),
                        var: arg,
                    });
                }
            }
        }
    }

    // Apply back-to-front per block so earlier indices stay valid; multiple
    // inserts at the same instruction index all land before the Construct.
    inserts.sort_by(|a, b| {
        b.block
            .cmp(&a.block)
            .then(b.at.unwrap_or(usize::MAX).cmp(&a.at.unwrap_or(usize::MAX)))
    });
    for ins in inserts {
        if let Some(block) = func.blocks.get_mut(ins.block) {
            let inc = ArcInstr::BurdenInc { var: ins.var };
            match ins.at {
                Some(at) => block.body.insert(at.min(block.body.len()), inc),
                None => block.body.push(inc),
            }
        }
    }
}

/// Phase 6.68b (probe): emit a keep-alive `BurdenInc` on a borrowed iter-element
/// view PUSHED `[own]` into a collection that OUTLIVES the source iterator (the
/// element-escape shape) — RL-1 duplication keep-alive.
///
/// `for w in coll do { result = result.push(value: w) }` projects `w` as a
/// `Project @__iter_next.1` borrow-view of the SOURCE collection's element backing
/// (a `collect_iter_element_defs` member, excluded from `owned_vars_needing_rc`, so
/// the base burden walk emits NO inc on it). The element is pushed `[own]` into a
/// SEPARATE `result` collection. Now TWO distinct `elem_dec_fn` paths reach the one
/// element backing: (a) the source's `ori_iter_drop` -> `ori_buffer_rc_dec` decs
/// each element when the source buffer frees, and (b) `result`'s `elem_dec_fn`
/// decs the stored copy at `result`'s drop. The backing started at rc 1 (owned by
/// the source), so without a keep-alive the first dec frees it and the second
/// double-frees (exit 134). The oracle emits one keep-alive `RcInc <view>` before
/// the push: the source iter-drop drops rc 2->1, `result`'s `elem_dec_fn` drops rc
/// 1->0 (the backing's true free). RL-1 (`RL1_emit_iff_not_elidable`): pushing a
/// still-shared borrowed element view `[own]` into a second collection is a
/// duplicating, non-move-once use -> emit the inc; the receiving collection's
/// `elem_dec_fn` is the balancing release (`RL1_duplication_balanced` /
/// `RL2_release_exactly_once`). The `[inc, elem_dec]` pair nets 0.
///
/// Gate (the over-fire boundary): the inc fires ONLY when (a) the pushed arg is in
/// `collect_iter_element_defs` (a borrowed element view — an OWNED, freshly-built
/// element already carries its own push-arg inc and is NOT in this set), (b) the
/// arg's repr is heap (`RcPointer | FatValue` — a scalar `[int]` element view
/// carries no backing the receiver's `elem_dec_fn` frees), (c) the push receiver
/// (`@push`/`ori_list_push` arg 0) is a collection (`is_collection_dst`), AND
/// (d) the receiver collection's lineage is RETURNED from the function
/// (`collection_receiver_returned`). Condition (d) is the precise
/// discriminator between the FAILING returned-result case (`collect_words` returns
/// the built list -> the caller's drop is a SECOND elem-dec distinct from the
/// in-callee `ori_iter_drop`) and the BENIGN in-scope case (`for w in coll do {
/// r = r.push(w) }; for w in r do .. }` all in ONE function: the source iter-drop
/// and the in-scope `r` drop are sequenced so the existing accounting balances
/// them, and a keep-alive there orphans a +1 -> leak). A scalar-only element use
/// (`p.len()`, no push) never reaches a push arg position. Probe-gated -> default
/// codegen byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
fn emit_iter_element_pushed_into_returned_collection_keepalive_inc(
    func: &mut ArcFunction,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
) {
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    if iter_element_defs.is_empty() {
        return;
    }
    let push_name = interner.intern("push");
    let list_push_name = interner.intern("ori_list_push");
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // A pushed arg warrants the element-escape keep-alive iff it is a heap
    // (`RcPtr`/`FatVal`) borrowed iter-element view of an enclosing collection.
    let warrants_keepalive = |arg: ArcVarId| -> bool {
        iter_element_defs.contains(&arg)
            && matches!(
                func.var_repr(arg),
                Some(ValueRepr::RcPointer | ValueRepr::FatValue)
            )
    };

    // True iff `callee(args)` is a collection-append whose receiver (args[0]) is a
    // collection lineage transferred OUT of the function. Returns the element-arg
    // positions (every `[own]` non-receiver arg that warrants the keep-alive).
    let push_escape_element_positions =
        |callee: Name, args: &[ArcVarId], arg_ownership: &[ArgOwnership]| -> Vec<ArcVarId> {
            if callee != push_name && callee != list_push_name {
                return Vec::new();
            }
            let Some(&recv) = args.first() else {
                return Vec::new();
            };
            // Receiver must be a collection whose lineage is RETURNED — the over-fire
            // boundary that excludes the in-scope-only case (an in-scope receiver is
            // iterated/dropped within this function, where the base accounting already
            // balances the source iter-drop against it).
            if !is_collection_dst(recv, func, pool) && !is_collection_dst(rep_of(recv), func, pool)
            {
                return Vec::new();
            }
            if !collection_receiver_returned(rep_of(recv), func, &rep_of) {
                return Vec::new();
            }
            // Element args: every owned NON-receiver position that warrants the
            // keep-alive (a borrowed iter-element view of an enclosing collection).
            let mut elems = Vec::new();
            for (pos, &arg) in args.iter().enumerate().skip(1) {
                if arg_ownership.get(pos) == Some(&ArgOwnership::Owned) && warrants_keepalive(arg) {
                    elems.push(arg);
                }
            }
            elems
        };

    let mut inserts: Vec<NestedIterKeepAlive> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        // Body `Apply @push(..)` (non-unwinding append).
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                for arg in push_escape_element_positions(*callee, args, arg_ownership) {
                    inserts.push(NestedIterKeepAlive {
                        block: b,
                        at: Some(i),
                        var: arg,
                    });
                }
            }
        }
        // Terminator `Invoke @push(..) normal/unwind` (a may-unwind COW append —
        // `result.push(value: w)` lowers to this). The keep-alive appends at the
        // END of the body, before the terminator reads the element arg.
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            for arg in push_escape_element_positions(*callee, args, arg_ownership) {
                inserts.push(NestedIterKeepAlive {
                    block: b,
                    at: None,
                    var: arg,
                });
            }
        }
    }

    // Apply back-to-front per block so earlier indices stay valid; multiple
    // inserts at the same instruction index all land before the push.
    inserts.sort_by(|a, b| {
        b.block
            .cmp(&a.block)
            .then(b.at.unwrap_or(usize::MAX).cmp(&a.at.unwrap_or(usize::MAX)))
    });
    for ins in inserts {
        if let Some(block) = func.blocks.get_mut(ins.block) {
            let inc = ArcInstr::BurdenInc { var: ins.var };
            match ins.at {
                Some(at) => block.body.insert(at.min(block.body.len()), inc),
                None => block.body.push(inc),
            }
        }
    }
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

/// Phase 6.68c: strip the surplus cross-call keep-alive `BurdenInc` on a
/// callee-returned fresh collection acquired at >=2 call sites and iter-consumed
/// in the caller (see the call-site comment for the leak shape).
///
/// For each qualifying lineage rep, [`compute_returned_collection_surplus_inc_strips`]
/// selects EXACTLY ONE `(block, instr_index)` `BurdenInc` whose removal nets the
/// lineage's explicit ops to 0 on every acquire-reachable terminal; this pass
/// removes those ops. Indices are resolved against the un-mutated body and applied
/// descending-within-block so an earlier removal never invalidates a later one.
/// Spec: Annex E §AIMS RL-1 + RL-2.
fn strip_returned_collection_multi_call_surplus_inc(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if std::env::var_os("ORI_DISABLE_RETURNED_COLLECTION_SURPLUS_INC_STRIP").is_some() {
        return;
    }
    let strips =
        compute_returned_collection_surplus_inc_strips(func, pool, interner, same_alloc_reps);
    if strips.is_empty() {
        return;
    }
    // Group by block; remove descending-within-block (a removal shifts later
    // indices, so earlier-index removals must follow later-index ones).
    let mut by_block: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    for (b, i) in strips {
        by_block.entry(b).or_default().push(i);
    }
    for (block_idx, mut indices) in by_block {
        let Some(block) = func.blocks.get_mut(block_idx) else {
            continue;
        };
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for i in indices {
            if i < block.body.len() && matches!(block.body[i], ArcInstr::BurdenInc { .. }) {
                block.body.remove(i);
            }
        }
    }
}

/// Compute the `(block, instr_index)` of each surplus cross-call `BurdenInc` to
/// strip for Phase 6.68c.
///
/// A lineage rep (keyed by `same_alloc_reps`) qualifies when ALL hold (the
/// over-fire boundary):
///  - (a) the function has >=2 returned SCALAR-list acquires (a USER-callee —
///    non-protocol-builtin, non-builtin-method — `RcPtr` `Tag::List` return whose
///    element is `ArcClass::Scalar`, so the buffer dec is decoupled from element
///    accounting — see [`returned_fresh_collection_acquire`]), and THIS rep is one
///    of them, LIVE-OUT across a DISTINCT other acquire (the cross-call live-across
///    shape that earns the base walk's spurious duplicating inc; the later call
///    borrows the source, not this result);
///  - (b) it is iter-consumed in the caller (`@iter(arg [own])` -> `ori_iter_drop`):
///    the iter-drop is the acquired ref's single release, so the lineage's EXPLICIT
///    burden ops alone must balance — a returned collection released by an explicit
///    scope-exit dec instead is already balanced by the base path and declines;
///  - (c) the lineage's explicit-op net (`Σ BurdenInc − Σ BurdenDec` over its rep)
///    is exactly +1 — the over-inc gate (a balanced net-0 sibling result, never
///    live across a call, declines);
///  - (d) it is NOT loop-carried (`compute_loop_blocks`): `same_alloc_reps` drops
///    the Jump-phi back-edge by design, so a loop-carried lineage's release
///    attributes to a different rep and the per-path net mis-computes (the M-series
///    blind spot);
///  - (e) EXACTLY ONE of the rep's inc sites is the structural surplus — a
///    `BurdenInc` located in ANOTHER acquire's call block where this lineage is NOT
///    consumed (the base walk's live-across duplicating inc). A genuine keep-alive
///    inc sits in a block that CONSUMES this lineage (before `@iter(this [own])`),
///    so it is never selected. The net-+1 gate proves a single over-inc; >1 surplus
///    candidate signals an unmodelled shape and declines.
///
/// Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`) + RL-2
/// (`RL2_release_exactly_once`).
pub(super) fn compute_returned_collection_surplus_inc_strips(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> Vec<(usize, usize)> {
    // Lineage rep = `same_alloc_reps` (NOT jump-threaded). `same_alloc_reps`
    // excludes the Jump-phi edge by design, keeping a returned collection's rep
    // tight to its SSA-alias chain (`{%5, %12, %22}` — the Let-Var aliases) without
    // merging into the downstream loop's block-param phi successors (separate reps)
    // or the co-merged source/result lineages a jump-threaded rep would union. The
    // tight rep is exactly what the imbalance diagnostic keys on (`rep=5 net=1`),
    // so the explicit-op net is the surplus-inc verdict for this lineage alone.
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let preds = crate::graph::compute_predecessors(func);
    let dom = crate::graph::DominatorTree::build(func);
    let loop_blocks = compute_loop_blocks_local(func, &preds, &dom);
    let unwind_blocks = compute_unwind_reachable_blocks(func);
    let iter_name = interner.intern("iter");

    let (acquires, acquire_call_blocks) =
        collect_returned_collection_acquires(func, pool, interner, same_alloc_reps);
    // The multi-call shape needs >=2 returned-fresh-collection acquires in the
    // function (the N>=2 source-borrow shape: two calls returning fresh results).
    if acquires.len() < 2 {
        return Vec::new();
    }

    let mut strips: Vec<(usize, usize)> = Vec::new();
    let mut seen_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for &(rep, _acquire_block) in &acquires {
        if !seen_reps.insert(rep) {
            continue;
        }
        // Call blocks of every OTHER returned-collection acquire — where this
        // lineage's spurious live-across inc could land.
        let other_acquire_blocks: FxHashSet<usize> = acquire_call_blocks
            .iter()
            .filter(|&&(other_rep, _)| other_rep != rep)
            .map(|&(_, cb)| cb)
            .collect();
        // The qualifying result is LIVE-OUT across a DISTINCT, LATER acquire of a
        // SECOND returned-fresh-collection — the cross-call live-across shape that
        // earned the spurious duplicating inc (the later acquire's call borrows the
        // source, NOT this result). A result NOT live across any other acquire (the
        // SECOND result, defined after both calls) is correctly balanced and skips.
        let live_across_other_acquire = acquires.iter().any(|&(other_rep, other_block)| {
            other_rep != rep
                && lineage_live_at_block_entry(func, same_alloc_reps, rep, other_block, &preds)
        });
        if !live_across_other_acquire {
            continue;
        }
        // Iter-consumed in the caller (the acquired-ref release is the iter-drop, so
        // the explicit ops alone must net 0). An inline `@iter(arg [own])` with a
        // lineage arg is the signal.
        if !lineage_iter_consumed_in_caller(func, same_alloc_reps, rep, iter_name) {
            continue;
        }
        // Lineage explicit-op delta (`Σ BurdenInc − Σ BurdenDec` per block) + inc
        // sites, both keyed by the same-alloc rep. Skip unwind-cleanup incs.
        let mut delta: Vec<i64> = vec![0; func.blocks.len()];
        let mut inc_sites: Vec<(usize, usize)> = Vec::new();
        let mut touches_loop = false;
        for (b, block) in func.blocks.iter().enumerate() {
            for (i, instr) in block.body.iter().enumerate() {
                match instr {
                    ArcInstr::BurdenInc { var } if rep_of(*var) == rep => {
                        delta[b] += 1;
                        if loop_blocks.contains(&b) {
                            touches_loop = true;
                        }
                        if !unwind_blocks.contains(&b) {
                            inc_sites.push((b, i));
                        }
                    }
                    _ if crate::aims::verify::burden_delta::whole_var_dec_target(instr)
                        .is_some_and(|v| rep_of(v) == rep) =>
                    {
                        delta[b] -= 1;
                        if loop_blocks.contains(&b) {
                            touches_loop = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        // Loop-carried lineages defer (the back-edge blind spot).
        if touches_loop {
            continue;
        }
        // Explicit-op net exactly +1 (over-inc by one) — the over-inc gate. A
        // balanced (net 0) iter-consumed lineage (the SECOND result, never live
        // across a call) declines here.
        if delta.iter().sum::<i64>() != 1 {
            continue;
        }
        // The surplus inc is STRUCTURAL: this lineage's `BurdenInc` located in a
        // block that performs a DIFFERENT returned-collection acquire WHERE THIS
        // LINEAGE IS NOT AN ARGUMENT — the base walk's spurious live-across-call
        // duplicating inc (the later call borrows the source, not this result). The
        // `other_acquire_blocks` set is every block carrying another such acquire
        // (the result's defining block for a body `Apply`, or the `Invoke`-
        // terminator's own block — the inc lands in that block before the call). A
        // genuine keep-alive inc (before `@iter(this [own])`) sits in a block that
        // CONSUMES this lineage, so it is never in this set. Net +1 + exactly one
        // such surplus inc is the precise verdict.
        let surplus: Vec<(usize, usize)> = inc_sites
            .iter()
            .copied()
            .filter(|&(b, _)| {
                other_acquire_blocks.contains(&b)
                    && !block_consumes_lineage(func, same_alloc_reps, rep, b)
            })
            .collect();
        // Strip EXACTLY ONE surplus inc (the net-+1 gate proves a single over-inc;
        // >1 candidate would mean an unmodelled shape — decline conservatively).
        if surplus.len() == 1 {
            strips.push(surplus[0]);
        }
    }
    strips
}

/// A list of `(same-alloc rep, block-index)` acquire sites — either the
/// first-live-block list or the call-block list of
/// [`collect_returned_collection_acquires`].
type ReturnedCollectionAcquires = Vec<(ArcVarId, usize)>;

/// Collect every freshness-preserving user-callee RcPtr-collection return acquire
/// in `func`, as two parallel lists keyed by the same-alloc rep:
/// - `acquires`: `(rep, first_live_block)` — the block the result first EXISTS in
///   (a body `Apply` defines it in `b`; an `Invoke`-terminator's result first lives
///   at the `normal` successor, where Phase-5 prepends its acquire inc).
/// - `acquire_call_blocks`: `(rep, call_block)` — the block CARRYING the acquire's
///   call (the terminator's own block for an `Invoke`; `b` for a body `Apply`). The
///   spurious live-across inc lands in ANOTHER acquire's `call_block`, so
///   `call_block` is what the surplus-inc strip keys on.
///
/// Keyed at the acquire-collection / surplus-selection concept seam.
/// Spec: Annex E §AIMS RL-1.
fn collect_returned_collection_acquires(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> (ReturnedCollectionAcquires, ReturnedCollectionAcquires) {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let mut acquires: Vec<(ArcVarId, usize)> = Vec::new();
    let mut acquire_call_blocks: Vec<(ArcVarId, usize)> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if returned_fresh_collection_acquire(*dst, *callee, func, pool, interner) {
                    acquires.push((rep_of(*dst), b));
                    acquire_call_blocks.push((rep_of(*dst), b));
                }
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            normal,
            ..
        } = &block.terminator
        {
            if returned_fresh_collection_acquire(*dst, *callee, func, pool, interner) {
                acquires.push((rep_of(*dst), normal.index()));
                acquire_call_blocks.push((rep_of(*dst), b));
            }
        }
    }
    (acquires, acquire_call_blocks)
}

/// Whether block `b` CONSUMES lineage `rep` — any body instruction or the
/// terminator references the rep (a genuine keep-alive inc sits in a consuming
/// block; the spurious cross-call inc does not).
fn block_consumes_lineage(
    func: &ArcFunction,
    reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    b: usize,
) -> bool {
    let rep_of = |v: ArcVarId| reps.get(&v).copied().unwrap_or(v);
    let Some(block) = func.blocks.get(b) else {
        return false;
    };
    let body_uses = block.body.iter().any(|instr| {
        // A `BurdenInc`/`BurdenDec` on the rep is NOT a consume (it is the op under
        // scrutiny); a genuine consume is a real use (call arg, project, etc.).
        !matches!(
            instr,
            ArcInstr::BurdenInc { .. } | ArcInstr::BurdenDec { .. }
        ) && instr.used_vars().iter().any(|&v| rep_of(v) == rep)
    });
    body_uses
        || block
            .terminator
            .used_vars()
            .iter()
            .any(|&v| rep_of(v) == rep)
}

/// Whether lineage `rep` is live at the ENTRY of `block` — its rep is used at or
/// after `block` along some forward path (a value live across a later call site
/// has its rep referenced in/after that block). Conservative liveness via forward
/// reachability of any using block from `block`.
fn lineage_live_at_block_entry(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block: usize,
    _preds: &[Vec<usize>],
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let reachable = forward_reachable_local(func, &[block]);
    for &b in &reachable {
        let Some(blk) = func.blocks.get(b) else {
            continue;
        };
        let uses_rep = blk
            .body
            .iter()
            .any(|instr| instr.used_vars().iter().any(|&v| rep_of(v) == rep))
            || blk.terminator.used_vars().iter().any(|&v| rep_of(v) == rep)
            || matches!(&blk.terminator, ArcTerminator::Jump { args, .. } if args.iter().any(|&a| rep_of(a) == rep));
        if uses_rep {
            return true;
        }
    }
    false
}

/// Whether `dst` (an `Apply`/`Invoke` result) is an `RcPtr` SCALAR-ELEMENT list
/// acquired from a USER callee return — the caller-side "I now own the callee's
/// returned `[scalar]` at rc=1" signal.
///
/// The callee must be a USER function (not a protocol builtin — `Name` not
/// `__`-prefixed — and not a known builtin method), `dst` an `RcPtr` `Tag::List`,
/// AND the list's element type a PRIMITIVE scalar (`pool.list_elem` `Tag` is
/// primitive). The lineage carries NO `fresh_rc_alloc_dst` self-alloc +1 (the
/// allocation happened inside the callee), so its EXPLICIT burden ops alone govern
/// balance — the property Phase 6.68c relies on.
///
/// SCALAR-ELEMENT gate (the over-fire boundary against the heap-element shape): a
/// `[str]`/`[[int]]`/`[{..}]` returned collection's buffer dec walks `elem_dec_fn`
/// over its heap elements, and those elements may be SHARED with the iterated
/// source (a `for w in words yield w` yields element VIEWS the source still holds),
/// so stripping the buffer's keep-alive inc frees the buffer one ref early and the
/// shared source elements get double-decced (a `[str]` double-free, exit 134). A
/// `[int]`/`[float]`/`[bool]` buffer has NO `elem_dec_fn` (scalar elements carry no
/// RC), so its buffer ref-count is decoupled from any element accounting and the
/// surplus-inc strip is sound. The heap-element shape is a DISTINCT sub-root (the
/// yield element-sharing accounting) left to a later cycle.
///
/// `ReturnContract.preserves_freshness` is NOT used: the shipped extractor
/// under-reports it for the `for...yield` `ori_list_take` finalizer (a finalizer
/// call is not recognized as a fresh producer), so gating on it misses the
/// canonical leaking shape. The non-builtin user-callee + RcPtr-scalar-list-result
/// signal is the reliable acquire marker; the explicit-op-net-+1 + iter-consumed +
/// live-across guards downstream supply the precise over-fire boundary.
fn returned_fresh_collection_acquire(
    dst: ArcVarId,
    callee: Name,
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
) -> bool {
    if !matches!(func.var_repr(dst), Some(ValueRepr::RcPointer)) {
        return false;
    }
    // `Tag::List` with a SCALAR element (`ArcClass::Scalar` — int/float/bool/char/
    // byte/duration/size/ordering; NOT `str`/`[T]`/`{K:V}`/`Set`, which the
    // classifier maps to `DefiniteRef`). A scalar element carries NO `elem_dec_fn`,
    // so the buffer dec is decoupled from any element accounting and the surplus-inc
    // strip is sound. `Tag::is_primitive()` is the WRONG gate here — it classifies
    // `Tag::Str` as primitive (discriminant < 16) yet `str` is a heap fat-pointer
    // whose buffer dec walks element backing; `ArcClassifier` correctly maps
    // `Idx::STR -> DefiniteRef`.
    let list_ty = pool.resolve_fully(func.var_type(dst));
    if pool.tag(list_ty) != ori_types::Tag::List {
        return false;
    }
    let elem_ty = pool.resolve_fully(pool.list_elem(list_ty));
    if !crate::classify::ArcClassification::is_scalar(
        &crate::classify::ArcClassifier::new(pool),
        elem_ty,
    ) {
        return false;
    }
    // USER callee only: a protocol builtin (`__`-prefixed) or a known builtin
    // method returns a collection too (`@map`/`@filter`/`@concat`), but those are
    // compiler-modelled and their results are recognized as self-allocs by the
    // base path — only the user-callee return acquires an externally-allocated
    // buffer whose explicit ops alone must balance.
    let is_protocol_builtin = interner
        .try_lookup(callee)
        .is_some_and(|n| n.starts_with("__"));
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    !is_protocol_builtin && !builtins.is_builtin(callee)
}

/// Whether lineage `rep` is iter-consumed by an inline `@iter(arg [own])` in the
/// caller (the acquired ref's release is then the paired `ori_iter_drop`).
fn lineage_iter_consumed_in_caller(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    iter_name: Name,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    func.blocks.iter().any(|block| {
        block.body.iter().any(|instr| {
            matches!(
                instr,
                ArcInstr::Apply { func: callee, args, arg_ownership, .. }
                    if *callee == iter_name
                        && args.first().is_some_and(|&a| rep_of(a) == rep)
                        && arg_ownership.first() == Some(&ArgOwnership::Owned)
            )
        })
    })
}

/// Forward-reachable block set from `starts` (inclusive) via CFG successors.
/// Local mirror of the burden-elim `forward_reachable` (private there).
fn forward_reachable_local(func: &ArcFunction, starts: &[usize]) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = starts.to_vec();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    visited
}

/// Blocks inside a natural loop (target of a back-edge `b -> h` where `h`
/// dominates `b`, plus the loop body). Local mirror of the burden-elim
/// `compute_loop_blocks` (private there) — the loop-carried scope guard for
/// Phase 6.68c.
fn compute_loop_blocks_local(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    dom: &crate::graph::DominatorTree,
) -> FxHashSet<usize> {
    use crate::ir::ArcBlockId;
    let mut loop_blocks: FxHashSet<usize> = FxHashSet::default();
    let n = func.blocks.len();
    for (b, block) in func.blocks.iter().enumerate() {
        let tail = ArcBlockId::new(u32::try_from(b).unwrap_or(u32::MAX));
        for h in crate::graph::successor_block_ids(&block.terminator) {
            if dom.dominates(h, tail) {
                loop_blocks.insert(h.index());
                loop_blocks.insert(b);
                let mut stack = vec![b];
                while let Some(x) = stack.pop() {
                    if x == h.index() {
                        continue;
                    }
                    for &p in &preds[x] {
                        if p < n && loop_blocks.insert(p) {
                            stack.push(p);
                        }
                    }
                }
            }
        }
    }
    loop_blocks
}

/// Record each arg of `callee` that iter-consumes an `RcPtr`-collection lineage as
/// an iter-consuming use of that rep. Two iter-consume kinds:
///  - USER callee: a param whose `ParamContract.iter_consumes` is true (a user fn
///    whose body `@iter [own]` -> `ori_iter_drop`s the arg).
///  - INLINE for-loop: the `@iter` PROTOCOL builtin (`for x in coll` lowers to
///    `Apply @iter(coll [own])` -> `ori_iter_drop`). `@iter` has no user
///    `MemoryContract`, so the contract-keyed path misses it; the iter-consume
///    signature is `callee == "iter"` with the arg passed `[own]` — the for-loop's
///    iterator owns the buffer and frees it via the paired `ori_iter_drop`.
#[expect(
    clippy::too_many_arguments,
    reason = "shared read-only context (func/pool/contracts/rep_of) + the accumulator; \
              bundling into a struct would not reduce the genuine input set"
)]
fn record_iter_consume_uses(
    callee: Name,
    args: &[ArcVarId],
    arg_ownership: &[ArgOwnership],
    iter_name: Name,
    block: usize,
    instr: Option<usize>,
    contracts: &FxHashMap<Name, MemoryContract>,
    func: &ArcFunction,
    pool: &Pool,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    uses_per_rep: &mut FxHashMap<ArcVarId, Vec<IterConsumeUse>>,
) {
    let mut record = |arg: ArcVarId| {
        if !matches!(func.var_repr(arg), Some(ValueRepr::RcPointer)) {
            return;
        }
        if !is_collection_dst(rep_of(arg), func, pool) && !is_collection_dst(arg, func, pool) {
            return;
        }
        uses_per_rep
            .entry(rep_of(arg))
            .or_default()
            .push(IterConsumeUse { block, instr });
    };

    // INLINE for-loop iter-consume: `@iter(coll [own])`. The for-loop's iterator
    // takes ownership of the collection buffer and frees it via the paired
    // `ori_iter_drop`, so an `[own]` `@iter` arg is an iter-consume position
    // identical in transfer to a `ParamContract.iter_consumes` user callee.
    if callee == iter_name {
        for (pos, &arg) in args.iter().enumerate() {
            if arg_ownership
                .get(pos)
                .is_some_and(|o| *o == ArgOwnership::Owned)
            {
                record(arg);
            }
        }
        return;
    }

    // USER callee iter-consume: a param contract whose `iter_consumes` is true.
    let Some(contract) = contracts.get(&callee) else {
        return;
    };
    for (pos, &arg) in args.iter().enumerate() {
        let Some(param) = contract.params.get(pos) else {
            continue;
        };
        if !param.iter_consumes {
            continue;
        }
        record(arg);
    }
}

/// Blocks reachable ONLY via an unwind edge or that are `Resume` cleanup pads —
/// where a lineage dec is the panic-path release of the keep-alive inc.
fn compute_unwind_reachable_blocks(func: &ArcFunction) -> FxHashSet<usize> {
    let mut unwind: FxHashSet<usize> = FxHashSet::default();
    // Seed: every `Invoke`/`InvokeIndirect` unwind target + every `Resume` block.
    let mut stack: Vec<usize> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        match &block.terminator {
            ArcTerminator::Invoke { unwind: u, .. }
            | ArcTerminator::InvokeIndirect { unwind: u, .. } => {
                if unwind.insert(u.index()) {
                    stack.push(u.index());
                }
            }
            ArcTerminator::Resume => {
                if unwind.insert(b) {
                    stack.push(b);
                }
            }
            _ => {}
        }
    }
    // A block reachable from an unwind seed only via the seed subgraph is unwind
    // cleanup. Conservative: include the full successor closure of each seed —
    // a normal-path block that is ALSO reachable from a non-unwind predecessor is
    // kept out by the seed-only nature of `Resume`/unwind targets in practice.
    while let Some(b) = stack.pop() {
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        for s in crate::graph::successor_block_ids(&block.terminator) {
            if unwind.insert(s.index()) {
                stack.push(s.index());
            }
        }
    }
    unwind
}

/// Whether `var` is an owned closure VALUE — `ValueRepr::FatValue` whose resolved
/// type is `Tag::Function` (the `RcStrategy::Closure` discriminator: a fat value
/// over a function type, whose `env_ptr` component is reference-counted). A `str`
/// (also `FatValue`) is excluded by the `Tag::Function` check; a scalar / aggregate
/// / `RcPointer` value is excluded by the repr check.
fn is_owned_closure_value(var: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    matches!(func.var_repr(var), Some(ValueRepr::FatValue))
        && pool.tag(pool.resolve_fully(func.var_type(var))) == ori_types::Tag::Function
}

/// Whether any member of closure lineage `rep` is TRANSFERRED OUT — its env-drop
/// runs ELSEWHERE, so a scope-exit dec here would double-free. Transfer kinds
/// (RL-2): `Return`, owned `Construct`/`Reuse`/`CollectionReuse`/`PartialApply`
/// capture arg, owned `Apply`/`ApplyIndirect`/`Invoke`/`InvokeIndirect` arg, `Set`
/// value, `Jump` arg. Invoking a closure as the RECEIVER of `ApplyIndirect` is NOT
/// a transfer (the closure survives the call as a value) — `is_owned_position`
/// reports the receiver position correctly, so a closure used only as an invoke
/// receiver is not flagged. `rep_of` threads Let/Jump aliases. Spec: Annex E §AIMS
/// RL-2 (`RL2_transfer_kinds_no_dec`).
fn closure_lineage_transferred_out(
    rep: ArcVarId,
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> bool {
    for block in &func.blocks {
        for instr in &block.body {
            // A PartialApply capture-arg consumes the closure into a new closure
            // env (transfer). `is_owned_position` covers Construct/Reuse/Apply/Set
            // owned args; PartialApply args are all captures (owned), so check them
            // explicitly.
            if let ArcInstr::PartialApply { args, .. } = instr {
                if args.iter().any(|&a| rep_of(a) == rep) {
                    return true;
                }
            }
            for (pos, &used) in instr.used_vars().iter().enumerate() {
                if rep_of(used) == rep && instr.is_owned_position(pos) {
                    return true;
                }
            }
        }
        match &block.terminator {
            ArcTerminator::Return { value } => {
                if rep_of(*value) == rep {
                    return true;
                }
            }
            ArcTerminator::Jump { args, .. } => {
                if args.iter().any(|&a| rep_of(a) == rep) {
                    return true;
                }
            }
            term => {
                for (pos, &used) in term.used_vars().iter().enumerate() {
                    if rep_of(used) == rep && term.is_owned_position(pos) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Phase 6.69 (probe): emit a scope-exit `BurdenDec` for every owned closure VALUE
/// (`is_owned_closure_value`) whose lineage the base burden walk left without a
/// release.
///
/// A closure value's `env_ptr` carries the captured allocations' RC; RL-2 mandates
/// a release at its non-transfer last use. The base burden walk emits this for a
/// `let`-bound closure but MISSES anonymous chained intermediates — a curried
/// `fst("hello")(0)` produces the `PartialApply` result + a closure-returning
/// `ApplyIndirect` result as intermediates that are never bound and fall outside
/// `owned_vars_needing_rc` / the move-alias transfer set, so their envs leak under
/// the flag. The oracle decs each once at its last (invoking) read.
///
/// Gate (the over-fire boundary): the dec fires ONLY when (a) the value is an owned
/// closure (`FatValue` + `Tag::Function`), (b) it is NOT a function param, (c) it is
/// NOT a borrowed def (`all_borrowed_defs` — a `Project`ed closure field is borrowed,
/// owned by its aggregate), (d) the lineage carries NO existing burden op (an
/// already-decced `let`-bound closure is skipped — adding a second dec double-frees),
/// AND (e) the lineage is NOT transferred out (a closure stored in an aggregate /
/// returned / passed `[own]` has its env freed by the consumer's drop). One dec is
/// emitted at the lineage's last (non-defining) use. Probe-gated -> default codegen
/// byte-identical. Spec: Annex E §AIMS RL-2 (`RL2_dec_at_last_use` /
/// `rl2_emits_dec(.LastReadBeforeScopeExit) = true`).
fn emit_owned_closure_scope_exit_dec(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    // Insertion point for one scope-exit dec: `at = None` => terminator-position
    // last use (append to body).
    struct ClosureDec {
        block: usize,
        at: Option<usize>,
        var: ArcVarId,
    }

    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // Iter-element-views + the `@__iter_next` `args[1]` elem_ty_marker PHANTOM. The
    // marker is a `Let { Literal(0) }` typed as the element type — for a `[closure]`
    // list it is `FatValue` + `Tag::Function`, matching `is_owned_closure_value`, but
    // is NOT a real closure (its LLVM repr is `i64 0`, so a `[Closure]` dec extracts
    // env from garbage). An iter-element closure VIEW (`Project @__iter_next.1` of a
    // `[closure]`) is a BORROW into the list buffer, freed by the list's
    // `elem_dec_fn`. Both are excluded here (the same suppression the keep-alive
    // passes apply). Spec: Annex E §AIMS Protocol Builtins + RL-2.
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);

    // Closure-value reps that are owned (not param, not borrowed, not an iter-element
    // view / marker). A param's var is excluded so a borrowed/owned closure
    // PARAMETER is never decced here (the caller owns it).
    let params: FxHashSet<ArcVarId> = func.params.iter().map(|p| p.var).collect();
    let mut closure_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    let consider_closure_dst = |dst: ArcVarId, closure_reps: &mut FxHashSet<ArcVarId>| {
        if !params.contains(&dst)
            && !all_borrowed_defs.contains(&dst)
            && !iter_element_defs.contains(&dst)
            && is_owned_closure_value(dst, func, pool)
        {
            closure_reps.insert(rep_of(dst));
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                consider_closure_dst(dst, &mut closure_reps);
            }
        }
        // A may-unwind user/closure call defines its result on the NORMAL path via
        // the `Invoke` / `InvokeIndirect` TERMINATOR — a closure-returning callee
        // (`make_adder(5)` lowered to `Invoke`) lands its closure here, not in the
        // body. Include the terminator dst so a closure value from a may-unwind call
        // is covered.
        if let ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } =
            &block.terminator
        {
            consider_closure_dst(*dst, &mut closure_reps);
        }
    }
    if closure_reps.is_empty() {
        return;
    }

    // Reps whose lineage already carries a burden op (the base walk handled them —
    // the let-bound closure case). Skip these: a second dec double-frees.
    let mut reps_with_burden: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var }
                | ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => {
                    reps_with_burden.insert(rep_of(*var));
                }
                _ => {}
            }
        }
    }

    // Per eligible rep, find the last (non-defining) use point and the member used
    // there; emit one `BurdenDec` after it.
    let mut inserts: Vec<ClosureDec> = Vec::new();
    for &rep in &closure_reps {
        if reps_with_burden.contains(&rep) {
            continue;
        }
        if closure_lineage_transferred_out(rep, func, &rep_of) {
            continue;
        }
        // Last use = max (block, instr-or-terminator) where a rep member is read.
        let mut last: Option<(usize, Option<usize>, ArcVarId)> = None;
        for (b, block) in func.blocks.iter().enumerate() {
            for (i, instr) in block.body.iter().enumerate() {
                let def = instr.defined_var();
                for &used in &instr.used_vars() {
                    if rep_of(used) == rep && Some(used) != def {
                        last = Some((b, Some(i), used));
                    }
                }
            }
            for &used in &block.terminator.used_vars() {
                if rep_of(used) == rep {
                    last = Some((b, None, used));
                }
            }
        }
        if let Some((b, at, var)) = last {
            inserts.push(ClosureDec { block: b, at, var });
        }
    }

    // Apply back-to-front per block so earlier indices stay valid.
    inserts.sort_by(|a, b| {
        b.block
            .cmp(&a.block)
            .then(b.at.unwrap_or(usize::MAX).cmp(&a.at.unwrap_or(usize::MAX)))
    });
    for ins in inserts {
        if let Some(block) = func.blocks.get_mut(ins.block) {
            let dec = ArcInstr::BurdenDec { var: ins.var };
            match ins.at {
                // Insert AFTER the last-use instruction (release once the value's
                // final read has executed).
                Some(at) => block.body.insert((at + 1).min(block.body.len()), dec),
                None => block.body.push(dec),
            }
        }
    }
}

/// Whether lineage `rep` is live at any program point AFTER the use at
/// `(block, instr)` — a later instruction operand, terminator arg, or downstream
/// block reference. A use that is the lineage's last reference returns false.
fn lineage_live_out_after_use(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block: usize,
    instr: Option<usize>,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let Some(start) = func.blocks.get(block) else {
        return false;
    };
    // Remaining instructions in the use's block after the use position.
    let after = match instr {
        Some(i) => i + 1,
        None => start.body.len(),
    };
    for later in start.body.iter().skip(after) {
        if later.used_vars().iter().any(|&v| rep_of(v) == rep) {
            return true;
        }
    }
    // A body use (instr = Some) is followed by this block's terminator.
    if instr.is_some()
        && start
            .terminator
            .used_vars()
            .iter()
            .any(|&v| rep_of(v) == rep)
    {
        return true;
    }
    // Downstream successor blocks.
    lineage_live_out(func, jt_reps, rep, block)
}

/// The actual arg var of lineage `rep` consumed at the call `u` (so the
/// keep-alive inc names the value the callee receives, not the rep).
fn lineage_arg_at_use(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    u: IterConsumeUse,
) -> Option<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let block = func.blocks.get(u.block)?;
    let used = match u.instr {
        Some(i) => block.body.get(i)?.used_vars(),
        None => block.terminator.used_vars(),
    };
    used.into_iter().find(|&v| rep_of(v) == rep)
}

/// A user-callee iter-consume use of an owned FRESH collection source at a
/// BORROWED arg position: the `(block, instr)` of the `Apply`/`Invoke` call whose
/// `ParamContract.iter_consumes` is true for the arg position carrying `rep`.
/// Excludes the inline `@iter(coll [own])` protocol-builtin position (that
/// transfer is already balanced by the base walk's iter-drop accounting); only the
/// USER-callee transfer the Phase-5 walk mis-models as a non-transfer borrow is in
/// scope here.
fn user_callee_iter_consume_uses_of_rep(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> Vec<IterConsumeUse> {
    let mut uses = Vec::new();
    let mut scan = |callee: Name, args: &[ArcVarId], block: usize, instr: Option<usize>| {
        let Some(contract) = contracts.get(&callee) else {
            return;
        };
        for (pos, &arg) in args.iter().enumerate() {
            if rep_of(arg) != rep {
                continue;
            }
            if contract.params.get(pos).is_some_and(|p| p.iter_consumes) {
                uses.push(IterConsumeUse { block, instr });
            }
        }
    };
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                scan(*callee, args, b, Some(i));
            }
        }
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            scan(*callee, args, b, None);
        }
    }
    uses
}

/// The full set of SSA vars in lineage `rep`: every var whose `rep_of` (jump-
/// threaded rep) is `rep`, PLUS the transitive closure of `Let { dst, value:
/// Var(src) }` aliases reachable from those vars (a Let-Var copy `dst = src`
/// extends the lineage but is NOT a jump-threaded edge, so `rep_of(dst)` may differ
/// from `rep`). Used to recognize a genuine downstream consume of the source that
/// reaches it through a Let-Var alias the jump-rep map does not connect (e.g. the
/// `%dst = Var(%threaded); @iter(%dst [own])` second iteration of a borrowed list).
fn lineage_member_vars(
    func: &ArcFunction,
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> FxHashSet<ArcVarId> {
    let mut members: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: every var the jump-rep map already maps to `rep`.
    for block in &func.blocks {
        for &(p, _) in &block.params {
            if rep_of(p) == rep {
                members.insert(p);
            }
        }
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                if rep_of(dst) == rep {
                    members.insert(dst);
                }
            }
        }
    }
    members.insert(rep);
    // Fixpoint: a `Let { dst, value: Var(src) }` with `src` already in the lineage
    // adds `dst`. Bounded by the var count.
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
                    if members.contains(src) && members.insert(*dst) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    members
}

/// Whether lineage `rep` is GENUINELY read/consumed anywhere OTHER than the single
/// call at `(skip_block, skip_instr)`. A genuine use is an operand of a NON-burden
/// body instruction (`Apply`/`Invoke`/`Construct`/`Project`/`Set`/`SetTag`/`Select`)
/// OR a non-`Jump` terminator (`Return`/`Branch`/`Switch`/`Invoke`/`InvokeIndirect`
/// arg). EXCLUDED (NOT genuine uses — pure SSA plumbing the source threads only to
/// carry its burden ops): `BurdenInc`/`BurdenDec`/`RcInc`/`RcDec` bookkeeping, a
/// `Jump` arg (a positional SSA rename into the successor block param), and a
/// `Let { value: Var(_) }` alias (the arg-aliasing copy). Lineage membership is the
/// jump-rep set PLUS the Let-Var alias closure (`lineage_member_vars`), so a
/// downstream `@iter(%alias [own])` second consume reached through a Let-Var copy
/// IS detected even though `rep_of(%alias) != rep`. A collection threaded through
/// Jump→param→Let-Var→burden-op chains with no genuine read is NOT live. Spec:
/// Annex E §AIMS RL-2.
fn lineage_genuinely_read_outside_call(
    func: &ArcFunction,
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    skip_block: usize,
    skip_instr: Option<usize>,
) -> bool {
    let members = lineage_member_vars(func, rep, rep_of);
    let in_lineage = |v: ArcVarId| members.contains(&v) || rep_of(v) == rep;
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            if b == skip_block && skip_instr == Some(i) {
                continue;
            }
            // RC bookkeeping is not a read.
            if matches!(
                instr,
                ArcInstr::BurdenInc { .. }
                    | ArcInstr::BurdenDec { .. }
                    | ArcInstr::RcInc { .. }
                    | ArcInstr::RcDec { .. }
            ) {
                continue;
            }
            // A `Let { value: Var(v) }` is a transparent SSA alias (the arg-aliasing
            // copy a borrowed Invoke arg threads through), not a read — same plumbing
            // class as a Jump arg. Its consumers are checked at their own sites.
            if matches!(
                instr,
                ArcInstr::Let {
                    value: ArcValue::Var(_),
                    ..
                }
            ) {
                continue;
            }
            if instr.used_vars().iter().any(|&v| in_lineage(v)) {
                return true;
            }
        }
        // A `Jump` terminator arg is a positional SSA rename, not a read. Every
        // other terminator (`Return`/`Branch`/`Switch`/`Invoke`/`InvokeIndirect`)
        // genuinely consumes its operands.
        let is_skip_terminator = b == skip_block && skip_instr.is_none();
        if is_skip_terminator {
            continue;
        }
        if matches!(&block.terminator, ArcTerminator::Jump { .. }) {
            continue;
        }
        if block.terminator.used_vars().iter().any(|&v| in_lineage(v)) {
            return true;
        }
    }
    false
}

/// Phase 6.66c — SINGLE borrowed-`Invoke`-arg iter-consume of an owned FRESH
/// collection source that is DEAD after the call (RL-2 `RL2_iter_consuming_no_caller_dec`).
///
/// A freshly-constructed collection (`let words = [..]`) passed at a BORROWED
/// terminator-`Invoke` (or body `Apply`) arg to a USER callee whose
/// `ParamContract.iter_consumes` is true transfers ownership inward — the callee's
/// `@iter [own]` -> `ori_iter_drop` is the SINGLE release. The Phase-5 walk does
/// NOT recognize the user-callee iter-consume as a transfer (only the inline
/// `@iter [own]` and direct-Owned-arg cases), so it emits a spurious FRESH-site
/// `BurdenInc` on the source plus a misplaced scope-exit `BurdenDec` that reaches
/// only the unwind edges -> the source buffer is incd once but never released on
/// the normal path -> leak (the source's owned element strings leak with it via
/// the never-run `elem_dec_fn`).
///
/// The fix strips EVERY normal-path burden op on the source lineage: the caller
/// emits NO inc and NO dec (`RL2_iter_consuming_no_caller_dec`); the source's
/// allocation rc=1 is released exactly once by the callee's iter-drop
/// (`RL2_release_exactly_once`). Unwind-path decs are panic-cleanup of a now-absent
/// inc and are ALSO stripped (a unwind dec with no inc over-decs the rc-1 source
/// the callee's unwind iter-drop already frees).
///
/// Discriminator (the over-fire boundary): the source rep must be (a) an owned
/// FRESH collection (`compute_fresh_owned_collection_reps` — a non-empty
/// List/Map/Set `Construct`, so there is a real FRESH inc to strip; a borrowed
/// PARAM source has no FRESH inc and is excluded), (b) consumed by EXACTLY ONE
/// user-callee iter-consume use (the N >= 2 multi-borrow case is Phase 6.66's), and
/// (c) have NO genuine read downstream of that call
/// (`!lineage_genuinely_read_outside_call` — a returned / re-read / re-consumed
/// source keeps its accounting; pure Jump→param→burden-op plumbing the source
/// threads only to carry its now-stripped burden ops is NOT a read). The inline
/// `@iter(coll [own])` position is NOT counted as a user-callee use (the base walk
/// already balances that transfer against its own iter-drop — stripping there would
/// orphan the iterator-handle accounting). Probe-gated -> default codegen
/// byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
fn suppress_single_borrowed_invoke_iter_consume_source(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if std::env::var_os("ORI_DISABLE_BORROWED_INVOKE_ITER_CONSUME_SUPPRESS").is_some() {
        return;
    }
    let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let fresh_reps =
        compute_fresh_owned_collection_reps(func, pool, &jt_reps, same_alloc_reps, interner);

    let mut reps_to_strip: FxHashSet<ArcVarId> = FxHashSet::default();
    for rep in fresh_reps {
        let uses = user_callee_iter_consume_uses_of_rep(func, contracts, rep, &rep_of);
        // (b) EXACTLY ONE user-callee iter-consume use (the N >= 2 case is Phase 6.66).
        if uses.len() != 1 {
            continue;
        }
        let u = uses[0];
        // (c) + (d): the iter-consuming call is the source's SOLE genuine consumer —
        // no genuine read downstream (a returned / re-read source keeps its
        // accounting). Pure Jump→param→burden-op plumbing does NOT count as a read,
        // so a source threaded only to carry its (now-stripped) burden ops qualifies.
        if lineage_genuinely_read_outside_call(func, rep, &rep_of, u.block, u.instr) {
            continue;
        }
        reps_to_strip.insert(rep);
    }
    if reps_to_strip.is_empty() {
        return;
    }
    // Strip every burden op (normal AND unwind path) on each stripped lineage's
    // SSA-alias members — the callee's iter-drop is the single release on every
    // exit (normal return + its own unwind cleanup pad).
    for block in &mut func.blocks {
        block.body.retain(|instr| match instr {
            ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } => {
                !reps_to_strip.contains(&rep_of(*var))
            }
            _ => true,
        });
    }
}

/// Phase 6.66g — AGGREGATE-FIELD iter-consume partial-dec rewrite (RL-2
/// field-grained iter-consume inward transfer).
///
/// A fresh/dead OWNED aggregate (`Construct Struct`/`Construct Tuple`, or the
/// owned result of a user callee returning one) passed at a BORROWED `Invoke`
/// terminator arg to a callee whose `ParamContract.iter_consumes_projected_field`
/// is `Some(field)` has that field's ownership transferred inward: the callee's
/// `@iter(Project param.field [own])` -> `ori_iter_drop` is the SINGLE release of
/// the projected collection (`for item in c.items` over a borrowed struct `c`).
///
/// The caller's whole-aggregate scope-exit `BurdenDec` (emitted by the Phase-6.5
/// edge-cleanup Category-1 dead-var release) would recursively re-free that
/// consumed field — a double-free, since the field's backing buffer is already
/// gone. This pass rewrites each such whole-var `BurdenDec aggregate` (on the
/// aggregate's same-alloc lineage) to a `BurdenDecPartial aggregate
/// skip_fields=[field]`: the aggregate shell and its OTHER owned fields still get
/// released, but the iter-consumed field is skipped (RL-2 transfer — no caller
/// dec for a transferred field).
///
/// SCOPE GUARD (over-fire boundary) — ALL required: (a) the arg sits at a
/// BORROWED `Invoke`/`InvokeIndirect` arg position (an Owned arg already
/// transfers the whole aggregate, no partial needed); (b) the callee's param
/// contract proves `iter_consumes_projected_field = Some(field)` AND NOT
/// whole-param `iter_consumes` (the whole-param case is Phase 6.66c); (c) the arg
/// var (via `same_alloc_reps`) actually carries a whole-var `BurdenDec` to
/// rewrite (a leak-shaped under-emission is left to the leak passes; this rewrite
/// only ever NARROWS an over-release). A `BurdenDec` that is already a partial /
/// field-grain op is left untouched (no double-rewrite). Probe-gated
/// (`ORI_DISABLE_AGG_FIELD_ITER_CONSUME_PARTIAL`) -> default codegen
/// byte-identical when no aggregate matches.
///
/// Lean: `AimsProof.Realization::RL2_iter_consuming_no_caller_dec` +
/// `RL2_iter_consuming_caller_dec_splits` (the field-grained iter-consume is the
/// same RL-2 inward transfer at field grain — the consumed field's release is the
/// callee's, the caller emits none for it). Spec: Annex E §AIMS RL-2.
/// (a)+(b) collection phase of [`rewrite_aggregate_iter_consume_field_decs`]: maps
/// each aggregate-rep to its single consumed field across borrowed `Invoke` args
/// whose callee param carries the field-grained iter-consume contract. A rep with
/// >1 distinct consumed field is dropped (no consistent partial skip).
fn collect_aggregate_iter_consume_fields(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> FxHashMap<ArcVarId, u32> {
    let mut rep_consumed_field: FxHashMap<ArcVarId, Option<u32>> = FxHashMap::default();
    for block in &func.blocks {
        let (callee, args, arg_ownership) = match &block.terminator {
            ArcTerminator::Invoke {
                func: callee,
                args,
                arg_ownership,
                ..
            } => (*callee, args, arg_ownership),
            // InvokeIndirect has no contract -> no field-grained signal.
            _ => continue,
        };
        let Some(contract) = contracts.get(&callee) else {
            continue;
        };
        for (i, &arg) in args.iter().enumerate() {
            // (a): borrowed arg position only.
            let owned = match arg_ownership.get(i) {
                Some(o) => *o == ArgOwnership::Owned,
                None => true, // direct-call default is Owned
            };
            if owned {
                continue;
            }
            let Some(param) = contract.params.get(i) else {
                continue;
            };
            // (b): field-grained iter-consume, NOT whole-param iter-consume.
            if param.iter_consumes {
                continue;
            }
            let Some(field) = param.iter_consumes_projected_field else {
                continue;
            };
            let rep = rep_of(arg);
            match rep_consumed_field.entry(rep) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(Some(field));
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if e.get() != &Some(field) {
                        // Conflicting consumed fields across call sites of the same
                        // allocation -> poison (no consistent partial skip).
                        *e.get_mut() = None;
                    }
                }
            }
        }
    }
    rep_consumed_field
        .into_iter()
        .filter_map(|(rep, f)| f.map(|field| (rep, field)))
        .collect()
}

fn rewrite_aggregate_iter_consume_field_decs(
    func: &mut ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if std::env::var_os("ORI_DISABLE_AGG_FIELD_ITER_CONSUME_PARTIAL").is_some() {
        return;
    }
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);

    let rep_consumed_field = collect_aggregate_iter_consume_fields(func, contracts, &rep_of);
    if rep_consumed_field.is_empty() {
        return;
    }

    // The consumed field's reference is transferred inward (the callee's iter-drop
    // is its single release). A whole-aggregate keep-alive `BurdenInc` on the same
    // rep incs EVERY heap field — including the consumed one — so leaving it while
    // the dec skips the consumed field strands a +1 on that field (a leak). Count
    // the per-rep whole-var inc/dec balance: when the rep carries a net-0
    // `BurdenInc`/`BurdenDec` keep-alive pair, remove ONE matching whole-var
    // `BurdenInc` alongside the dec→partial rewrite — the surviving fields are then
    // accounted by the partial dec alone (each constructed at rc=1, released once),
    // and the consumed field is freed exactly once by the callee. A rep with MORE
    // incs than decs (a genuine duplication beyond the keep-alive) keeps the
    // surplus incs intact (only one keep-alive inc is paired off).
    let mut rep_incs: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    let mut rep_decs: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::BurdenInc { var } => {
                    let r = rep_of(*var);
                    if rep_consumed_field.contains_key(&r) {
                        *rep_incs.entry(r).or_default() += 1;
                    }
                }
                ArcInstr::BurdenDec { var } => {
                    let r = rep_of(*var);
                    if rep_consumed_field.contains_key(&r) {
                        *rep_decs.entry(r).or_default() += 1;
                    }
                }
                _ => {}
            }
        }
    }
    // Reps whose whole-var inc count == dec count carry the net-0 keep-alive pair;
    // for those, remove ONE whole-var inc on the rep.
    let mut inc_removal_budget: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for &rep in rep_consumed_field.keys() {
        let incs = rep_incs.get(&rep).copied().unwrap_or(0);
        let decs = rep_decs.get(&rep).copied().unwrap_or(0);
        if incs > 0 && incs == decs {
            inc_removal_budget.insert(rep, 1);
        }
    }

    // (c): rewrite each whole-var `BurdenDec var` whose rep matches to a
    // `BurdenDecPartial var skip_fields=[field]`; remove one matching keep-alive
    // `BurdenInc` per budgeted rep. Field-grain ops are left as-is.
    for block in &mut func.blocks {
        block.body.retain_mut(|instr| {
            match instr {
                ArcInstr::BurdenInc { var } => {
                    let r = rep_of(*var);
                    if let Some(budget) = inc_removal_budget.get_mut(&r) {
                        if *budget > 0 {
                            *budget -= 1;
                            return false; // drop this keep-alive inc
                        }
                    }
                    true
                }
                ArcInstr::BurdenDec { var } => {
                    if let Some(field) = rep_consumed_field.get(&rep_of(*var)) {
                        let v = *var;
                        let f = *field;
                        *instr = ArcInstr::BurdenDecPartial {
                            var: v,
                            skip_fields: vec![f],
                        };
                    }
                    true
                }
                _ => true,
            }
        });
    }
}

/// Phase 6.66d — ITER-CONSUME + TRANSFER-THROUGH-RETURN source-dec suppression
/// (RL-1 keep-alive inc + RL-2 iter-consume transfer + the proven
/// iter-consume/return OVERLAP balance).
///
/// An owned param that is BOTH iter-consumed via an `@iter(arg [own])` call AND
/// transferred through the function's own `Return` is the overlap shape:
///
/// ```text
/// @iter_then_return <T> (x: [T]) -> [T] = {
///     let n = x.iter().count();   // x iter-CONSUMED — ori_iter_drop frees it
///     x                            // x ALSO returned (same allocation)
/// };
/// ```
///
/// The base Phase-5 walk treats the iter-consume as the param's last use and
/// emits a normal-path source `BurdenDec` before the `@iter` call (the
/// `[burden_inc, burden_dec]` pair around `@iter`). After the iter-consume frees
/// the +1, that premature dec leaves the returned param at refcount 0 — the
/// caller reads a freed allocation (UAF).
///
/// The keep-alive `BurdenInc` is correct (it funds the iter-consume); only the
/// normal-path `BurdenDec` is wrong on the overlap. This pass strips the
/// normal-path source `BurdenDec`(s) on a `transfers_through_return ∧ Owned ∧
/// iter_consumes` param lineage, keeping the keep-alive inc — so the param's
/// kept-from-arrival reference survives the iter-drop as the live Return value,
/// and the caller's own scope-exit dec is the single release.
///
/// Unwind-path decs are panic cleanup, left intact: on the unwind edge the
/// allocation has not yet flowed to the Return, so its release is still owed.
///
/// SCOPE GUARD (over-fire boundary) — ALL required: (a) the lineage is one of the
/// CURRENT function's OWN params (read from `contracts.get(&func.name)`), (b) that
/// param's contract proves `transfers_through_return ∧ access == Owned ∧
/// iter_consumes`, (c) the param's rep is actually iter-consumed in the body
/// (`collect_iter_consume_uses_per_rep`), AND (d) the param's rep flows to a
/// normal-path `Return` (a same-alloc sibling reaches a `Return { value }`). A
/// param that is returned but NOT iter-consumed keeps its accounting (the base
/// walk balances it); an iter-consumed param NOT returned is the dead-after case
/// Phase 6.66c owns. Probe-gated
/// (`ORI_DISABLE_ITER_CONSUME_RETURN_SOURCE_SUPPRESS`) → default codegen
/// byte-identical when no param matches.
///
/// Lean: `AimsProof.Realization::RL2_iter_consume_return_overlap_{gap,cured,
/// minimal,balanced}` (the overlap requires exactly one keep-alive inc; the
/// source dec is the over-emission) + `RL2_iter_consuming_no_caller_dec`. Spec:
/// Annex E §AIMS RL-1 + RL-2.
fn suppress_iter_consume_transferred_return_source_dec(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if std::env::var_os("ORI_DISABLE_ITER_CONSUME_RETURN_SOURCE_SUPPRESS").is_some() {
        return;
    }
    // (a)+(b): the current function's OWN params proving the overlap contract.
    let Some(self_contract) = contracts.get(&func.name) else {
        return;
    };
    // (b): `transfers_through_return ∧ Owned`. The iter-consume half is the
    // body-scan cross-check below ((c) `collect_iter_consume_uses_per_rep`),
    // which directly detects the `@iter(arg [own])` consume in this function's
    // own body — the param's contract `iter_consumes` flag tracks consume by a
    // SEPARATE callee, not the inline `@iter`, so the body scan is the reliable
    // signal for the same-function overlap shape.
    let overlap_param_vars: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            self_contract
                .params
                .get(*i)
                .is_some_and(|p| p.transfers_through_return && p.access == AccessClass::Owned)
        })
        .map(|(_, p)| p.var)
        .collect();
    if overlap_param_vars.is_empty() {
        return;
    }

    let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let overlap_reps: FxHashSet<ArcVarId> = overlap_param_vars.iter().map(|&v| rep_of(v)).collect();

    // (c): the param rep must actually be iter-consumed in the body — defensive
    // cross-check of the contract's `iter_consumes` against the realized IR.
    let iter_name = interner.intern("iter");
    let iter_uses = collect_iter_consume_uses_per_rep(func, pool, contracts, iter_name, &rep_of);

    // (d): the param rep must flow to a normal-path `Return` — a same-alloc
    // sibling reaches a `Return { value }` on a non-unwind block.
    let unwind_blocks = compute_unwind_reachable_blocks(func);
    let mut returned_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for (b, block) in func.blocks.iter().enumerate() {
        if unwind_blocks.contains(&b) {
            continue;
        }
        if let ArcTerminator::Return { value } = &block.terminator {
            returned_reps.insert(rep_of(*value));
        }
    }

    let reps_to_strip: FxHashSet<ArcVarId> = overlap_reps
        .into_iter()
        .filter(|rep| iter_uses.contains_key(rep) && returned_reps.contains(rep))
        .collect();
    if reps_to_strip.is_empty() {
        return;
    }

    // Strip NORMAL-path `BurdenDec` on each matched lineage; keep keep-alive
    // `BurdenInc` (funds the iter-consume) and unwind-path decs (panic cleanup).
    for (b, block) in func.blocks.iter_mut().enumerate() {
        if unwind_blocks.contains(&b) {
            continue;
        }
        block.body.retain(|instr| {
            !matches!(
                instr,
                ArcInstr::BurdenDec { var } if reps_to_strip.contains(&rep_of(*var))
            )
        });
    }
}

/// Phase 6.66f — SHARING-VIEW slice + iter-consume surplus-inc suppression
/// (RL-1 duplication + RL-2 iter-consume transfer + RL-2 release-exactly-once).
///
/// A FRESH owned collection (`let words = [..]`) BORROWED into a seamless-slice
/// producer (`words.take(2)` / `.slice(..)` / `.substring(..)` / `.drop(..)` —
/// [`sharing_view_relocation_names`]) AND iter-consumed by the inline
/// `@iter [own]` -> `ori_iter_drop` is the slice+iter-interaction shape.
///
/// The runtime accounting (verified via `ORI_TRACE_RC`): the sharing-view
/// producer rc-INCs the shared backing buffer (rc 1 -> 2) so the surviving slice
/// holds its own ref (released by the slice's own scope-exit dec), and the
/// `@iter [own]` -> `ori_iter_drop` is the source allocation's single TRANSFER
/// release (`RL2_iter_consuming_no_caller_dec`). The source's correct burden
/// ledger therefore carries ZERO incs: the one slice-share duplication is funded
/// by the producer's CODEGEN inc (not a burden inc), and the iter-consume is a
/// transfer (no caller keep-alive). The base Phase-5 walk diverges — it treats
/// the live-across iter-consume of the slice's source as a duplication needing a
/// caller keep-alive `BurdenInc`, emitting one (or more) on the source lineage
/// BEYOND the producer's own inc — so the rc-1 buffer nets to rc>=1 and never
/// reaches 0 (the buffer plus its owned element strings leak via the never-run
/// `elem_dec_fn`).
///
/// This pass strips every NORMAL-path `BurdenInc` on the iter-consumed
/// sharing-view source lineage. Unwind-path ops are panic cleanup, left intact.
/// Normal-path source `BurdenDec`s are also stripped (the iter-drop is the single
/// release; a burden dec would over-release the transferred ref).
///
/// Discriminators (the over-fire boundary) — ALL required:
/// - (a) the source is an owned FRESH collection (`compute_fresh_owned_collection_reps`
///   — a non-empty List/Map/Set `Construct`; a borrowed PARAM source has no FRESH
///   producer inc and is excluded);
/// - (b) the source is iter-consumed (inline `@iter [own]` OR a user callee whose
///   `ParamContract.iter_consumes` is true — `collect_iter_consume_uses_per_rep`);
/// - (c) the source is borrowed into a sharing-view producer
///   (`sharing_view_relocation_names`: `slice` / `substring` / `take` / `drop`)
///   whose RESULT is a surviving slice (the slice is read after the producer call;
///   a slice produced and immediately dead is the no-survivor case the base walk
///   already balances). The producer's own codegen inc funds the slice's ref, so
///   the source needs no burden inc.
///
/// Lean: `RL1_duplication_balanced` (the single slice-share dup is funded by the
/// producer inc, lifecycle `[]` for the source's burden ledger) +
/// `RL2_iter_consuming_no_caller_dec` + `RL2_release_exactly_once`. Probe-gated
/// (`ORI_DISABLE_SHARING_VIEW_ITER_CONSUME_SURPLUS`) -> default codegen
/// byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
fn suppress_sharing_view_iter_consume_surplus_inc(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if std::env::var_os("ORI_DISABLE_SHARING_VIEW_ITER_CONSUME_SURPLUS").is_some() {
        return;
    }
    let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let fresh_reps =
        compute_fresh_owned_collection_reps(func, pool, &jt_reps, same_alloc_reps, interner);
    if fresh_reps.is_empty() {
        return;
    }
    let iter_name = interner.intern("iter");
    let iter_uses = collect_iter_consume_uses_per_rep(func, pool, contracts, iter_name, &rep_of);
    let sharing_names = sharing_view_relocation_names(interner);

    // A source rep qualifies when it is (b) iter-consumed AND (c) borrowed into a
    // sharing-view producer whose result is read after the producer call (a
    // surviving slice). The surviving-slice read is the discriminator separating
    // this leak shape from the dead-after-slice case the base walk already balances.
    let mut reps_to_strip: FxHashSet<ArcVarId> = FxHashSet::default();
    for &rep in &fresh_reps {
        if !iter_uses.contains_key(&rep) {
            continue;
        }
        if rep_borrowed_into_surviving_sharing_view(func, rep, &rep_of, &sharing_names) {
            reps_to_strip.insert(rep);
        }
    }
    if reps_to_strip.is_empty() {
        return;
    }

    // Unwind/Resume cleanup blocks keep their burden ops (panic-path release of the
    // producer's inc); only NORMAL-path source burden ops are stripped.
    let unwind_blocks = compute_unwind_reachable_blocks(func);
    for (b, block) in func.blocks.iter_mut().enumerate() {
        if unwind_blocks.contains(&b) {
            continue;
        }
        block.body.retain(|instr| match instr {
            ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } => {
                !reps_to_strip.contains(&rep_of(*var))
            }
            _ => true,
        });
    }
}

/// Whether lineage `rep` is BORROWED into a sharing-view producer (`take` / `slice`
/// / `substring` / `drop`) whose RESULT is read AFTER the producer call (a
/// surviving seamless slice). The producer's codegen inc funds the surviving
/// slice's shared-buffer ref, so the source needs no burden inc — its iter-consume
/// is the single transfer release.
///
/// The producer call is an `Apply`/`Invoke` to a `sharing_names` callee whose
/// position-0 (receiver) arg traces (`rep_of`) to `rep`. "Surviving" = the
/// producer's RESULT dst is GENUINELY read by some later non-burden instruction
/// (an `@len` / `Project` / `Return` / aggregate field — anything other than pure
/// SSA-rename plumbing). A producer result that is immediately dead is the
/// no-survivor case the base walk already balances. Spec: Annex E §AIMS RL-2.
fn rep_borrowed_into_surviving_sharing_view(
    func: &ArcFunction,
    rep: ArcVarId,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    sharing_names: &FxHashSet<Name>,
) -> bool {
    // Collect the result dsts of sharing-view producers whose receiver is `rep`.
    let mut result_dsts: Vec<ArcVarId> = Vec::new();
    let mut scan = |callee: Name, args: &[ArcVarId], dst: Option<ArcVarId>| {
        if !sharing_names.contains(&callee) {
            return;
        }
        // Position 0 is the receiver (the borrowed sliced collection).
        if args.first().is_some_and(|&recv| rep_of(recv) == rep) {
            if let Some(d) = dst {
                result_dsts.push(d);
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                dst,
                ..
            } = instr
            {
                scan(*callee, args, Some(*dst));
            }
        }
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            dst,
            ..
        } = &block.terminator
        {
            scan(*callee, args, Some(*dst));
        }
    }
    if result_dsts.is_empty() {
        return false;
    }
    // The result is a SURVIVING slice when any producer result's lineage is
    // genuinely read after the producer (a borrow read / Project / Return / field
    // store) — `lineage_genuinely_read_outside_call` over the result rep with no
    // skip site (the producer call itself defines, never reads, the result).
    result_dsts.iter().any(|&d| {
        let dst_rep = rep_of(d);
        lineage_genuinely_read_outside_call(func, dst_rep, rep_of, usize::MAX, None)
    })
}

/// Step-B' pass: free a genuinely-leaked OWNED collection-source whose lineage
/// flows (as a Jump-arg) into a block param that is dead at a normal terminal
/// (loop-exit / Return) without a freeing dec there.
///
/// The canonical shape is `m.keys()` / `m.values()` / `s.split()` /
/// `set.to_list()`: a FRESH owned collection (the map/set/str) is BORROWED by
/// the conversion builtin (so it survives the call), then loop-carried and dead
/// at the post-loop block — the burden walk emits the freeing dec only on the
/// unwind edges (RL-4), never on the normal dead-block-param path (RL-5), so the
/// source buffer plus its owned element strings leak.
///
/// Part 1 — the jump-threaded leaked-source discriminator. The committed
/// `same_alloc_reps` excludes Jump-arg→block-param edges BY DESIGN (to bound the
/// fresh-inc-elision blast radius), so the source's lineage rep differs across the
/// phi. [`compute_jump_threaded_reps`] threads that positional SSA rename LOCALLY
/// here. The per-path alloc-aware net (`compute_burden_entry_nets` over the
/// jump-threaded delta `Σ alloc(+1) + Σ BurdenInc − Σ BurdenDec*`) is positive
/// at a terminal block exactly when the lineage is leaked on that path (the
/// alloc's `+1` unbalanced by any release).
///
/// Part 2 — the freeing mechanism that COMPOSES with `elem_dec_fn`. The emitted
/// whole-var `BurdenDec` lowers (Phase 7) to `RcDec { HeapPointer }`; the LLVM
/// emitter routes a `Tag::Map`/`Set`/`List` `HeapPointer` dec through
/// `ori_buffer_rc_dec`, which reads the V5 header's `elem_dec_fn`/`elem_count`
/// and walks the element-drop glue — freeing the buffer AND its owned element
/// strings (SSO-vs-heap, slice provenance handled by the runtime). A bare
/// whole-var dec on the WRONG var (the conversion RESULT, freed by
/// `ori_iter_drop`) would double-free; this pass emits ONLY for the source.
///
/// SEED-not-reuse exclusions (so it never double-frees a for-loop cluster — a
/// naive whole-block dead-param dec double-frees the iterator-owned buffer): the
/// leaked lineage rep must NOT intersect
/// `collect_iter_element_defs` (iterator element views), any `ori_iter_drop`
/// Apply argument (iterator handles freed by the runtime drop), the borrowed-def
/// set, or a lineage that already carries a `BurdenDec` reaching the terminal
/// block on the same path (already freed). Spec: Annex E §AIMS RL-5.
fn emit_burden_dead_collection_source_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
) {
    let releases = compute_dead_collection_source_releases(func, pool, interner);
    // Emit at the TOP of each terminal block: the source is dead at block entry
    // (RL-5 dead-at-entry), so the dec releases exactly the leaked reference
    // before the block's own (scalar) computation runs.
    for (block_idx, var) in releases {
        if let Some(block) = func.blocks.get_mut(block_idx) {
            block.body.insert(0, ArcInstr::BurdenDec { var });
        }
    }
}

/// Emit the dead OWNED-COLLECTION / mutation-result freeing decs (RL-2
/// `ScopeExit` / `ApplyToBorrowedParam`) for the burden path.
///
/// Distinct from [`emit_burden_dead_collection_source_decs`]: that pass frees a
/// borrowed-then-dead CONVERSION SOURCE (`m.keys()` / `s.split()`) at a
/// dead-block-param loop-exit sink. THIS pass frees a FRESH owned collection
/// that is bound as a body-local (a mutation RESULT `let ys = xs.sort()` /
/// `xs.insert(..)` / `xs.set(..)`, or a read-only `let m = {..}; m.contains_key(..)`),
/// last-used at a BORROWED position, then dead at function scope exit.
///
/// The leak (under sole-emitter Phase-7 lowering): a multiply-used / duplicated
/// FRESH owned collection gets a fresh-site `BurdenInc` (RL-1, the use is a
/// duplicating use) plus per-path scope-exit `BurdenDec`s — the inc/dec pairs net
/// the EXPLICIT ops to 0, but the allocation's implicit `+1` is never released.
/// The compiled-Lean `rcBalance` (Realization.lean) mandates the lifecycle
/// EXCLUDING the alloc net `-1`; the impl emits net 0 → the buffer leaks. This is
/// an impl-conformance fix (the Lean is correct): emit ONE additional whole-var
/// `BurdenDec` on the lineage's last-live value at each last-use sink, netting the
/// lineage to release the alloc. The `RcDec { HeapPointer }` it lowers to routes a
/// `Tag::Map`/`Set`/`List` value through `ori_buffer_rc_dec` (the V5 `elem_dec_fn`
/// walk) so a heap-str-element collection ALSO frees its owned element strings.
///
/// SEED-not-reuse exclusions (so it never double-frees): the lineage must be a
/// FRESH owned collection that is NEVER owned-consumed / transferred-out (only
/// borrowed reads + scope exit — a sort/insert SOURCE is excluded because it is
/// consumed owned by the mutator, while the RESULT survives), is NOT an
/// iterator-element view, NOT an iterator handle freed by `ori_iter_drop`, NOT
/// borrowed-def, and whose alloc-aware net at the sink is POSITIVE (net 0 =
/// already freed — `let xs = [1,2,3]; xs.length()` and branch-merge phi shapes
/// net 0 and never fire). Spec: Annex E §AIMS RL-2.
fn emit_burden_dead_owned_collection_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    let releases =
        compute_dead_owned_collection_releases(func, pool, interner, contracts, same_alloc_reps);
    // Placement (RL-2 / RL-4): the dec must follow the lineage's last borrowed READ.
    // When that last use is a BODY instruction, end-of-body (after the read, before
    // the terminator) is correct. When it is the TERMINATOR's borrowed arg (an
    // `Invoke @insert(elem [borrow]) normal/unwind` — the runtime COPIES + incs the
    // element via `key_inc`/`val_inc` DURING the call), end-of-body would place the
    // dec BEFORE the call reads the element → use-after-free / double-free. The dec
    // belongs on the NORMAL successor edge, after the borrowed call returns (RL-4
    // `RL4_edge_release_balanced` — released exactly once on the path the value
    // survives onto). Spec: Annex E §AIMS RL-2 + RL-4. Same apply-Direct seed as the
    // release computation so the placement resolves against the merged forwarder
    // lineage.
    let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
    for (block_idx, var) in releases {
        match dead_collection_dec_placement(func, &jt_reps, block_idx, var) {
            DeadDecPlacement::EndOfBody => {
                if let Some(block) = func.blocks.get_mut(block_idx) {
                    block.body.push(ArcInstr::BurdenDec { var });
                }
            }
            DeadDecPlacement::NormalSuccessorFront(succ_idx) => {
                if let Some(succ) = func.blocks.get_mut(succ_idx) {
                    succ.body.insert(0, ArcInstr::BurdenDec { var });
                }
            }
        }
    }
}

/// Phase 6.85 — dead-no-use INLINE-AGGREGATE freeing (RL-2 `ScopeExit`). A bare
/// `let a = Doc { field: <heap> }` / `let c = Link(..)` / `let t = (.., ..)` binds
/// an inline struct / enum / tuple (`ValueRepr::Aggregate`) with a heap-bearing
/// field, dead with ZERO uses. The Phase-5 walk emits ZERO burden ops on the
/// no-use aggregate, so the heap field leaks (the user `@drop` never runs). This
/// pass emits ONE whole-var `BurdenDec` at the END of the lineage's defining
/// block; Phase-7 lowers it through `RcStrategy::from_repr(Aggregate, ..)` to
/// `RcDec [AggFields]`/`[InlineEnum]`, whose drop-glue walks the heap field(s) —
/// byte-identical to the oracle's scope-exit dec. Distinct from the dead-owned-
/// COLLECTION pass (`RcPointer` list/map/set buffers, Phase 6.8): these are bare
/// inline aggregates with no self-buffer. A no-use value has no terminator-
/// borrowed-arg read to sequence after, so end-of-body (the scope-exit point,
/// before the terminator) is the placement. SEED-not-reuse: owned-consumed /
/// returned / iterator-managed lineages are excluded so it never double-frees a
/// nested node or a transferred value. Probe-gated -> default codegen
/// byte-identical. Spec: Annex E §AIMS RL-2.
fn emit_burden_dead_no_use_aggregate_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    let releases = compute_dead_no_use_aggregate_releases(func, pool, interner, contracts);
    for (block_idx, var) in releases {
        if let Some(block) = func.blocks.get_mut(block_idx) {
            block.body.push(ArcInstr::BurdenDec { var });
        }
    }
}

/// Compute the `(dead_successor_idx, var)` branch-dead releases for
/// [`emit_burden_branch_dead_value_decs`].
///
/// A FRESH owned non-scalar value (heap `str` `FatValue` / collection `RcPointer` /
/// heap-bearing inline aggregate) defined before a control-flow split, USED (and
/// released) on one branch but DEAD on a sibling early-exit branch, leaks its
/// allocation on the dead branch under sole-emitter lowering: the base burden
/// walk emits the lineage's single release on the value-survives branch only, so
/// the early-exit edge (`?`-None return, `if/else` arm that never reads the value)
/// carries no release. RL-4 edge-cleanup mandates one dec on that specific edge:
/// the value is Owned, non-scalar, live at the split block's exit, dead at the
/// dead successor's entry, and not a Jump arg (no ownership handed to a successor
/// block param). The dec lands at the FRONT of the dead successor.
///
/// SEED-not-reuse (identical discipline to the dead-no-use-aggregate pass so the
/// passes never both fire on one lineage): returned / owned-consumed /
/// PrimOp-operand (concat / COW) / user-call-arg / owned-moved / iterator-managed
/// / borrowed lineages are excluded — each is either a transfer (the consumer
/// releases) or already-released. Single-predecessor dead successor only: a
/// merge-point successor with ≥2 predecessors could double-count across edges, so
/// the canonical safe edge is the single-pred branch (the predicate-stack
/// `apply_edge_decs` targets the same single-pred edges). Spec: Annex E §AIMS RL-4
/// (`RL4_edge_dec_decision` + `RL4_edge_release_balanced` + `RL4_jump_arg_exempt`).
fn compute_branch_dead_value_releases(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<(usize, ArcVarId)> {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let list_take_name = for_yield_result_finalizer_name(interner);

    // Candidate lineages: a FRESH owned heap `str` self-allocation ONLY — a
    // `Let { Literal(String) }` (the immutable heap-str the early-`?`/branch-dead
    // shape leaks). Scoped to `str` (FatValue + `Tag::Str`) deliberately: a
    // collection (RcPointer) or an inline aggregate (struct / Option / Result /
    // tuple) moved into a struct field / sum payload is an RL-2 OWNERSHIP TRANSFER
    // whose consumer drop frees it — emitting an edge dec there double-frees (the
    // `slice_element_into_*_field` / `owned_collection_field_into_struct` shapes).
    // A `str` is immutable with no COW-through-borrow hazard and is not a
    // transfer-into-aggregate candidate here, so the str-only carve-out is the
    // sound boundary (mirroring the prior borrowed-str carve-out). The
    // `fresh_self_alloc_dst` set covers the String-literal case via `is_str_dst`.
    let mut candidate_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let Some(dst) = fresh_self_alloc_dst(instr, list_take_name) {
                if is_str_dst(dst, func, pool) {
                    candidate_reps.insert(rep_of(dst));
                }
            }
        }
    }
    if candidate_reps.is_empty() {
        return Vec::new();
    }

    // Exclusion sets (SEED-not-reuse — same set the dead-no-use-aggregate pass
    // uses). A transferred / consumed / already-released lineage must NOT receive
    // an edge dec.
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    let borrowed_defs = crate::aims::emit_rc::collect_all_borrowed_defs(func, pool);
    let iter_drop_handles = compute_iter_drop_handle_lineages(func, &jt_reps, interner);
    let iterator_bearing = compute_dead_iterator_handle_candidates(func, pool, &jt_reps);
    let owned_consumed = compute_owned_consumed_lineages(func, &jt_reps, &jt_reps);
    let returned = compute_returned_lineages(func, &jt_reps);
    let primop_operands = compute_primop_operand_lineages(func, &jt_reps);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let user_call_args =
        compute_user_call_arg_lineages(func, pool, &jt_reps, &builtins, contracts, &jt_reps);
    let owned_moved = compute_collection_owned_moved_str_lineages(func, &jt_reps);

    let mut excluded_reps: FxHashSet<ArcVarId> = iter_drop_handles;
    excluded_reps.extend(iterator_bearing);
    excluded_reps.extend(owned_consumed);
    excluded_reps.extend(returned);
    excluded_reps.extend(primop_operands);
    excluded_reps.extend(user_call_args);
    excluded_reps.extend(owned_moved);
    for &val in iter_element_defs.iter().chain(borrowed_defs.iter()) {
        excluded_reps.insert(rep_of(val));
    }

    let preds = crate::graph::compute_predecessors(func);
    // Unwind-cleanup blocks (every `Invoke`/`InvokeIndirect` unwind target +
    // `Resume` + their successor closure). The unwind path's RC is owned by the
    // panic-cleanup machinery, NOT a normal-path scope-exit dec — emitting an
    // edge dec into an unwind block double-frees / hits the wrong-layout cleanup
    // (`extract_value on non-struct value`). RL-4 edge-cleanup applies to NORMAL
    // CFG edges only (unwind edges foil post-dominance).
    let unwind_blocks = compute_unwind_reachable_blocks(func);
    let dom = crate::graph::DominatorTree::build(func);
    let mut releases: Vec<(usize, ArcVarId)> = Vec::new();
    let mut emitted: FxHashSet<(usize, ArcVarId)> = FxHashSet::default();
    for &rep in &candidate_reps {
        if excluded_reps.contains(&rep) {
            continue;
        }
        // A genuine branch-dead value HAS a use somewhere (its release lives on
        // the value-survives branch); a ZERO-use value is the dead-no-use-aggregate
        // pass's domain (it emits the scope-exit dec at the defining block).
        if !lineage_has_any_use(func, &jt_reps, rep) {
            continue;
        }
        let Some((def_block, def_var)) = lineage_defining_block_and_var(func, &jt_reps, rep) else {
            continue;
        };
        let def_block_id = crate::ir::ArcBlockId::new(u32::try_from(def_block).unwrap_or(u32::MAX));
        // RL-4 edge condition per (split block B, successor S): the lineage is
        // live-out of B (reaches its release on a sibling path) AND dead in the S
        // subtree (no reference anywhere from S onward). The dec lands at S's
        // front. Restrict S to a single-predecessor NORMAL block so the dec cannot
        // double-count across merge edges or land on an unwind-cleanup path.
        for (b, block) in func.blocks.iter().enumerate() {
            // Only NORMAL control-flow splits (`Branch` / `Switch`) carry the
            // value-survives-vs-dead branch shape; the `Invoke` normal/unwind split
            // is handled by the unwind-block exclusion below, never an edge dec.
            if !matches!(
                block.terminator,
                ArcTerminator::Branch { .. } | ArcTerminator::Switch { .. }
            ) {
                continue;
            }
            if !lineage_live_out(func, &jt_reps, rep, b) {
                continue;
            }
            for succ in crate::graph::successor_block_ids(&block.terminator) {
                let s = succ.index();
                // Never emit into an unwind-cleanup block (panic-path RC is the
                // cleanup machinery's, not a normal scope-exit dec).
                if unwind_blocks.contains(&s) {
                    continue;
                }
                // The value's def block MUST DOMINATE the dead successor S: the
                // value is then allocated on EVERY path reaching S, so it genuinely
                // leaks at S (vs a value defined INSIDE a sibling branch, which is
                // not live across this split at all — `lineage_live_out` can
                // over-report via jt-rep merging when a String-literal operand of a
                // later branch shares a rep). Dominance is the precise live-across-
                // the-branch test. Spec: Annex E §AIMS RL-4 (`live at exit`).
                if !dom.dominates(def_block_id, succ) {
                    continue;
                }
                // Jump-arg exemption (RL-4): a value handed to S as a Jump arg
                // transfers ownership to S's block param — `lineage_live_out_from_block`
                // already sees the param as a use, so a dead-in-subtree S never
                // received the value as a Jump arg. Single-predecessor S only.
                if preds.get(s).is_none_or(|p| p.len() != 1) {
                    continue;
                }
                if lineage_live_out_from_block(func, &jt_reps, rep, s) {
                    continue;
                }
                // Do not place the dec into the value's own defining block (the
                // value is defined there, not dead-on-entry); the split must be a
                // proper successor downstream of the def.
                if s == def_block {
                    continue;
                }
                if emitted.insert((s, def_var)) {
                    releases.push((s, def_var));
                }
            }
        }
    }
    releases
}

/// Phase 6.86 — branch-dead fresh-value freeing (RL-4 edge cleanup). A FRESH owned
/// non-scalar value used (and released) on one branch but DEAD on a sibling
/// early-exit branch leaks on the dead branch under sole-emitter lowering — the
/// base burden walk releases it only on the value-survives branch. This pass emits
/// ONE `BurdenDec` at the FRONT of the dead single-predecessor successor; Phase-7
/// lowers it (`FatValue` / `RcPointer` / `Aggregate` repr) to the matching `RcDec`,
/// byte-identical to the oracle's edge-cleanup dec. SEED-not-reuse: transferred /
/// consumed / iterator-managed / borrowed lineages excluded so it never
/// double-frees. Probe-gated -> default codegen byte-identical. Spec: Annex E
/// §AIMS RL-4.
fn emit_burden_branch_dead_value_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    let releases = compute_branch_dead_value_releases(func, pool, interner, contracts);
    for (succ_idx, var) in releases {
        if let Some(succ) = func.blocks.get_mut(succ_idx) {
            succ.body.insert(0, ArcInstr::BurdenDec { var });
        }
    }
}

/// Whether a callee BORROW-READS an aggregate receiver argument WITHOUT aliasing
/// it into the result — i.e. the receiver survives the call and is releasable on
/// the successor edges. Returns `false` for a callee that may return a view into
/// the receiver, iter-consumes it, or upgrades it to an owned transfer (those keep
/// or own the release elsewhere).
///
/// Reuses the proven [`borrowed_arg_release_verdict`] gate for the escape-safe
/// builtin classes (conversion / survives-transform / accessor-retain / builtin
/// scalar-read), and adds the user-fn borrow-read case the verdict declines (it
/// returns `None` for a Borrowed-access non-iter-consuming non-return-aliasing user
/// fn — the case where the inline dec is kept for a COLLECTION SOURCE; for a fresh
/// dead-after AGGREGATE that case is a plain `Both` relocation). The user-fn arm
/// requires a `MemoryContract` proving NO return-view aliasing (`return_alias ==
/// None && !return_payload_contains_param`), Borrowed access, and NOT iter-consume.
/// Spec: Annex E §AIMS RL-2 + RL-4.
fn aggregate_borrow_read_relocatable(
    callee: Name,
    arg_index: usize,
    names: &EscapeSafeBorrowedNames<'_>,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> bool {
    // The escape-safe builtin classes already resolve to `Both` (or PostDominator
    // for sharing views — excluded here: a slice/substring of an aggregate is not
    // this shape). Treat a `Both` verdict as relocatable.
    if borrowed_arg_release_verdict(callee, arg_index, true, names, contracts)
        == Some(EdgeRelease::Both)
    {
        return true;
    }
    // User-fn borrow-read (the f06/f12 `desc_len` / `get_size` shape): the verdict
    // returns `None` for a Borrowed-access non-iter-consuming non-return-aliasing
    // user fn. For a fresh dead-after aggregate receiver that IS a `Both`
    // relocation. Require the contract to prove no return-view aliasing + Borrowed
    // access + not iter-consume.
    let Some(param) = contracts.get(&callee).and_then(|c| c.params.get(arg_index)) else {
        return false;
    };
    param.return_alias.is_none()
        && !param.return_payload_contains_param
        && !param.iter_consumes
        && param.access == AccessClass::Borrowed
}

/// Compute the `(call_block, recv, normal_succ, unwind_succ)` relocations for
/// [`relocate_borrowed_terminator_aggregate_dec`].
///
/// A FRESH burden-carrying inline aggregate (a sum variant / struct / tuple /
/// Option / Result holding a heap field — `is_burden_carrying_aggregate`) passed at
/// a BORROWED terminator-`Invoke` receiver position to a borrow-read callee, dead
/// after the call, leaks its moved-in heap field under sole-emitter lowering: the
/// Phase-5 walk emits a matched `BurdenInc recv; BurdenDec recv` pair in the call
/// block, the Phase-3 coalesce peephole cancels the adjacent pair to net-0, and no
/// scope-exit release survives. RL-2 mandates the single scope-exit release of a
/// fresh owned aggregate whose last use is a borrowed call
/// (`rl2_emits_dec(.LastReadBeforeScopeExit)` + `RL2_release_exactly_once`); the
/// moved-in field is an `RL2_transfer_kinds_no_dec` `ConstructArg` transfer INTO
/// the aggregate so the aggregate's drop is the field's sole release. The release
/// relocates to BOTH dead successor edges (RL-4 `RL4_edge_release_balanced`).
///
/// SEED-not-reuse exclusions (the SAME discipline as the dead-no-use-aggregate /
/// branch-dead-value passes so it never double-frees a transferred or
/// already-balanced lineage): the receiver must be FRESH (defined by a `Construct`
/// OR an aggregate-returning call result), carry the coalesce-doomed `BurdenInc` +
/// `BurdenDec` PAIR in the call block, be DEAD after the call (`!lineage_live_out`),
/// and NOT be returned / owned-consumed / owned-moved-into-collection. The callee
/// must borrow-read without return-view aliasing (`aggregate_borrow_read_relocatable`).
/// Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-2 + RL-4.
fn compute_borrowed_terminator_aggregate_relocations(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<(usize, ArcVarId, usize, usize)> {
    let conversion_names = collection_conversion_names(interner);
    let survives_transform_names = borrow_survives_transform_names(interner);
    let accessor_retain_names = crate::borrow::accessor_retain_builtin_names(interner);
    let sharing_view_names = sharing_view_relocation_names(interner);
    let fresh_str_names = fresh_str_producing_method_names(interner);
    let set_algebra_names = set_algebra_relocation_names(interner);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let escape_safe_names = EscapeSafeBorrowedNames {
        conversion: &conversion_names,
        survives_transform: &survives_transform_names,
        accessor_retain: &accessor_retain_names,
        sharing_view: &sharing_view_names,
        fresh_str: &fresh_str_names,
        set_algebra: &set_algebra_names,
        builtins: &builtins,
    };

    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // SEED-not-reuse exclusion sets (shared with the dead-no-use-aggregate pass).
    let returned = compute_returned_lineages(func, &jt_reps);
    let owned_consumed = compute_owned_consumed_lineages(func, &jt_reps, &jt_reps);
    let owned_moved = compute_collection_owned_moved_str_lineages(func, &jt_reps);

    let mut relocations: Vec<(usize, ArcVarId, usize, usize)> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        // Terminator must be an `Invoke` with a BORROWED receiver (arg 0).
        let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            normal,
            unwind,
            ..
        } = &block.terminator
        else {
            continue;
        };
        let Some(&recv) = args.first() else {
            continue;
        };
        let recv_borrowed = arg_ownership
            .first()
            .is_none_or(|o| *o == crate::ir::ArgOwnership::Borrowed);
        if !recv_borrowed {
            continue;
        }
        // The receiver must be a FRESH burden-carrying inline aggregate (heap-bearing
        // struct / tuple / Option / Result / sum variant). A Value-variant (scalar
        // payload, triviality Trivial) is excluded — no heap field to free, a
        // spurious `RcDec [InlineEnum]` on it is unsound.
        if !is_burden_carrying_aggregate(recv, func, pool) {
            continue;
        }
        let rep = rep_of(recv);
        // The receiver must carry the coalesce-doomed `BurdenInc recv` AND
        // `BurdenDec recv` pair in the call block — the Phase-5 mis-emission this
        // pass corrects. Absent the pair, there is nothing to relocate.
        let has_inc = block
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::BurdenInc { var } if rep_of(*var) == rep));
        let has_dec = block
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::BurdenDec { var } if rep_of(*var) == rep));
        if !has_inc || !has_dec {
            continue;
        }
        // The callee must borrow-read without return-view aliasing.
        if !aggregate_borrow_read_relocatable(*callee, 0, &escape_safe_names, contracts) {
            continue;
        }
        // The receiver must die after the call (not live-out — a live-out aggregate
        // needs joint multi-use accounting this dec-relocation does not model).
        if lineage_live_out(func, &jt_reps, rep, b) {
            continue;
        }
        // SEED-not-reuse: a returned / owned-consumed / owned-moved-into-collection
        // lineage has its release elsewhere — relocating here double-frees.
        if returned.contains(&rep) || owned_consumed.contains(&rep) || owned_moved.contains(&rep) {
            continue;
        }
        relocations.push((b, recv, normal.index(), unwind.index()));
    }
    relocations
}

/// Phase 6.87: strip the coalesce-doomed pre-call `BurdenInc recv` + `BurdenDec
/// recv` pair on a fresh borrow-read-into-call inline aggregate and emit ONE
/// `BurdenDec recv` at the front of both dead successor edges (RL-4 `Both`).
/// Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS RL-2 + RL-4.
fn relocate_borrowed_terminator_aggregate_dec(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    let relocations =
        compute_borrowed_terminator_aggregate_relocations(func, pool, interner, contracts);
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    // Strip the coalesce-doomed inc + dec pair from each call block. The
    // Phase-5 walk emitted the targeted pair immediately before the
    // terminator, so the LAST matching inc and LAST matching dec are the
    // pair to remove — an earlier borrow-read call on the same rep may have
    // left an earlier (legitimate) pair that a first-match strip would
    // wrongly remove, double-releasing on the earlier site and leaking the
    // pre-terminator pair into coalescing.
    for &(b, recv, _, _) in &relocations {
        let rep = rep_of(recv);
        if let Some(block) = func.blocks.get_mut(b) {
            if let Some(idx) = block
                .body
                .iter()
                .rposition(|i| matches!(i, ArcInstr::BurdenInc { var } if rep_of(*var) == rep))
            {
                block.body.remove(idx);
            }
            if let Some(idx) = block
                .body
                .iter()
                .rposition(|i| matches!(i, ArcInstr::BurdenDec { var } if rep_of(*var) == rep))
            {
                block.body.remove(idx);
            }
        }
    }
    // Insert the single relocated release at the front of both successor edges.
    for &(_, recv, normal, unwind) in &relocations {
        if let Some(succ) = func.blocks.get_mut(normal) {
            succ.body.insert(0, ArcInstr::BurdenDec { var: recv });
        }
        if normal != unwind {
            if let Some(succ) = func.blocks.get_mut(unwind) {
                succ.body.insert(0, ArcInstr::BurdenDec { var: recv });
            }
        }
    }
}

/// Set of project-borrowed-view vars whose whole-var `BurdenDec` is a redundant
/// SECOND release of a heap field the source aggregate's `[AggFields]` /
/// `[InlineEnum]` drop already frees — the spurious dec to STRIP for
/// [`strip_redundant_project_borrowed_view_decs`].
///
/// A `let v = w.field` borrow-view of a LOCAL owned aggregate `src` is a TF-4
/// borrowed view (RL-4: a borrowed view emits no release). `src`'s own scope-exit
/// `RcDec [AggFields]`/`[InlineEnum]` (the aggregate drop-glue walk) IS the field's
/// single release (RL-2 `RL2_release_exactly_once`). When the Phase-5 walk ALSO
/// emits a whole-var `BurdenDec` on the borrow-view, the field is freed TWICE ->
/// double-free.
///
/// The discriminator is the per-allocation alloc-aware NET, NOT membership in
/// `collect_project_borrowed_defs` (a too-broad membership strip orphans
/// last-owner collection-field views: an `RcPtr` collection-field view that is the
/// buffer's last owner carries an unpaired dec too, and stripping it leaks).
/// Attribute the source aggregate's field-drop as the projected field's release:
///  - a `FatValue` str-field / `RcPtr` collection-field view whose source aggregate
///    is SINGLE-REF (the aggregate carries a freeing `BurdenDec` and NO keep-alive
///    `BurdenInc` raised its fields above rc 1) nets +1 surplus on the view's dec
///    -> STRIP. The aggregate `[AggFields]`/`[InlineEnum]` drop already releases
///    the field once.
///  - a view whose source aggregate IS bumped by a paired `BurdenInc` (the
///    aggregate is shared, rc >= 2 at the projection point) nets 0 -> KEEP. The
///    projection dec releases the EXTRA reference, not a redundant second release.
///
/// Precondition (the aggregate-drop-fires guard): `src` MUST carry a whole-var
/// freeing `BurdenDec`. A tuple/aggregate with NO scope-exit dec (the field
/// release is carried by the view dec alone) is NOT a double-free shape and is
/// left untouched (the strip would leak it).
fn compute_redundant_project_borrowed_view_dec_strips(
    func: &ArcFunction,
    pool: &Pool,
    project_borrowed_defs: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let let_alias_map = build_let_alias_map(func);
    let resolve_root = |var: ArcVarId| -> ArcVarId {
        let mut current = var;
        for _ in 0..64 {
            match let_alias_map.get(&current) {
                Some(&src) => current = src,
                None => break,
            }
        }
        current
    };

    // Direct-Project source of each borrow-view dst (the aggregate it projects).
    // Keyed by the Project dst; the value is the Project's `value` operand.
    let mut project_source: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    // Every var that carries a whole-var freeing `BurdenDec` (the aggregate-drop-
    // fires signal) and every var bumped by a `BurdenInc` (the paired-keep signal).
    let mut has_freeing_dec: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut has_keepalive_inc: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Project { dst, value, .. } => {
                    project_source.insert(*dst, *value);
                }
                ArcInstr::BurdenDec { var } => {
                    has_freeing_dec.insert(*var);
                }
                ArcInstr::BurdenInc { var } => {
                    has_keepalive_inc.insert(*var);
                }
                _ => {}
            }
        }
    }

    // The source aggregate `src` is single-ref iff neither `src` nor any of its
    // Let-Var aliases carries a `BurdenInc` (a keep-alive inc bumps the aggregate
    // fields above rc 1 -> the projection dec releases the extra ref -> KEEP).
    // Walk the alias-class of `src` for any inc.
    let alias_class_inc = |src: ArcVarId| -> bool {
        if has_keepalive_inc.contains(&src) {
            return true;
        }
        // A Let-Var alias `%a = %src` (or chain) carrying an inc bumps the same
        // allocation. Scan every var whose resolve_root == resolve_root(src).
        let src_root = resolve_root(src);
        for &inc_var in &has_keepalive_inc {
            if resolve_root(inc_var) == src_root {
                return true;
            }
        }
        false
    };
    // The source aggregate's drop fires iff `src` or a Let-Var alias of it carries
    // a freeing `BurdenDec` (its `[AggFields]`/`[InlineEnum]` drop walks the field).
    let alias_class_freeing_dec = |src: ArcVarId| -> bool {
        if has_freeing_dec.contains(&src) {
            return true;
        }
        let src_root = resolve_root(src);
        for &dec_var in &has_freeing_dec {
            if resolve_root(dec_var) == src_root {
                return true;
            }
        }
        false
    };

    let mut strips: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::BurdenDec { var: view } = instr else {
                continue;
            };
            // (1) The dec target is a borrow-view (non-take Project + alias closure).
            if !project_borrowed_defs.contains(view) {
                continue;
            }
            // (2) Trace the view (through Let-Var aliases) to its defining Project's
            // source aggregate. A view that is not (transitively) a Project dst —
            // e.g. a borrowed PARAM closed over by `propagate_borrowed_closure` —
            // is NOT this double-free shape (no source aggregate frees its field).
            let view_root = resolve_root(*view);
            let Some(&src) = project_source
                .get(&view_root)
                .or_else(|| project_source.get(view))
            else {
                continue;
            };
            let src_root = resolve_root(src);
            // (3) The source is a heap-bearing INLINE aggregate (struct / tuple /
            // enum / Option / Result whose `[AggFields]`/`[InlineEnum]` drop walks
            // a heap field).
            if !matches!(func.var_repr(src_root), Some(ValueRepr::Aggregate)) {
                continue;
            }
            if !is_burden_carrying_aggregate(src_root, func, pool) {
                continue;
            }
            // (4) The aggregate-drop-fires precondition: `src` carries a freeing
            // dec (its drop releases the field). Without it, the view dec is the
            // field's ONLY release (a tuple with no scope-exit dec) -> KEEP.
            if !alias_class_freeing_dec(src) && !alias_class_freeing_dec(src_root) {
                continue;
            }
            // (5) The net discriminator: the source aggregate is SINGLE-REF (no
            // keep-alive inc bumped its fields). A paired-inc shared aggregate's
            // projection dec releases the extra ref -> KEEP. SINGLE-REF -> the
            // aggregate drop + the view dec double-free the rc-1 field -> STRIP.
            if alias_class_inc(src) || alias_class_inc(src_root) {
                continue;
            }
            strips.insert(*view);
        }
    }
    strips
}

/// Phase 6.96 (probe) — strip the spurious whole-var `BurdenDec` on a
/// project-borrowed-view whose source aggregate's `[AggFields]`/`[InlineEnum]`
/// drop already frees the projected heap field (RL-4 borrowed view emits no
/// release; RL-2 the aggregate drop is the field's single release).
///
/// The Phase-5 walk emits a spurious last-use `BurdenDec` on a borrow-view of a
/// local owned aggregate (`let v = w.field`); the aggregate's own scope-exit
/// `RcDec [AggFields]` ALSO frees that field -> double-free. This pass removes the
/// view's dec when the alloc-aware net proves the aggregate drop is the single
/// release (per [`compute_redundant_project_borrowed_view_dec_strips`]). The
/// paired-inc shared-aggregate view (the projection dec releases an extra ref)
/// nets 0 and is left intact. Probe-gated -> default codegen byte-identical.
/// Spec: Annex E §AIMS RL-2 + RL-4.
fn strip_redundant_project_borrowed_view_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    project_borrowed_defs: &FxHashSet<ArcVarId>,
) {
    let strips =
        compute_redundant_project_borrowed_view_dec_strips(func, pool, project_borrowed_defs);
    if strips.is_empty() {
        return;
    }
    for block in &mut func.blocks {
        block
            .body
            .retain(|instr| !matches!(instr, ArcInstr::BurdenDec { var } if strips.contains(var)));
    }
}

/// Strip set for the comparison-operand keep-alive cure
/// ([`compute_comparison_operand_keepalive_strips`]): which `BurdenInc` vars and
/// which `(block_idx, BurdenDec var)` whole-var dec sites to remove.
pub(super) struct ComparisonOperandStrips {
    /// Spurious comparison-operand keep-alive `BurdenInc` vars to drop (M3).
    pub(super) inc_strips: FxHashSet<ArcVarId>,
    /// `(block_idx, var)` whole-var `BurdenDec` sites to drop — the misplaced
    /// branch dec the surviving operand dec already covers (M4).
    pub(super) dec_strips: FxHashSet<(usize, ArcVarId)>,
}

/// Compute the M3 + M4 strips for the USED-and-compared aggregate-with-heap-field
/// derived-`Eq` / derived-`Clone` `a == b` / `a != c` leak.
///
/// ROOT: a multi-use aggregate `%src` (struct / enum / Option / Result holding a
/// heap field) compared via `==` / `!=` gets ONE construct-keep-alive `BurdenInc
/// %src` (RL-1 duplication). Each comparison move-alias `%op = %src` whose SOLE
/// non-RC use is a `Binary(Eq|NotEq)` operand is wrongly classified `dup_alias_dst`
/// (`use_counts(%src) >= 2`), so the burden walk emits a SPURIOUS keep-alive
/// `BurdenInc %op` even though a `==` / `!=` operand is an RL-1 BORROW-READ
/// (`incElidable`, not a duplicating use). The spurious operand incs net the
/// `%src` allocation +1 on every path -> the heap field LEAKS.
///
/// The compiled-Lean oracle (`AimsProof.Realization`): a borrow-read operand emits
/// no inc (`RL1_emit_iff_not_elidable` — an inc iff `!incElidable`); the operand
/// DEC alone releases the construct keep-alive (`rcBalance` net-0). The fix is
/// case-(a) impl-conformance:
///
/// - M3: strip the spurious operand `BurdenInc %op` (KEEP its paired `BurdenDec
///   %op` — that dec balances the construct keep-alive `BurdenInc %src`).
/// - M4: on a block that ALSO carries a surviving comparison-operand alias
///   `BurdenDec` of the SAME `%src` lineage, strip the whole-var `BurdenDec %src`
///   (the operand dec already releases `%src` on that branch; the oracle emits the
///   whole-var release only on the COMPLEMENT branch that does NOT re-compare
///   `%src`). RL-2 `RL2_release_exactly_once`: `%src` released exactly once per
///   concrete path.
///
/// GATE (the over-fire boundary — the inline-struct projected-field shape, e.g.
/// `Config { settings, name }`): fires ONLY on lineages with a surviving
/// comparison-operand alias. A `Config` whose fields are PROJECTED (`Project
/// %c.0`) + independently freed has NO `Binary(Eq|NotEq)` operand alias, so no inc
/// is in `inc_strips` and no block carries an operand dec -> the M4 whole-var
/// strip never fires on it. The discriminator is the comparison-operand alias
/// membership, NEVER `use_counts` or aggregate membership. Probe-gated -> default
/// RECURSION GATE (the over-fire boundary for SELF-ALLOCATING aggregates): a
/// RECURSIVE boxed aggregate (`Node { value, next: Option<Node> }` — each `Cons`
/// node self-allocates a heap box) has a MULTI-NODE allocation chain whose
/// comparison-operand alias decs and whole-var node decs target DISTINCT
/// allocations that the lineage-root collapse cannot separate. The M3+M4 net
/// reasoning holds only for an INLINE NON-RECURSIVE aggregate (`is_self_allocating
/// _aggregate` false) whose single allocation the operand dec cleanly releases.
/// Self-allocating roots are EXCLUDED -> recursive-enum derived-`Eq` shapes are
/// left to the predicate-stack-mirrored baseline.
///
/// codegen byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn compute_comparison_operand_keepalive_strips(
    func: &ArcFunction,
    pool: &Pool,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> ComparisonOperandStrips {
    let let_alias_map = build_let_alias_map(func);
    let resolve_root = |var: ArcVarId| -> ArcVarId {
        let mut current = var;
        for _ in 0..64 {
            match let_alias_map.get(&current) {
                Some(&src) => current = src,
                None => break,
            }
        }
        current
    };

    // M3: the comparison-operand aliases (dst -> compared-aggregate lineage root)
    // whose spurious keep-alive `BurdenInc dst` is stripped.
    let operand_alias_root =
        compute_comparison_operand_aliases(func, pool, &resolve_root, same_alloc_reps);
    if operand_alias_root.is_empty() {
        return ComparisonOperandStrips {
            inc_strips: FxHashSet::default(),
            dec_strips: FxHashSet::default(),
        };
    }
    let inc_strips: FxHashSet<ArcVarId> = operand_alias_root.keys().copied().collect();

    // M4: the misplaced branch whole-var dec sites the surviving operand dec
    // already releases.
    let dec_strips = compute_comparison_operand_dec_strips(
        func,
        &operand_alias_root,
        &inc_strips,
        &resolve_root,
    );

    ComparisonOperandStrips {
        inc_strips,
        dec_strips,
    }
}

/// Comparison-operand aliases for [`compute_comparison_operand_keepalive_strips`]:
/// each `Let { dst, value: Var(src) }` whose `dst` is an `Aggregate` and whose
/// SOLE non-RC use is a `Binary(Eq|NotEq)` operand (an RL-1 borrow-read). Maps
/// `dst` -> lineage root of `src` (the compared aggregate). Self-allocating
/// (recursive boxed) roots are excluded by the recursion gate. `resolve_root`
/// traces Let-Var aliases to the lineage root.
fn compute_comparison_operand_aliases(
    func: &ArcFunction,
    pool: &Pool,
    resolve_root: &impl Fn(ArcVarId) -> ArcVarId,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    // FORWARDER-TRANSFER EXEMPTION to the same-root guard: a comparison whose two
    // operands trace to one `same_alloc` rep is NORMALLY a balanced self-comparison
    // (`a == a`), so the same-root guard declines the strip. But when the shared
    // allocation arises because one operand is a `transfers_through_return ∧ Direct`
    // forwarder RESULT (`let result = take_and_return(b)` where `result` aliases the
    // arg `b`'s allocation), the two operands (`result` and the live source `a`) are
    // GENUINELY DISTINCT owned co-references: the alloc was rc-INC'd (the `b = a`
    // duplication funds the transfer) so it carries TWO live refs, each owing one
    // release (RL-2 `RL2_release_exactly_once`). The forwarder-result lineage rep set
    // identifies exactly this case; an operand whose lineage IS a forwarder result is
    // exempted from the same-root exclusion so its spurious keep-alive inc is M3-
    // stripped (leaving its dec as one of the two genuine releases). Empty when
    // `ORI_DISABLE_COMPARISON_FORWARDER_SAME_ROOT_EXEMPT=1` (the guard stays purely
    // structural — the pre-cure behavior). Spec: Annex E §AIMS RL-1 + RL-2.
    let forwarder_result_reps = if comparison_forwarder_same_root_exempt_disabled() {
        FxHashSet::default()
    } else {
        let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
        compute_forwarder_result_reps(func, &jt_reps, same_alloc_reps)
    };
    let sa_rep = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let is_forwarder_transfer_pair = |a: ArcVarId, b: ArcVarId| -> bool {
        // A genuine 2-ref forwarder-transfer comparison: the two operands share an
        // allocation rep AND at least one operand's rep is a forwarder result.
        forwarder_result_reps.contains(&sa_rep(a)) || forwarder_result_reps.contains(&sa_rep(b))
    };

    // Per-var non-RC use tally + whether EVERY non-RC use is a `Binary(Eq|NotEq)`
    // operand. An RC op is NOT a use; a `Let { Var }` reference IS a use of its
    // source (counted when that source's occurrences are walked).
    let mut non_rc_use_count: FxHashMap<ArcVarId, u32> = FxHashMap::default();
    let mut all_uses_are_compare_operand: FxHashMap<ArcVarId, bool> = FxHashMap::default();
    // SAME-ROOT GUARD: operand vars of a `Binary(Eq|NotEq)` whose TWO operands
    // trace to the SAME `same_alloc` rep. M3/M4's net reasoning assumes the two
    // operands are DISTINCT allocations (two operand decs release two distinct
    // refs); when both alias ONE allocation, the two operand decs release the SAME
    // ref, so an added M4 whole-var strip over-releases (-1 double-free). Such
    // operands are EXCLUDED -> the same-root comparison's incs/decs stay at the
    // balanced baseline. RL-2 `RL2_release_exactly_once`: one allocation released
    // exactly once per path.
    let mut same_root_operands: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut note_use = |var: ArcVarId, is_compare_operand: bool| {
        *non_rc_use_count.entry(var).or_default() += 1;
        let entry = all_uses_are_compare_operand.entry(var).or_insert(true);
        *entry = *entry && is_compare_operand;
    };
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                value:
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(bop),
                        args,
                    },
                ..
            } = instr
            {
                let is_cmp = matches!(bop, ori_ir::BinaryOp::Eq | ori_ir::BinaryOp::NotEq);
                for &arg in args {
                    note_use(arg, is_cmp);
                }
                // A binary compare with both operands in ONE allocation class is a
                // same-root comparison; exclude its operands from the strip — UNLESS
                // the shared allocation arises from a forwarder transfer (the two
                // operands are then genuinely distinct co-references owing two
                // releases, so the strip MUST fire — see the forwarder-transfer
                // exemption above).
                if is_cmp
                    && args.len() == 2
                    && crate::aims::emit_rc::same_alloc(same_alloc_reps, args[0], args[1])
                    && !is_forwarder_transfer_pair(args[0], args[1])
                {
                    for &arg in args {
                        same_root_operands.insert(arg);
                    }
                }
                continue;
            }
            if whole_var_dec_or_inc(instr) {
                continue;
            }
            for &uv in &instr.used_vars() {
                note_use(uv, false);
            }
        }
        for &uv in &block.terminator.used_vars() {
            note_use(uv, false);
        }
    }

    let mut operand_alias_root: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            // A `Let { dst, Var(src) }` aliasing a compared lineage, OR a `Let {
            // dst, Literal(String) }` defining a fresh heap-str compared directly
            // (`a == "lit"`). The Literal case is its OWN allocation root; its
            // spurious operand keep-alive inc leaks the fresh literal buffer.
            let (dst, root_src): (&ArcVarId, ArcVarId) = match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => (dst, *src),
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Literal(crate::ir::LitValue::String(_)),
                    ..
                } => (dst, *dst),
                _ => continue,
            };
            // Widened from `Aggregate`-only to also cover heap-str / FatValue /
            // RcPointer compared operands (str `==`, list/map/set `==`). A `==` /
            // `!=` operand of any of these reprs is an RL-1 borrow-read
            // (`incElidable`); the spurious keep-alive inc leaks the same way.
            if !matches!(
                func.var_repr(*dst),
                Some(ValueRepr::Aggregate | ValueRepr::FatValue | ValueRepr::RcPointer)
            ) {
                continue;
            }
            // SAME-ROOT GUARD: skip an operand whose comparison's two operands
            // share one allocation class (over-strip -> double-free otherwise).
            if same_root_operands.contains(dst) {
                continue;
            }
            let uses = non_rc_use_count.get(dst).copied().unwrap_or(0);
            let all_cmp = all_uses_are_compare_operand
                .get(dst)
                .copied()
                .unwrap_or(false);
            if uses != 1 || !all_cmp {
                continue;
            }
            let root = resolve_root(root_src);
            // RECURSION GATE: a self-allocating (recursive boxed) aggregate's
            // multi-node allocation chain breaks the single-allocation net
            // reasoning -> excluded; only inline non-recursive aggregates fire.
            if is_self_allocating_aggregate(root, func, pool) {
                continue;
            }
            operand_alias_root.insert(*dst, root);
        }
    }
    operand_alias_root
}

/// Whether `instr` is a whole-var or field-grain burden RC op (not a value use).
fn whole_var_dec_or_inc(instr: &ArcInstr) -> bool {
    matches!(
        instr,
        ArcInstr::BurdenInc { .. }
            | ArcInstr::BurdenDec { .. }
            | ArcInstr::BurdenDecPartial { .. }
            | ArcInstr::BurdenDecVariant { .. }
            | ArcInstr::BurdenDecField { .. }
    )
}

/// M4 dec strips for [`compute_comparison_operand_keepalive_strips`]: the
/// `(block_idx, var)` whole-var `BurdenDec` sites the surviving comparison-operand
/// alias dec already releases. On a block that carries a surviving operand dec of
/// root R (an alias whose inc was stripped but whose dec was kept), the whole-var
/// `BurdenDec R` is the misplaced double-count (the oracle emits R's whole-var
/// release only on the complement branch). Gated on R carrying a surviving
/// construct keep-alive inc — without one, the whole-var dec is R's sole release
/// (KEEP). Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
fn compute_comparison_operand_dec_strips(
    func: &ArcFunction,
    operand_alias_root: &FxHashMap<ArcVarId, ArcVarId>,
    inc_strips: &FxHashSet<ArcVarId>,
    resolve_root: &impl Fn(ArcVarId) -> ArcVarId,
) -> FxHashSet<(usize, ArcVarId)> {
    let compared_roots: FxHashSet<ArcVarId> = operand_alias_root.values().copied().collect();
    let mut root_has_keepalive_inc: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::BurdenInc { var } = instr {
                let r = resolve_root(*var);
                if compared_roots.contains(&r) && !inc_strips.contains(var) {
                    root_has_keepalive_inc.insert(r);
                }
            }
        }
    }

    let mut dec_strips: FxHashSet<(usize, ArcVarId)> = FxHashSet::default();
    for (b, block) in func.blocks.iter().enumerate() {
        let mut roots_freed_by_operand_here: FxHashSet<ArcVarId> = FxHashSet::default();
        for instr in &block.body {
            if let ArcInstr::BurdenDec { var } = instr {
                if let Some(&root) = operand_alias_root.get(var) {
                    roots_freed_by_operand_here.insert(root);
                }
            }
        }
        if roots_freed_by_operand_here.is_empty() {
            continue;
        }
        for instr in &block.body {
            let ArcInstr::BurdenDec { var } = instr else {
                continue;
            };
            // Skip the operand-alias decs themselves (they ARE the release).
            if operand_alias_root.contains_key(var) {
                continue;
            }
            let root = resolve_root(*var);
            if roots_freed_by_operand_here.contains(&root) && root_has_keepalive_inc.contains(&root)
            {
                dec_strips.insert((b, *var));
            }
        }
    }
    dec_strips
}

/// Phase 6.97 (probe) — strip the spurious comparison-operand keep-alive
/// `BurdenInc` (M3) + the misplaced branch whole-var `BurdenDec` (M4) for the
/// USED-and-compared aggregate-with-heap-field derived-`Eq` / derived-`Clone`
/// `a == b` / `a != c` leak (per
/// [`compute_comparison_operand_keepalive_strips`]).
///
/// A `==` / `!=` operand is an RL-1 borrow-read (`incElidable`), so its keep-alive
/// inc is spurious; the operand DEC alone releases the compared aggregate's
/// construct keep-alive. The whole-var release lands on the COMPLEMENT branch
/// (RL-2 `RL2_release_exactly_once`). Net-matched: M3 removes the inc, the
/// operand dec balances the construct keep-alive, M4 removes the redundant
/// branch whole-var dec the operand dec already covers. Probe-gated -> default
/// codegen byte-identical. Spec: Annex E §AIMS RL-1 + RL-2.
fn strip_comparison_operand_keepalive(
    func: &mut ArcFunction,
    pool: &Pool,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    let strips = compute_comparison_operand_keepalive_strips(func, pool, same_alloc_reps);
    if strips.inc_strips.is_empty() && strips.dec_strips.is_empty() {
        return;
    }
    for (b, block) in func.blocks.iter_mut().enumerate() {
        block.body.retain(|instr| match instr {
            ArcInstr::BurdenInc { var } => !strips.inc_strips.contains(var),
            ArcInstr::BurdenDec { var } => !strips.dec_strips.contains(&(b, *var)),
            _ => true,
        });
    }
}

/// Phase 6.9 — dead in-function ITERATOR-HANDLE freeing (RL-2 `ScopeExit`). An
/// `@iter` / `@rev` / `@enumerate` result is a FRESH owned `Tag::Iterator` /
/// `DoubleEndedIterator` handle (`ValueRepr::RcPointer`, no RC header — the
/// source collection's buffer is MOVED into the iterator state). It must be freed
/// by a scope-exit `RcDec [Iterator]` (= `ori_iter_drop`) at its last use. When
/// the handle is MOVED into an aggregate field (`(int, Iterator<int>)` tuple /
/// a struct field), the freeing burden transfers to the AGGREGATE — its
/// scope-exit `RcDec [AggFields]` walks to the iterator field and `ori_iter_drop`s
/// it. Either freeing value lowers through `RcStrategy::from_repr` (Iterator for the
/// bare handle, `AggregateFields` for the aggregate) to the same op the default path
/// emits.
///
/// The leak under sole-emitter Phase-7 lowering: iterator handles carry NO RC
/// burden in `BURDEN_TABLE` / `TypeRegistry::burden` (they are `UnmanagedPtr`, no
/// refcount), so `collect_owned_burdens` never collects them and the Phase-5 walk
/// emits zero burden ops on the handle / its aggregate — under the flag the value
/// is never freed. The freeing is a DESTRUCTOR call (`ori_iter_drop`), not a
/// refcount dec, which the burden registry does not model. This pass restores the
/// RL-2 single-release the compiled-Lean `rcBalance` mandates for a fresh owned
/// value: emit ONE whole-var `BurdenDec` on the dead-at-scope-exit handle /
/// aggregate lineage.
///
/// SEED-not-reuse exclusions (so it never double-frees a for-loop cluster): the
/// freeing-value lineage must NOT be in `compute_iter_drop_handle_lineages` (a
/// for-loop `@iter` arg or `ori_iter_drop` arg — the loop lowering already emits
/// the `@ori_iter_drop` Apply on every exit path), NOT returned / transferred-out,
/// and NOT live past the sink block. A bare/aggregate handle that is consumed by
/// `iter_next` / `ori_iter_drop` is for-loop-managed → excluded. Spec: Annex E
/// §AIMS RL-2.
fn emit_burden_dead_iterator_handle_decs(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
) {
    let releases = compute_dead_iterator_handle_releases(func, pool, interner);
    for release in releases {
        match release {
            IterHandleRelease::EndOfBody { block_idx, var } => {
                if let Some(block) = func.blocks.get_mut(block_idx) {
                    block.body.push(ArcInstr::BurdenDec { var });
                }
            }
            IterHandleRelease::SuccessorFront { succ_idx, var } => {
                if let Some(succ) = func.blocks.get_mut(succ_idx) {
                    succ.body.insert(0, ArcInstr::BurdenDec { var });
                }
            }
        }
    }
}

/// Phase 6.10 — take-project iterator-handle SOURCE freeing (RL-2 `ScopeExit` +
/// bypass-safe per-class drop). Mirrors the predicate-stack `dead_cleanup`
/// emission via the shared [`TakeMoveFacts`](crate::aims::emit_rc::take_project)
/// SSOT (AIMS Invariant 5 — consumes the existing take-project analysis, no
/// parallel emitter).
///
/// An enum carrying an `Iterator` payload (`MaybeIter = Empty | Holds(it:
/// Iterator<int>)`) whose iterator is PROJECTED OUT on a match arm and consumed
/// at an owned position is a take-project source. The Phase-5 walk treats the
/// source enum as a normal owned `InlineEnum` (`BurdenInc` at the copy,
/// `BurdenDec` at last-use) — but the last-use is the `Project` on the consume
/// arm, so the spurious `BurdenDec` frees the source (and its iterator payload
/// via the `InlineEnum` drop) BEFORE the projected iterator is consumed
/// (use-after-free), AND no dec lands on the bypass / Empty paths (leak).
///
/// The predicate stack's `dead_cleanup` overrides the normal walk for in-class
/// take-project vars: it SKIPS the in-class last-use dec and emits ONE scope-exit
/// dec at each lineage's bypass-safe entry, deduplicated by `let_alias_rep`. This
/// pass restores that behavior on the burden path in two steps:
///   1. STRIP every Phase-5 `BurdenInc` / `BurdenDec` whose var is an in-class
///      take-project source (the net-0 copy/last-use pairs + the spurious
///      consume-arm dec) — these are the ops `dead_cleanup`'s `is_in_class` skip
///      suppresses.
///   2. EMIT ONE `BurdenDec` per let-alias rep at its `is_bypass_safe_entry_for_var`
///      block (RL-2 scope exit, fires once per CFG path; downstream bypass-safe
///      blocks inherit via SSA flow). Phase-7 lowers `BurdenDec [InlineEnum]` ->
///      `RcDec [InlineEnum]` (= the `InlineEnum` drop walking the iterator field ->
///      `ori_iter_drop`), matching the oracle.
///
/// Gated to take-project sources whose lineage carries an iterator handle (the
/// `compute_dead_iterator_handle_candidates` set), so it never touches a
/// non-iterator take-project class. Returned lineages (RL-2 transfers) are
/// excluded. Probe-gated -> default codegen byte-identical. Spec: Annex E §AIMS
/// RL-2.
fn emit_burden_take_project_source_decs(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    take_move_facts: &crate::aims::emit_rc::take_project::TakeMoveFacts,
) {
    let plan = compute_take_project_source_plan(func, state_map, pool, take_move_facts);
    if plan.strip_vars.is_empty() && plan.emits.is_empty() {
        return;
    }

    // Step 1 — strip the spurious in-class source ops (net-0 copy/last-use pairs
    // + the consume-arm dec). The oracle's `dead_cleanup` `is_in_class` skip
    // suppresses exactly these; the bypass-safe-entry dec below is the sole source
    // release.
    for block in &mut func.blocks {
        block.body.retain(|instr| match instr {
            ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var } => {
                !plan.strip_vars.contains(var)
            }
            _ => true,
        });
    }

    // Step 2 — emit ONE BurdenDec per let-alias rep at its bypass-safe entry. The
    // emits are pre-deduplicated by rep in `compute_take_project_source_plan`; a
    // bypass-safe-entry block is where the source enum is dead on that CFG path.
    for (block_idx, var) in plan.emits {
        if let Some(block) = func.blocks.get_mut(block_idx) {
            block.body.insert(0, ArcInstr::BurdenDec { var });
        }
    }
}

/// The strip-and-emit plan for [`emit_burden_take_project_source_decs`].
struct TakeProjectSourcePlan {
    /// In-class take-project source vars whose Phase-5 `BurdenInc`/`BurdenDec` ops
    /// must be stripped (the spurious copy/last-use pairs the oracle suppresses).
    strip_vars: FxHashSet<ArcVarId>,
    /// `(block_idx, var)` bypass-safe-entry releases, one per let-alias rep.
    emits: Vec<(usize, ArcVarId)>,
}

/// Compute the take-project source strip-and-emit plan. Returns an empty plan when
/// the function has no iterator-bearing take-project class.
fn compute_take_project_source_plan(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    take_move_facts: &crate::aims::emit_rc::take_project::TakeMoveFacts,
) -> TakeProjectSourcePlan {
    let empty = TakeProjectSourcePlan {
        strip_vars: FxHashSet::default(),
        emits: Vec::new(),
    };

    // Gate to iterator-bearing take-project sources. A take-project site is a
    // `Project %enum.payload` whose source is an `Enum`/`Option`/`Result` and whose
    // projected payload is a `Tag::Iterator` / `DoubleEndedIterator` (per
    // `is_take_project`). The `value` of each site IS the iterator-bearing source
    // enum — independent of how it was defined (a `Construct Variant(...)` literal
    // OR an `Apply`/`Invoke` call RESULT, e.g. `build(use_holds:)`). Reusing the
    // take-project sites as the source set (rather than the Phase-6.9
    // `Construct`-only candidate set) covers the call-result source.
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let mut source_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for (_blk, src) in crate::aims::emit_rc::take_project::collect_take_project_sites(func, pool) {
        source_reps.insert(rep_of(src));
    }
    if source_reps.is_empty() {
        return empty;
    }

    // Collect in-class take-project SOURCE-ENUM vars whose rep is a take-project
    // source. `is_in_class` is the over-approximating membership set (the same set
    // `dead_cleanup` / edge-cleanup skip), covering the source enum + its Let-alias
    // siblings + phi-merged block params.
    //
    // EXCLUDE bare iterator handles (`is_iterator_handle_dst`): the iterator
    // PROJECTED OUT of the enum (`%12 = Project %enum.1`, a `Tag::Iterator`
    // `RcPointer`) is ALSO in-class, but it is NOT the source enum — it is a
    // separate freeing value. In the UNUSED-binding case (`Holds(it) -> 42`) the
    // oracle emits a `RcDec [Iterator]` on the projected iterator directly (it is
    // dead-unused), freed by Phase-6.9's iterator-handle pass (case c — projected
    // handle); in the CONSUME case it is transferred to `@count [own]`. Stripping
    // its ops would remove that freeing dec -> leak. This pass owns ONLY the source
    // ENUM (InlineEnum/Aggregate) lineage; the projected handle stays Phase-6.9's.
    let mut strip_vars: FxHashSet<ArcVarId> = FxHashSet::default();
    for raw in 0..func.var_types.len() {
        let var = ArcVarId::new(u32::try_from(raw).unwrap_or(u32::MAX));
        if take_move_facts.is_in_class(var)
            && source_reps.contains(&rep_of(var))
            && !is_iterator_handle_dst(var, func, pool)
        {
            strip_vars.insert(var);
        }
    }
    if strip_vars.is_empty() {
        return empty;
    }

    // Returned lineages are RL-2 transfers — the caller inherits the release, so a
    // source dec here double-frees. Match the Phase-6.9 returned-lineage guard.
    let returned = compute_returned_lineages(func, &jt_reps);

    // Emit ONE BurdenDec per let-alias rep at its bypass-safe entry, iterating the
    // per-block ENTRY-STATE map exactly as the oracle's `dead_cleanup` does. The
    // entry-state `var` is the SSA value live at the block's entry (defined in a
    // dominating block OR the block's param), so `BurdenDec { var }` at the block
    // front is dominance-safe — a globally-iterated `var` would be inserted at a
    // block it does not reach (LLVM "does not dominate all uses"). `let_alias_rep`
    // dedup: two vars share a rep iff connected by `Let { dst, Var(src) }` chains
    // (the same runtime value), so the per-(rep, block) dedup prevents a double-free
    // on alias siblings, while phi-merged params with distinct reps (different
    // runtime values) each get a dec at their own bypass-safe entry.
    let mut emitted_rep_blocks: FxHashSet<(ArcVarId, usize)> = FxHashSet::default();
    let mut emits: Vec<(usize, ArcVarId)> = Vec::new();
    for b in 0..func.blocks.len() {
        let Some(entry_states) = state_map.block_entry_states(crate::aims::emit_rc::block_id(b))
        else {
            continue;
        };
        for &var in entry_states.keys() {
            if !take_move_facts.is_in_class(var)
                || !source_reps.contains(&rep_of(var))
                || is_iterator_handle_dst(var, func, pool)
            {
                continue;
            }
            if returned.contains(&rep_of(var)) {
                continue;
            }
            if !take_move_facts.is_bypass_safe_entry_for_var(var, b) {
                continue;
            }
            let Some(alias_rep) = take_move_facts.let_alias_rep(var) else {
                continue;
            };
            if emitted_rep_blocks.insert((alias_rep, b)) {
                emits.push((b, var));
            }
        }
    }

    // Dead-at-bypass-entry fallback (the OUTER-runtime-gate shape): when the
    // take-project consume is itself gated behind an outer runtime branch
    // (`if flag then <match consumes> else 0`), the source enum is dead-at-entry
    // (AIMS `Cardinality = Absent`) on the BYPASS edge — so it is ABSENT from that
    // block's entry-state map and the entry-states loop above emits NO release
    // there. The enum's iterator payload then leaks on every bypass path (its only
    // release, the consume arm's dec, lives in the unreachable `then` sub-CFG). RL-4
    // (`RL4_edge_dec_decision`): a value live at the branch's exit but dead at the
    // bypass successor's entry, not handed off, owes exactly one edge dec at that
    // successor's front.
    //
    // The fallback emits ONE `BurdenDec` per (alias_rep, bypass-safe-entry block)
    // for an in-class source var that is dead-at-entry there yet DOMINANCE-SAFE:
    //   - the var is in the take-project SOURCE class (`is_in_class` + `source_reps`
    //     + `let_alias_rep`) — the SAME landed `TakeMoveFacts` membership the
    //     entry-states loop uses (no new tracker / no use-count proxy);
    //   - the var is dead at the bypass-safe-entry block (NOT already emitted there
    //     via the entry-states pass — `emitted_rep_blocks` dedups);
    //   - the var's DEFINING block strictly dominates the bypass-safe-entry block
    //     (`DominatorTree::dominates`), so `BurdenDec { var }` at the block front is
    //     dominance-safe (LLVM "value dominates all uses" holds);
    //   - not returned (RL-2 transfer), not an iterator-handle (Phase-6.9 owns it).
    //
    // Over-fire boundary: the bypass-safe-entry set already excludes any block
    // forward/backward-reachable from a take-project (the consume site), so a block
    // where the source is consumed is never admitted — the dec lands only on a path
    // where the source genuinely dies unconsumed. Disjoint runtime paths from the
    // consume arm's dec (mutual-exclusion via the bypass-safe partition), so no
    // double-free. Spec: Annex E §AIMS RL-4 + RL-2.
    if *TAKE_PROJECT_BYPASS_ENTRY_RELEASE_DISABLED {
        return TakeProjectSourcePlan { strip_vars, emits };
    }
    let doms = crate::graph::DominatorTree::build(func);
    let def_block = take_project_source_def_blocks(func);
    for b in 0..func.blocks.len() {
        let b_id = crate::aims::emit_rc::block_id(b);
        for &var in &strip_vars {
            if is_iterator_handle_dst(var, func, pool) {
                continue;
            }
            if returned.contains(&rep_of(var)) {
                continue;
            }
            if !take_move_facts.is_bypass_safe_entry_for_var(var, b) {
                continue;
            }
            let Some(alias_rep) = take_move_facts.let_alias_rep(var) else {
                continue;
            };
            // Skip if the entry-states pass already placed a dec for this rep here.
            if emitted_rep_blocks.contains(&(alias_rep, b)) {
                continue;
            }
            // Dominance-safety: the source var's def block must strictly dominate `b`
            // (the var is live across the dominating-block → `b` edge, dead in `b`).
            let Some(&db) = def_block.get(&var) else {
                continue;
            };
            if db == b {
                continue;
            }
            if !doms.dominates(crate::aims::emit_rc::block_id(db), b_id) {
                continue;
            }
            if emitted_rep_blocks.insert((alias_rep, b)) {
                emits.push((b, var));
            }
        }
    }

    TakeProjectSourcePlan { strip_vars, emits }
}

/// Per-var defining block for take-project source members: the block index where
/// the var is bound (a block param, or an instruction `dst`). Used by the
/// dead-at-bypass-entry fallback to prove the source var's def dominates the
/// bypass-safe-entry block (dominance-safe `BurdenDec` placement).
fn take_project_source_def_blocks(func: &ArcFunction) -> FxHashMap<ArcVarId, usize> {
    let mut def_block: FxHashMap<ArcVarId, usize> = FxHashMap::default();
    for (b, block) in func.blocks.iter().enumerate() {
        for &(param, _) in &block.params {
            def_block.insert(param, b);
        }
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                def_block.insert(dst, b);
            }
        }
    }
    def_block
}

/// Where a dead iterator-handle release dec lands: at the END of the block body
/// (the handle / aggregate is defined + dies in the same block — bare `Apply`
/// `@iter`, `Construct` aggregate) OR prepended to a SUCCESSOR block (the handle
/// is defined by this block's `Invoke @iter` TERMINATOR, so it is born on the
/// successor edge and dies there — the dec belongs at the successor's front,
/// after the handle is materialized, per RL-2 scope exit on that path).
#[derive(Debug)]
enum IterHandleRelease {
    EndOfBody { block_idx: usize, var: ArcVarId },
    SuccessorFront { succ_idx: usize, var: ArcVarId },
}

/// Compute the `(last_live_block_idx, var)` dead iterator-handle releases for
/// [`emit_burden_dead_iterator_handle_decs`].
///
/// The freeing-value candidate set is two kinds:
///   (a) a bare iterator handle — an `@iter`-family `Apply`/`Invoke` RESULT of
///       `Tag::Iterator` / `DoubleEndedIterator` repr (the freeing value IS the
///       handle, lowered `RcStrategy::Iterator`);
///   (b) a fresh `Construct` aggregate (Tuple / Struct / Enum) that consumes an
///       iterator handle at an owned position — the handle transferred ownership
///       into the aggregate, so the AGGREGATE is the freeing value (lowered
///       `RcStrategy::AggregateFields` / `InlineEnum`, whose drop walks to the
///       iterator field).
///
/// Returns at most one release per (lineage, last-live block): the lineage's SSA
/// value live at the END of the block after which it is dead and is NOT freed by a
/// for-loop `@ori_iter_drop`.
fn compute_dead_iterator_handle_releases(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
) -> Vec<IterHandleRelease> {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let candidate_reps = compute_dead_iterator_handle_candidates(func, pool, &jt_reps);
    if candidate_reps.is_empty() {
        return Vec::new();
    }

    // SEED-not-reuse exclusions. A for-loop iterator handle flows into `@iter_next`
    // and `@ori_iter_drop` — the loop lowering ALREADY frees it on every exit path,
    // so `compute_iter_drop_handle_lineages` (which collects `@iter` + `ori_iter_drop`
    // args) holds it → excluded (a dec here double-frees). Returned lineages are
    // RL-2 transfers (the caller inherits the release). A lineage consumed at an
    // owned position somewhere is transferred-out (the consume releases / re-binds
    // it) — though a candidate aggregate that is owned-consumed at scope exit is
    // handled by the per-block owned-consume guard below, not excluded wholesale.
    let iter_drop_handles = compute_iter_drop_handle_lineages(func, &jt_reps, interner);
    let returned = compute_returned_lineages(func, &jt_reps);

    let mut excluded: FxHashSet<ArcVarId> = iter_drop_handles;
    excluded.extend(returned);

    let mut releases: Vec<IterHandleRelease> = Vec::new();
    for &rep in &candidate_reps {
        if excluded.contains(&rep) {
            continue;
        }
        // Case 1: the lineage is DEFINED by an `Invoke @iter` TERMINATOR (a
        // may-unwind iter call). The handle is born on the normal-successor edge,
        // never referenced in the defining block body — its sink is the front of
        // the normal successor (where it is dead). The unwind edge is owned by the
        // predicate-stack's RL-4 source cleanup (the iterator was not constructed
        // on that path), so only the normal edge gets the handle dec.
        if let Some((succ_idx, var)) = iter_handle_invoke_terminator_sink(func, &jt_reps, rep) {
            if !lineage_live_out_from_block(func, &jt_reps, rep, succ_idx) {
                releases.push(IterHandleRelease::SuccessorFront { succ_idx, var });
            }
            continue;
        }
        // Case 2: the lineage is defined + dies in one block (a bare `Apply @iter`
        // result, or a `Construct` aggregate). End-of-body sink in the last block
        // holding a live reference, after which it is dead.
        for b in 0..func.blocks.len() {
            let Some(var) = lineage_last_reference_in_block(func, &jt_reps, rep, b) else {
                continue;
            };
            if lineage_live_out(func, &jt_reps, rep, b) {
                continue;
            }
            // A block that consumes the lineage at an OWNED position transfers it
            // out (moved into a callee / a parent Construct field) — the consume
            // already re-binds ownership, so a dec here double-frees.
            if block_owned_consumes_lineage(func, rep, &jt_reps, b) {
                continue;
            }
            releases.push(IterHandleRelease::EndOfBody { block_idx: b, var });
        }
    }
    releases
}

/// When the lineage `rep` is DEFINED by an `Invoke @iter`-family TERMINATOR
/// (the iterator handle is born on the normal-successor edge), return
/// `(normal_successor_idx, handle_var)`. The handle is materialized on the normal
/// edge and never referenced in the defining block, so its scope-exit release
/// lands at the front of the normal successor. Returns `None` when no candidate
/// member is defined by an `Invoke` terminator.
fn iter_handle_invoke_terminator_sink(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
) -> Option<(usize, ArcVarId)> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    for block in &func.blocks {
        if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
            if rep_of(*dst) == rep {
                return Some((normal.index(), *dst));
            }
        }
    }
    None
}

/// Whether the lineage `rep` is live anywhere in `start_idx` OR its successor
/// subgraph (the block ITSELF included — used for the Invoke-successor sink where
/// the dec lands at the front of `start_idx`, so a live reference INSIDE
/// `start_idx` means the handle survives and must not be freed at its front).
fn lineage_live_out_from_block(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    start_idx: usize,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = vec![start_idx];
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        if b != start_idx && block.params.iter().any(|&(p, _)| rep_of(p) == rep) {
            return true;
        }
        for instr in &block.body {
            if instr.used_vars().iter().any(|&v| rep_of(v) == rep) {
                return true;
            }
        }
        if block
            .terminator
            .used_vars()
            .iter()
            .any(|&v| rep_of(v) == rep)
        {
            return true;
        }
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    false
}

/// The freeing-value candidate reps (under `jt_reps`) for the dead-iterator-handle
/// pass: bare `@iter`-family iterator-handle results, plus fresh `Construct`
/// aggregates that consume an iterator handle at an owned field position.
fn compute_dead_iterator_handle_candidates(
    func: &ArcFunction,
    pool: &Pool,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    // Iterator-handle reps: vars typed `Tag::Iterator` / `DoubleEndedIterator`,
    // `ValueRepr::RcPointer` (the only iterator repr). Used to recognise both the
    // bare candidate (the handle itself) and the aggregate trigger (a Construct
    // arg that is an iterator handle).
    let mut handle_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for raw in 0..func.var_types.len() {
        let var = ArcVarId::new(u32::try_from(raw).unwrap_or(u32::MAX));
        if is_iterator_handle_dst(var, func, pool) {
            handle_reps.insert(rep_of(var));
        }
    }
    if handle_reps.is_empty() {
        return FxHashSet::default();
    }
    let mut candidates: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                // (a) Bare iterator-handle result: an `Apply`/`Invoke` whose dst is
                // an iterator handle (`@iter` / `@rev` / `@enumerate` produce a fresh
                // owned iterator). (c) Iterator PROJECTED OUT of an enum/aggregate: a
                // `Project` whose dst is an iterator handle
                // (`%it = Project %enum.payload`). Both are the freeing value
                // (lowered `RcStrategy::Iterator` -> `ori_iter_drop`); identical
                // body, so they share an arm. For (c) on a take-project UNUSED arm
                // (`Holds(it) -> 42`) the projected iterator is dead and freed here;
                // on a CONSUME arm it is transferred at an owned position to `@count`
                // and the per-block owned-consume guard suppresses the dec (the
                // source enum's own dec is suppressed by the Phase-6.10 strip, so the
                // projected handle is the sole release for the iterator payload).
                ArcInstr::Apply { dst, .. }
                | ArcInstr::ApplyIndirect { dst, .. }
                | ArcInstr::Project { dst, .. }
                    if is_iterator_handle_dst(*dst, func, pool) =>
                {
                    candidates.insert(rep_of(*dst));
                }
                // (b) Aggregate consuming an iterator handle: a `Construct`
                // Tuple/Struct/Enum whose args include an iterator handle. The
                // aggregate dst is the freeing value (its `AggFields`/`InlineEnum`
                // drop walks the iterator field).
                ArcInstr::Construct { dst, args, .. }
                    if args.iter().any(|&a| handle_reps.contains(&rep_of(a))) =>
                {
                    candidates.insert(rep_of(*dst));
                }
                _ => {}
            }
        }
        if let ArcTerminator::Invoke { dst, .. } = &block.terminator {
            if is_iterator_handle_dst(*dst, func, pool) {
                candidates.insert(rep_of(*dst));
            }
        }
    }
    candidates
}

/// Whether `dst` holds an owned iterator handle (`Tag::Iterator` /
/// `DoubleEndedIterator`, `ValueRepr::RcPointer`). These map to
/// `RcStrategy::Iterator` (= `ori_iter_drop`), NOT a refcount dec.
fn is_iterator_handle_dst(dst: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    if !matches!(func.var_repr(dst), Some(ValueRepr::RcPointer)) {
        return false;
    }
    matches!(
        pool.tag(pool.resolve_fully(func.var_type(dst))),
        ori_types::Tag::Iterator | ori_types::Tag::DoubleEndedIterator
    )
}

/// The lineage's SSA value referenced (at ANY position) in `block_idx`, returning
/// the LAST such reference, else `None`. Unlike [`lineage_last_use_in_block`]
/// (borrowed positions only), this admits the DEFINING site as a reference — a
/// bare unused iterator handle (`let _it = [..].iter(); 0`) and an aggregate never
/// re-read have their only reference at the defining instruction's dst, which is
/// the value to dec at scope exit.
fn lineage_last_reference_in_block(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block_idx: usize,
) -> Option<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let block = func.blocks.get(block_idx)?;
    let mut found: Option<ArcVarId> = None;
    for instr in &block.body {
        if let Some(dst) = instr.defined_var() {
            if rep_of(dst) == rep {
                found = Some(dst);
            }
        }
        for &arg in &instr.used_vars() {
            if rep_of(arg) == rep {
                found = Some(arg);
            }
        }
    }
    for &arg in &block.terminator.used_vars() {
        if rep_of(arg) == rep {
            found = Some(arg);
        }
    }
    found
}

/// Where a dead-owned-collection-or-str release dec is emitted for `var` in
/// `block_idx`: at the END of the block body (the last borrowed use is a body
/// instruction) OR prepended to the NORMAL SUCCESSOR block (the last use is the
/// block's terminator borrowed arg — the value is read DURING the call and
/// survives onto the normal-return edge, so the release follows there per RL-4).
enum DeadDecPlacement {
    EndOfBody,
    NormalSuccessorFront(usize),
}

/// Decide the placement for a dead-owned-collection release of the lineage of
/// `var` in `block_idx`. When the block's terminator is a borrowed `Invoke` whose
/// args include the lineage (the last use is the terminator), the dec follows on
/// the normal-successor edge (RL-4). Otherwise end-of-body (the last use is a body
/// read; the dec follows it before the terminator).
fn dead_collection_dec_placement(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    block_idx: usize,
    var: ArcVarId,
) -> DeadDecPlacement {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let rep = rep_of(var);
    let Some(block) = func.blocks.get(block_idx) else {
        return DeadDecPlacement::EndOfBody;
    };
    // If the lineage is used by the terminator (a borrowed `Invoke` arg — the
    // last-use sink predicate already established this block makes ONLY borrowed
    // uses of the lineage), the read happens DURING the call: release on the normal
    // successor edge. Only `Invoke` has a normal successor that can carry the dec;
    // a `Return`/`Jump`/`Branch` terminator never has a borrowed-Invoke last use of
    // an owned collection (it would be a transfer / handled elsewhere).
    if let ArcTerminator::Invoke { args, normal, .. } = &block.terminator {
        if args.iter().any(|&a| rep_of(a) == rep) {
            return DeadDecPlacement::NormalSuccessorFront(normal.index());
        }
    }
    DeadDecPlacement::EndOfBody
}

/// The defining block index + SSA value of the lineage `rep`'s defining instruction
/// (the body instr whose `defined_var` belongs to the lineage). Returns `None` when
/// no body instruction in the function defines the lineage (e.g. a block param). A
/// fresh-owned-collection rep is always defined by a body `Construct` / `Apply`, so
/// this resolves the scope-exit sink for the no-use case (the value dies at the end
/// of the block that defined it, having never been used).
fn lineage_defining_block_and_var(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
) -> Option<(usize, ArcVarId)> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    for (b, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let Some(dst) = instr.defined_var() {
                if rep_of(dst) == rep {
                    return Some((b, dst));
                }
            }
        }
    }
    None
}

/// Whether the lineage `rep` is referenced at ANY position (borrowed OR owned, body
/// instr OR terminator OR block param) ANYWHERE after its defining instruction. A
/// fresh-owned-collection rep with NO such reference is genuinely DEAD with ZERO
/// uses (Dead / Absent) — RL-2 mandates an immediate scope-exit cleanup dec on it
/// ("unused owned non-scalar -> immediate `RcDec` at definition", Spec: Annex E
/// §AIMS RL-2). The defining `Construct`'s own `dst` is NOT a use; only operand reads
/// count. Distinguishes the no-use case (this returns false) from the borrowed-read
/// last-use case (handled by the borrowed-read sink) and the owned-transfer case
/// (excluded upstream).
fn lineage_has_any_use(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    for block in &func.blocks {
        if block.params.iter().any(|&(p, _)| rep_of(p) == rep) {
            return true;
        }
        for instr in &block.body {
            if instr.used_vars().iter().any(|&v| rep_of(v) == rep) {
                return true;
            }
        }
        if block
            .terminator
            .used_vars()
            .iter()
            .any(|&v| rep_of(v) == rep)
        {
            return true;
        }
    }
    false
}

/// Compute the `(last_use_sink_block_idx, var)` dead owned-collection / mutation-
/// result releases for [`emit_burden_dead_owned_collection_decs`].
///
/// Returns at most one release per (lineage, last-use sink block): the lineage's
/// SSA value live at the END of each block that makes the last borrowed use of a
/// FRESH owned-collection lineage whose alloc-aware net is still positive there
/// (the allocation unreleased at the sink's exit). PLUS the no-use case: a
/// fresh-owned-collection lineage with ZERO uses anywhere receives a scope-exit
/// cleanup dec at the END of its defining block (RL-2 unused-owned), gated on the
/// same exclusions + the same alloc-aware-net-positive discriminator.
fn compute_dead_owned_collection_releases(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> Vec<(usize, ArcVarId)> {
    // Seed the jump-threaded reps with the apply-Direct transfer edges so a
    // `transfers_through_return ∧ ReturnAliasShape::Direct` forwarder RESULT shares
    // its transferred owned arg's allocation rep. The result is then part of the
    // arg's fresh-owned-collection lineage; the alloc-aware net fires ONE scope-exit
    // dec when the whole chain nets `+1` (trivial `@id` — leaked) and leaves it when
    // the chain nets `0` (multi-borrow-then-return — already released). Scoped to
    // this pass only (the forwarder-RESULT leak); every other `compute_jump_threaded_reps`
    // consumer stays apply-Direct-free. Spec: Annex E §AIMS RL-2.
    let jt_reps = compute_jump_threaded_reps(func, Some(same_alloc_reps));
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // The leaked-lineage candidate set: FRESH owned-collection self-alloc lineages
    // (a `Construct` of List/Map/Set, or an `Apply`/`Invoke` returning an owned
    // collection — a mutation RESULT like `xs.sort()` / `m.insert(..)`). The
    // alloc-aware net gates which actually leak; the exclusions below remove every
    // transferred / iterator-managed / conversion-source lineage so the net-gate
    // never fires on a double-free.
    let fresh_collection_reps =
        compute_fresh_owned_collection_reps(func, pool, &jt_reps, same_alloc_reps, interner);
    if fresh_collection_reps.is_empty() {
        return Vec::new();
    }

    // Exclusion sets (SEED-not-reuse — never touch a for-loop / iterator cluster).
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    let borrowed_defs = crate::aims::emit_rc::collect_all_borrowed_defs(func, pool);
    let iter_drop_handles = compute_iter_drop_handle_lineages(func, &jt_reps, interner);
    // Owned-consumed lineages are transferred out (moved into a callee / Construct
    // field / COW-mutator owned receiver / `@iter`): the consume already
    // releases / re-binds them, so they are NOT leaked-at-scope-exit values. The
    // mutation SOURCE (`xs` of `xs.sort()`) is owned-consumed by the mutator →
    // excluded; a COW-mutation RESULT (`ys`, a distinct allocation) survives and is
    // eligible. A `transfers_through_return ∧ Direct` forwarder owned-arg is the
    // exception: `same_alloc_reps` merges its RESULT with the arg, so the
    // owned-position consume is a PASS-THROUGH (the result carries the allocation
    // forward) — `compute_owned_consumed_lineages` skips it so the forwarder lineage
    // stays eligible for the alloc-aware net.
    let owned_consumed = compute_owned_consumed_lineages(func, &jt_reps, same_alloc_reps);
    // RETURNED lineages are RL-2 transfers (the caller inherits the release): a
    // freeing dec on a returned collection double-frees with the caller's release.
    let returned = compute_returned_lineages(func, &jt_reps);
    // PrimOp operands (list-concat `+` / comparison `==` / …) have their RC
    // lifecycle owned by the PrimOp lowering — a dead-collection dec double-frees.
    let primop_operands = compute_primop_operand_lineages(func, &jt_reps);
    // USER-FUNCTION-CALL args: a collection passed to a non-builtin call is the
    // callee's concern (transfer OR borrow-by-callee — and the arg ownership is not
    // final at this phase). A scope-exit dec on such a lineage double-frees a
    // transfer or trips the A' COW-through-borrow blocker; builtin borrowed reads
    // (`@length`/`@contains_key`) are NOT in this set (the safe last-use reads).
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let user_call_args =
        compute_user_call_arg_lineages(func, pool, &jt_reps, &builtins, contracts, same_alloc_reps);
    // OWNED-MOVED-into-collection strs/collections: a value MOVED into a `Construct`
    // (collection-literal element) is the collection's sole reference (no inc) — its
    // `elem_dec_fn` is the only release, so a scope-exit dec on the local double-
    // frees. A COW-mutator borrowed element (`insert` key/val, `push` element) is
    // COPIED (val_inc) — its LOCAL survives and IS freed here (RL-2 last-use dec).
    let owned_moved = compute_collection_owned_moved_str_lineages(func, &jt_reps);
    // The conversion SOURCES (`m` of `m.keys()`) are handled by the dedicated
    // dead-collection-source pass at their loop-exit dead-block-param sink. Exclude
    // them here so the two passes never both fire on the same lineage.
    let conversion_sources = compute_conversion_source_reps(func, pool, &jt_reps, interner);

    let mut excluded_reps: FxHashSet<ArcVarId> = iter_drop_handles;
    excluded_reps.extend(owned_consumed);
    excluded_reps.extend(returned);
    excluded_reps.extend(primop_operands);
    excluded_reps.extend(user_call_args);
    excluded_reps.extend(owned_moved);
    excluded_reps.extend(conversion_sources);
    for &v in iter_element_defs.iter().chain(borrowed_defs.iter()) {
        excluded_reps.insert(rep_of(v));
    }

    // FORWARDER-RESULT lineage reps: a lineage whose allocation flows through a
    // `transfers_through_return ∧ Direct` forwarder (`%dst = Apply/Invoke
    // @id(%arg [own])` with `same_alloc(arg, dst)` — the apply-Direct merge). The
    // freeing dec for these fires at the lineage's SINGLE post-dominating dead sink
    // (the genuine scope-exit), NOT at every per-block dead sink: a branchy
    // multi-borrow-then-return forwarder (`@use_twice` with `&&` short-circuit)
    // already releases the result's own `binc`/`bdec` pair on each branch sink, so a
    // per-block `exit_net > 0` fire at those sinks double-frees. The post-dominating
    // sink is the one block dominating no further use yet post-dominating the
    // allocation — the unique RL-2 scope-exit point. The trivial `@id` has exactly
    // one dead sink (its post-dominator), so it is unaffected. Spec: Annex E §AIMS
    // RL-2 (`RL2_release_exactly_once`).
    let forwarder_result_reps = compute_forwarder_result_reps(func, &jt_reps, same_alloc_reps);

    let preds = crate::graph::compute_predecessors(func);
    let mut releases: Vec<(usize, ArcVarId)> = Vec::new();

    for &rep in &fresh_collection_reps {
        if excluded_reps.contains(&rep) {
            continue;
        }
        // Per-block alloc-aware delta: `Σ alloc(+1) for a FRESH member`
        // `+ Σ BurdenInc(member) − Σ whole-var BurdenDec*(member)`.
        let delta =
            compute_owned_collection_delta(func, pool, rep, &jt_reps, same_alloc_reps, interner);
        let nets =
            crate::aims::verify::burden_delta::compute_burden_entry_nets(func, &preds, &delta);

        // Convergence guard: `entry_net` is authoritative only on a
        // converged result. On `!converged` the nets are stale (freeze-on-disagree
        // / cap exhaustion), so an alloc-aware-net release decision derived from
        // them would be non-deterministic. Decline (the verifier reports the
        // non-convergence as a HARD failure; a missed release is a leak surfaced
        // there, never a UAF from a wrong release). No-op on every converged
        // function (Spec: Annex E §AIMS RL-2).
        if !nets.converged {
            continue;
        }

        // No-use case (RL-2 unused-owned): a fresh-owned-collection lineage with
        // ZERO uses anywhere (Dead / Absent) has no borrowed-read last-use sink, so
        // the borrowed-read loop below never fires on it — the value would leak (and
        // its element `@drop` side-effects would be silently elided). RL-2 mandates an
        // immediate scope-exit cleanup dec at the defining site. Land the dec at the
        // END of the defining block (the scope-exit point, before the terminator),
        // gated on the SAME alloc-aware-net-positive discriminator: the allocation's
        // `+1` is unreleased there. The exclusions already removed every transferred /
        // owned-consumed / returned / iterator-managed lineage, so a survivor with no
        // use is a genuine dead-no-use value (Spec: Annex E §AIMS RL-2).
        if !lineage_has_any_use(func, &jt_reps, rep) {
            if let Some((def_block, def_var)) = lineage_defining_block_and_var(func, &jt_reps, rep)
            {
                // A no-use burden-carrying AGGREGATE is Phase 6.85's domain
                // (`emit_burden_dead_no_use_aggregate_decs`). This pass's no-use path
                // handles collection buffers only; firing here would double-free the
                // aggregate against Phase 6.85's scope-exit dec.
                if is_burden_carrying_aggregate(def_var, func, pool) {
                    continue;
                }
                if let Some(Some(entry_net)) = nets.entry_net.get(def_block) {
                    if entry_net + delta[def_block] > 0 {
                        releases.push((def_block, def_var));
                    }
                }
            }
            continue;
        }

        // Candidate borrowed-read-dead sinks for this lineage (block, live SSA var).
        let mut sinks: Vec<(usize, ArcVarId)> = Vec::new();
        for (b, &block_entry_net) in nets.entry_net.iter().enumerate() {
            // The last-use sink: a block that references the lineage at a BORROWED
            // position and after which the lineage is dead (no successor uses it).
            // The value survives its borrowed reads here (RL-2 dec follows the last
            // read), then dies — so the surplus alloc is released at this block.
            let Some(var) = lineage_last_use_in_block(func, &jt_reps, rep, b) else {
                continue;
            };
            if lineage_live_out(func, &jt_reps, rep, b) {
                continue;
            }
            // The lineage must be a borrowed-read-only sink: a block that consumes
            // the lineage at an OWNED position transfers ownership (already
            // released) — a dec here would double-free.
            if block_owned_consumes_lineage(func, rep, &jt_reps, b) {
                continue;
            }
            // Alloc-aware surplus at this block's EXIT: the net entering the block
            // plus the block's own delta. Positive = the allocation is still
            // unreleased here (the leak); 0 = already freed (don't fire); < 0 =
            // over-released (skip).
            let Some(entry_net) = block_entry_net else {
                continue;
            };
            let exit_net = entry_net + delta[b];
            if exit_net <= 0 {
                continue;
            }
            sinks.push((b, var));
        }
        // A FORWARDER-RESULT lineage already carries its own `binc`/`bdec` release
        // pairs on each branch sink (the multi-borrow-then-return `@use_twice` with
        // `&&` short-circuit branches); a per-block fire at every such sink
        // double-frees. The unbalanced surplus is a SINGLE allocation `+1`, released
        // exactly once: fire ONLY when there is exactly ONE candidate sink (the
        // trivial `@id` / multi-hop / non-generic forwarder — straight-line result
        // borrowed-read then dead). A multi-sink forwarder lineage is left to the
        // joint per-element/branch accounting (NOT this single-`+1` shape); firing
        // across its branch sinks over-releases. Non-forwarder lineages keep the
        // per-sink emission (their sinks are genuine independent per-path releases).
        // Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
        if forwarder_result_reps.contains(&rep) && sinks.len() != 1 {
            continue;
        }
        releases.extend(sinks);
    }
    releases
}

/// Whether `var`'s representation is an inline aggregate (`ValueRepr::Aggregate`)
/// whose type carries an RC burden (`classify_triviality == NonTrivial` — a
/// heap-bearing struct / tuple / Option / Result / enum field). The whole-var
/// `BurdenDec` lowers (Phase 7) through `RcStrategy::from_repr(Aggregate, ..)` to
/// `RcDec [AggFields]` (struct / tuple) or `RcDec [InlineEnum]` (sum type), whose
/// drop-glue walks the heap field(s). Scalars + non-burden-carrying aggregates
/// (`{ x: int, y: int }`) are excluded — they have no field drop-glue. The
/// triviality classifier is the SSOT both `ArcClassifier` and the burden registry
/// synthesis consume (no parallel heap-ness derivation). Spec: Annex E §AIMS RL-2.
pub(super) fn is_burden_carrying_aggregate(var: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    if !matches!(func.var_repr(var), Some(ValueRepr::Aggregate)) {
        return false;
    }
    if var.index() >= func.var_types.len() {
        return false;
    }
    matches!(
        ori_types::classify_triviality(func.var_type(var), pool),
        ori_types::Triviality::NonTrivial
    )
}

/// Whether `var` is a SELF-ALLOCATING burden-carrying aggregate: a
/// `ValueRepr::Aggregate` that is heap-boxed because its type is RECURSIVE
/// (self-referential — a boxed `Cons`/`Branch`/`Link` node). A recursive
/// aggregate's `Construct` allocates a heap node (RC header), so its lineage
/// owns a single-release allocation just like a collection buffer; the
/// fresh-owned dead-collection accounting (alloc-aware net + one `RcDec
/// [InlineEnum]`/`[AggFields]` at the dead sink) applies. NON-recursive inline
/// aggregates (`Doc { content: str }`, `Config { settings, name }`) are inline
/// (no self-buffer) — their heap FIELDS allocate separately and may be released
/// by independent field-projection decs, so the collection-buffer alloc-net
/// over-fires on them. Those are the no-use Phase-6.85 / field-walk domain, NOT
/// this pass. Spec: Annex E §AIMS RL-2.
fn is_self_allocating_aggregate(var: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    is_burden_carrying_aggregate(var, func, pool)
        && pool.aggregate_type_is_recursive(func.var_type(var))
}

/// Compute the `(defining_block_idx, var)` dead-no-use INLINE-AGGREGATE releases
/// for [`emit_burden_dead_no_use_aggregate_decs`].
///
/// The dead-no-use inline-aggregate candidate class (`[AggFields]`/`[InlineEnum]`
/// scope-exit release): a bare `let a = Doc { field: <heap> }` / `let c = Link(..)`
/// / `let t = (.., ..)` binds an inline struct / enum / tuple
/// (`ValueRepr::Aggregate`) whose type
/// `is_burden_carrying_aggregate` (a heap-bearing field), dead with ZERO uses
/// anywhere. The oracle emits one scope-exit `RcDec [AggFields]`/`[InlineEnum]`;
/// the Phase-5 walk emits ZERO burden ops on a no-use aggregate (no duplicating
/// use -> no inc, no last-use sink -> no dec), so the heap field is never freed.
/// RL-2 (`rl2_emits_dec(.ScopeExit)=true` + `RL2_release_exactly_once`) mandates
/// the single scope-exit release; this pass restores conformance by emitting ONE
/// whole-var `BurdenDec` at the END of the lineage's defining block.
///
/// FIELD-WALK NET model: an inline aggregate has NO self-buffer `+1` (it is not
/// heap-allocated); the dec balances the HEAP FIELD's implicit `+1` owned by the
/// `AggFields`/`InlineEnum` drop-glue — distinct accounting from the
/// collection-buffer alloc-net (the collection's own buffer carries the `+1`).
/// The candidate carries ZERO burden ops, so there is no inc to net against; the
/// emission is exactly the missing dec.
///
/// SEED-not-reuse exclusions (the SAME set as the dead-owned-collection pass, so
/// it never double-frees an owned-consumed / returned / iterator-managed
/// lineage): the candidate must NOT be owned-consumed into a parent Construct or
/// callee (a nested `Link(.., next: Link(..))` inner node is consumed into the
/// outer Construct -> only the OUTERMOST `let c` fires), NOT returned (RL-2
/// transfer -> the caller decs), NOT a `PrimOp` operand, NOT a user-call arg, NOT
/// owned-moved, NOT a borrowed def, NOT iterator-managed (the take-project source
/// pass owns iterator-bearing aggregates). Probe-gated -> default codegen
/// byte-identical. Spec: Annex E §AIMS RL-2.
fn compute_dead_no_use_aggregate_releases(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<(usize, ArcVarId)> {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // Candidate lineages: a `Construct` whose `dst` is a burden-carrying inline
    // aggregate (struct / tuple / Option / Result / enum holding a heap field).
    // An empty-arg Construct (`Nil` / a unit variant) allocates no heap field, so
    // it is skipped — only an aggregate with a heap-bearing payload leaks.
    let mut candidate_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, args, .. } = instr {
                if !args.is_empty() && is_burden_carrying_aggregate(*dst, func, pool) {
                    candidate_reps.insert(rep_of(*dst));
                }
            }
        }
    }
    if candidate_reps.is_empty() {
        return Vec::new();
    }

    // Exclusion sets (SEED-not-reuse — identical discipline to the dead-owned-
    // collection pass so the two never both fire on one lineage). `None`
    // same-alloc map: this pass's `jt_reps` is Let+Jump only (no apply-Direct
    // forwarder seed — a dead-no-use aggregate is not a forwarder result), so the
    // Direct-pass-through skips inside `compute_owned_consumed_lineages` /
    // `compute_user_call_arg_lineages` are no-ops (byte-identical to the
    // unseeded threading).
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    let borrowed_defs = crate::aims::emit_rc::collect_all_borrowed_defs(func, pool);
    let iter_drop_handles = compute_iter_drop_handle_lineages(func, &jt_reps, interner);
    // ITERATOR-BEARING aggregates are Phase 6.9's domain (an aggregate holding an
    // iterator handle frees via `RcDec [AggFields]`/`[InlineEnum]` walking to the
    // iterator field -> `ori_iter_drop`, emitted by `emit_burden_dead_iterator_handle_decs`).
    // Excluding the `compute_dead_iterator_handle_candidates` set (which recognises a
    // `Construct` with an iterator-handle arg as case (b)) prevents this pass from
    // double-freeing the same aggregate. SEED-not-reuse: consumes Phase 6.9's own
    // candidate computation, no parallel logic. Spec: Annex E §AIMS RL-2.
    let iterator_bearing = compute_dead_iterator_handle_candidates(func, pool, &jt_reps);
    // OWNED-CONSUMED: a nested aggregate is an owned arg of the parent Construct
    // (`is_owned_position` true for every Construct arg) -> consumed into the
    // outer aggregate's drop-glue. Excluding it leaves ONLY the outermost
    // dead-no-use lineage, whose single dec walks the whole nested tree.
    let owned_consumed = compute_owned_consumed_lineages(func, &jt_reps, &jt_reps);
    // RETURNED: an aggregate returned (or transferred to the caller) is an RL-2
    // transfer; a dec here double-frees against the caller's release.
    let returned = compute_returned_lineages(func, &jt_reps);
    // PrimOp operands (a struct compared via a derived-`Eq` `PrimOp` lowering) own
    // their RC lifecycle through the PrimOp — a dec double-frees.
    let primop_operands = compute_primop_operand_lineages(func, &jt_reps);
    // USER-CALL args: an aggregate passed to a non-builtin call is the callee's
    // concern (the arg ownership is not final at this phase).
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let user_call_args =
        compute_user_call_arg_lineages(func, pool, &jt_reps, &builtins, contracts, &jt_reps);
    // OWNED-MOVED-into-collection: an aggregate moved into a `Construct` element
    // is the collection's sole reference (`elem_dec_fn` frees it).
    let owned_moved = compute_collection_owned_moved_str_lineages(func, &jt_reps);

    let mut excluded_reps: FxHashSet<ArcVarId> = iter_drop_handles;
    excluded_reps.extend(iterator_bearing);
    excluded_reps.extend(owned_consumed);
    excluded_reps.extend(returned);
    excluded_reps.extend(primop_operands);
    excluded_reps.extend(user_call_args);
    excluded_reps.extend(owned_moved);
    for &val in iter_element_defs.iter().chain(borrowed_defs.iter()) {
        excluded_reps.insert(rep_of(val));
    }

    let mut releases: Vec<(usize, ArcVarId)> = Vec::new();
    for &rep in &candidate_reps {
        if excluded_reps.contains(&rep) {
            continue;
        }
        // A genuine dead-no-use value: ZERO references anywhere after its
        // defining `Construct` (RL-2 unused-owned). A used aggregate (borrowed
        // read, owned consume) is NOT a no-use value and is handled by the
        // walk's normal last-use / transfer paths.
        if lineage_has_any_use(func, &jt_reps, rep) {
            continue;
        }
        // Emit the single scope-exit dec at the END of the lineage's defining
        // block (the scope-exit point, before the terminator). Phase-7 lowers
        // `BurdenDec` on the Aggregate repr to `RcDec [AggFields]`/`[InlineEnum]`,
        // byte-identical to the oracle.
        if let Some((def_block, def_var)) = lineage_defining_block_and_var(func, &jt_reps, rep) {
            releases.push((def_block, def_var));
        }
    }
    releases
}

/// Compute the `(terminal_block_idx, var)` dead-collection-source releases for
/// [`emit_burden_dead_collection_source_decs`].
///
/// Returns at most one release per (lineage, terminal block): the block-param /
/// in-scope alias of the leaked owned collection-source at each normal terminal
/// block whose jump-threaded per-path net is positive.
fn compute_dead_collection_source_releases(
    func: &ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
) -> Vec<(usize, ArcVarId)> {
    let jt_reps = compute_jump_threaded_reps(func, None);
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);

    // Exclusion sets (SEED-not-reuse — never touch a for-loop cluster).
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);
    let borrowed_defs = crate::aims::emit_rc::collect_all_borrowed_defs(func, pool);
    let iter_drop_handles = compute_iter_drop_handle_lineages(func, &jt_reps, interner);
    // Lineages CONSUMED at an owned position somewhere (COW-mutated by `push` /
    // `set` / `insert` / concat, MOVED into a callee, transferred to a Construct
    // field, or `@iter`-consumed): the consume already releases / re-binds the
    // value, so it is NOT a borrowed-then-dead leaked source. The conversion
    // shapes (`m.keys()` / `s.split()`) consume their source ONLY at a BORROWED
    // position, so they survive this filter; a loop-reassigned `xs = xs.push(i)`
    // or a COW-mutated list is owned-consumed → excluded (the `push_corruption` /
    // loop-reassignment over-fires).
    // No apply-Direct seed here (this pass's `jt_reps` is Let+Jump only): passing
    // `&jt_reps` as the same-alloc map makes the forwarder pass-through skip a no-op
    // (an Apply result never shares a Let/Jump rep with its arg), byte-identical to
    // before — the dead-collection-SOURCE shape is not the forwarder-RESULT leak.
    let owned_consumed = compute_owned_consumed_lineages(func, &jt_reps, &jt_reps);

    // A lineage rep is an excluded (non-source) lineage if ANY of its members is
    // an iterator-element view, an iterator handle freed by `ori_iter_drop`, a
    // borrowed value, or owned-consumed somewhere. Compute the excluded rep set
    // once.
    let mut excluded_reps: FxHashSet<ArcVarId> = iter_drop_handles;
    excluded_reps.extend(owned_consumed);
    for &v in iter_element_defs.iter().chain(borrowed_defs.iter()) {
        excluded_reps.insert(rep_of(v));
    }

    // The leaked-source rep set: the lineage of a value passed at a BORROWED arg
    // position to a COLLECTION-CONVERSION builtin (`@keys` / `@values` /
    // `@split` / `@to_list`). This is the precise leaked shape — the
    // map/set/str SOURCE that the conversion borrows, surviving the call, then
    // dying at the post-loop block. Restricting to the conversion-borrowed-arg
    // lineage (vs any owned collection source) excludes loop-carried / COW
    // sources (`root = [1]`, `xs = xs.push(i)`) whose freeing the loop edge
    // cleanup already owns — those over-fire as a second dec (double-free).
    let source_reps = compute_conversion_source_reps(func, pool, &jt_reps, interner);
    if source_reps.is_empty() {
        return Vec::new();
    }

    let preds = crate::graph::compute_predecessors(func);
    let mut releases: Vec<(usize, ArcVarId)> = Vec::new();

    for &rep in &source_reps {
        if excluded_reps.contains(&rep) {
            continue;
        }
        // Per-block jump-threaded alloc-aware delta for this lineage:
        // `Σ alloc(+1) for a FRESH owned collection-source member`
        // `+ Σ BurdenInc(member) − Σ whole-var BurdenDec*(member)`.
        let mut delta: Vec<i64> = vec![0; func.blocks.len()];
        // Allocation `+1` per FRESH owned collection-source member of the lineage
        // (attributed to the block that defines it — body instr or terminator).
        for (b, block) in func.blocks.iter().enumerate() {
            for instr in &block.body {
                if let Some(dst) = collection_source_body_dst(instr, func, pool) {
                    if rep_of(dst) == rep {
                        delta[b] += 1;
                    }
                }
                if matches!(instr, ArcInstr::BurdenInc { var } if rep_of(*var) == rep) {
                    delta[b] += 1;
                } else if crate::aims::verify::burden_delta::whole_var_dec_target(instr).map(rep_of)
                    == Some(rep)
                {
                    delta[b] -= 1;
                }
            }
            // Terminator-defined collection sources (`Invoke @keys(...)`).
            if let ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } =
                &block.terminator
            {
                if is_owned_collection_dst(*dst, func, pool) && rep_of(*dst) == rep {
                    delta[b] += 1;
                }
            }
        }
        let nets =
            crate::aims::verify::burden_delta::compute_burden_entry_nets(func, &preds, &delta);

        // Convergence guard: the dead-sink leak release reads
        // `entry_net` at the sink; it is authoritative only on a converged
        // result. On `!converged` (freeze-on-disagree / cap exhaustion) the nets
        // are stale — decline rather than emit a release off a non-deterministic
        // net (the verifier reports the non-convergence as a HARD failure).
        // No-op on every converged function (Spec: Annex E §AIMS RL-2).
        if !nets.converged {
            continue;
        }

        for b in 0..func.blocks.len() {
            // The leaked source dies at the DEAD-SINK block for this lineage: it
            // arrives (a lineage member is this block's param) but is NOT freed
            // and does NOT flow onward live — no successor carries a lineage
            // member via Jump-arg, and the block neither uses nor re-defines it
            // (RL-5 dead-at-entry). An unwind sink (`Resume`) already received the
            // RL-4 edge dec, so it is excluded.
            if !is_lineage_dead_sink(func, &jt_reps, rep, b) {
                continue;
            }
            let Some(entry_net) = nets.entry_net[b] else {
                continue;
            };
            // A leaked lineage nets > 0 at the dead sink (the alloc's `+1`
            // unbalanced by any release on this path). Net 0 = already freed
            // (e.g. by the iterator drop); net < 0 = over-released (skip — a dec
            // here would worsen it). Emit exactly one dec for the surplus.
            if entry_net <= 0 {
                continue;
            }
            // The var to dec: the lineage's block-param at this sink (the
            // dead-at-entry collection-source). Freeing it frees the same
            // allocation; the block-param is the in-scope SSA value at the sink.
            let Some(var) = lineage_block_param(func, &jt_reps, rep, b) else {
                continue;
            };
            releases.push((b, var));
        }
    }
    releases
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

/// Whether `dst` holds a FRESH OWNED collection / string: an `RcPtr`/`FatVal`
/// value whose resolved tag is `List` / `Map` / `Set` / `Str`. The SOURCES are:
/// - a `Construct` of a collection ctor (the map/set/list literal source);
/// - a `Let { Literal(String) }` (a heap string literal — the `str.split()`
///   source `s` whose split parts are slice-views into its buffer);
/// - an `Apply` / `Invoke` whose result is an owned collection (the conversion
///   builtins `@keys` / `@values` / `@split` / `@to_list` etc. return a fresh
///   owned `[T]`). The iterator-from-collection RESULT is excluded later by the
///   iterator-handle / iter-element exclusion sets.
///
/// `Str` is included as a source because a borrowed-then-dead `str` (the source
/// of `s.split()`) leaks its buffer on the normal dead-block-param path exactly
/// like a map/set source; the whole-var dec lowers to `RcStrategy::FatPointer`
/// → `ori_rc_dec` on the data ptr (slice-aware), freeing the underlying buffer.
///
/// Returns `false` for scalars, non-collection/string results, and iterator
/// handles (`Tag::Iterator`).
fn is_owned_collection_dst(dst: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    if !matches!(
        func.var_repr(dst),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) {
        return false;
    }
    let resolved = pool.resolve_fully(func.var_type(dst));
    matches!(
        pool.tag(resolved),
        ori_types::Tag::List | ori_types::Tag::Map | ori_types::Tag::Set | ori_types::Tag::Str
    )
}

/// Whether a body instruction DEFINES a FRESH owned collection / string source,
/// returning its `dst`. Sources: `Construct` (collection ctor), `Let {
/// Literal(String) }` (heap string literal), `Apply` / `ApplyIndirect` results
/// (nounwind-call form of `@keys`/`@values`/`@split`/`@to_list`).
#[expect(
    clippy::match_same_arms,
    reason = "the heap-ctor/call arm and the Let{String} arm are distinct \
              instruction shapes that share a `Some(*dst)` body; merging them \
              obscures the two FRESH-source categories the pass distinguishes \
              (mirrors `fresh_self_alloc_dst`)"
)]
fn collection_source_body_dst(
    instr: &ArcInstr,
    func: &ArcFunction,
    pool: &Pool,
) -> Option<ArcVarId> {
    let dst = match instr {
        ArcInstr::Construct { dst, .. }
        | ArcInstr::Apply { dst, .. }
        | ArcInstr::ApplyIndirect { dst, .. } => *dst,
        ArcInstr::Let {
            dst,
            value: ArcValue::Literal(crate::ir::LitValue::String(_)),
            ..
        } => *dst,
        _ => return None,
    };
    is_owned_collection_dst(dst, func, pool).then_some(dst)
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

/// Seamless-slice producers whose result SHARES the receiver's backing buffer via
/// the `SLICE_FLAG` cap encoding: `slice` / `substring` (the
/// [`crate::borrow::sharing_builtin_names`] SSOT) plus `take` / `drop` (also
/// `make_slice_cap` slice views — `ori_list_slice_take` / `ori_list_slice_drop`).
/// The producer rc-INCs the shared buffer (rc 1 -> 2); a receiver dec placed
/// BEFORE the producer reads it frees the buffer early (UAF). The relocation moves the
/// receiver's inline dec to AFTER the borrowed read (its true last use), where the
/// buffer is live. Spec: Annex E §AIMS RL-2 + RL-4.
fn sharing_view_relocation_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    let mut names = crate::borrow::sharing_builtin_names(interner);
    names.insert(interner.intern("take"));
    names.insert(interner.intern("drop"));
    names
}

/// Collection-transform builtins that BORROW the receiver and whose result holds
/// its OWN ref to its backing buffer, so the borrowed receiver SURVIVES the call,
/// is dead on each successor, and its scope-exit dec relocates to BOTH the normal
/// and unwind edges (RL-2 `ApplyToBorrowedParam` caller dec + RL-4
/// `RL4_edge_release_balanced`). Two result shapes qualify because the receiver's
/// lineage is released exactly once on the successor edge: `filter`/`map` produce
/// a FRESH non-aliasing `[T]` buffer (runtime-verified distinct address + size,
/// source ref independent of the result); `clone` is an rc-INC of the SAME buffer
/// (rc 1 -> 2, the result owns its own ref, so the relocated source dec drops
/// rc 2 -> 1 and the result's own dec drops 1 -> 0 — balanced single free).
///
/// EXCLUDES seamless-slice / shared-buffer methods (`slice`/`take`/`substring`):
/// empirically those over-fire (a relocated source dec double-frees on several
/// slice/take shapes where the source is not single-dead-after-call) — they need
/// their own per-shape dec-placement accounting (Spec: Annex E §AIMS RL-2 + RL-4).
fn borrow_survives_transform_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ["filter", "map", "clone"]
        .iter()
        .map(|n| interner.intern(n))
        .collect()
}

/// Set-algebra ops whose borrowed `other` arg (arg 1) has its surviving
/// elements rc-inc'd into a FRESH result set by the runtime
/// (`inc_copied_set_elements` on every `ori_set_{union,intersection,difference}`
/// path), never aliased uninc'd. The receiver (arg 0) is consumed (COW); only
/// `other` is borrowed-and-element-retained, so `other`'s premature inline dec
/// relocates to BOTH successor edges of the may-unwind call (the receiver
/// relocation's arg-1 sibling). Spec: Annex E §AIMS RL-1 + RL-2 + RL-4.
fn set_algebra_relocation_names(interner: &ori_ir::StringInterner) -> FxHashSet<Name> {
    ["union", "intersection", "difference"]
        .iter()
        .map(|n| interner.intern(n))
        .collect()
}

fn compute_conversion_source_reps(
    func: &ArcFunction,
    pool: &Pool,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let conversion_names = collection_conversion_names(interner);
    // The receiver (arg 0) at a borrowed position of a conversion builtin.
    let consider = |callee: &Name,
                    args: &[ArcVarId],
                    ownership: &[crate::ir::ArgOwnership],
                    reps: &mut FxHashSet<ArcVarId>| {
        if !conversion_names.contains(callee) {
            return;
        }
        let Some(&recv) = args.first() else { return };
        // Receiver must be borrowed (the conversion does not consume it).
        let borrowed = ownership
            .first()
            .is_none_or(|o| *o == crate::ir::ArgOwnership::Borrowed);
        if borrowed && is_owned_collection_dst(recv, func, pool) {
            reps.insert(rep_of(recv));
        }
    };
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                consider(callee, args, arg_ownership, &mut reps);
            }
        }
        if let ArcTerminator::Invoke {
            func: callee,
            args,
            arg_ownership,
            ..
        } = &block.terminator
        {
            consider(callee, args, arg_ownership, &mut reps);
        }
    }
    reps
}

/// Lineage reps (under `jt_reps`) CONSUMED at an OWNED arg position by ANY body
/// instruction OR terminator across the function — a COW-mutator receiver
/// (`push`/`set`/`insert`/`remove`/`sort`/`reverse`), a concat operand, a value
/// MOVED into a callee / Construct field, or an `@iter`-consumed collection. Such
/// a lineage is released / re-bound by that consume, so it is NOT a
/// borrowed-then-dead leaked source. The conversion sources (`m`/`s` of
/// `m.keys()` / `s.split()`) are consumed ONLY at BORROWED positions → excluded
/// from this set → eligible for the dead-source dec.
fn compute_owned_consumed_lineages(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let sa_rep = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    // A `transfers_through_return ∧ ReturnAliasShape::Direct` forwarder call
    // (`%dst = Apply/Invoke @id(%arg [own])`) passes `%arg` at an owned position
    // but does NOT consume it — the result `%dst` IS the same allocation (recorded
    // as an apply-Direct edge in `same_alloc_reps`, so `sa_rep(arg) == sa_rep(dst)`).
    // The allocation survives as `%dst`; the genuine release is `%dst`'s scope-exit
    // dec, NOT a consume of `%arg`. Treating it as owned-consumed would exclude the
    // whole forwarder lineage from the dead-owned-collection net (the forwarder-
    // RESULT leak). A non-forwarder owned-arg (`@push(xs [own], ..)`, a Construct
    // field) has no such same-alloc result and stays a real consume.
    let is_direct_passthrough = |arg: ArcVarId, dst: ArcVarId| sa_rep(arg) == sa_rep(dst);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let passthrough_dst = match instr {
                ArcInstr::Apply { dst, .. } | ArcInstr::ApplyIndirect { dst, .. } => Some(*dst),
                _ => None,
            };
            for (pos, &arg) in instr.used_vars().iter().enumerate() {
                if instr.is_owned_position(pos) {
                    if passthrough_dst.is_some_and(|dst| is_direct_passthrough(arg, dst)) {
                        continue;
                    }
                    reps.insert(rep_of(arg));
                }
            }
        }
        let term = &block.terminator;
        let term_passthrough_dst = match term {
            ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } => {
                Some(*dst)
            }
            _ => None,
        };
        for (pos, &arg) in term.used_vars().iter().enumerate() {
            if term.is_owned_position(pos) {
                if term_passthrough_dst.is_some_and(|dst| is_direct_passthrough(arg, dst)) {
                    continue;
                }
                reps.insert(rep_of(arg));
            }
        }
    }
    reps
}

/// Lineage reps (under `jt_reps`) of every value passed as an argument to a
/// NON-BUILTIN user-function call (`Apply`/`Invoke`/`ApplyIndirect`/`InvokeIndirect`
/// whose callee is not a known builtin). A collection passed to a user function is
/// the CALLEE's concern — it is either transferred (owned arg, the callee releases)
/// or borrowed-by-the-callee, and crucially the call's arg ownership is NOT yet
/// final at this phase: a non-unwinding `Invoke @user(coll [borrow])` lowers to an
/// `Apply @user(coll [own])` at Phase 7, so a "borrowed" Phase-6.8 arg may become an
/// owned transfer. EITHER way a scope-exit dead-collection dec on such a lineage is
/// unsafe (double-free on transfer; the A' COW-through-borrow blocker on
/// borrow-by-callee). Builtin borrowed reads (`@length`/`@contains_key`/`@first`)
/// are NOT in this set — they truly borrow and are the safe last-use reads the pass
/// frees after.
fn compute_user_call_arg_lineages(
    func: &ArcFunction,
    pool: &Pool,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    builtins: &crate::borrow::BuiltinOwnershipSets,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let sa_rep = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    // `dst` of the call being considered (the forwarder result), or `None`. A
    // `transfers_through_return ∧ Direct` forwarder arg whose `same_alloc_reps` rep
    // equals the result's rep is a PASS-THROUGH: the callee returns the allocation
    // as the result, so the arg is NOT the callee's concern (no transfer / borrow
    // ambiguity — the result IS the same allocation, freed at its own scope exit).
    // Skipping it keeps the forwarder lineage eligible for the dead-owned net. A
    // genuine user-call arg (no same-alloc result) stays excluded.
    //
    // BORROWED-STR + BORROWED-READ-ONLY-COLLECTION carve-out: a value passed at an
    // EXPLICITLY Borrowed arg position that SURVIVES the call (the caller retains
    // ownership) needs ONE RL-2 scope-exit release the caller emits, not the
    // callee's concern (`ApplyToBorrowedParam` — RL-2 NON-transfer). It stays
    // ELIGIBLE for the dead-owned net (which fires the single release iff
    // alloc-aware-net +1); the `returned` / `owned_consumed` / `primop_operands`
    // exclusions + the borrowed-read-dead-sink + `exit_net > 0` gates still guard
    // every transfer / already-balanced shape. Two un-excluded classes:
    //  - FatValue `str` at a Borrowed position: a `str` is immutable — no
    //    COW-through-borrow hazard — so any Borrowed str arg survives.
    //  - COLLECTION (`[T]`/`{K:V}`/`Set`) at a Borrowed position whose callee's
    //    `ParamContract.borrowed_read_only` is `true` (the param flows ONLY to
    //    borrowed positions in the callee — no COW-mutation, no transfer, no
    //    iter-consume). Borrow inference leaves a COW-mutated receiver param
    //    `access: Borrowed`, so the call-site Borrowed annotation alone does NOT
    //    prove non-COW — the per-param `borrowed_read_only` contract fact does. A
    //    COW-mutating / iter-consuming callee (`borrowed_read_only == false`) stays
    //    EXCLUDED (un-excluding a COW-shared buffer's lineage double-frees).
    // Owned-position args (genuine transfer) and indirect/unknown callees stay
    // excluded. Spec: Annex E §AIMS RL-2 (`RL2_borrowed_param_emits_caller_dec` +
    // `RL2_release_exactly_once`).
    let consider = |callee: Option<Name>,
                    args: &[ArcVarId],
                    ownership: &[crate::ir::ArgOwnership],
                    dst: Option<ArcVarId>,
                    reps: &mut FxHashSet<ArcVarId>| {
        // A known builtin (borrowing read OR a COW-mutator handled elsewhere) is
        // safe; an indirect closure call (callee `None`) or an unknown user
        // function name is NOT.
        if let Some(name) = callee {
            if builtins.is_builtin(name) {
                return;
            }
        }
        for (pos, &arg) in args.iter().enumerate() {
            if !matches!(
                func.var_repr(arg),
                Some(ValueRepr::RcPointer | ValueRepr::FatValue)
            ) {
                continue;
            }
            if dst.is_some_and(|d| sa_rep(d) == sa_rep(arg)) {
                continue;
            }
            let arg_borrowed = ownership.get(pos) == Some(&crate::ir::ArgOwnership::Borrowed);
            // A `str` at an EXPLICITLY Borrowed position is caller-owned + survives
            // (immutable — no COW hazard). Keep eligible.
            if arg_borrowed && is_str_dst(arg, func, pool) {
                continue;
            }
            // A COLLECTION at a Borrowed position whose callee param is proven
            // `borrowed_read_only` survives the call (pure borrow-read, no COW /
            // transfer / iter-consume). Keep eligible. The contract fact is the
            // sole discriminator vs a COW-mutated borrowed param (which stays
            // excluded). Indirect / unknown callees (no contract) stay excluded.
            if arg_borrowed
                && is_collection_dst(arg, func, pool)
                && callee.is_some_and(|name| {
                    contracts
                        .get(&name)
                        .and_then(|c| c.params.get(pos))
                        .is_some_and(|p| p.borrowed_read_only)
                })
            {
                continue;
            }
            reps.insert(rep_of(arg));
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee,
                    args,
                    arg_ownership,
                    dst,
                    ..
                } => consider(Some(*callee), args, arg_ownership, Some(*dst), &mut reps),
                ArcInstr::ApplyIndirect { args, dst, .. } => {
                    consider(None, args, &[], Some(*dst), &mut reps);
                }
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee,
                args,
                arg_ownership,
                dst,
                ..
            } => {
                consider(Some(*callee), args, arg_ownership, Some(*dst), &mut reps);
            }
            ArcTerminator::InvokeIndirect { args, dst, .. } => {
                consider(None, args, &[], Some(*dst), &mut reps);
            }
            _ => {}
        }
    }
    reps
}

/// Lineage reps (under `jt_reps`) of every collection used as an operand of a
/// `Let { PrimOp }` (`RcPointer`/`FatValue` repr). A `PrimOp` operand's RC lifecycle
/// is owned by that `PrimOp`'s lowering: a list-concat `Binary(Add)` operand is
/// CONSUMED by `ori_list_concat_cow` (frees both operands — the COW-shared-LHS
/// `xs + ys; xs.length()` hazard); a list comparison `Binary(Eq)`/`Ne`/`Lt`…
/// operand is BORROWED and balanced by the existing COW-operand inc/dec around the
/// compare (the comparison-literal `x == [..]` hazard). EITHER way the operand's
/// release is already accounted, so the dead-owned-collection pass MUST NOT emit a
/// freeing dec on it (double-free). Scoped to the dead-owned-collection pass; the
/// shared `compute_owned_consumed_lineages` is unchanged (the B' conversion-source
/// pass does not see `PrimOp` operands).
fn compute_primop_operand_lineages(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                value: crate::ir::ArcValue::PrimOp { args, .. },
                ..
            } = instr
            {
                for &arg in args {
                    if matches!(
                        func.var_repr(arg),
                        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                    ) {
                        reps.insert(rep_of(arg));
                    }
                }
            }
        }
    }
    reps
}

/// Lineage reps (under `jt_reps`) of every value passed to an `ori_iter_drop`
/// Apply — the iterator handles the runtime drop frees. A leaked-source release
/// MUST NOT touch these (their buffer is owned by the iterator state and freed
/// by `ori_iter_drop`; a dec here double-frees the iterator-owned buffer).
fn compute_iter_drop_handle_lineages(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let iter_drop = interner.intern("ori_iter_drop");
    let iter_fn = interner.intern("iter");
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                // `ori_iter_drop(handle)` — the handle's lineage is iterator-owned.
                // `iter(coll)` — the collection is MOVED into the iterator state;
                // the iterator's `ori_iter_drop` frees that buffer, so the moved
                // collection's lineage is iterator-managed too (e.g. the keys
                // list of `map_keys_str`, consumed by `@iter` and freed by the
                // loop's `ori_iter_drop`).
                if *callee == iter_drop || *callee == iter_fn {
                    for &a in args {
                        reps.insert(rep_of(a));
                    }
                }
            }
        }
    }
    reps
}

/// The lineage's block PARAM (under `jt_reps`) at `block_idx`, or `None`. This
/// is the dead-at-entry SSA value the emitted `BurdenDec` references at the sink.
fn lineage_block_param(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block_idx: usize,
) -> Option<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let block = func.blocks.get(block_idx)?;
    block
        .params
        .iter()
        .find(|&&(param, _)| rep_of(param) == rep)
        .map(|&(param, _)| param)
}

/// Whether `block_idx` is the DEAD-SINK for the leaked-source lineage `rep`
/// (RL-5 dead-at-entry): a lineage member ARRIVES as this block's param, but the
/// lineage does NOT flow onward live and is not freed here. Concretely, ALL hold:
/// - the lineage has a block PARAM here (it arrived via a Jump-arg);
/// - the block's terminator is NOT `Resume` (an unwind sink already got the
///   RL-4 edge dec — a dec here would double-free on the unwind path);
/// - NO successor receives a lineage member via a Jump-arg / branch-arg (the
///   lineage does not flow onward — it dies at this block);
/// - the block body does NOT consume the lineage member at an owned position
///   that already transfers ownership (no double-free with an in-block release).
///
/// The "no successor carries it" check is the structural liveness test that
/// makes this the death block: a for-loop's back-edge block re-passes the
/// iterator/collection params to the loop header, so it is NEVER a dead sink —
/// only the post-loop block (which drops them) qualifies.
fn is_lineage_dead_sink(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block_idx: usize,
) -> bool {
    let Some(block) = func.blocks.get(block_idx) else {
        return false;
    };
    // Must arrive as a block param of this lineage.
    if lineage_block_param(func, jt_reps, rep, block_idx).is_none() {
        return false;
    }
    // Unwind sinks already received the RL-4 edge dec.
    if matches!(block.terminator, ArcTerminator::Resume) {
        return false;
    }
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    // The lineage must NOT already be RELEASED in THIS block: a whole-var
    // `BurdenDec*` of the lineage here means the burden walk already emits the
    // freeing dec (the merge-phi case `xs = if c then [..] else [..]` — both
    // branch Constructs jump into a block param whose last-use dec frees it).
    // Emitting a sink dec on top of that existing dec double-frees.
    for instr in &block.body {
        if crate::aims::verify::burden_delta::whole_var_dec_target(instr).map(rep_of) == Some(rep) {
            return false;
        }
        // A body instr that moves the lineage out at an OWNED position already
        // transfers ownership (a transfer release); a pure borrowed read
        // (`@len(x [borrow])`) does not, so it does not disqualify the sink.
        if owned_position_consumes_lineage(instr, rep, jt_reps) {
            return false;
        }
    }
    // The lineage must NOT be LIVE-OUT: no block transitively reachable from this
    // block's successors references a lineage member (as a param, operand, or
    // terminator arg). A loop header whose param flows around the loop back to a
    // later body block that re-passes it is therefore NOT a dead sink — the
    // lineage is live through the loop. Only the post-loop block (no successor
    // re-references the lineage) qualifies. This is the structural liveness test
    // that the per-block direct-terminator-arg check alone cannot make (the
    // lineage can flow to a successor's successor via a Jump-arg in a sibling
    // block, e.g. bb3→bb6→bb5 carrying `%14`).
    if lineage_live_out(func, jt_reps, rep, block_idx) {
        return false;
    }
    true
}

/// Whether lineage `rep` is LIVE-OUT of `block_idx`: some block transitively
/// reachable from `block_idx`'s successors references a lineage member (as a
/// block param, an instruction operand, or a terminator arg). A reference at the
/// candidate block itself does NOT count — only downstream blocks. Used to
/// reject loop-header / mid-loop blocks whose lineage flows around the loop.
fn lineage_live_out(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block_idx: usize,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let Some(start) = func.blocks.get(block_idx) else {
        return false;
    };
    // BFS over the successor subgraph (excluding the start block itself).
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = crate::graph::successor_block_ids(&start.terminator)
        .iter()
        .map(|b| b.index())
        .collect();
    while let Some(b) = stack.pop() {
        if b == block_idx || !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        // Param reference.
        if block.params.iter().any(|&(p, _)| rep_of(p) == rep) {
            return true;
        }
        // Operand reference in any body instr (a use keeps the lineage alive).
        for instr in &block.body {
            if instr.used_vars().iter().any(|&v| rep_of(v) == rep) {
                return true;
            }
        }
        // Terminator arg reference.
        if block
            .terminator
            .used_vars()
            .iter()
            .any(|&v| rep_of(v) == rep)
        {
            return true;
        }
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    false
}

/// Whether `instr` consumes a lineage member at an OWNED arg position (an
/// ownership transfer that already releases the value). A borrowed-position read
/// does NOT count. Used to keep the dead-sink dec from double-freeing a value
/// the block body already moves out.
fn owned_position_consumes_lineage(
    instr: &ArcInstr,
    rep: ArcVarId,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> bool {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    for (pos, &arg) in instr.used_vars().iter().enumerate() {
        if instr.is_owned_position(pos) && rep_of(arg) == rep {
            return true;
        }
    }
    false
}

/// Lineage reps (under `jt_reps`) of every FRESH owned-collection self-allocation
/// def-site: a non-empty `Construct` of a List/Map/Set, OR a COW-MUTATOR
/// `Apply`/`Invoke` result whose receiver type is a collection (`xs.sort()` /
/// `m.insert(..)` / `a.union(..)` — a `crate::borrow::all_cow_method_names`
/// builtin whose result is always `Unique`, RC == 1, a genuine fresh allocation).
///
/// Restricting Apply/Invoke results to the COW-mutator name set is load-bearing:
/// a `slice` / `substring` / `take` / `drop` result SHARES the receiver's backing
/// buffer (NOT a fresh allocation — `crate::borrow::sharing_builtin_names`), and
/// an arbitrary user function returning a collection is a transfer; neither
/// allocates a buffer this lineage owns, so a freeing dec there would double-free
/// the shared/transferred buffer. The empty-literal Construct (`{}`/`[]`) is
/// excluded (no backing buffer). `Str` is NOT included: a borrowed-then-dead
/// `str` source is the conversion-source pass's domain.
///
/// This is the leaked-lineage candidate set for [`compute_dead_owned_collection_releases`];
/// the alloc-aware net + the Return-transfer / owned-consumed exclusions gate which
/// candidates actually leak.
fn compute_fresh_owned_collection_reps(
    func: &ArcFunction,
    pool: &Pool,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let sa_rep = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let cow_names = crate::borrow::all_cow_method_names(interner);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    // A USER-FUNCTION call (`Invoke`/`Apply` to a non-builtin `Name`) returning a
    // collection is a FRESH owned allocation when the result is NOT a
    // `transfers_through_return ∧ Direct` forwarder pass-through. A forwarder
    // returns its owned arg verbatim (`@id(xs) = xs`); `same_alloc_reps` (which
    // folds `ApplyAliasSource::Direct`) merges the result with the arg, so
    // `sa_rep(dst) == sa_rep(arg)` for some arg. A genuine builder
    // (`@to_array(list) -> [int] = [h, ...]` / `@build(n) -> [int]`) returns a NEW
    // buffer the callee allocated — no same-alloc merge with any arg → fresh. The
    // alloc-aware net gates which actually leak; the exclusions in the caller
    // (returned / owned-consumed / user-call-arg / forwarder single-sink) remove
    // every transferred lineage so the net-gate never fires on a double-free. A
    // buffer-SHARING-view result (`@head(xs) = xs.slice(..)`) shares the arg's
    // backing but is NOT same-alloc-merged; the alloc-aware net still gates it (the
    // slice carries no fresh-alloc `+1` of its own) so it is not over-freed. Spec:
    // Annex E §AIMS RL-2.
    let user_call_fresh_result = |callee: Name, args: &[ArcVarId], dst: ArcVarId| -> bool {
        if builtins.is_builtin(callee) {
            return false;
        }
        // Direct-transfer forwarder result: merged with an arg → not a fresh alloc.
        if args.iter().any(|&arg| sa_rep(arg) == sa_rep(dst)) {
            return false;
        }
        true
    };
    // The fresh-LOCAL-Construct lineage reps: a non-empty collection Construct
    // DEFINED in this function (not a param). A COW-mutator result is freeable at
    // scope exit ONLY when its consumed receiver traces to such a fresh local
    // allocation (`let xs = [..]; xs.sort()` — the in-place reuse / COW-copy is
    // genuinely this function's allocation). When the receiver is a BORROWED PARAM
    // (`@check(list); list.push(..)`), the COW-through-borrowed-param interaction
    // (the A' blocker — the borrowed original may be mutated in place under the
    // flag) is NOT this pass's domain; excluding param-source COW results avoids
    // the double-free.
    let fresh_local_construct_reps =
        compute_fresh_local_construct_reps(func, pool, jt_reps, interner);
    let cow_receiver_is_fresh_local = |args: &[ArcVarId]| -> bool {
        args.first()
            .is_some_and(|&recv| fresh_local_construct_reps.contains(&rep_of(recv)))
    };
    // A collection-CONVERSION result (`m.values()` / `m.keys()` / `set.to_list()`)
    // is a FRESH owned collection the runtime allocates from the receiver's
    // contents — `let vals = m.values(); vals.len()` leaks the result list at
    // scope exit (the conversion result is borrowed-read then dead, never freed).
    // The conversion SOURCE is freed by the dedicated conversion-source pass; the
    // RESULT is freed here.
    let conversion_names = collection_conversion_names(interner);
    // An iterator-consumer result (`@collect`/`@collect_set`) is a FRESH owned
    // collection the runtime allocates distinct from the iterator + source (the
    // source is freed by `ori_iter_drop`); freed here as a dead-at-scope-exit value.
    let iter_consumer_names = iterator_consumer_collection_names(interner);
    // A set-algebra result (`a.union(b)` / `a.difference(b)` / `a.intersection(b)`)
    // is a FRESH owned Set the runtime allocates from the operands' contents,
    // distinct from both operands — borrowed-read then dead at scope exit.
    let set_algebra_names = collection_set_algebra_names(interner);
    // A fresh-str-producing method result (`@debug()` / `@to_str()`) is a FRESH owned
    // `str` the callee synthesises from the receiver, distinct from the receiver's
    // buffer — borrowed-read then dead at scope exit (`let s = p.debug(); s.contains(..)
    // && s.contains(..)`). The str analogue of the conversion / set-algebra producers;
    // freed here as a dead-at-scope-exit value. Spec: Annex E §AIMS RL-2.
    let fresh_str_names = fresh_str_producing_method_names(interner);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let dst = match instr {
                // A non-empty collection Construct is a fresh buffer allocation;
                // an empty literal (`{}`/`[]`) has no buffer — skip it.
                ArcInstr::Construct { dst, args, .. } if !args.is_empty() => *dst,
                // A heap `str` LITERAL is a fresh FatPointer allocation; a fresh
                // str borrowed-then-dead at scope exit (a lookup arg `contains_key(m,
                // "key")` / a comparison-free read) leaks its buffer just like a
                // collection. The STORE-into-collection exclusion (below, in the
                // caller) removes strs that a Construct/COW-mutator takes ownership
                // of (the collection's `elem_dec_fn` frees those).
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Literal(crate::ir::LitValue::String(_)),
                    ..
                } => *dst,
                // A COW-mutator result is a genuine fresh allocation (always
                // Unique) ONLY when its receiver is a fresh local Construct; non-COW
                // Apply results (slices, user fns) and param-receiver COW results
                // are excluded.
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } if cow_names.contains(callee) && cow_receiver_is_fresh_local(args) => *dst,
                // A collection-conversion result is a fresh owned collection.
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if conversion_names.contains(callee) => *dst,
                // An iterator-consumer result (`@collect`/`@collect_set`) is a fresh
                // owned collection — distinct allocation from the consumed iterator.
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if iter_consumer_names.contains(callee) => *dst,
                // A set-algebra result (`@union`/`@difference`/`@intersection`) is a
                // fresh owned Set — distinct allocation from both operands.
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if set_algebra_names.contains(callee) => *dst,
                // A fresh-str-producing method result (`@debug()` / `@to_str()`) is a
                // fresh owned `str` — distinct allocation from the receiver. The name
                // is a builtin / derived-trait method (so the `user_call_fresh_result`
                // arm below excludes it via `is_builtin`), but the result genuinely
                // allocates, so it gets its own name-set arm. Spec: Annex E §AIMS RL-2.
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if fresh_str_names.contains(callee) => *dst,
                // A user-function call returning a fresh owned collection (a builder
                // like `@to_array` / `@build`), a fresh owned `str` (a builder like
                // `@make_label`), OR a fresh owned RECURSIVE/inline AGGREGATE (a builder
                // like
                // `@build_list` returning a boxed `Cons` chain / a struct-with-heap-
                // field), gated to exclude Direct-transfer forwarders. The aggregate
                // result is `ValueRepr::Aggregate` (RcStrategy `InlineEnum`/`AggFields`);
                // its lineage's RC ops act on the boxed node (recursive enum) or the
                // heap field (inline struct); a str result is a `FatValue` whose RC op
                // acts on the fat-pointer buffer. The same fresh-owned single-release
                // accounting (alloc +1 surplus over the dup-use keep-alive net) applies
                // to all three. A `str + str` concat is a `PrimOp Binary(Add)`, NOT an
                // `Apply` to a non-builtin name, so it is structurally excluded here.
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } if (is_collection_or_str_dst(*dst, func, pool)
                    || is_self_allocating_aggregate(*dst, func, pool))
                    && user_call_fresh_result(*callee, args, *dst) =>
                {
                    *dst
                }
                _ => continue,
            };
            // A non-empty `Construct` of a SELF-ALLOCATING (boxed recursive) aggregate
            // (a `Cons`/`Branch`/`Link` node) is a fresh owned heap allocation
            // alongside collections + str literals. Its inner nodes are owned args of
            // the parent Construct -> `compute_owned_consumed_lineages` excludes them
            // in the caller, so only the outermost head lineage fires. Inline
            // non-recursive aggregates are excluded (no self-buffer; field-walk /
            // Phase-6.85 domain). Spec: Annex E §AIMS RL-2.
            if is_collection_or_str_dst(dst, func, pool)
                || is_self_allocating_aggregate(dst, func, pool)
            {
                reps.insert(rep_of(dst));
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            // A fresh user-call result freeable lineage includes a `str` (a derived
            // `@debug()` lowers to `%dst = Invoke @debug(%recv) normal .. unwind ..`);
            // the cow / conversion / iter-consumer / set-algebra branches return
            // collections only, so widening to `is_collection_or_str_dst` admits the
            // str user-call result without affecting them. Spec: Annex E §AIMS RL-2.
            let dst_is_freeable = is_collection_or_str_dst(*dst, func, pool)
                || is_self_allocating_aggregate(*dst, func, pool);
            let is_fresh = (cow_names.contains(callee) && cow_receiver_is_fresh_local(args))
                || conversion_names.contains(callee)
                || iter_consumer_names.contains(callee)
                || set_algebra_names.contains(callee)
                || fresh_str_names.contains(callee)
                || (dst_is_freeable && user_call_fresh_result(*callee, args, *dst));
            if is_fresh && dst_is_freeable {
                reps.insert(rep_of(*dst));
            }
        }
    }
    reps
}

/// Lineage reps (under `jt_reps`) of every FRESH local collection allocation —
/// fresh-local-EQUIVALENT: a non-empty `Construct` DEFINED in this function (a
/// local allocation, NOT a function param), PLUS the transitive closure of
/// COW-mutator results whose receiver is itself fresh-local-equivalent. A
/// COW-mutator (`push` / `reverse` / `concat` / `sort` / `set` / `insert` /
/// `remove`) consumes its receiver owned and produces a result that IS the same
/// allocation (in-place reuse / COW realloc); when the chain ROOT is a fresh local
/// Construct, every link is a single function-owned allocation. So the receiver of
/// a chained mutation (`[1].push(2).push(3)` — the second push's receiver is the
/// first push RESULT, not a `Construct`) is fresh-local-equivalent, making the
/// chain TAIL freeable at its borrowed-read scope-exit sink. A borrowed-param
/// receiver is the A' COW-through-borrow blocker's domain (excluded — never seeds
/// the closure). Spec: Annex E §AIMS RL-2.
fn compute_fresh_local_construct_reps(
    func: &ArcFunction,
    pool: &Pool,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    // Function-param reps — a COW receiver tracing here is NOT a fresh local.
    let param_reps: FxHashSet<ArcVarId> = func.params.iter().map(|p| rep_of(p.var)).collect();
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    // Seed: non-empty fresh local `Construct` results.
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, args, .. } = instr {
                if !args.is_empty() && is_collection_dst(*dst, func, pool) {
                    let r = rep_of(*dst);
                    if !param_reps.contains(&r) {
                        reps.insert(r);
                    }
                }
            }
        }
    }
    // Transitive closure over COW-mutator chains: a COW-mutator result whose
    // receiver (arg 0) is already fresh-local-equivalent is itself
    // fresh-local-equivalent (same allocation, transformed in place). Iterate to a
    // fixpoint so an N-link chain (`xs.push().push().reverse()`) is fully covered.
    let cow_names = crate::borrow::all_cow_method_names(interner);
    let receiver_in = |args: &[ArcVarId], set: &FxHashSet<ArcVarId>| -> bool {
        args.first()
            .is_some_and(|&recv| set.contains(&rep_of(recv)))
    };
    loop {
        let mut grew = false;
        for block in &func.blocks {
            for instr in &block.body {
                let cow_chain_dst = match instr {
                    // A COW-mutator method (`push`/`reverse`/`sort`/…) whose receiver
                    // is fresh-local-equivalent.
                    ArcInstr::Apply {
                        dst,
                        func: callee,
                        args,
                        ..
                    } if cow_names.contains(callee) && receiver_in(args, &reps) => Some(*dst),
                    // A list-concat `PrimOp Binary(Add)` (`xs + ys`) consumes BOTH
                    // RcPointer operands and reuses one's allocation; its result is
                    // fresh-local-equivalent when EVERY RcPointer operand is. The
                    // `list_concat_consumed_operands` SSOT filters to RcPointer list
                    // operands (str/closure operands are FatValue, borrowed by
                    // `ori_str_concat` — not a consuming list concat).
                    ArcInstr::Let { dst, .. } => {
                        let operands =
                            crate::lower::burden_lower::list_concat_consumed_operands(instr, func);
                        if !operands.is_empty()
                            && operands.iter().all(|&o| reps.contains(&rep_of(o)))
                        {
                            Some(*dst)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(dst) = cow_chain_dst {
                    if is_collection_dst(dst, func, pool) {
                        let r = rep_of(dst);
                        if !param_reps.contains(&r) && reps.insert(r) {
                            grew = true;
                        }
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
                if cow_names.contains(callee)
                    && is_collection_dst(*dst, func, pool)
                    && receiver_in(args, &reps)
                {
                    let r = rep_of(*dst);
                    if !param_reps.contains(&r) && reps.insert(r) {
                        grew = true;
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    reps
}

/// Whether `dst` holds an owned List/Map/Set (an `RcPtr`/`FatVal` whose resolved
/// tag is `List`/`Map`/`Set`). Shared by the fresh-collection detector and the
/// alloc-aware delta. `Str` excluded — a COW-mutator RESULT is never a `Str`.
fn is_collection_dst(dst: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    if !matches!(
        func.var_repr(dst),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) {
        return false;
    }
    matches!(
        pool.tag(pool.resolve_fully(func.var_type(dst))),
        ori_types::Tag::List | ori_types::Tag::Map | ori_types::Tag::Set
    )
}

/// Whether `dst` holds an owned `str` (a `FatValue`/`RcPointer` whose resolved tag
/// is `Str`). A `str` is immutable — passed at a Borrowed position it has no
/// COW-through-borrow hazard, so a dup-read-then-dead borrowed-str arg stays
/// caller-owned and needs the RL-2 scope-exit release (the borrowed-str carve-out
/// in [`compute_user_call_arg_lineages`]). Spec: Annex E §AIMS RL-2.
fn is_str_dst(dst: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    if !matches!(
        func.var_repr(dst),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) {
        return false;
    }
    matches!(
        pool.tag(pool.resolve_fully(func.var_type(dst))),
        ori_types::Tag::Str
    )
}

/// Whether `dst` holds an owned List/Map/Set/Str (the collection set PLUS `Str`).
/// Used for the fresh-candidate detection where a heap `str` literal is a freeable
/// borrowed-then-dead allocation alongside collections.
fn is_collection_or_str_dst(dst: ArcVarId, func: &ArcFunction, pool: &Pool) -> bool {
    if !matches!(
        func.var_repr(dst),
        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
    ) {
        return false;
    }
    matches!(
        pool.tag(pool.resolve_fully(func.var_type(dst))),
        ori_types::Tag::List | ori_types::Tag::Map | ori_types::Tag::Set | ori_types::Tag::Str
    )
}

/// Lineage reps (under `jt_reps`) of every `str`/collection OWNED-MOVED into a
/// collection — passed as an arg to a `Construct` (collection literal element). A
/// `Construct` arg is MOVED (the runtime does NOT inc it — the collection's
/// reference IS the value's sole reference, rc stays 1): its `elem_dec_fn` is the
/// SOLE release on buffer teardown, so a scope-exit dec on the value's local
/// lineage double-frees with the collection's element drop → exclude it.
///
/// A COW-mutator borrowed element arg (`insert(m, key, val)` key/value, `push(xs,
/// v)` element) is COPIED, NOT moved: the runtime `copy_nonoverlapping`s the
/// element bytes into the buffer AND calls `key_inc`/`val_inc`/`elem_inc` so the
/// buffer's COPY has its OWN reference (rc → 2). The original LOCAL reference
/// SURVIVES the borrowed call and is dead afterward (RL-2 `ApplyToBorrowedParam`
/// mandates a dec on the local). So a copied element is NOT in this owned-move set
/// — its local IS freed by the dead-owned-collection-or-str scope-exit pass. The
/// receiver (arg 0) is the consumed collection, handled by `owned_consumed`.
/// Spec: Annex E §AIMS RL-2.
fn compute_collection_owned_moved_str_lineages(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            // Construct args are OWNED-MOVED elements (the collection owns the sole
            // reference; the runtime does not inc them).
            if let ArcInstr::Construct { args, .. } = instr {
                for &arg in args {
                    reps.insert(rep_of(arg));
                }
            }
        }
    }
    reps
}

/// Lineage reps (under `jt_reps`) of every value flowing to a `Return { value }`
/// terminator (a lineage member is the returned value). A returned collection is
/// an RL-2 TRANSFER (the caller inherits the release obligation), so a scope-exit
/// dead-collection dec MUST NOT fire on it — that would double-free with the
/// caller's release.
fn compute_returned_lineages(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            reps.insert(rep_of(*value));
        }
    }
    reps
}

/// Per-block alloc-aware delta for a fresh owned-collection lineage `rep`:
/// `Σ alloc(+1) for a FRESH member defined in the block`
/// `+ Σ BurdenInc(member) − Σ whole-var BurdenDec*(member)`. Mirrors the delta
/// [`compute_dead_collection_source_releases`] builds. The allocation `+1` is
/// attributed ONLY to a genuine fresh allocation (a non-empty `Construct` or a
/// COW-mutator `Apply`/`Invoke` result) — a sharing-method / user-fn collection
/// result allocates no buffer this lineage owns, so it contributes no `+1` (it is
/// already excluded from the candidate set, but the delta is kept consistent).
fn compute_owned_collection_delta(
    func: &ArcFunction,
    pool: &Pool,
    rep: ArcVarId,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> Vec<i64> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let sa_rep = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let cow_names = crate::borrow::all_cow_method_names(interner);
    let conversion_names = collection_conversion_names(interner);
    let iter_consumer_names = iterator_consumer_collection_names(interner);
    let set_algebra_names = collection_set_algebra_names(interner);
    let fresh_str_names = fresh_str_producing_method_names(interner);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    // A COW-mutator, a collection-conversion result, an iterator-consumer result
    // (`@collect`/`@collect_set`), a set-algebra result
    // (`@union`/`@difference`/`@intersection`), OR a fresh-str-producing method
    // result (`@debug()`/`@to_str()`) is a fresh allocation that contributes its
    // `+1` to the lineage's alloc-aware net. A USER-FUNCTION call
    // returning a collection ALSO allocates a fresh result `+1`, but ONLY when it
    // is NOT a `transfers_through_return ∧ Direct` forwarder (which returns an
    // existing allocation, NOT a new one — counting its `+1` would double-count the
    // source's alloc and over-free the shared buffer). The Direct-transfer
    // discriminator is the `same_alloc_reps` merge: a forwarder result shares an
    // arg's rep. Spec: Annex E §AIMS RL-2.
    let user_call_allocates = |callee: &Name, args: &[ArcVarId], dst: ArcVarId| -> bool {
        if builtins.is_builtin(*callee) {
            return false;
        }
        !args.iter().any(|&arg| sa_rep(arg) == sa_rep(dst))
    };
    let allocates = |callee: &Name| {
        cow_names.contains(callee)
            || conversion_names.contains(callee)
            || iter_consumer_names.contains(callee)
            || set_algebra_names.contains(callee)
            || fresh_str_names.contains(callee)
    };
    let mut delta: Vec<i64> = vec![0; func.blocks.len()];
    for (b, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            // A heap `str` literal allocation contributes its `+1` (mirrors the
            // collection alloc) when the lineage is the candidate str.
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Literal(crate::ir::LitValue::String(_)),
                ..
            } = instr
            {
                if is_collection_or_str_dst(*dst, func, pool) && rep_of(*dst) == rep {
                    delta[b] += 1;
                }
            }
            let alloc_dst = match instr {
                ArcInstr::Construct { dst, args, .. } if !args.is_empty() => Some(*dst),
                ArcInstr::Apply {
                    dst, func: callee, ..
                } if allocates(callee) => Some(*dst),
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    ..
                } if (is_collection_or_str_dst(*dst, func, pool)
                    || is_self_allocating_aggregate(*dst, func, pool))
                    && user_call_allocates(callee, args, *dst) =>
                {
                    Some(*dst)
                }
                _ => None,
            };
            if let Some(d) = alloc_dst {
                // A fresh collection buffer, a fresh user-call `str` result, OR a
                // fresh SELF-ALLOCATING (boxed recursive) aggregate node contributes
                // its implicit `+1` to the lineage's alloc-aware net. The recursive
                // aggregate's lineage RC ops act on the boxed node; a str result's act
                // on the fat-pointer buffer; so the same single-release net
                // (`alloc(+1) + ΣBurdenInc − ΣBurdenDec`) applies. (A str LITERAL's
                // `+1` is counted by the dedicated `Let { String }` arm above; `d`
                // reaches a str only via the user-call-result arm.)
                if (is_collection_or_str_dst(d, func, pool)
                    || is_self_allocating_aggregate(d, func, pool))
                    && rep_of(d) == rep
                {
                    delta[b] += 1;
                }
            }
            if matches!(instr, ArcInstr::BurdenInc { var } if rep_of(*var) == rep) {
                delta[b] += 1;
            } else if crate::aims::verify::burden_delta::whole_var_dec_target(instr).map(rep_of)
                == Some(rep)
            {
                delta[b] -= 1;
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            ..
        } = &block.terminator
        {
            // Mirrors the recognizer's Invoke-terminator freeable test: a fresh
            // user-call `str` result (a derived `@debug()` Invoke) is freeable
            // alongside collections + aggregates. Spec: Annex E §AIMS RL-2.
            let dst_is_freeable = is_collection_or_str_dst(*dst, func, pool)
                || is_self_allocating_aggregate(*dst, func, pool);
            let allocates_here =
                allocates(callee) || (dst_is_freeable && user_call_allocates(callee, args, *dst));
            if allocates_here && dst_is_freeable && rep_of(*dst) == rep {
                delta[b] += 1;
            }
        }
    }
    delta
}

/// The jump-threaded reps (`jt_reps`, apply-Direct-seeded) of every FORWARDER-
/// RESULT lineage: a lineage where a `transfers_through_return ∧ Direct` forwarder
/// `%dst = Apply/Invoke @id(%arg [own])` merged its result `%dst` with its owned
/// arg `%arg` (apply-Direct edge, recorded in `same_alloc_reps`). The result IS the
/// same allocation as the arg, flowing forward through the forwarder; these lineages
/// get the additional agreed-terminal-net gate in [`compute_dead_owned_collection_releases`]
/// (the per-block sink alone over-fires on a multi-borrow-then-return forwarder).
fn compute_forwarder_result_reps(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let sa_rep = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    let record = |args: &[ArcVarId], dst: ArcVarId, reps: &mut FxHashSet<ArcVarId>| {
        // An apply-Direct forwarder merges the result with exactly one owned arg.
        if args.iter().any(|&arg| sa_rep(arg) == sa_rep(dst)) {
            reps.insert(rep_of(dst));
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply { dst, args, .. } | ArcInstr::ApplyIndirect { dst, args, .. } => {
                    record(args, *dst, &mut reps);
                }
                _ => {}
            }
        }
        match &block.terminator {
            ArcTerminator::Invoke { dst, args, .. }
            | ArcTerminator::InvokeIndirect { dst, args, .. } => {
                record(args, *dst, &mut reps);
            }
            _ => {}
        }
    }
    reps
}

/// The lineage's SSA value live at `block_idx` if this block makes a BORROWED use
/// of a `rep` member (an operand at a non-owned position), else `None`. The
/// returned var is the actual SSA value referenced here (the in-scope value to
/// dec at the block's last-use sink). A block that references the lineage ONLY at
/// an owned position returns `None` (that is a transfer, not a borrowed read).
fn lineage_last_use_in_block(
    func: &ArcFunction,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    rep: ArcVarId,
    block_idx: usize,
) -> Option<ArcVarId> {
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    let block = func.blocks.get(block_idx)?;
    let mut found: Option<ArcVarId> = None;
    for instr in &block.body {
        for (pos, &arg) in instr.used_vars().iter().enumerate() {
            if rep_of(arg) == rep && !instr.is_owned_position(pos) {
                found = Some(arg);
            }
        }
    }
    for (pos, &arg) in block.terminator.used_vars().iter().enumerate() {
        if rep_of(arg) == rep && !block.terminator.is_owned_position(pos) {
            found = Some(arg);
        }
    }
    found
}

/// Whether `block_idx` consumes a `rep` member at an OWNED position (a body instr
/// or the terminator) — an ownership transfer that already releases the value, so
/// a dead-owned-collection dec here would double-free.
fn block_owned_consumes_lineage(
    func: &ArcFunction,
    rep: ArcVarId,
    jt_reps: &FxHashMap<ArcVarId, ArcVarId>,
    block_idx: usize,
) -> bool {
    let Some(block) = func.blocks.get(block_idx) else {
        return false;
    };
    for instr in &block.body {
        if owned_position_consumes_lineage(instr, rep, jt_reps) {
            return true;
        }
    }
    let rep_of = |v: ArcVarId| jt_reps.get(&v).copied().unwrap_or(v);
    for (pos, &arg) in block.terminator.used_vars().iter().enumerate() {
        if block.terminator.is_owned_position(pos) && rep_of(arg) == rep {
            return true;
        }
    }
    false
}

/// Compute the set of FRESH self-allocation `BurdenInc` def-sites whose paired
/// fresh inc is REDUNDANT under Phase-7 lowering (the allocation already
/// supplies the lineage's `+1`, so lowering the fresh inc would over-count by
/// one and leak the value).
///
/// A FRESH self-allocation (`Construct` / `Reuse` / `CollectionReuse` /
/// `PartialApply` / `Let { String }`) is created at runtime RC = 1: the alloc
/// IS the `+1`. The Phase-5 burden walk emits a paired `BurdenInc` at that
/// def-site for per-value ledger symmetry on the predicate-stack-ON path (where
/// burden ops are codegen no-ops). Under sole-emitter lowering that fresh inc
/// becomes a real `RcInc`, double-counting the allocation's implicit `+1`.
///
/// The fresh inc is ELIDABLE iff removing it preserves the lineage's alloc-aware
/// net = 0 — i.e. the value is genuinely single-reference / read-only and the
/// alloc's `+1` alone balances every release. It is LOAD-BEARING (kept) iff the
/// lineage flows into a COW-mutation operand: a value-mutation site
/// (`push`/`set`/`insert`/`remove`/`sort`/`reverse` at an owned `Apply`/`Invoke`
/// arg, or a collection `+`/concat `PrimOp Binary` with an `RcPtr` operand) reads
/// the operand's runtime refcount to choose copy-vs-mutate-in-place. Dropping
/// the fresh inc there leaves the value at RC = 1 at the mutation point → the
/// COW helper mutates the SHARED value in place, corrupting an aliased holder.
/// Keeping the inc raises the runtime RC ≥ 2 so the COW helper COPIES (RL-1: a
/// COW-mutation operand of a value re-read afterward is a DUPLICATING use, so
/// the inc is not elidable per `AimsProof.Realization::RL1_emit_iff_not_elidable`).
///
/// Discriminator = per-`(var, same-alloc-lineage)` COW-operand flow, the DIRECT
/// measure of fresh-inc load-bearingness — NOT use-count / type / def-block
/// uniqueness. Spec: Annex E §AIMS RL-1 (`!incElidable`) + RL-comp
/// net-preservation.
fn compute_elidable_fresh_self_alloc_incs(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    // Conversion-source lineage reps (`m.keys()`/`s.split()`/`@to_list` SOURCES,
    // computed by the caller where `pool` is available) — the Cure B (Decision 10)
    // phi-threading eligibility extension: these loop-carried conversion sources
    // get the phi-threaded per-path alloc-aware net so their surplus fresh-site
    // inc is elided (was mis-attributed across the Jump-arg→block-param rename and
    // left unbalanced → leak). Empty = pre-change behaviour.
    conversion_source_reps: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    // Lineage reps whose value flows into a COW-mutation operand — the fresh inc
    // for any FRESH self-alloc in such a lineage is LOAD-BEARING (keep it): the
    // COW helper reads the operand's runtime refcount, and the fresh inc raises
    // it to ≥ 2 so the helper COPIES instead of mutating the shared value.
    let cow_mutated_reps =
        compute_cow_mutated_lineage_reps(func, same_alloc_reps, interner, contracts);
    let list_take_name = for_yield_result_finalizer_name(interner);
    // Per-lineage alloc-aware static net = `Σ self-alloc(+1) + Σ BurdenInc −
    // Σ BurdenDec*` over the SSA-alias lineage (M3). Counting the allocation's
    // implicit `+1` per the compiled-Lean `rcBalance` (a released FRESH value's
    // full lifecycle counting alloc nets 0). A redundant fresh-site inc shows up
    // as net == 1 (the inc is the surplus over balance); eliding it brings the
    // lineage back to 0. A net != 1 means the fresh inc is balancing a
    // COW-consume / move-alias dec (e.g. `length_one`: net 0 with all incs →
    // eliding drops to −1, a double-free) → keep.
    let lineage_net =
        compute_lineage_alloc_aware_net(func, same_alloc_reps, interner, conversion_source_reps);

    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);

    // DUP-FUNDED lineages (RL-1 owned-call-arg duplication cure): a lineage
    // carrying >= 1 kept genuine-duplication alias inc (owned-call-arg family
    // per `compute_funded_call_arg_dup_aliases` — the FUNDED set, so a raw
    // member whose alias-site inc Phase 5 never kept is excluded from the
    // debit/clearance machinery). For such a lineage:
    //  - the per-path net DEBITS every owned hand-off of a lineage member
    //    (-1 each: owned call args, aggregate-store args, `Set.value`,
    //    `Return`) so the N kept duplication incs read as FUNDED transfers,
    //    never surplus — adjusted net == 1 identifies the fresh-site inc as
    //    the one redundant source over the hand-off ledger;
    //  - the COW gate is refined per-site: every RC-READING consume of a
    //    lineage member is either FUNDED by a kept duplication inc (the
    //    consumed member is a kept dup alias — its own inc raises rc >= 2 at
    //    the site, so the COW helper copies) or TERMINAL (no lineage use
    //    forward-reachable after the site — in-place mutation of the last
    //    reference is sound), in which case the lineage-wide cow flag does
    //    not bar the elision.
    // Zero-duplication lineages take neither branch — byte-identical nets and
    // verdicts. Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`) + RL-2.
    let call_arg_dups =
        crate::lower::burden_lower::compute_funded_call_arg_dup_aliases(func, contracts, interner);
    // STORE-FAMILY funded lineages (RL-1 store-family duplication cure): the
    // fresh-local aggregate-STORE twin of the call-arg family. TWO layers:
    //  - the funded ALIAS set (`compute_funded_store_dup_aliases` — kept
    //    store-site dup incs) joins the cow-clearance funding closure: a
    //    consumed member backed by its own kept inc holds rc >= 2 at the
    //    consume site, so the COW helper copies;
    //  - the store-LINEAGE rep admission (`compute_store_handoff_reps` — any
    //    fresh-rooted lineage with >= 1 aggregate-store hand-off of an RC
    //    member) takes the hand-off-DEBITED net, pricing each store consume
    //    -1 (the container drop is the downstream release). The kept dup incs
    //    then read as FUNDED transfers; the surplus over that ledger is the
    //    fresh-site keep-alive (net == 1) or the fresh-site keep-alive PLUS
    //    the execution-final read alias's keep-alive pair inc (net == 2, the
    //    multi-store + post-store-read shape — the carrier designation below).
    // Spec: Annex E §AIMS RL-1 (`RL1_duplication_balanced`) + RL-2.
    let (store_dups, store_lineage_reps) = store_family_inputs(func, contracts, same_alloc_reps);
    let mut dup_funded_reps: FxHashSet<ArcVarId> =
        call_arg_dups.iter().map(|&v| rep_of(v)).collect();
    dup_funded_reps.extend(store_lineage_reps.iter().copied());
    // FORWARD-Jump-exported lineages take the debited net too: a forward Jump
    // arg exports the reference into a block-param lineage (`same_alloc_reps`
    // excludes the phi edge by design — the threaded continuation owns the
    // release), so the UNDEBITED net reads the export's funding inc as a +1
    // surplus and elides it, leaving the paired dec to free the buffer BEFORE
    // the Jump (the loop-entry `let s = [0]; for .. { s = .. }` double-free).
    // The debited net prices the export -1 (`compute_lineage_handoff_debits`
    // forward-Jump arm), restoring the balanced verdict. Per-rep admission
    // only; no carrier designation. Spec: Annex E §AIMS RL-2 + RL-4.
    if !crate::lower::burden_lower::store_family_funding_disabled() {
        dup_funded_reps.extend(compute_forward_jump_export_reps(func, same_alloc_reps));
    }
    let funded_dup_aliases: FxHashSet<ArcVarId> = call_arg_dups
        .iter()
        .chain(store_dups.iter())
        .copied()
        .collect();
    let (debited_net, cow_cleared_reps) = if dup_funded_reps.is_empty() {
        (FxHashMap::default(), FxHashSet::default())
    } else {
        (
            compute_dup_funded_debited_net(
                func,
                same_alloc_reps,
                interner,
                contracts,
                &dup_funded_reps,
                list_take_name,
            ),
            compute_dup_funded_cow_cleared_reps(
                func,
                same_alloc_reps,
                interner,
                contracts,
                &dup_funded_reps,
                &funded_dup_aliases,
            ),
        )
    };
    // RL-2 execution-final read-alias release carriers for the store-family
    // lineages (per-rep; only consulted at debited net == 2).
    let store_carriers =
        compute_store_family_final_read_carriers(func, same_alloc_reps, &store_lineage_reps);

    let mut elidable: FxHashSet<ArcVarId> = FxHashSet::default();
    // Elision verdict for one fresh-self-alloc result `dst`: elide its surplus
    // fresh-site inc ONLY when the lineage net == 1 AND the lineage never flows
    // into a COW-mutation operand. Removing exactly one fresh inc restores the
    // alloc-aware balance to 0; any other net means the fresh inc is load-bearing
    // (move-alias dec, unbalanced dup) — keep it (eliding would net −1 =
    // double-free). Shared by the block-body and `Invoke`-terminator fresh-alloc
    // scans below — one verdict home for both call forms. Dup-funded lineages
    // consume the hand-off-debited net + the per-site refined cow clearance.
    // Spec: Annex E §AIMS RL-1.
    let decide = |dst: ArcVarId, elidable: &mut FxHashSet<ArcVarId>| {
        let rep = rep_of(dst);
        let dup_funded = dup_funded_reps.contains(&rep);
        let cow =
            cow_mutated_reps.contains(&rep) && !(dup_funded && cow_cleared_reps.contains(&rep));
        let net = if dup_funded {
            debited_net.get(&rep).copied().unwrap_or(0)
        } else {
            lineage_net.get(&rep).copied().unwrap_or(0)
        };
        let verdict = !cow && net == 1;
        // Store-family net == 2: the fresh-site keep-alive AND the
        // execution-final read alias's keep-alive pair inc are the two surplus
        // supplies — elide the fresh inc AND designate the final read alias as
        // the lineage's RL-2 release carrier (its inc is suppressed; its
        // last-use dec, placed after the final read, becomes the base's single
        // release). Only a UNIQUE execution-final carrier qualifies
        // (`compute_store_family_final_read_carriers` declines loop re-reach /
        // per-arm finals). Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
        let carrier = (!cow && net == 2 && store_lineage_reps.contains(&rep))
            .then(|| store_carriers.get(&rep).copied())
            .flatten();
        if tracing::enabled!(target: "ori_arc::aims::realize", tracing::Level::TRACE) {
            tracing::trace!(
                target: "ori_arc::aims::realize",
                fn_name = interner.lookup(func.name),
                dst = dst.raw(),
                rep = rep.raw(),
                lineage_net = net,
                cow_mutated = cow,
                dup_funded,
                release_carrier = carrier.map(ArcVarId::raw),
                elidable = verdict || carrier.is_some(),
                "fresh-self-alloc inc-elision decision"
            );
        }
        if verdict {
            elidable.insert(dst);
        } else if let Some(c) = carrier {
            elidable.insert(dst);
            elidable.insert(c);
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            // A FRESH self-alloc is either a `Construct`/literal/`ori_list_take`
            // (`fresh_self_alloc_dst`) or a self-allocating builtin collection-source
            // `Apply` result (`fresh_collection_source_apply_dst`) — both create a
            // fresh rc=1 buffer whose Phase-5 fresh-site inc the M1 alloc-aware-net
            // elision can drop when the lineage net is +1 (Spec: Annex E §AIMS RL-1).
            if let Some(dst) = fresh_rc_alloc_dst(instr, func, interner, list_take_name) {
                decide(dst, &mut elidable);
            }
        }
        // An `Invoke`-terminator self-allocating builtin result (`s.insert(..)`
        // COW-result) is the same fresh-rc=1 shape via the may-unwind terminator;
        // its surplus fresh-site inc is elidable on the identical net == 1 ∧ !cow
        // verdict. Spec: Annex E §AIMS RL-1.
        if let Some((dst, _)) = fresh_rc_alloc_dst_terminator(&block.terminator, func, interner) {
            decide(dst, &mut elidable);
        }
    }
    elidable
}

/// Per-block burden delta + alloc-site bitmap for ONE lineage (selected by the
/// `belongs` predicate over `same_alloc_reps`/phi-threaded membership). Returns
/// `(delta, alloc_in_block)` indexed by block:
/// - `delta[b]` = `Σ alloc(+1) + Σ BurdenInc(+1) − Σ whole-var BurdenDec*(−1)`
///   for lineage members in block `b`.
/// - `alloc_in_block[b]` = whether the lineage's allocation occurs in block `b`.
///   Tracked SEPARATELY — a block's NET delta can be 0 even when it allocates
///   (alloc +1 offset by paired decs), so `delta[b] > 0` is NOT a sound alloc
///   predicate.
///
/// Body-defined fresh self-allocs (`Construct`/literal/`Apply`-builtin) attribute
/// the alloc to their own block; an `Invoke`-TERMINATOR self-allocating builtin
/// result attributes the alloc to its `normal` successor block — where the result
/// FIRST lives and where Phase-5 prepends its fresh-site `BurdenInc` (so the
/// per-path net counts the allocation on the path the result is defined). Spec:
/// Annex E §AIMS RL-2.
fn compute_lineage_block_deltas(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    list_take_name: ori_ir::Name,
    belongs: &impl Fn(ArcVarId) -> bool,
) -> (Vec<i64>, Vec<bool>) {
    let n = func.blocks.len();
    let mut delta: Vec<i64> = vec![0; n];
    let mut alloc_in_block: Vec<bool> = vec![false; n];
    for (b, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            if let Some(dst) = fresh_rc_alloc_dst(instr, func, interner, list_take_name) {
                if belongs(dst) {
                    delta[b] += 1;
                    alloc_in_block[b] = true;
                }
            }
            if matches!(instr, ArcInstr::BurdenInc { var } if belongs(*var)) {
                delta[b] += 1;
            } else if crate::aims::verify::burden_delta::whole_var_dec_target(instr)
                .is_some_and(belongs)
            {
                delta[b] -= 1;
            }
        }
        if let Some((dst, normal)) =
            fresh_rc_alloc_dst_terminator(&block.terminator, func, interner)
        {
            if belongs(dst) {
                let nb = normal.index();
                if nb < n {
                    delta[nb] += 1;
                    alloc_in_block[nb] = true;
                }
            }
        }
    }
    (delta, alloc_in_block)
}

/// The store-family funding inputs for the elision verdict: the funded
/// store-dup alias set + the store-lineage rep admission set; both empty under
/// `ORI_DISABLE_STORE_FAMILY_FUNDING=1`.
fn store_family_inputs(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> (FxHashSet<ArcVarId>, FxHashSet<ArcVarId>) {
    if crate::lower::burden_lower::store_family_funding_disabled() {
        (FxHashSet::default(), FxHashSet::default())
    } else {
        (
            crate::lower::burden_lower::compute_funded_store_dup_aliases(func, contracts),
            compute_store_handoff_reps(func, same_alloc_reps),
        )
    }
}

/// Per-SSA-alias-lineage alloc-aware PER-PATH terminal net (M3 — the shared
/// Phase-7 accounting foundation). For each same-alloc rep, runs the per-path
/// forward burden dataflow (`compute_burden_entry_nets`) over per-block deltas
/// `Σ fresh-self-alloc(+1) + Σ BurdenInc − Σ whole-var BurdenDec*` attributed to
/// the lineage rep. The fresh-self-alloc `+1` models the compiled-Lean
/// `rcBalance` allocation term (`AimsProof.Realization::rcBalance` — a value
/// allocated at RC = 1, counting the allocation, nets 0 when released exactly
/// once on the path).
///
/// Per-path is load-bearing: a flat op-count summed across the CFG
/// double-counts mutually-exclusive paths (e.g. an `Invoke` normal release vs
/// its unwind-edge dec), masking the true per-path balance. The dataflow seeds
/// `entry_net[entry]=0`, propagates predecessor exits, and records merge
/// disagreement. The returned net for a rep is the agreed terminal net on the
/// reachable terminal blocks; `None` for a rep with merge disagreement
/// (conservatively NOT elidable).
///
/// A correctly-balanced released lineage nets 0 per path; a lineage carrying a
/// redundant fresh-site inc nets 1 per path (the surplus inc). `BurdenDecField`
/// is excluded (field-grain, per `burden_delta::whole_var_dec_target`).
pub(super) fn compute_lineage_alloc_aware_net(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    // Cure B (Decision 10): conversion-source lineage reps eligible for
    // phi-threaded attribution (alongside `ori_list_take` for_yield results).
    conversion_source_reps: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, i64> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let preds = crate::graph::compute_predecessors(func);
    let list_take_name = for_yield_result_finalizer_name(interner);

    // PHI-AWARE attribution, scoped to the `for_yield` `ori_list_take` result.
    // `same_alloc_reps` EXCLUDES the Jump-arg → block-param POSITIONAL rename edge
    // BY DESIGN, so a for_yield result jump-threaded across a later loop's init
    // block (the `for_yield_*_two_call` shape) has its lone downstream release
    // attributed to a DIFFERENT rep — the unthreaded net then sees `alloc(+1) +
    // fresh-inc(+1) − premature-paired-dec(−1) = +1`, mis-eliding the fresh inc
    // and leaving `alloc(+1) − premature-dec − true-dec = −1` = a double-free.
    // The phi-threaded view unions the Jump-phi forward edges so the WHOLE
    // single-allocation chain nets together (→ 0, the fresh inc is load-bearing,
    // kept). Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
    //
    // SCOPE GUARD — `ori_list_take`-rooted lineages ONLY. A loop-INVARIANT fresh
    // value (a closure env `PartialApply` threaded unchanged through a loop's
    // block-params) is also a `fresh_self_alloc_dst`, but its loop-body keep-alive
    // inc/dec PAIRS are net-0 and live on SEPARATE unthreaded reps; folding them
    // into the alloc class via phi-threading would leave the alloc's `+1`
    // spuriously unbalanced (mis-eliding the keep-alive inc → a leak of the
    // hof_closure_capture_in_loop / coll_map_index_in_loop shapes). The
    // jump-threaded-result double-free is SPECIFIC to the `ori_list_take`
    // finalizer (allocated in a POST-loop block, threaded forward into its
    // consumers), so only those lineages take the threaded attribution; every
    // other fresh-alloc kind stays on the unthreaded net (behaviour-identical to
    // pre-change).
    let phi = compute_list_take_phi_attribution(func, same_alloc_reps, list_take_name);
    let phi_rep_of = |v: ArcVarId| phi.threaded.get(&v).copied().unwrap_or(v);
    // Cure B (Decision 10): a CONVERSION SOURCE (`m.keys()` / `s.split()` /
    // `@to_list` source borrowed by the conversion, dying at a post-loop
    // dead-block-param sink) ALSO takes the phi-threaded attribution. Its
    // fresh-site inc's per-path net is mis-attributed across the Jump-arg→
    // block-param SSA rename when unthreaded (`same_alloc_reps` excludes the phi
    // edge) → the surplus inc is left unbalanced → leak. Same single-source guard
    // as the `ori_list_take` path: keep only a conversion-source rep whose
    // phi-threaded class merges exactly ONE conversion source (a multi-source
    // phi-merge falls back to the unthreaded, double-free-safe net). DISJOINT from
    // the SCOPE-GUARD closure-env surface above: a conversion source is borrowed +
    // dies at a conversion sink, NOT a loop-invariant threaded-unchanged
    // `PartialApply`, so this does NOT re-enable the
    // hof_closure_capture_in_loop / coll_map_index_in_loop over-fire.
    let conv_eligible: FxHashSet<ArcVarId> = {
        let mut per_phi: FxHashMap<ArcVarId, FxHashSet<ArcVarId>> = FxHashMap::default();
        for &csr in conversion_source_reps {
            per_phi.entry(phi_rep_of(csr)).or_default().insert(csr);
        }
        per_phi
            .values()
            .filter(|members| members.len() == 1)
            .flatten()
            .copied()
            .collect()
    };
    // A `same_alloc_reps` rep takes the phi-threaded attribution iff it is an
    // `ori_list_take` result OR a conversion source whose phi-threaded class
    // merges exactly one such root (the single-alloc / single-source guard).
    let use_phi_for =
        |sar: ArcVarId| -> bool { phi.eligible.contains(&sar) || conv_eligible.contains(&sar) };

    // Collect every lineage rep that has at least one fresh self-alloc member —
    // the only reps an elision decision queries. Scans BOTH block-body
    // (`Construct`/literal/`Apply`-builtin) and terminator (`Invoke`-builtin)
    // fresh self-allocs so an `Invoke`-form self-allocating builtin result is a
    // tracked rep. Spec: Annex E §AIMS RL-1.
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let Some(dst) = fresh_rc_alloc_dst(instr, func, interner, list_take_name) {
                reps.insert(rep_of(dst));
            }
        }
        if let Some((dst, _)) = fresh_rc_alloc_dst_terminator(&block.terminator, func, interner) {
            reps.insert(rep_of(dst));
        }
    }

    let mut result: FxHashMap<ArcVarId, i64> = FxHashMap::default();
    for rep in reps {
        // Attribution predicate. For an `ori_list_take` result with a single-alloc
        // phi class, ATTRIBUTE via the phi-threaded rep (counting the jump-threaded
        // downstream release); every other lineage uses the unthreaded
        // `same_alloc_reps` rep (behaviour-identical to pre-change).
        let use_phi = use_phi_for(rep);
        let target_phi_rep = phi_rep_of(rep);
        let belongs = |var: ArcVarId| -> bool {
            if use_phi {
                phi_rep_of(var) == target_phi_rep
            } else {
                rep_of(var) == rep
            }
        };
        let (delta, alloc_in_block) =
            compute_lineage_block_deltas(func, interner, list_take_name, &belongs);
        if let Some(n) = agreed_alloc_reachable_terminal_net(func, &preds, &delta, &alloc_in_block)
        {
            result.insert(rep, n);
        }
    }
    result
}

/// Agreed per-path terminal net for ONE lineage's per-block `delta`, restricted
/// to terminal blocks forward-REACHABLE from the lineage's allocation blocks.
///
/// `None` on merge disagreement OR divergent terminal nets across paths
/// (conservatively NOT elidable). A terminal block NOT reachable from any alloc
/// block (an unwind `Resume` landing pad reached BEFORE the value was allocated
/// — e.g. a `for_yield` result born post-loop, unreachable from a mid-loop body's
/// unwind edge) is NOT a release path for the lineage: its net reflects "value
/// never existed here" (0), not a balance verdict — counting it as a divergent
/// terminal would spuriously reject a per-normal-path-balanced lineage. RL-2
/// release accounting applies only where the value is live. Shared by the
/// unthreaded/phi-threaded lineage net AND the dup-funded debited net — one
/// terminal-net verdict home.
fn agreed_alloc_reachable_terminal_net(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    delta: &[i64],
    alloc_in_block: &[bool],
) -> Option<i64> {
    agreed_terminal_net_over(func, preds, delta, alloc_in_block, false)
}

/// [`agreed_alloc_reachable_terminal_net`] core with a terminal-kind switch:
/// `return_only = true` restricts agreement to `Return` terminals — the
/// dup-funded debited net consumes that mode (an `Invoke`'s owned hand-off
/// debit sits in the call's own block, so a `Resume` landing pad entered
/// mid-lineage reads a partial ledger; the panic path's residue is the
/// pre-existing unwind accounting, not a verdict on the fresh inc).
fn agreed_terminal_net_over(
    func: &ArcFunction,
    preds: &[Vec<usize>],
    delta: &[i64],
    alloc_in_block: &[bool],
    return_only: bool,
) -> Option<i64> {
    use crate::aims::verify::burden_delta::compute_burden_entry_nets;
    let nets = compute_burden_entry_nets(func, preds, delta);
    // Convergence guard: this terminal-net verdict reads `entry_net`, which is
    // authoritative only on a converged result. On `!converged` the nets are
    // stale (freeze-on-disagree / cap exhaustion) and `disagree_blocks` may be
    // empty (the cap-exhausted-without-disagreement case), so a debited-net
    // release/elision decision derived from them would be non-deterministic.
    // Decline (the verifier reports the non-convergence as a HARD failure; a
    // missed elision is a leak surfaced there, never a UAF from a wrong
    // release). No-op on every converged function (Spec: Annex E §AIMS RL-2).
    if !nets.converged {
        return None;
    }
    if !nets.disagree_blocks.is_empty() {
        return None;
    }
    let alloc_blocks: Vec<usize> = (0..func.blocks.len())
        .filter(|&b| alloc_in_block[b])
        .collect();
    let alloc_reachable = forward_reachable_from(func, &alloc_blocks);
    let mut terminal_net: Option<i64> = None;
    for (b, block) in func.blocks.iter().enumerate() {
        let Some(eb) = nets.entry_net[b] else {
            continue;
        };
        if !alloc_reachable.contains(&b) {
            continue;
        }
        let is_terminal = if return_only {
            matches!(block.terminator, ArcTerminator::Return { .. })
        } else {
            matches!(
                block.terminator,
                ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
            )
        };
        if !is_terminal {
            continue;
        }
        let exit = eb + delta[b];
        match terminal_net {
            None => terminal_net = Some(exit),
            Some(t) if t != exit => return None,
            Some(_) => {}
        }
    }
    terminal_net
}

/// Per-block owned hand-off debit sites of ONE lineage (selected by `belongs`):
/// `-1` per lineage-member reference consumed at an ownership-transferring
/// position — owned call args (`Apply` / `ApplyIndirect` structural;
/// `Invoke` / `InvokeIndirect` terminator structural; PLUS contract-proven
/// `ParamContract.access == Owned` for named callees whose Phase-5 call-site
/// annotation is still the borrowed default), aggregate-store args
/// (`Construct` / `Reuse` / `CollectionReuse` owned positions), `Set.value`,
/// list-concat `PrimOp Binary(Add)` `RcPointer` operands, `Return.value`,
/// and FORWARD `Jump` args (a forward Jump hand-off exports the reference
/// into a block-param lineage — `same_alloc_reps` excludes the phi edge by
/// design, so the threaded continuation's accounting owns the release; the
/// for-loop source threading `Jump bbH(m, ..)` is the canonical shape).
/// BACK-EDGE Jump args (target dominates source) are NOT debited — the
/// per-iteration re-thread would make loop-block deltas non-convergent
/// (merge disagreement), and the back-edge arg is the loop's own param
/// rename, not an export.
/// `Invoke` hand-offs attribute to the call's own block (the callee consumes
/// the reference on both the normal and unwind paths).
fn compute_lineage_handoff_debits(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    belongs: &impl Fn(ArcVarId) -> bool,
) -> Vec<i64> {
    let dom = crate::graph::DominatorTree::build(func);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let contract_owned = |callee: Name, pos: usize| -> bool {
        crate::lower::burden_lower::contract_consuming_arg_position(
            contracts, &builtins, interner, callee, pos,
        )
    };
    let mut debits: Vec<i64> = vec![0; func.blocks.len()];
    for (b, block) in func.blocks.iter().enumerate() {
        for instr in &block.body {
            for (pos, &var) in instr.used_vars().iter().enumerate() {
                if !belongs(var) {
                    continue;
                }
                let owned = instr.is_owned_position(pos)
                    || matches!(
                        instr,
                        ArcInstr::Apply { func: callee, .. } if contract_owned(*callee, pos)
                    );
                if owned {
                    debits[b] -= 1;
                }
            }
            if let ArcInstr::Set { value, .. } = instr {
                if belongs(*value) {
                    debits[b] -= 1;
                }
            }
            if let ArcInstr::Let {
                value: ArcValue::PrimOp { op, args },
                ..
            } = instr
            {
                if matches!(op, PrimOp::Binary(ori_ir::BinaryOp::Add)) {
                    for &arg in args {
                        if belongs(arg) && matches!(func.var_repr(arg), Some(ValueRepr::RcPointer))
                        {
                            debits[b] -= 1;
                        }
                    }
                }
            }
        }
        match &block.terminator {
            ArcTerminator::Return { value } => {
                if belongs(*value) {
                    debits[b] -= 1;
                }
            }
            ArcTerminator::Jump { target, args } => {
                let src_id = crate::ir::ArcBlockId::new(u32::try_from(b).unwrap_or(u32::MAX));
                // Forward edge only (back-edge: target dominates source).
                if !dom.dominates(*target, src_id) {
                    for &arg in args {
                        if belongs(arg) {
                            debits[b] -= 1;
                        }
                    }
                }
            }
            ArcTerminator::Invoke {
                func: callee, args, ..
            } => {
                let term = &block.terminator;
                for (pos, &var) in term.used_vars().iter().enumerate() {
                    if belongs(var)
                        && (term.is_owned_position(pos)
                            || args.get(pos).is_some_and(|&a| a == var)
                                && contract_owned(*callee, pos))
                    {
                        debits[b] -= 1;
                    }
                }
            }
            ArcTerminator::InvokeIndirect { .. } => {
                let term = &block.terminator;
                for (pos, &var) in term.used_vars().iter().enumerate() {
                    if belongs(var) && term.is_owned_position(pos) {
                        debits[b] -= 1;
                    }
                }
            }
            _ => {}
        }
    }
    debits
}

/// Per-rep hand-off-DEBITED alloc-aware terminal net for the DUP-FUNDED
/// lineages (reps carrying >= 1 kept owned-call-arg duplication inc).
///
/// Net = `Σ alloc(+1) + Σ BurdenInc − Σ whole-var BurdenDec − Σ owned
/// hand-offs` per path. With every hand-off debited, a fully-funded lineage
/// (each duplication inc paired to its consumer's hand-off, the original
/// reference paired to its terminal hand-off / explicit release) nets 0; the
/// one redundant source over that ledger — the fresh-site inc — shows as
/// net == 1 and is elided by the caller's verdict. Restricted to the passed
/// reps; every other lineage keeps the undebited `compute_lineage_alloc_aware_net`
/// (zero-duplication lineages byte-identical). Spec: Annex E §AIMS RL-1 + RL-2.
fn compute_dup_funded_debited_net(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    dup_funded_reps: &FxHashSet<ArcVarId>,
    list_take_name: ori_ir::Name,
) -> FxHashMap<ArcVarId, i64> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let preds = crate::graph::compute_predecessors(func);
    let mut result: FxHashMap<ArcVarId, i64> = FxHashMap::default();
    for &rep in dup_funded_reps {
        let belongs = |var: ArcVarId| rep_of(var) == rep;
        let (mut delta, alloc_in_block) =
            compute_lineage_block_deltas(func, interner, list_take_name, &belongs);
        let debits = compute_lineage_handoff_debits(func, contracts, interner, &belongs);
        for (d, debit) in delta.iter_mut().zip(debits) {
            *d += debit;
        }
        if let Some(n) = agreed_terminal_net_over(func, &preds, &delta, &alloc_in_block, true) {
            result.insert(rep, n);
        }
    }
    result
}

/// DUP-FUNDED reps whose every RC-READING consume site is COW-SAFE without the
/// fresh-site keep-alive inc: the consumed lineage member is a kept
/// duplication alias (owned-call-arg OR store family — its OWN preceding inc
/// raises the runtime rc >= 2 at the site while the source's original
/// reference stays live — the COW helper copies), OR the site is the
/// lineage's TERMINAL consume (no lineage-member use forward-reachable after
/// it — in-place mutation of the last reference is sound). Aggregate-store
/// args (`Construct` / `Reuse` / `CollectionReuse` / `Set.value`) read no
/// refcount and never block. A rep in the returned set takes the fresh-inc
/// elision verdict despite its lineage-wide `cow_mutated_reps` flag.
/// `funded_dup_aliases` is the UNION of the two funded families. Spec:
/// Annex E §AIMS RL-1.
fn compute_dup_funded_cow_cleared_reps(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    dup_funded_reps: &FxHashSet<ArcVarId>,
    funded_dup_aliases: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let member_uses = collect_dup_funded_member_uses(func, same_alloc_reps, dup_funded_reps);
    let funded = close_funded_over_move_chain(func, funded_dup_aliases);
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut blocked: FxHashSet<ArcVarId> = FxHashSet::default();
    // A non-funded RC-READING consume of a lineage member blocks its rep
    // unless the site is terminal for the lineage.
    for (var, block_idx, site_idx) in collect_rc_reading_consume_sites(func, interner, contracts) {
        let rep = rep_of(var);
        if !dup_funded_reps.contains(&rep) || funded.contains(&var) {
            continue;
        }
        let Some(uses) = member_uses.get(&rep) else {
            continue;
        };
        let reachable = reachable_cache
            .entry(block_idx)
            .or_insert_with(|| compute_successor_reachable(func, block_idx));
        let used_after = uses
            .iter()
            .any(|&(ub, ui)| (ub == block_idx && ui > site_idx) || reachable.contains(&ub));
        if used_after {
            blocked.insert(rep);
        }
    }
    dup_funded_reps
        .iter()
        .copied()
        .filter(|rep| !blocked.contains(rep))
        .collect()
}

/// Same-alloc lineage reps with >= 1 aggregate-STORE hand-off of an RC member:
/// a `Construct` / `Reuse` / `CollectionReuse` arg or a `Set.value` whose repr
/// is `RcPointer` / `FatValue`. The Phase-7 admission for the store-family
/// hand-off-DEBITED net (`compute_dup_funded_debited_net` prices each store
/// consume -1 — the container's drop is the downstream release), covering the
/// fresh-local lineages whose surplus fresh-site keep-alive the undebited net
/// cannot see (the store's matched release is another var's drop glue).
/// Per-rep admission only; the net == 1 / net == 2 + carrier verdicts plus the
/// COW clearance bound the elision. Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn compute_store_handoff_reps(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let store_args: &[ArcVarId] = match instr {
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. } => args,
                ArcInstr::Set { value, .. } => std::slice::from_ref(value),
                _ => &[],
            };
            for &a in store_args {
                if is_rcptr(a) {
                    reps.insert(rep_of(a));
                }
            }
        }
    }
    reps
}

/// Same-alloc lineage reps with >= 1 FORWARD-Jump arg export of an RC member
/// (target does NOT dominate the source block — back-edge args are the loop's
/// own param rename, not an export; mirrors the
/// `compute_lineage_handoff_debits` forward-Jump arm). Admission for the
/// hand-off-DEBITED net in [`compute_elidable_fresh_self_alloc_incs`]: the
/// export's downstream release lives in the phi-excluded block-param lineage,
/// so the undebited net mis-reads the export's funding inc as surplus.
/// Spec: Annex E §AIMS RL-2 + RL-4.
pub(super) fn compute_forward_jump_export_reps(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    let dom = crate::graph::DominatorTree::build(func);
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for (b, block) in func.blocks.iter().enumerate() {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let src_id = crate::ir::ArcBlockId::new(u32::try_from(b).unwrap_or(u32::MAX));
            if !dom.dominates(*target, src_id) {
                for &arg in args {
                    if is_rcptr(arg) {
                        reps.insert(rep_of(arg));
                    }
                }
            }
        }
    }
    reps
}

/// Execution-final read-alias RELEASE CARRIER per store-family lineage rep
/// (RL-2 `RL2_release_exactly_once`): the UNIQUE `Let { Var }` read alias
/// carrying a surviving keep-alive `BurdenInc` + `BurdenDec` pair whose last
/// use no lineage-member use follows (same-block-later OR successor-reachable
/// — a loop re-reach declines; per-arm finals yield multiple candidates and
/// decline). Consumed by the net == 2 verdict in
/// [`compute_elidable_fresh_self_alloc_incs`]: the carrier's inc is elided
/// with the fresh-site inc, leaving its last-use dec — placed after the final
/// read — as the lineage's single surviving release.
///
/// Decline fences (each leaves the rep carrier-less — no elision fires):
/// - a candidate consumed at ANY moved position (owned call arg /
///   aggregate-store arg / `Set.value` / `Jump` arg / `Return`) is NOT a read
///   alias — its ops are funding/transfer arrangements, never the keep-alive
///   pair (eliding the inc of a pre-consume pair under-funds the consume);
/// - zero candidates (no surviving pair) or multiple execution-final
///   candidates (per-arm finals) — conservative.
///
/// Mirrors the designation algorithm of
/// `compute_call_result_element_final_read_releases` on the post-elimination
/// op-carrying IR. Spec: Annex E §AIMS RL-1 + RL-2.
pub(super) fn compute_store_family_final_read_carriers(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    store_reps: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, ArcVarId> {
    if store_reps.is_empty() {
        return FxHashMap::default();
    }
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let census = collect_store_family_census(func, &rep_of, store_reps);
    let mut reachable_cache: FxHashMap<usize, FxHashSet<usize>> = FxHashMap::default();
    let mut carriers: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for &rep in store_reps {
        let Some(sites_of_rep) = census.lineage_sites.get(&rep) else {
            continue;
        };
        let mut finals: Vec<ArcVarId> = Vec::new();
        for (&v, sites) in &census.use_sites {
            if rep_of(v) != rep
                || !census.alias_dsts.contains(&v)
                || census.moved_vars.contains(&v)
                || census.inc_count.get(&v).copied().unwrap_or(0) != 1
                || census.dec_count.get(&v).copied().unwrap_or(0) == 0
            {
                continue;
            }
            let Some(&(lb, li)) = sites.last() else {
                continue;
            };
            let reachable = reachable_cache
                .entry(lb)
                .or_insert_with(|| compute_successor_reachable(func, lb));
            let used_after = sites_of_rep
                .iter()
                .any(|&(ub, ui)| (ub == lb && ui > li) || reachable.contains(&ub));
            if !used_after {
                finals.push(v);
            }
        }
        if let [one] = finals.as_slice() {
            carriers.insert(rep, *one);
        }
    }
    carriers
}

/// One-walk census over the post-elimination IR for the carrier designation:
/// surviving whole-var burden-op counts, `Let { Var }` alias dsts, MOVED vars
/// (owned positions / `Set.value` / `Jump` args / `Return`), and the per-var +
/// per-rep use sites (burden ops are accounting markers, not uses; terminator
/// sites carry `usize::MAX`).
#[derive(Default)]
struct StoreFamilyCensus {
    inc_count: FxHashMap<ArcVarId, u32>,
    dec_count: FxHashMap<ArcVarId, u32>,
    alias_dsts: FxHashSet<ArcVarId>,
    moved_vars: FxHashSet<ArcVarId>,
    use_sites: FxHashMap<ArcVarId, Vec<(usize, usize)>>,
    lineage_sites: FxHashMap<ArcVarId, Vec<(usize, usize)>>,
}

fn collect_store_family_census(
    func: &ArcFunction,
    rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    store_reps: &FxHashSet<ArcVarId>,
) -> StoreFamilyCensus {
    let mut census = StoreFamilyCensus::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::BurdenInc { var } => {
                    *census.inc_count.entry(*var).or_default() += 1;
                    continue;
                }
                ArcInstr::BurdenDec { var }
                | ArcInstr::BurdenDecPartial { var, .. }
                | ArcInstr::BurdenDecVariant { var } => {
                    *census.dec_count.entry(*var).or_default() += 1;
                    continue;
                }
                ArcInstr::BurdenDecField { .. } => continue,
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(_),
                    ..
                } => {
                    census.alias_dsts.insert(*dst);
                }
                ArcInstr::Set { value, .. } => {
                    census.moved_vars.insert(*value);
                }
                _ => {}
            }
            for (pos, &v) in instr.used_vars().iter().enumerate() {
                if instr.is_owned_position(pos) {
                    census.moved_vars.insert(v);
                }
                let r = rep_of(v);
                if store_reps.contains(&r) {
                    census
                        .use_sites
                        .entry(v)
                        .or_default()
                        .push((block_idx, instr_idx));
                    census
                        .lineage_sites
                        .entry(r)
                        .or_default()
                        .push((block_idx, instr_idx));
                }
            }
        }
        let term = &block.terminator;
        for (pos, &v) in term.used_vars().iter().enumerate() {
            if term.is_owned_position(pos)
                || matches!(
                    term,
                    ArcTerminator::Return { .. } | ArcTerminator::Jump { .. }
                )
            {
                census.moved_vars.insert(v);
            }
            let r = rep_of(v);
            if store_reps.contains(&r) {
                census
                    .use_sites
                    .entry(v)
                    .or_default()
                    .push((block_idx, usize::MAX));
                census
                    .lineage_sites
                    .entry(r)
                    .or_default()
                    .push((block_idx, usize::MAX));
            }
        }
    }
    census
}

/// Every RC-READING consume site `(var, block, site_idx)` in `func`: owned
/// call-arg positions (structural or contract-consuming), list-concat `Add`
/// `RcPointer` operands, and may-COW user-call args (mirrors
/// `compute_cow_mutated_lineage_reps`'s conservative interprocedural class).
/// `iter` / `__`-protocol consumes are NOT rc-READING mutation sites: the
/// iterator machinery releases the buffer (RL-2 iter-consume transfer) but
/// never COW-mutates it in place — the fresh inc is not their guard.
/// Terminator sites carry `usize::MAX` as their own index so the terminator's
/// OWN recorded use never counts as a use after itself.
fn collect_rc_reading_consume_sites(
    func: &ArcFunction,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> Vec<(ArcVarId, usize, usize)> {
    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    let list_take_name = for_yield_result_finalizer_name(interner);
    let builtins = crate::borrow::BuiltinOwnershipSets::new(interner);
    let contract_owned = |callee: Name, pos: usize| -> bool {
        crate::lower::burden_lower::contract_consuming_arg_position(
            contracts, &builtins, interner, callee, pos,
        )
    };
    let callee_may_cow = |callee: Name, pos: usize| -> bool {
        callee_may_cow_arg(contracts, &builtins, interner, list_take_name, callee, pos)
    };
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let non_mutating_callee = |callee: Name| -> bool {
        callee == iter_name
            || interner
                .try_lookup(callee)
                .is_none_or(|n| n.starts_with("__"))
    };
    let mut sites: Vec<(ArcVarId, usize, usize)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            match instr {
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    if non_mutating_callee(*callee) {
                        continue;
                    }
                    for (pos, &arg) in args.iter().enumerate() {
                        let rc_reading = is_rcptr(arg)
                            && (instr.is_owned_position(pos)
                                || contract_owned(*callee, pos)
                                || callee_may_cow(*callee, pos));
                        if rc_reading {
                            sites.push((arg, block_idx, instr_idx));
                        }
                    }
                }
                ArcInstr::ApplyIndirect { .. } => {
                    for (pos, &var) in instr.used_vars().iter().enumerate() {
                        if instr.is_owned_position(pos) && is_rcptr(var) {
                            sites.push((var, block_idx, instr_idx));
                        }
                    }
                }
                ArcInstr::Let {
                    value:
                        ArcValue::PrimOp {
                            op: PrimOp::Binary(ori_ir::BinaryOp::Add),
                            args,
                        },
                    ..
                } => {
                    for &arg in args {
                        if is_rcptr(arg) {
                            sites.push((arg, block_idx, instr_idx));
                        }
                    }
                }
                _ => {}
            }
        }
        let term_idx = usize::MAX;
        match &block.terminator {
            ArcTerminator::Invoke {
                func: callee, args, ..
            } if !non_mutating_callee(*callee) => {
                let term = &block.terminator;
                for (pos, &arg) in args.iter().enumerate() {
                    let rc_reading = is_rcptr(arg)
                        && (term.is_owned_position(pos)
                            || contract_owned(*callee, pos)
                            || callee_may_cow(*callee, pos));
                    if rc_reading {
                        sites.push((arg, block_idx, term_idx));
                    }
                }
            }
            ArcTerminator::InvokeIndirect { .. } => {
                let term = &block.terminator;
                for (pos, &var) in term.used_vars().iter().enumerate() {
                    if term.is_owned_position(pos) && is_rcptr(var) {
                        sites.push((var, block_idx, term_idx));
                    }
                }
            }
            _ => {}
        }
    }
    sites
}

/// Funded membership closed over the single-use move chain: the admitted
/// kept-dup alias may reach its consume through Let hops (`%4 = %2;
/// %6 = %4; insert(%6 [own])`) — the hop carries the SAME funded reference.
fn close_funded_over_move_chain(
    func: &ArcFunction,
    call_arg_dups: &FxHashSet<ArcVarId>,
) -> FxHashSet<ArcVarId> {
    let mut funded: FxHashSet<ArcVarId> = call_arg_dups.clone();
    let (alias_next, _) = crate::lower::burden_lower::collect_move_edges_and_store_consumes(func);
    let mut frontier: Vec<ArcVarId> = funded.iter().copied().collect();
    while let Some(v) = frontier.pop() {
        if let Some(&next) = alias_next.get(&v) {
            if funded.insert(next) {
                frontier.push(next);
            }
        }
    }
    funded
}

/// Per-rep member use sites (body `(block, instr)`; terminator `(block,
/// usize::MAX)`) for the dup-funded cow clearance's terminal-consume
/// reachability check.
fn collect_dup_funded_member_uses(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    dup_funded_reps: &FxHashSet<ArcVarId>,
) -> FxHashMap<ArcVarId, Vec<(usize, usize)>> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let mut member_uses: FxHashMap<ArcVarId, Vec<(usize, usize)>> = FxHashMap::default();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            for &v in &instr.used_vars() {
                let r = rep_of(v);
                if dup_funded_reps.contains(&r) {
                    member_uses
                        .entry(r)
                        .or_default()
                        .push((block_idx, instr_idx));
                }
            }
        }
        for v in block.terminator.used_vars() {
            let r = rep_of(v);
            if dup_funded_reps.contains(&r) {
                member_uses
                    .entry(r)
                    .or_default()
                    .push((block_idx, usize::MAX));
            }
        }
    }
    member_uses
}

/// Forward-reachable block set from `start`'s SUCCESSORS (`start` itself only
/// when a cycle re-reaches it) — the terminal-consume reachability used by the
/// dup-funded cow clearance (mirrors the Phase-5 discriminator).
fn compute_successor_reachable(func: &ArcFunction, start: usize) -> FxHashSet<usize> {
    let starts: Vec<usize> = func
        .blocks
        .get(start)
        .map(|b| {
            crate::graph::successor_block_ids(&b.terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect()
        })
        .unwrap_or_default();
    forward_reachable_from(func, &starts)
}

/// Whether a `for_yield` `ori_list_take` RESULT (the lineage rep `result_rep`) is
/// OWNERSHIP-TRANSFERRED OUT of the current function — either ITER-consumed (its
/// lineage flows to an `@iter [own]` position: a second for-loop / `ori_iter_drop`)
/// OR RETURNED (its lineage is the `Return` terminator value).
///
/// The move-vs-borrow discriminator for the joint yield-element RC: a transferred
/// result frees / hands off its own element copies elsewhere — an ITER-consumed
/// result via `ori_iter_drop`, a RETURNED result via the CALLER's consumption
/// (which the callee cannot see). Either way the yielded elements need NO extra RC
/// inside this function (the `yield_identity_str_list{,_two_calls,_borrowed_param}`
/// canaries: `clone_list` RETURNS the result, the caller iter-consumes it). A
/// result NEITHER iter-consumed NOR returned (index-consumed / length-consumed in
/// the same function) owns its element copies locally → the yielded elements need
/// the duplicating inc (RL-1), and `@__index`-extracted views need their own
/// release (RL-2).
fn for_yield_result_transferred_out(
    func: &ArcFunction,
    result_rep: ArcVarId,
    phi_rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    iter_name: ori_ir::Name,
) -> bool {
    for block in &func.blocks {
        // RETURNED: the terminator returns a value in the result's lineage.
        if let ArcTerminator::Return { value } = &block.terminator {
            if phi_rep_of(*value) == result_rep {
                return true;
            }
        }
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                if *callee == iter_name
                    && args.iter().zip(arg_ownership).any(|(&a, own)| {
                        matches!(own, ArgOwnership::Owned) && phi_rep_of(a) == result_rep
                    })
                {
                    return true;
                }
            }
        }
    }
    false
}

/// A `for_yield` `ori_list_take` result passed OWNED to `@iter` (locally
/// iter-consumed) AND NOT returned. The in-function `IterState::Drop` releases the
/// result's element refs, so a borrowed iter-element view duplicated into the
/// result needs its `+1` here. The RETURNED case is EXCLUDED — its element release
/// is the caller's, so an inc inside the callee leaks. Spec: Annex E §AIMS RL-1.
fn for_yield_result_iter_consumed_not_returned(
    func: &ArcFunction,
    result_rep: ArcVarId,
    phi_rep_of: &impl Fn(ArcVarId) -> ArcVarId,
    iter_name: ori_ir::Name,
) -> bool {
    let mut iter_consumed = false;
    for block in &func.blocks {
        if let ArcTerminator::Return { value } = &block.terminator {
            if phi_rep_of(*value) == result_rep {
                return false;
            }
        }
        for instr in &block.body {
            if let ArcInstr::Apply {
                func: callee,
                args,
                arg_ownership,
                ..
            } = instr
            {
                if *callee == iter_name
                    && args.iter().zip(arg_ownership).any(|(&a, own)| {
                        matches!(own, ArgOwnership::Owned) && phi_rep_of(a) == result_rep
                    })
                {
                    iter_consumed = true;
                }
            }
        }
    }
    iter_consumed
}

/// `ORI_DISABLE_FOR_YIELD_RESULT_PREMATURE_RELEASE_RELOCATION=1` declines the
/// Phase-6.95b relocation. Read once at first access.
static FOR_YIELD_RESULT_PREMATURE_RELEASE_RELOCATION_DISABLED: LazyLock<bool> =
    LazyLock::new(|| {
        std::env::var("ORI_DISABLE_FOR_YIELD_RESULT_PREMATURE_RELEASE_RELOCATION").as_deref()
            == Ok("1")
    });

/// Phase 6.95b (probe): relocate a PREMATURE normal-path release of an eligible
/// `for_yield` `ori_list_take` RESULT list whose allocation is read again in a
/// LATER block via a same-allocation sibling alias.
///
/// Shape (`let copied = for w in words yield w; copied[0].length() + copied[1].length()`):
/// the result list (`%20 = ori_list_take`) is read via sibling `Let`-Var aliases
/// (`%24 = %20` in `bb3`, `%29 = %20` in `bb4`) across TWO blocks. The base walk
/// emits the list's single normal-path release at `%24`'s SSA last-use (`bb3`,
/// after `@__index(%24)`), but `%29` (the SAME allocation) is `@__index`-read in
/// `bb4` — so the early dec frees the list before `bb4` reads it (use-after-free /
/// `-134`). The base walk's `live_out` suppressor suppresses `%20`'s own dec
/// (live-out via `%29`) but NOT `%24`'s (a sibling Let-Var alias dead-out of `bb3`):
/// the live-out set is per-SSA-var, not allocation-grain.
///
/// The cure RELOCATES the single premature normal-path `BurdenDec` to AFTER the
/// lineage's execution-final normal-path value-read — one release, moved later
/// (`RL2_release_exactly_once` preserved; net unchanged, `RL3_elision_net_preserving`).
/// NOT a removal, NOT an addition — a placement move. Unwind-path (`Resume`)
/// releases are untouched (status-quo unwind behavior preserved).
///
/// Admission gates (ALL hold; ANY failure declines — the status-quo premature
/// free is the migration floor, never a regression introduced here):
///  (a) the lineage rep is an ELIGIBLE non-transferred-out `ori_list_take` result
///      (the result owns its element copies; its own release frees them).
///  (b) EXACTLY ONE normal-path `BurdenDec` on a lineage member (`dec_block`,
///      `dec_pos`); zero or >1 declines (the multi-release shape is out of family).
///  (c) a member is READ (borrow/owned use, excluding the dec itself) in a block
///      `read_block` that is FORWARD-REACHABLE from `dec_block` AND distinct from
///      it — the premature-free condition (the allocation is read after its only
///      normal-path release on some path).
///  (d) a UNIQUE execution-final normal-path read site exists (single
///      `(final_block, final_pos)`); a non-unique final read declines.
///  (e) `final_block` is NOT in a CFG cycle (no loop-carried relocation — that is
///      the foreclosed back-edge territory).
/// One relocation plan for [`relocate_for_yield_result_premature_release`]:
/// strip the premature normal-path dec at `(strip_block, strip_pos)` and place
/// ONE dec on `var` after the lineage's execution-final read at
/// `(place_block, place_pos)`.
struct ForYieldDecRelocation {
    strip_block: usize,
    strip_pos: usize,
    place_block: usize,
    place_pos: usize,
    var: ArcVarId,
}

/// Gate (a): eligible non-transferred-out `ori_list_take` result reps whose
/// premature normal-path release may be relocatable.
fn for_yield_eligible_take_reps(
    func: &ArcFunction,
    take_name: Name,
    iter_name: Name,
    phi_rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> FxHashSet<ArcVarId> {
    let mut eligible_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == take_name
                    && matches!(
                        func.var_repr(*dst),
                        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                    )
                {
                    let rep = phi_rep_of(*dst);
                    if !for_yield_result_transferred_out(func, rep, phi_rep_of, iter_name) {
                        eligible_reps.insert(rep);
                    }
                }
            }
        }
    }
    eligible_reps
}

/// Gates (b)-(e) for one eligible `rep`: produce a relocation plan when the rep
/// has EXACTLY ONE normal-path `BurdenDec` (b) that is genuinely premature — a
/// later forward-reachable value-read exists (c) with a UNIQUE execution-final
/// read site (d) whose block is NOT in a CFG cycle (e). `None` declines.
fn for_yield_relocation_plan_for_rep(
    func: &ArcFunction,
    rep: ArcVarId,
    phi_rep_of: &impl Fn(ArcVarId) -> ArcVarId,
) -> Option<ForYieldDecRelocation> {
    let in_lineage = |v: ArcVarId| phi_rep_of(v) == rep;

    // Gate (b): the single normal-path `BurdenDec` on a lineage member.
    let mut dec_sites: Vec<(usize, usize, ArcVarId)> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        if matches!(block.terminator, ArcTerminator::Resume) {
            continue;
        }
        for (i, instr) in block.body.iter().enumerate() {
            if let ArcInstr::BurdenDec { var } = instr {
                if in_lineage(*var) {
                    dec_sites.push((b, i, *var));
                }
            }
        }
    }
    if dec_sites.len() != 1 {
        return None;
    }
    let (dec_block, dec_pos, _dec_var) = dec_sites[0];

    // Gate (c) + (d): the execution-final normal-path member value-read, and the
    // premature-free condition (a read forward-reachable from `dec_block`,
    // distinct from `dec_block`). A "value-read" is any non-`BurdenDec`/
    // non-`BurdenInc`/non-`RcDec`/non-`RcInc` body use OR a terminator use of a
    // lineage member at a borrow/owned position.
    let reachable_from_dec = forward_reachable_from(func, &[dec_block]);
    let mut read_sites: Vec<(usize, usize)> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        if matches!(block.terminator, ArcTerminator::Resume) {
            continue;
        }
        for (i, instr) in block.body.iter().enumerate() {
            if matches!(
                instr,
                ArcInstr::BurdenDec { .. }
                    | ArcInstr::BurdenInc { .. }
                    | ArcInstr::RcDec { .. }
                    | ArcInstr::RcInc { .. }
            ) {
                continue;
            }
            // A `Let { Var(src) }` alias hop is a lineage edge, not a genuine
            // value-read of the allocation — skip (it never frees / reads bytes).
            if let ArcInstr::Let {
                value: ArcValue::Var(_),
                ..
            } = instr
            {
                continue;
            }
            if instr.used_vars().iter().any(|v| in_lineage(*v)) {
                read_sites.push((b, i));
            }
        }
    }
    if read_sites.is_empty() {
        return None;
    }
    // Premature-free: at least one read is in a DISTINCT block forward-reachable
    // from `dec_block` (the allocation is read after its only release on some
    // path).
    let premature = read_sites
        .iter()
        .any(|&(b, _)| b != dec_block && reachable_from_dec.contains(&b));
    if !premature {
        return None;
    }
    // Gate (d): the unique execution-final read site. Order by (block, pos); the
    // final read is the maximum. A tie across two distinct blocks is a
    // branch-split final read handled by taking the single maximal block.
    let max_block = read_sites.iter().map(|&(b, _)| b).max().unwrap_or(0);
    let final_pos = read_sites
        .iter()
        .filter(|&&(b, _)| b == max_block)
        .map(|&(_, p)| p)
        .max()
        .unwrap_or(0);

    // Gate (e): `max_block` not in a CFG cycle (no loop-carried relocation).
    if for_yield_block_in_cycle(func, max_block) {
        return None;
    }

    // Only relocate when the final read is strictly AFTER the dec in CFG order
    // (the dec is genuinely premature). When the dec already sits at-or-after the
    // final read in the same block, leave it.
    if max_block == dec_block && final_pos <= dec_pos {
        return None;
    }

    // Use the same lineage var the stripped dec targeted (its allocation identity
    // is the lineage rep; the placed dec frees the same allocation).
    Some(ForYieldDecRelocation {
        strip_block: dec_block,
        strip_pos: dec_pos,
        place_block: max_block,
        place_pos: final_pos,
        var: dec_sites[0].2,
    })
}

/// Gate (e) helper: `block` is in a CFG cycle when a successor of it can reach
/// it again.
fn for_yield_block_in_cycle(func: &ArcFunction, block: usize) -> bool {
    forward_reachable_from(func, &[block]).contains(&block)
        && func.blocks.get(block).is_some_and(|blk| {
            crate::graph::successor_block_ids(&blk.terminator)
                .iter()
                .any(|s| {
                    let si = s.index();
                    si != block && forward_reachable_from(func, &[si]).contains(&block)
                })
        })
}

/// Apply each plan: strip the premature dec (descending position), then place
/// the new dec after the final read (descending position). Strips and places
/// target distinct `(block, pos)` so ordering across plans is independent;
/// sorting each group descending keeps indices stable within a block.
fn apply_for_yield_dec_relocations(func: &mut ArcFunction, plans: &[ForYieldDecRelocation]) {
    let mut strips: Vec<(usize, usize)> =
        plans.iter().map(|p| (p.strip_block, p.strip_pos)).collect();
    strips.sort_unstable_by(|a, b| b.cmp(a));
    for (b, i) in strips {
        if let Some(block) = func.blocks.get_mut(b) {
            if i < block.body.len() && matches!(block.body[i], ArcInstr::BurdenDec { .. }) {
                block.body.remove(i);
            }
        }
    }
    // Placement positions were computed against the PRE-strip body; recompute the
    // safe insertion index by clamping (a strip in the same block at a lower index
    // shifts later positions left by one). Apply per-block, descending placement
    // position, after adjusting for same-block strips below the placement.
    let mut places: Vec<(usize, usize, ArcVarId)> = plans
        .iter()
        .map(|p| {
            let strips_below = plans
                .iter()
                .filter(|q| q.strip_block == p.place_block && q.strip_pos <= p.place_pos)
                .count();
            (
                p.place_block,
                p.place_pos.saturating_sub(strips_below),
                p.var,
            )
        })
        .collect();
    places.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    for (b, after, var) in places {
        if let Some(block) = func.blocks.get_mut(b) {
            let pos = (after + 1).min(block.body.len());
            block.body.insert(pos, ArcInstr::BurdenDec { var });
        }
    }
}

fn relocate_for_yield_result_premature_release(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    if *FOR_YIELD_RESULT_PREMATURE_RELEASE_RELOCATION_DISABLED {
        return;
    }
    let _ = pool;
    let take_name = for_yield_result_finalizer_name(interner);
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());
    let threaded = compute_phi_threaded_alloc_reps(func, same_alloc_reps);
    let phi_rep_of = |v: ArcVarId| threaded.get(&v).copied().unwrap_or(v);

    let eligible_reps = for_yield_eligible_take_reps(func, take_name, iter_name, &phi_rep_of);
    if eligible_reps.is_empty() {
        return;
    }

    // One relocation plan per eligible rep: strip the premature dec, place ONE
    // after the final read.
    let plans: Vec<ForYieldDecRelocation> = eligible_reps
        .iter()
        .filter_map(|&rep| for_yield_relocation_plan_for_rep(func, rep, &phi_rep_of))
        .collect();
    if plans.is_empty() {
        return;
    }

    apply_for_yield_dec_relocations(func, &plans);
}

/// A pending burden-op insertion for [`emit_for_yield_index_consumed_element_rc`]:
/// insert `BurdenInc`/`BurdenDec` (`is_inc`) on `var` AFTER `block`'s instruction
/// index `after`.
struct ForYieldElemInsert {
    block: usize,
    after: usize,
    var: ArcVarId,
    is_inc: bool,
}

/// Phase 6.95 (probe): `for_yield` NON-iter-consumed element RC. For each
/// `ori_list_take` result that is NOT iter-consumed (per
/// [`for_yield_result_iter_consumed`] = false — index-consumed / length-consumed
/// / returned):
/// - **yield-element-inc** (RL-1): on the element value moved at an OWNED position
///   into the result's `ori_list_push(scratch, w [own])` — the push DUPLICATES the
///   borrowed iter-element view into the result buffer, so the result needs its
///   own `+1` (the iter-element-view exclusion drops it by default; without the
///   inc the source `IterState::Drop` AND the result `elem_dec_fn` both free the
///   element → double-free);
/// - **index-result-element-dec** (RL-2): on each `@__index(result [borrow], _) ->
///   view` extracted view, after the index call — the extracted `FatValue` view of
///   a heap element needs its own release (the result buffer's `elem_dec_fn` frees
///   the stored copies; the indexed VIEW is a separate borrow the oracle decs).
///
/// Probe-gated (runs only inside `emit_burden_path_probe_tail` behind
/// `predicate_stack_rc_disabled`) -> default codegen byte-identical. Spec: Annex E
/// §AIMS RL-1 (`RL1_emits_inc = !incElidable`) + RL-2 (`RL2_release_exactly_once`).
fn emit_for_yield_index_consumed_element_rc(
    func: &mut ArcFunction,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) {
    let take_name = for_yield_result_finalizer_name(interner);
    let push_name = interner.intern("ori_list_push");
    let index_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.name());
    let iter_name =
        interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name());

    let threaded = compute_phi_threaded_alloc_reps(func, same_alloc_reps);
    let phi_rep_of = |v: ArcVarId| threaded.get(&v).copied().unwrap_or(v);

    let ForYieldEligibleResults {
        eligible_result_reps,
        scratch_to_result_rep,
        iter_consumed_scratch_to_result,
    } = collect_for_yield_eligible_results(func, take_name, iter_name, &threaded);
    if eligible_result_reps.is_empty() && iter_consumed_scratch_to_result.is_empty() {
        return;
    }
    // Borrowed iter-element views (`Project(__iter_next.1)`, TF-4) — the
    // discriminator for the transferred-out yield-element-inc.
    let iter_element_defs = crate::aims::emit_rc::collect_iter_element_defs(func, interner);

    // Result vars that ALREADY carry a surviving `BurdenDec` — the base walk's own
    // last-use release of the `@__index` self-inc result (owned +1 per emit.rs RL-1
    // inc-elision). The index-result-element-dec below is a COMPENSATION for the
    // case where that base dec was suppressed (the iter-element-view exclusion);
    // when the base dec SURVIVES, a second dec here over-releases the owned `+1`
    // (RL2_release_exactly_once: inc :: [dec, dec] nets -1 -> double-free). Skip the
    // compensation dec for any `@__index` result the base walk already releases.
    let already_decced: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .flat_map(|b| b.body.iter())
        .filter_map(|instr| match instr {
            ArcInstr::BurdenDec { var } => Some(*var),
            _ => None,
        })
        .collect();

    // Collect insertion points so we mutate after the read scan.
    let mut inserts: Vec<ForYieldElemInsert> = Vec::new();
    for (b, block) in func.blocks.iter().enumerate() {
        for (i, instr) in block.body.iter().enumerate() {
            let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            else {
                continue;
            };
            // yield-element-inc: `ori_list_push(scratch, w [own])` whose scratch
            // belongs to an index-consumed result. `w` (args[1]) is the duplicated
            // element view → emit a BurdenInc on it BEFORE the push (so the push's
            // owned consume + the result's copy are both balanced).
            if *callee == push_name && args.len() >= 2 {
                if let Some(&scratch) = args.first() {
                    let w = args[1];
                    // Only heap (non-scalar) element values carry RC.
                    let w_is_heap = matches!(
                        func.var_repr(w),
                        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                    );
                    // Non-transferred-out (index/length-consumed) result: the
                    // existing eligibility gate.
                    let via_eligible = scratch_to_result_rep.contains_key(&scratch);
                    // Iter-consumed-not-returned result: fire ONLY when `w` is a
                    // borrowed iter-element view (the duplicating push shape;
                    // never an owned-fresh `Construct` push).
                    let via_iter_consumed = iter_consumed_scratch_to_result.contains_key(&scratch)
                        && iter_element_defs.contains(&w);
                    if w_is_heap && (via_eligible || via_iter_consumed) {
                        inserts.push(ForYieldElemInsert {
                            block: b,
                            after: i.saturating_sub(1),
                            var: w,
                            is_inc: true,
                        });
                    }
                }
            }
            // index-result-element-dec: `@__index(result [borrow], _) -> view`
            // whose receiver belongs to an eligible (non-iter-consumed) result.
            // Emit a BurdenDec on the extracted `view` (the dst) AFTER the index
            // call.
            if *callee == index_name
                && args
                    .first()
                    .is_some_and(|&recv| eligible_result_reps.contains(&phi_rep_of(recv)))
                && matches!(
                    func.var_repr(*dst),
                    Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                )
                // The base walk already emits this result's single release —
                // compensating here over-releases the owned `+1` (RL2_release_exactly_once).
                && !already_decced.contains(dst)
            {
                inserts.push(ForYieldElemInsert {
                    block: b,
                    after: i,
                    var: *dst,
                    is_inc: false,
                });
            }
        }
    }
    let _ = pool;
    // Apply insertions back-to-front per block so earlier indices stay valid.
    inserts.sort_by(|a, b| b.block.cmp(&a.block).then(b.after.cmp(&a.after)));
    for ins in inserts {
        let op = if ins.is_inc {
            ArcInstr::BurdenInc { var: ins.var }
        } else {
            ArcInstr::BurdenDec { var: ins.var }
        };
        let body = &mut func.blocks[ins.block].body;
        let pos = (ins.after + 1).min(body.len());
        body.insert(pos, op);
    }
}

/// The three eligible-result maps consumed by
/// [`emit_for_yield_index_consumed_element_rc`].
struct ForYieldEligibleResults {
    eligible_result_reps: FxHashSet<ArcVarId>,
    scratch_to_result_rep: FxHashMap<ArcVarId, ArcVarId>,
    iter_consumed_scratch_to_result: FxHashMap<ArcVarId, ArcVarId>,
}

/// Scan `ori_list_take` results into the three eligibility maps.
///
/// - `eligible_result_reps`: NON-iter-consumed result lineage reps (phi-threaded)
///   whose yielded elements need the duplicating RC.
/// - `scratch_to_result_rep`: each result's SCRATCH buffer arg (the
///   `ori_list_take(scratch)` arg) -> result phi rep, so the matching
///   `ori_list_push(scratch, w)` is attributed to an eligible result.
/// - `iter_consumed_scratch_to_result`: ITER-CONSUMED-not-returned result scratch
///   args. The buffer is consumed IN-FUNCTION by `@iter`, but a yielded BORROWED
///   iter-element view is still shared with the surviving iter source
///   (`IterState::Drop` decs the source ref AND the result `elem_dec_fn` decs the
///   copy), so the push needs its own duplicating `+1` (RL-1). The RETURNED case
///   is EXCLUDED — its element release is the caller's.
fn collect_for_yield_eligible_results(
    func: &ArcFunction,
    take_name: ori_ir::Name,
    iter_name: ori_ir::Name,
    threaded: &FxHashMap<ArcVarId, ArcVarId>,
) -> ForYieldEligibleResults {
    let phi_rep_of = |v: ArcVarId| threaded.get(&v).copied().unwrap_or(v);
    let mut eligible_result_reps: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut scratch_to_result_rep: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let mut iter_consumed_scratch_to_result: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst,
                func: callee,
                args,
                ..
            } = instr
            {
                if *callee == take_name
                    && matches!(
                        func.var_repr(*dst),
                        Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                    )
                {
                    let result_rep = phi_rep_of(*dst);
                    if !for_yield_result_transferred_out(func, result_rep, &phi_rep_of, iter_name) {
                        eligible_result_reps.insert(result_rep);
                        if let Some(&scratch) = args.first() {
                            scratch_to_result_rep.insert(scratch, result_rep);
                        }
                    } else if for_yield_result_iter_consumed_not_returned(
                        func,
                        result_rep,
                        &phi_rep_of,
                        iter_name,
                    ) {
                        if let Some(&scratch) = args.first() {
                            iter_consumed_scratch_to_result.insert(scratch, result_rep);
                        }
                    }
                }
            }
        }
    }
    ForYieldEligibleResults {
        eligible_result_reps,
        scratch_to_result_rep,
        iter_consumed_scratch_to_result,
    }
}

/// The phi-attribution view for [`compute_lineage_alloc_aware_net`]: the
/// phi-threaded allocation-equivalence reps PLUS the precomputed set of
/// `same_alloc_reps`-reps eligible for the phi-threaded net attribution
/// (`ori_list_take` results whose phi-threaded class merges exactly one such
/// result).
struct ListTakePhiAttribution {
    threaded: FxHashMap<ArcVarId, ArcVarId>,
    eligible: FxHashSet<ArcVarId>,
}

/// Build the phi-attribution view scoped to `ori_list_take` results. The
/// `eligible` set carries the single-alloc guard: a `same_alloc_reps`-rep is
/// eligible iff it is an `ori_list_take` result AND its phi-threaded class merges
/// exactly one such result (a 2-result phi-merge — the `04B.2-cross-class-uaf`
/// shape — falls back to the unthreaded, double-free-safe net).
fn compute_list_take_phi_attribution(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    list_take_name: ori_ir::Name,
) -> ListTakePhiAttribution {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let threaded = compute_phi_threaded_alloc_reps(func, same_alloc_reps);
    let phi_rep_of = |v: ArcVarId| threaded.get(&v).copied().unwrap_or(v);

    // `ori_list_take`-result `same_alloc_reps`-reps, grouped by phi-threaded class.
    let mut take_reps_per_phi_class: FxHashMap<ArcVarId, FxHashSet<ArcVarId>> =
        FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            else {
                continue;
            };
            if *callee == list_take_name
                && matches!(
                    func.var_repr(*dst),
                    Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                )
            {
                take_reps_per_phi_class
                    .entry(phi_rep_of(*dst))
                    .or_default()
                    .insert(rep_of(*dst));
            }
        }
    }
    // Single-alloc guard: keep only reps in a phi class merging exactly one take.
    let mut eligible: FxHashSet<ArcVarId> = FxHashSet::default();
    for take_reps in take_reps_per_phi_class.values() {
        if take_reps.len() == 1 {
            eligible.extend(take_reps.iter().copied());
        }
    }
    ListTakePhiAttribution { threaded, eligible }
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

/// The set of block indices forward-reachable from any block in `starts`
/// (inclusive of the starts), via `successor_block_ids` CFG edges. Used by
/// [`compute_lineage_alloc_aware_net`] to restrict the terminal-net agreement to
/// terminal blocks a lineage's allocation can actually reach — an unwind landing
/// pad reached BEFORE the allocation is not a release path for that lineage.
fn forward_reachable_from(func: &ArcFunction, starts: &[usize]) -> FxHashSet<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = starts.to_vec();
    while let Some(b) = stack.pop() {
        if !visited.insert(b) {
            continue;
        }
        let Some(block) = func.blocks.get(b) else {
            continue;
        };
        for s in crate::graph::successor_block_ids(&block.terminator) {
            stack.push(s.index());
        }
    }
    visited
}

/// The interned `ori_list_take` finalizer name — the `for x in coll yield expr`
/// comprehension's result builder. The loop lowering (`lower/control_flow/
/// for_yield.rs`, `for_yield_option.rs`) allocates a growable scratch via
/// `ori_list_new`, pushes each body result, then `ori_list_take`s the scratch:
/// the runtime MOVES the data buffer out of the scratch `OriList` (freeing only
/// the struct, not the buffer — `ori_rt/src/list/mod.rs:269`), yielding a FRESH
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
    let collect_set_protocol = interner.intern("__collect_set");
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
    // reads the buffer's rc to COW it (`ori_rt/src/list/mod.rs:269`).
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
