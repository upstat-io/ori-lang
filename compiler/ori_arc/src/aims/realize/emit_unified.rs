//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Phase 1 sub-step B of [`super::realize_rc_reuse()`].

#[cfg(test)]
mod burden_lowering_tests;

use std::sync::LazyLock;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{MemoryContract, ReturnAliasShape};
use crate::aims::emit_rc::DeferredDec;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::apply_aliases::build_let_alias_map;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, PrimOp, RcAtomicity, RcStrategy,
    ValueRepr,
};

use super::metrics;
use super::walk;

/// `ORI_DISABLE_BURDEN_ELIM=1` bypasses Phase 2.5 burden-op elimination, read
/// once at first access. Isolates Phase 5 emission from elimination for
/// diagnostic bisection.
static BURDEN_ELIM_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_BURDEN_ELIM").as_deref() == Ok("1"));

/// Per-phase RC-op snapshot for post-walk pass debugging.
///
/// Emits one `tracing::trace!` per block summarising every `RcInc`/`RcDec` by
/// `ArcVarId`. Gated behind `tracing::enabled!` — zero overhead when the
/// `ori_arc::aims::realize` target is below trace level.
///
/// `ORI_LOG=ori_arc::aims::realize=trace` activates it; bisects which post-walk
/// pass (`emit_dead_invoke_dsts`, `emit_edge_cleanup`,
/// `emit_project_escape_incs`, `coalesce_block_rc`) rewrote a block's RC ops.
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
#[expect(
    clippy::too_many_lines,
    reason = "single phase-ordered RC realization pipeline — the Phase 1→2.5→6.5→7→3 \
              sequence is one cohesive orchestration; splitting mid-sequence fragments \
              the load-bearing phase order (PL-2/PL-3/PL-4) and hides the pipeline shape"
)]
pub(super) fn emit_rc_unified(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    contracts: &FxHashMap<Name, MemoryContract>,
    // Probe path (Spec: Annex E §AIMS). `true`: skip the predicate-stack walk +
    // cleanup; mechanically lower surviving burden ops to `RcInc`/`RcDec`
    // (Phase 7) so the burden path alone emits RC. `false` (default): predicate
    // stack emits RC, burden ops stay codegen no-op markers.
    predicate_stack_rc_disabled: bool,
) -> (
    usize,
    Vec<DeathEvent>,
    Vec<AllocEvent>,
    metrics::SynergyMetrics,
) {
    use crate::aims::emit_rc::{
        coalesce_block_rc, collect_all_borrowed_defs, collect_inline_enum_projected_defs,
        collect_iter_element_defs, collect_project_borrowed_defs, compute_function_project_sources,
        emit_dead_invoke_dsts, emit_edge_cleanup, DeferredDec,
    };

    debug_assert!(
        !func.var_reprs.is_empty(),
        "var_reprs must be populated before RC emission"
    );

    let all_borrowed_defs = collect_all_borrowed_defs(func, pool);
    let project_borrowed_defs = collect_project_borrowed_defs(func, pool);
    let iter_element_defs = collect_iter_element_defs(func, interner);
    let inline_enum_projected_defs = collect_inline_enum_projected_defs(func, pool);
    let func_project_sources = compute_function_project_sources(func);

    let ReturnTransferSetup {
        return_transfer_params,
        alias_to_param,
        return_project_inc_targets,
    } = build_return_transfer_setup(func, pool, contracts);

    // Per-class take-project facts (union-find + CFG reachability), computed
    // once per function. Each source seeds a connected-component class over
    // Let-alias + Jump-arg→block-param edges, with its own bypass-safe blocks
    // and entries. `dead_cleanup` (sources 1, 2) and `edge_cleanup` query
    // `is_in_class` / `class_of` / `is_bypass_safe_entry_for_var` to drop
    // exactly once per CFG path — no double-free, no leak.
    let take_move_facts = crate::aims::emit_rc::take_project::analyze(func, pool);

    // Same-allocation union-find reps (`Let{Var}` + apply Direct/Conditional;
    // EXCLUDES Jump-arg phi), computed once per function for the per-block
    // walk's `class_alive_after` obligation-table same-alloc gate.
    let same_alloc_reps =
        crate::aims::emit_rc::compute_same_alloc_reps(func, state_map.apply_result_aliases());
    let iter_fn_name = interner.intern("iter");
    let predecessors = crate::graph::compute_predecessors(func);
    let mut all_death_events = Vec::new();
    let mut all_alloc_events = Vec::new();

    // Deferred decs routed to edge cleanup. Each entry:
    // - `None` target: emit on ALL outgoing edges (Phase B deferred parents)
    // - `Some(succ)` target: emit only on edge to `succ` (merge-edge decs)
    let mut block_deferred: FxHashMap<usize, Vec<DeferredDec>> = FxHashMap::default();
    let mut synergy = metrics::SynergyMetrics::default();

    // Cross-block dec-emitter map + post-dominator tree. A class member's dec is
    // suppressed only when another member covers its RC slot on every path
    // (`class_member_suppresses`). Post-dominance — not raw block order — gates
    // cross-block suppression: a branch (neither arm post-dominates) keeps one
    // dec per path, else under-emission leaks.
    let post_doms = crate::graph::PostDominatorTree::build(func);

    // `build_global_pin4_emits` also returns the retained-lineage map, filtered
    // to lineages dying within their SSA-alias class. A within-class copy that
    // transfers out (Construct / owned-arg / Jump-arg / Return) is balanced by
    // the enclosing value's drop, so it leaves the map and the class dedups
    // normally; a copy that dies in-class keeps its own dec, netting `1 + N`
    // decs per path (rc_per_path_invariant).
    let env = RealizeEnv {
        state_map,
        pool,
        post_doms: &post_doms,
        all_borrowed_defs: &all_borrowed_defs,
        project_borrowed_defs: &project_borrowed_defs,
        iter_element_defs: &iter_element_defs,
        inline_enum_projected_defs: &inline_enum_projected_defs,
        func_project_sources: &func_project_sources,
        take_move_facts: &take_move_facts,
        return_transfer_params: &return_transfer_params,
        alias_to_param: &alias_to_param,
        return_project_inc_targets: &return_project_inc_targets,
        same_alloc_reps: &same_alloc_reps,
        iter_fn_name,
    };
    let (global_pin4_emits, lineage_roots) = build_global_pin4_emits(func, &env);

    // Phases 1 / 1.5 / 2 / 2.1 are the predicate-stack `RcInc`/`RcDec`
    // emitter. The probe suppresses them entirely so the burden path (Phase
    // 2.5 elimination + Phase 7 lowering below) is the sole real-RC emitter.
    if !predicate_stack_rc_disabled {
        // Phase 1: per-block RC emission via unified forward walk.
        for block_idx in 0..func.blocks.len() {
            let (death_events, alloc_events, walk_metrics) = emit_block_rc(
                func,
                block_idx,
                &env,
                &global_pin4_emits,
                &lineage_roots,
                &predecessors,
                &mut block_deferred,
            );
            synergy.merge(&walk_metrics);
            all_death_events.extend(death_events);
            all_alloc_events.extend(alloc_events);
        }
        trace_phase_snapshot("after_phase_1_walk", func, interner);

        // Phase 1.5: dead Invoke result cleanup.
        emit_dead_invoke_dsts(func, state_map, pool, &all_borrowed_defs);
        trace_phase_snapshot("after_phase_1_5_dead_invoke", func, interner);

        // Phase 2: inter-block edge cleanup (with deferred parent decs).
        emit_edge_cleanup(
            func,
            state_map,
            pool,
            &all_borrowed_defs,
            &take_move_facts,
            &block_deferred,
            false,
        );
        trace_phase_snapshot("after_phase_2_edge_cleanup", func, interner);

        // Phase 2.1: insert RcInc for projected children that escape via
        // terminator args, where the parent aggregate was edge-dec'd by
        // Phase 2 above. Edge cleanup may have created trampoline blocks with
        // AggFields dec — these dec ALL fields including projected ones still
        // live in the successor. The RcInc compensates.
        super::project_escape::emit_project_escape_incs(
            func,
            state_map,
            pool,
            &func_project_sources,
            &all_borrowed_defs,
        );
        trace_phase_snapshot("after_phase_2_1_escape_incs", func, interner);
    }

    // Phase 2.5: DP-2/DP-3 burden-op elimination.
    eliminate_burden_ops_phase(func, state_map, interner, predicate_stack_rc_disabled);

    if predicate_stack_rc_disabled {
        emit_burden_path_probe_tail(
            func,
            state_map,
            pool,
            interner,
            &all_borrowed_defs,
            &take_move_facts,
            &block_deferred,
            &same_alloc_reps,
        );
    }

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }
    trace_phase_snapshot("after_phase_3_coalesce", func, interner);

    let rc_count = count_rc_ops(func);
    (rc_count, all_death_events, all_alloc_events, synergy)
}

/// Phase 2.5: DP-2/DP-3 burden-op elimination. Consumes post-emission IR with
/// full burden ops present; removes redundant `BurdenInc` / `BurdenDec*` sites
/// whose lattice state satisfies `is_rc_inc_elidable` / `is_rc_dec_unnecessary`.
/// Runs BEFORE Phase 3 coalesce so coalesce operates on the post-elimination IR.
///
/// Coexistence-handshake scope (CH-comp): DP-2/DP-3 elision over the burden
/// baseline is sound on the default path because the predicate stack co-emits
/// the RC the lattice proves redundant. Under the probe
/// (`predicate_stack_rc_disabled`) the burden path is the SOLE real-RC emitter —
/// the lattice "redundant" verdict assumes a co-emitter that is off, so eliding
/// a sole-emitter release leaks. Skipping elimination on the probe path at worst
/// over-retains a genuinely-redundant pair that lowers to a net-zero
/// `RcInc`/`RcDec` (harmless); the default path is unchanged.
fn eliminate_burden_ops_phase(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    interner: &ori_ir::StringInterner,
    predicate_stack_rc_disabled: bool,
) {
    if !*BURDEN_ELIM_DISABLED && !predicate_stack_rc_disabled {
        super::eliminate_burden_ops(func, state_map);
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
              context (state map, pool, interner, borrowed defs, take-move facts, \
              deferred decs, same-alloc reps) the two phases below consume; \
              bundling into a struct fragments the single probe-tail orchestration"
)]
fn emit_burden_path_probe_tail(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    all_borrowed_defs: &FxHashSet<ArcVarId>,
    take_move_facts: &crate::aims::emit_rc::take_project::TakeMoveFacts,
    block_deferred: &FxHashMap<usize, Vec<DeferredDec>>,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
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

    let elidable_fresh_incs =
        compute_elidable_fresh_self_alloc_incs(func, same_alloc_reps, interner);
    lower_burden_ops_to_rc(func, pool, &elidable_fresh_incs);
    trace_phase_snapshot("after_phase_7_burden_lowering", func, interner);
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
/// uniqueness (all dead-ends per `decisions/LEDGER.md §B.2`). Spec: Annex E
/// §AIMS RL-1 (`!incElidable`) + RL-comp net-preservation.
fn compute_elidable_fresh_self_alloc_incs(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    // Lineage reps whose value flows into a COW-mutation operand — the fresh inc
    // for any FRESH self-alloc in such a lineage is LOAD-BEARING (keep it): the
    // COW helper reads the operand's runtime refcount, and the fresh inc raises
    // it to ≥ 2 so the helper COPIES instead of mutating the shared value.
    let cow_mutated_reps = compute_cow_mutated_lineage_reps(func, same_alloc_reps, interner);
    // Per-lineage alloc-aware static net = `Σ self-alloc(+1) + Σ BurdenInc −
    // Σ BurdenDec*` over the SSA-alias lineage (M3). Counting the allocation's
    // implicit `+1` per the compiled-Lean `rcBalance` (a released FRESH value's
    // full lifecycle counting alloc nets 0). A redundant fresh-site inc shows up
    // as net == 1 (the inc is the surplus over balance); eliding it brings the
    // lineage back to 0. A net != 1 means the fresh inc is balancing a
    // COW-consume / move-alias dec (e.g. `length_one`: net 0 with all incs →
    // eliding drops to −1, a double-free) → keep.
    let lineage_net = compute_lineage_alloc_aware_net(func, same_alloc_reps);

    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);

    let mut elidable: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            let Some(dst) = fresh_self_alloc_dst(instr) else {
                continue;
            };
            // Only elide non-scalar RcPtr/FatValue self-allocs (scalars carry no
            // burden ops; aggregates may have heap children whose RC the fresh
            // inc does not double-count — restrict to single-pointer reprs the
            // allocation's own `+1` covers).
            if !matches!(
                func.var_repr(dst),
                Some(ValueRepr::RcPointer | ValueRepr::FatValue)
            ) {
                continue;
            }
            let rep = rep_of(dst);
            if cow_mutated_reps.contains(&rep) {
                continue;
            }
            // Elide ONLY when the lineage net == 1: removing exactly one fresh
            // inc restores the alloc-aware balance to 0. Any other net means the
            // fresh inc is load-bearing for a non-elision reason (move-alias dec,
            // unbalanced dup) — keep it (eliding would net −1 = double-free).
            if lineage_net.get(&rep).copied().unwrap_or(0) != 1 {
                continue;
            }
            elidable.insert(dst);
        }
    }
    elidable
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
fn compute_lineage_alloc_aware_net(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
) -> FxHashMap<ArcVarId, i64> {
    use crate::aims::verify::burden_delta::{compute_burden_entry_nets, whole_var_dec_target};
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let preds = crate::graph::compute_predecessors(func);

    // Collect every lineage rep that has at least one fresh self-alloc member —
    // the only reps an elision decision queries.
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let Some(dst) = fresh_self_alloc_dst(instr) {
                if matches!(
                    func.var_repr(dst),
                    Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                ) {
                    reps.insert(rep_of(dst));
                }
            }
        }
    }

    let mut result: FxHashMap<ArcVarId, i64> = FxHashMap::default();
    for rep in reps {
        // Per-block delta for this lineage rep: alloc(+1) for each fresh
        // self-alloc whose lineage rep == `rep`, +1 per `BurdenInc`, −1 per
        // whole-var `BurdenDec*`.
        let mut delta: Vec<i64> = vec![0; func.blocks.len()];
        for (b, block) in func.blocks.iter().enumerate() {
            for instr in &block.body {
                if let Some(dst) = fresh_self_alloc_dst(instr) {
                    if rep_of(dst) == rep
                        && matches!(
                            func.var_repr(dst),
                            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
                        )
                    {
                        delta[b] += 1;
                    }
                }
                if matches!(instr, ArcInstr::BurdenInc { var } if rep_of(*var) == rep) {
                    delta[b] += 1;
                } else if whole_var_dec_target(instr).map(rep_of) == Some(rep) {
                    delta[b] -= 1;
                }
            }
        }
        let nets = compute_burden_entry_nets(func, &preds, &delta);
        if !nets.disagree_blocks.is_empty() {
            // Merge disagreement on this lineage → conservatively NOT elidable
            // (omit from the map; the elision gate's `!= 1` check then keeps it).
            continue;
        }
        // Agreed terminal net across reachable terminal blocks.
        let mut terminal_net: Option<i64> = None;
        for (b, block) in func.blocks.iter().enumerate() {
            let Some(eb) = nets.entry_net[b] else {
                continue;
            };
            if !matches!(
                block.terminator,
                ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable
            ) {
                continue;
            }
            let exit = eb + delta[b];
            match terminal_net {
                None => terminal_net = Some(exit),
                // Divergent terminal nets across paths → not elidable.
                Some(t) if t != exit => {
                    terminal_net = None;
                    break;
                }
                Some(_) => {}
            }
        }
        if let Some(n) = terminal_net {
            result.insert(rep, n);
        }
    }
    result
}

/// The `dst` of a FRESH self-allocation instruction (created at runtime RC = 1),
/// or `None`. Mirrors the FRESH-site set `fresh_site_burden_inc_dst`
/// (`burden_lower/emit.rs`) treats as self-allocating: `Construct` / `Reuse` /
/// `CollectionReuse` / `PartialApply` / `Let { String }`. `Apply` / `Invoke`
/// results are NOT self-allocs — they inherit an owned reference from the callee
/// (the callee's allocation, not a fresh one created here), so their fresh inc
/// is the caller's genuine acquire, never the redundant alloc double-count.
#[expect(
    clippy::match_same_arms,
    reason = "the heap-ctor arm and the Let{String} arm are distinct instruction \
              shapes that happen to share a `Some(*dst)` body; merging them would \
              obscure the two FRESH-site categories the burden walk distinguishes"
)]
fn fresh_self_alloc_dst(instr: &ArcInstr) -> Option<ArcVarId> {
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
        _ => None,
    }
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
/// - an `RcPtr` arg passed to any NON-protocol-builtin `Apply` / `Invoke` (a
///   user function / stdlib method `Invoke @<name>` whose `<name>` is not
///   `__`-prefixed). Such a callee may COW-mutate a collection param
///   INTERPROCEDURALLY (e.g. `@check` doing `list.push(...)` on a borrowed
///   param), reading the CALLER's runtime refcount through the call boundary;
///   eliding the caller's fresh inc drops the value to RC = 1 at the callee's
///   COW point → mutate-in-place corrupts the caller's still-live holder (the
///   `arc_borrowed_param_cow_push_use_after` shape). The borrowed-param contract
///   does NOT surface that internal COW (the param is borrowed-readonly at the
///   ABI boundary yet the callee still reads rc), so a contract check cannot
///   prove non-COW — conservatively keep the fresh inc for any non-builtin call
///   argument. Protocol builtins (`__`-prefixed `Apply` — `__index` etc.) are
///   compiler-interceptable with known read-only ownership (`arc.md §Protocol
///   Builtins`) and stay elidable.
fn compute_cow_mutated_lineage_reps(
    func: &ArcFunction,
    same_alloc_reps: &FxHashMap<ArcVarId, ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let rep_of = |v: ArcVarId| same_alloc_reps.get(&v).copied().unwrap_or(v);
    let is_rcptr = |v: ArcVarId| {
        matches!(
            func.var_repr(v),
            Some(ValueRepr::RcPointer | ValueRepr::FatValue)
        )
    };
    // True iff a callee MAY COW/share an RcPtr arg. A callee
    // that internally COW-mutates a collection param reads the caller's runtime
    // refcount across the call boundary, so the caller's fresh inc is
    // load-bearing. Protocol builtins (`__`-prefixed `Apply` — `__index`,
    // `__iter_next`, etc.) are compiler-interceptable with known read-only
    // ownership (`arc.md §Protocol Builtins`); a fresh value flowing ONLY into
    // those is safely elidable. EVERY other call (a user function / stdlib
    // method `Invoke`/`Apply` by `Name`) is conservatively COW-risk —
    // INTERPROCEDURALLY, the callee may COW-mutate a collection param internally
    // (e.g. `@check` doing `list.push(...)` on a borrowed param) whose runtime
    // refcount is the CALLER's; the borrowed-param contract does NOT surface
    // that internal COW (the param is borrowed-readonly at the ABI boundary yet
    // the callee still reads rc), so the contract cannot prove non-COW. Keep the
    // fresh inc for any non-builtin call argument — sound over-approximation;
    // the directive's RL-1 "not elidable on a duplicating/COW use" governs.
    let callee_may_cow = |callee: Name| -> bool { !interner.lookup(callee).starts_with("__") };
    let mut reps: FxHashSet<ArcVarId> = FxHashSet::default();
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
                    reps.insert(rep_of(arg));
                }
            }
            // Interprocedural COW: an RcPtr arg to a may-COW user `Apply`.
            // `Apply.args` are positional user args (no leading closure), so the
            // used-var index IS the param index.
            if let ArcInstr::Apply {
                func: callee, args, ..
            } = instr
            {
                for &arg in args {
                    if is_rcptr(arg) && callee_may_cow(*callee) {
                        reps.insert(rep_of(arg));
                    }
                }
            }
            // Collection `+`/concat: a Binary PrimOp with an RcPtr operand.
            if let ArcInstr::Let {
                value: ArcValue::PrimOp { op, args },
                ..
            } = instr
            {
                if matches!(op, PrimOp::Binary(_)) {
                    for &arg in args {
                        if is_rcptr(arg) {
                            reps.insert(rep_of(arg));
                        }
                    }
                }
            }
        }
        // Terminator-position owned consume (Invoke/InvokeIndirect owned args).
        let term_used = block.terminator.used_vars();
        for (pos, &arg) in term_used.iter().enumerate() {
            if block.terminator.is_owned_position(pos) && is_rcptr(arg) {
                reps.insert(rep_of(arg));
            }
        }
        // Interprocedural COW at an `Invoke` terminator: RcPtr args to a may-COW
        // callee. `Invoke.args` are positional user args (param index == arg idx).
        if let ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            for &arg in args {
                if is_rcptr(arg) && callee_may_cow(*callee) {
                    reps.insert(rep_of(arg));
                }
            }
        }
    }
    reps
}

/// Phase 7 (probe): mechanically lower surviving whole-var burden ops to real
/// RC instructions.
///
/// `BurdenInc { var }` → `RcInc { var, count: 1, strategy, atomicity }` and
/// whole-var `BurdenDec { var }` → `RcDec { var, strategy, atomicity }`, with
/// the canonical `RcStrategy::from_var` (same strategy the predicate-stack
/// emitter embeds) and `atomicity = Atomic` (RL-19/20/21 thread-local dispatch
/// pending).
///
/// Field-grain `BurdenDecPartial` / `BurdenDecField` / `BurdenDecVariant` are
/// NOT rewritten — codegen already emits their per-field / per-variant drop glue
/// (`skip_fields`-aware partial drop, `SetTag` pre-drop variant walk, `Set`
/// old-value field drop); a whole-var `RcDec` would double-drop. They reach
/// codegen unchanged.
///
/// `Scalar` reprs cannot reach here — `emit_burden_ops` filters them (Spec:
/// Annex E §AIMS RE-2 / DP-1). A `Scalar` or out-of-range `var_repr` leaves the
/// burden op in place rather than synthesizing an unsound `RcDec`.
///
/// `elidable_fresh_incs` (per `compute_elidable_fresh_self_alloc_incs`): FRESH
/// self-allocation `BurdenInc` def-sites whose paired fresh inc is REDUNDANT
/// under lowering — the allocation already supplies the lineage's `+1`. The
/// FIRST `BurdenInc` encountered for such a var is left as a no-op `BurdenInc`
/// marker (codegen no-ops it) instead of lowering to `RcInc`, so the lineage's
/// alloc-aware net stays 0 (no leak). Subsequent `BurdenInc`s for the same var
/// (genuine dup-alias acquires) still lower — only the ONE redundant fresh-site
/// inc per var is elided.
///
/// Spec: Annex E §AIMS RL-comp (lowered `BurdenInc`/`BurdenDec` net-preservation).
fn lower_burden_ops_to_rc(
    func: &mut ArcFunction,
    pool: &Pool,
    elidable_fresh_incs: &FxHashSet<ArcVarId>,
) {
    let mut fresh_inc_elided: FxHashSet<ArcVarId> = FxHashSet::default();
    for block_idx in 0..func.blocks.len() {
        let body_len = func.blocks[block_idx].body.len();
        for instr_idx in 0..body_len {
            let (ArcInstr::BurdenInc { var } | ArcInstr::BurdenDec { var }) =
                func.blocks[block_idx].body[instr_idx]
            else {
                continue;
            };
            // Elide the ONE redundant fresh-site inc: the first `BurdenInc` for
            // an elidable FRESH self-alloc var stays a codegen-no-op marker
            // (allocation already supplies the `+1`). Later incs for the var are
            // genuine dup-alias acquires and lower normally.
            if matches!(
                func.blocks[block_idx].body[instr_idx],
                ArcInstr::BurdenInc { .. }
            ) && elidable_fresh_incs.contains(&var)
                && fresh_inc_elided.insert(var)
            {
                continue;
            }
            // RE-2 backstop: scalars carry no RcStrategy. emit_burden_ops never
            // emits whole-var burden ops on scalars (burden_carries_rc filter),
            // so a Scalar/absent repr here is a contract violation — leave the
            // burden op in place (codegen no-ops it) rather than emit unsound RC.
            let Some(repr) = func.var_repr(var) else {
                continue;
            };
            if matches!(repr, crate::ir::ValueRepr::Scalar) {
                continue;
            }
            let ty = func.var_type(var);
            let strategy = RcStrategy::from_var(repr, pool, ty);
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
}

/// Function-wide realization-context borrows shared by the per-block RC
/// emitters.
///
/// Converged analysis inputs that [`build_global_pin4_emits`] and
/// [`emit_block_rc`] both read to build a per-block [`BlockCtx`].
#[derive(Clone, Copy)]
struct RealizeEnv<'a> {
    state_map: &'a AimsStateMap,
    pool: &'a Pool,
    post_doms: &'a crate::graph::PostDominatorTree,
    all_borrowed_defs: &'a FxHashSet<ArcVarId>,
    project_borrowed_defs: &'a FxHashSet<ArcVarId>,
    iter_element_defs: &'a FxHashSet<ArcVarId>,
    inline_enum_projected_defs: &'a FxHashSet<ArcVarId>,
    func_project_sources: &'a FxHashMap<ArcVarId, ArcVarId>,
    take_move_facts: &'a crate::aims::emit_rc::take_project::TakeMoveFacts,
    return_transfer_params: &'a FxHashSet<ArcVarId>,
    alias_to_param: &'a FxHashMap<ArcVarId, FxHashSet<usize>>,
    return_project_inc_targets: &'a FxHashMap<ArcVarId, RcStrategy>,
    same_alloc_reps: &'a FxHashMap<ArcVarId, ArcVarId>,
    iter_fn_name: ori_ir::Name,
}

/// Build the function-level dec-emitter map: every emitting SSA-alias-class
/// member tagged with its block index. Consumed by `class_member_suppresses`
/// (with the post-dominator tree) so a class spanning blocks decs once per path.
///
/// Also returns the retained-lineage map (`lineage_roots`).
fn build_global_pin4_emits(
    func: &ArcFunction,
    env: &RealizeEnv<'_>,
) -> (
    crate::aims::emit_rc::dead_cleanup::emission_site::GlobalPin4Emits,
    FxHashMap<ArcVarId, ArcVarId>,
) {
    use crate::aims::emit_rc::dead_cleanup::emission_site::pin4_class_emits_dec_set;
    use crate::aims::emit_rc::{
        block_id, collect_borrowed_defs, collect_defined_vars, compute_child_effective_last_use,
        precompute_block_uses, BlockCtx,
    };

    let RealizeEnv {
        state_map,
        pool,
        post_doms,
        all_borrowed_defs,
        project_borrowed_defs,
        iter_element_defs,
        inline_enum_projected_defs,
        func_project_sources,
        take_move_facts,
        return_transfer_params,
        alias_to_param,
        return_project_inc_targets,
        same_alloc_reps,
        iter_fn_name,
    } = *env;
    let empty_global =
        crate::aims::emit_rc::dead_cleanup::emission_site::GlobalPin4Emits::default();
    // Empty placeholder for the ctx `lineage_roots` field during this pre-pass:
    // dec-emitter prediction (`pin4_class_emits_dec_set`) never reads it; the
    // real lineage map is built from `retained_roots` AFTER the loop.
    let empty_lineage: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    let mut retained_roots: FxHashSet<ArcVarId> = FxHashSet::default();
    let mut global = crate::aims::emit_rc::dead_cleanup::emission_site::GlobalPin4Emits::default();
    for block_idx in 0..func.blocks.len() {
        let use_info = precompute_block_uses(&func.blocks[block_idx]);
        let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
        let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx], func, pool);
        let child_elu = compute_child_effective_last_use(
            &func.blocks[block_idx],
            &use_info,
            func_project_sources,
        );
        let is_unwind = matches!(
            func.blocks[block_idx].terminator,
            crate::ir::ArcTerminator::Resume
        );
        let ctx = BlockCtx {
            func,
            blk: block_id(block_idx),
            state_map,
            defined_in_block: &defined_in_block,
            borrowed_defs: &borrowed_defs,
            all_borrowed_defs,
            project_borrowed_defs,
            iter_element_defs,
            inline_enum_projected_defs,
            use_info: &use_info,
            pool,
            child_effective_last_use: &child_elu,
            take_move_facts,
            return_transfer_params,
            global_pin4_emits: &empty_global,
            lineage_roots: &empty_lineage,
            post_doms,
            alias_to_param,
            return_project_inc_targets,
            same_alloc_reps,
        };
        for (class_id, members) in pin4_class_emits_dec_set(&ctx, is_unwind) {
            let entry = global.entry(class_id).or_default();
            for (var, site) in members {
                entry.insert((var, block_idx, site));
            }
        }
        // Same per-block ctx feeds the pre-walk retained-copy-root prediction.
        super::walk::predict_retained_roots(&ctx, iter_fn_name, &mut retained_roots);
    }
    let mut lineage_roots =
        super::walk::build_lineage_map(func, &retained_roots, state_map.apply_result_aliases());

    // Keep a retained lineage ONLY when its reference dies within the class
    // (some member is a predicted dec emitter in `global`). A retained alias that
    // transfers out (Construct / owned-arg / Jump-arg — RL-2 suppresses its dec
    // prediction, so it is absent from `global`) is balanced by the enclosing
    // value's drop and needs no class dec; keeping it would over-split the class
    // into a spurious lineage and double-free. Filtering to within-class-dying
    // lineages nets exactly `1 + (retained dying in class)` decs per path.
    let emitter_vars: FxHashSet<ArcVarId> = global
        .values()
        .flat_map(|s| s.iter().map(|&(v, _, _)| v))
        .collect();
    let mut root_dies_in_class: FxHashMap<ArcVarId, bool> = FxHashMap::default();
    for (&v, &root) in &lineage_roots {
        let dies = emitter_vars.contains(&v);
        let e = root_dies_in_class.entry(root).or_insert(false);
        *e = *e || dies;
    }
    lineage_roots.retain(|_v, root| root_dies_in_class.get(&*root).copied().unwrap_or(false));
    (global, lineage_roots)
}

/// Emit RC operations for a single block via the unified forward walk.
///
/// Returns `(death_events, alloc_events, walk_metrics)`.
fn emit_block_rc(
    func: &mut ArcFunction,
    block_idx: usize,
    env: &RealizeEnv<'_>,
    global_pin4_emits: &crate::aims::emit_rc::dead_cleanup::emission_site::GlobalPin4Emits,
    lineage_roots: &FxHashMap<ArcVarId, ArcVarId>,
    predecessors: &[Vec<usize>],
    block_deferred: &mut FxHashMap<usize, Vec<DeferredDec>>,
) -> (Vec<DeathEvent>, Vec<AllocEvent>, metrics::SynergyMetrics) {
    use crate::aims::emit_rc::{
        block_id, collect_borrowed_defs, collect_defined_vars, compute_child_effective_last_use,
        emit_dead_at_entry_decs, emit_terminator_rc, precompute_block_uses, BlockCtx,
    };

    let RealizeEnv {
        state_map,
        pool,
        post_doms,
        all_borrowed_defs,
        project_borrowed_defs,
        iter_element_defs,
        inline_enum_projected_defs,
        func_project_sources,
        take_move_facts,
        return_transfer_params,
        alias_to_param,
        return_project_inc_targets,
        same_alloc_reps,
        iter_fn_name,
    } = *env;

    let blk = block_id(block_idx);
    let use_info = precompute_block_uses(&func.blocks[block_idx]);
    let defined_in_block = collect_defined_vars(&func.blocks[block_idx]);
    let borrowed_defs = collect_borrowed_defs(&func.blocks[block_idx], func, pool);
    let child_elu =
        compute_child_effective_last_use(&func.blocks[block_idx], &use_info, func_project_sources);

    let old_body = std::mem::take(&mut func.blocks[block_idx].body);
    let mut new_body: Vec<ArcInstr> = Vec::with_capacity(old_body.len() * 2);

    let ctx = BlockCtx {
        func,
        blk,
        state_map,
        defined_in_block: &defined_in_block,
        borrowed_defs: &borrowed_defs,
        all_borrowed_defs,
        project_borrowed_defs,
        iter_element_defs,
        inline_enum_projected_defs,
        use_info: &use_info,
        pool,
        child_effective_last_use: &child_elu,
        take_move_facts,
        return_transfer_params,
        global_pin4_emits,
        lineage_roots,
        post_doms,
        alias_to_param,
        return_project_inc_targets,
        same_alloc_reps,
    };

    let (deferred_parents, merge_edge_decs) = emit_dead_at_entry_decs(&ctx, &mut new_body);

    let walk::BodyWalkResult {
        terminator_deferred,
        death_events,
        alloc_events,
        walk_metrics,
    } = walk::walk_body_unified(
        &ctx,
        &old_body,
        &mut new_body,
        iter_fn_name,
        deferred_parents,
    );

    emit_terminator_rc(&ctx, block_idx, &mut new_body);

    let edge_deferred = match &func.blocks[block_idx].terminator {
        ArcTerminator::Return { .. } | ArcTerminator::Resume | ArcTerminator::Unreachable => {
            for &(var, strategy) in &terminator_deferred {
                new_body.push(ArcInstr::RcDec {
                    var,
                    strategy,
                    atomicity: RcAtomicity::default_atomic(),
                });
            }
            Vec::new()
        }
        _ => terminator_deferred,
    };

    func.blocks[block_idx].body = new_body;
    if !edge_deferred.is_empty() {
        let tagged: Vec<_> = edge_deferred
            .into_iter()
            .map(|(var, strat)| (None, var, strat))
            .collect();
        block_deferred.insert(block_idx, tagged);
    }
    route_merge_edge_decs(
        func,
        block_idx,
        &merge_edge_decs,
        predecessors,
        block_deferred,
    );

    (death_events, alloc_events, walk_metrics)
}

/// Route merge-edge decs to per-predecessor edge cleanup.
///
/// Each predecessor that DEFINES the variable gets the dec on its edge
/// to the merge block ONLY (not all outgoing edges). This preserves
/// successor identity so edge cleanup doesn't fire on unrelated edges.
///
/// Take-project alias-class members never reach this routing: the
/// `dead_cleanup.rs` `is_in_class` checks skip them entirely (their
/// natural scope-exit drops in non-projecting predecessors handle the
/// cleanup, and `is_ownership_transfer` at the take-project `Project`
/// site suppresses the source's last-use drop).
fn route_merge_edge_decs(
    func: &ArcFunction,
    block_idx: usize,
    merge_edge_decs: &[(ArcVarId, RcStrategy)],
    predecessors: &[Vec<usize>],
    block_deferred: &mut FxHashMap<usize, Vec<DeferredDec>>,
) {
    if merge_edge_decs.is_empty() {
        return;
    }
    let preds = &predecessors[block_idx];
    for &(var, strategy) in merge_edge_decs {
        for &pred_idx in preds {
            if func.blocks[pred_idx].defines_var(var) {
                block_deferred
                    .entry(pred_idx)
                    .or_default()
                    .push((Some(block_idx), var, strategy));
            }
        }
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

/// Per-function return-transfer setup — transfer-param set, alias→param map, and
/// Project-Inc targets. Built once in `emit_rc_unified`, threaded into the
/// per-block walk.
struct ReturnTransferSetup {
    return_transfer_params: FxHashSet<ArcVarId>,
    alias_to_param: FxHashMap<ArcVarId, FxHashSet<usize>>,
    return_project_inc_targets: FxHashMap<ArcVarId, RcStrategy>,
}

/// Pre-compute the return-transfer surface from the function's
/// `MemoryContract`. Empty when no contract is available — equivalent to the
/// pre-fix behavior.
fn build_return_transfer_setup(
    func: &ArcFunction,
    pool: &Pool,
    contracts: &FxHashMap<Name, MemoryContract>,
) -> ReturnTransferSetup {
    let return_transfer_params: FxHashSet<ArcVarId> = contracts
        .get(&func.name)
        .map(|c| {
            func.params
                .iter()
                .enumerate()
                .filter(|(i, _)| c.params.get(*i).is_some_and(|p| p.transfers_through_return))
                .map(|(_, param)| param.var)
                .collect()
        })
        // INVARIANT: a missing contract is a legitimate absence (not an error)
        // — a function without a MemoryContract has no return-transfer params,
        // so the empty set is the correct realization, not a lossy fallback.
        .unwrap_or_default();

    let alias_to_param: FxHashMap<ArcVarId, FxHashSet<usize>> = if return_transfer_params.is_empty()
    {
        FxHashMap::default()
    } else {
        let param_vars: FxHashMap<ArcVarId, usize> = func
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| (p.var, i))
            .collect();
        crate::aims::interprocedural::build_alias_to_param_map(func, &param_vars, Some(contracts))
    };

    let return_project_inc_targets: FxHashMap<ArcVarId, RcStrategy> = contracts
        .get(&func.name)
        .map(|c| build_return_project_inc_targets(func, c, pool))
        // INVARIANT: missing contract is legitimate absence (not an error) —
        // no contract means no Project-Inc targets, so the empty map is the
        // correct realization, not a lossy fallback.
        .unwrap_or_default();

    ReturnTransferSetup {
        return_transfer_params,
        alias_to_param,
        return_project_inc_targets,
    }
}

/// Precompute the per-Project compensating-Inc target map for return-transfer.
///
/// Identifies every `Project { dst, value, field }` whose `dst` flows to a
/// `Return` AND whose `value` resolves (via Let-alias chain) to a param `p`
/// with `ParamContract.return_alias == Some(Project { field: F })`, `F == field`.
/// Fires regardless of `p`'s access class — the Inc compensates for the
/// `AggFields` walk at whichever scope holds the parent allocation on return
/// (callee Owned scope-exit drop, or caller Owned arg drop after the Apply).
///
/// Each `dst` maps to its `RcStrategy`; the realize walk emits `RcInc dst` right
/// after the Project. Without it, the `[AggFields]` field-walk decrements the
/// projected allocation to 0 before the consumer of `dst` reads it — UAF.
///
/// Empty when the contract has no Project `return_alias` entries — skips the
/// Project-instruction scan in the common case.
#[expect(clippy::too_many_lines, reason = "pre-existing")]
fn build_return_project_inc_targets(
    func: &ArcFunction,
    contract: &MemoryContract,
    pool: &Pool,
) -> FxHashMap<ArcVarId, RcStrategy> {
    use crate::aims::emit_rc::rc_strategy;

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

    // Path 1: contract-driven (existing behavior). Direct callers with
    // Owned param + apply_aliases recognition → caller suppresses scope-
    // exit dec; callee F-prj fires the compensating Inc on the Project's
    // dst to balance the param's `[AggFields]` field-walk dec.
    let project_return_params: FxHashMap<ArcVarId, u32> = func
        .params
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let pc = contract.params.get(i)?;
            match pc.return_alias? {
                ReturnAliasShape::Project { field } => Some((p.var, field)),
                ReturnAliasShape::Direct => None,
            }
        })
        .collect();

    let return_values_literal: FxHashSet<ArcVarId> = func
        .blocks
        .iter()
        .filter_map(|b| match &b.terminator {
            ArcTerminator::Return { value } => Some(*value),
            _ => None,
        })
        .collect();

    let mut result: FxHashMap<ArcVarId, RcStrategy> = FxHashMap::default();

    // Path 1 emission.
    if !project_return_params.is_empty() {
        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Project {
                    dst, value, field, ..
                } = instr
                {
                    if !return_values_literal.contains(dst) {
                        continue;
                    }
                    let root = resolve_root(*value);
                    if project_return_params.get(&root) == Some(field) {
                        if let Some(strategy) = rc_strategy(func, *dst, pool) {
                            result.insert(*dst, strategy);
                        }
                    }
                }
            }
        }
    }

    // closure_env_alias Path 2: closure-body return-projection. When the
    // function has a BORROWED param (closure body's env exposed as a
    // borrow) AND the body returns a Project of that param via a Let/Jump
    // chain, the caller's ApplyIndirect treats the result as Owned per
    // TF-5a's CONSERVATIVE classification (no contract). The borrow needs
    // a compensating Inc to convert it to Owned at the return point;
    // without this, caller's dec on the borrow → double-free.
    //
    // Restricted to Borrow params to avoid regression on Owned-param
    // direct-call cases (Path 1 + apply_aliases handles those). Builds
    // an expanded `return_values` set via inverse Let-alias chain AND
    // Jump-arg → block-param edges (closure match-arm dispatch).
    let has_borrow_param = func.params.iter().enumerate().any(|(i, _p)| {
        contract
            .params
            .get(i)
            .is_some_and(|pc| matches!(pc.access, crate::aims::lattice::AccessClass::Borrowed))
    });
    if has_borrow_param {
        let mut return_values_chain: FxHashSet<ArcVarId> = return_values_literal.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for block in &func.blocks {
                for instr in &block.body {
                    if let ArcInstr::Let {
                        dst,
                        value: crate::ir::ArcValue::Var(src),
                        ..
                    } = instr
                    {
                        if return_values_chain.contains(dst) && !return_values_chain.contains(src) {
                            return_values_chain.insert(*src);
                            changed = true;
                        }
                    }
                }
                if let ArcTerminator::Jump { target, args } = &block.terminator {
                    let target_block = &func.blocks[target.index()];
                    for (arg, (param_var, _ty)) in args.iter().zip(target_block.params.iter()) {
                        if return_values_chain.contains(param_var)
                            && !return_values_chain.contains(arg)
                        {
                            return_values_chain.insert(*arg);
                            changed = true;
                        }
                    }
                }
            }
        }

        let borrow_param_vars: FxHashSet<ArcVarId> = func
            .params
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let pc = contract.params.get(i)?;
                if matches!(pc.access, crate::aims::lattice::AccessClass::Borrowed) {
                    Some(p.var)
                } else {
                    None
                }
            })
            .collect();

        for block in &func.blocks {
            for instr in &block.body {
                if let ArcInstr::Project { dst, value, .. } = instr {
                    if !return_values_chain.contains(dst) {
                        continue;
                    }
                    if result.contains_key(dst) {
                        continue;
                    }
                    let root = resolve_root(*value);
                    if borrow_param_vars.contains(&root) {
                        if let Some(strategy) = rc_strategy(func, *dst, pool) {
                            result.insert(*dst, strategy);
                        }
                    }
                }
            }
        }
    }

    result
}
