//! Unified RC emission: per-block walk with inline death/alloc event collection.
//!
//! Extracted from `realize/mod.rs` to stay under the 500-line file limit.
//! Called by [`super::realize_rc_reuse()`] as Phase 1 sub-step B.

use std::sync::LazyLock;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{MemoryContract, ReturnAliasShape};
use crate::aims::emit_rc::DeferredDec;
use crate::aims::emit_reuse::{AllocEvent, DeathEvent};
use crate::aims::intraprocedural::apply_aliases::build_let_alias_map;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, RcAtomicity, RcStrategy};

use super::metrics;
use super::walk;

/// `ORI_DISABLE_BURDEN_ELIM=1` bypasses Phase 2.5 burden-op elimination. Read
/// once at first access; permanent isolation harness letting Phase 5 emission
/// be evaluated alone (emission on, elimination off) for diagnostic bisection.
static BURDEN_ELIM_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_BURDEN_ELIM").as_deref() == Ok("1"));

/// Per-phase RC-op snapshot for post-walk pass debugging.
///
/// Emits one `tracing::trace!` event per block summarising every
/// `RcInc`/`RcDec` instruction by `ArcVarId`. Gated behind
/// `tracing::enabled!` so the iteration is skipped entirely when
/// `ori_arc::aims::realize` is not at trace level — zero overhead in
/// normal debug runs.
///
/// Activate with `ORI_LOG=ori_arc::aims::realize=trace`. Used to
/// bisect which post-walk pass (`emit_dead_invoke_dsts`,
/// `emit_edge_cleanup`, `emit_project_escape_incs`, `coalesce_block_rc`)
/// modifies a specific block's RC ops without inline `tracing::debug!`
/// insertions. § Debugging.
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
/// Replaces `emit_rc_ops()` with a forward walk that routes all decisions
/// through `decide()` and collects reuse events inline, eliminating the
/// separate `collect_death_events()` / `collect_alloc_events()` scans.
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
    // Per-class take-project facts via union-find +
    // CFG reachability. Precomputed once per function. Each
    // take-project source seeds its own connected-component class
    // (Let-alias + Jump-arg → block-param edges); each class has
    // its own bypass-safe blocks and bypass-safe entries. Consumers
    // (`dead_cleanup` source 1, `dead_cleanup` source 2,
    // `edge_cleanup`) query via `is_in_class`, `class_of`, and
    // `is_bypass_safe_entry_for_var` to coordinate exactly-one drop
    // per CFG path without double-free or leak.
    let take_move_facts = crate::aims::emit_rc::take_project::analyze(func, pool);
    // Same-allocation union-find reps (Let{Var} + apply Direct/Conditional;
    // EXCLUDES Jump-arg phi). Precomputed once per function for the per-block
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

    // Function-level cross-block dec-emitter map + post-dominator tree. The
    // same-block / same-instruction dec-suppression gates suppress a class
    // member's dec only when ANOTHER member covers its RC slot on every path
    // (`class_member_suppresses`). Post-dominance — not raw block order — gates
    // cross-block suppression so a branch (neither arm post-dominates) keeps one
    // dec per path (under-emission leak otherwise).
    let post_doms = crate::graph::PostDominatorTree::build(func);
    // `build_global_pin4_emits` also returns the retained-lineage map, filtered
    // to lineages that die within their SSA-alias class. A within-class retained
    // copy that transfers out (Construct / owned-arg / Jump-arg / Return) is
    // balanced by the enclosing value's drop, so it is dropped from the map and
    // its class dedups normally; a copy that dies in-class keeps its own dec so
    // the class nets `1 + N` decs per path (rc_per_path_invariant).
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

    // Phase 2.5: DP-2/DP-3 burden-op elimination.
    // Consumes post-emission IR with full burden ops present; removes
    // redundant `BurdenInc` / `BurdenDec*` sites whose lattice state
    // satisfies `is_rc_inc_elidable` / `is_rc_dec_unnecessary`. Runs
    // BEFORE Phase 3 coalesce so coalesce operates on the post-elimination
    // IR with redundant ops already removed.
    if !*BURDEN_ELIM_DISABLED {
        super::eliminate_burden_ops(func, state_map);
    }
    trace_phase_snapshot("after_phase_2_5_burden_elim", func, interner);

    // Phase 3: RC coalescing peephole — merge adjacent RC ops per block.
    for block in &mut func.blocks {
        coalesce_block_rc(&mut block.body);
    }
    trace_phase_snapshot("after_phase_3_coalesce", func, interner);

    let rc_count = count_rc_ops(func);
    (rc_count, all_death_events, all_alloc_events, synergy)
}

/// Function-wide realization-context borrows shared by the per-block RC
/// emitters.
///
/// Bundles the converged analysis inputs that [`build_global_pin4_emits`]
/// and [`emit_block_rc`] both read to build a [`BlockCtx`] per block: the
/// state map, the pool, the borrowed/iter/inline-enum/project def sets, the
/// take-project facts, the return-transfer sets, the alias/inc-target maps,
/// the same-alloc reps, the post-dominator tree, and the interned `iter`
/// name. Every field is a shared borrow read together by the per-block walk.
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

/// Emit RC operations for a single block via the unified forward walk.
///
/// Returns `(death_events, alloc_events, walk_metrics)`.
/// Build the function-level dec-emitter map: every emitting SSA-alias-class
/// member tagged with its block index. Consumed by `class_member_suppresses`
/// (with the post-dominator tree) so a class spanning blocks decs once per path.
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
    // Consumption-aware filter — keep a retained lineage ONLY when its reference
    // DIES within the class (some lineage member is a predicted dec emitter in
    // `global`). A retained alias that later TRANSFERS OUT (consumed by a
    // Construct / owned-arg / Jump-arg — RL-2 ownership transfer suppresses its
    // dec prediction, so it is ABSENT from `global`) is balanced by the
    // enclosing value's drop and needs NO class dec; keeping it would over-split
    // the class into a spurious extra lineage and double-free (the broad-split
    // 28 -> 1950 over-application). Filtering to within-class-dying lineages
    // leaves only genuine distinct owned references, so the per-lineage dedup
    // emits exactly `1 + (retained that die in class)` per path — the correct
    // count (string_sso's b/c die at the comparisons; a transferred copy drops).
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

/// Precompute the per-Project compensating-Inc target map for return-transfer.
///
/// Identifies every `Project { dst, value, field }` instruction whose `dst`
/// flows to a `Return` terminator AND whose `value` resolves (via Let-alias
/// chain) to a function param `p` whose `ParamContract.return_alias` is
/// `Some(Project { field: F })` with `F == field`. Fires regardless of `p`'s
/// own access class — the Inc compensates for the `AggFields` walk that fires
/// at whichever scope holds the parent allocation when the call returns,
/// callee-side (Owned-callee scope-exit drop) or caller-side (Owned-caller
/// arg drop after the Apply).
///
/// Per-function setup bundle for return-transfer / alias / Project-Inc state.
/// Built once in `emit_rc_unified` and threaded into the per-block walk.
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
        .unwrap_or_default();

    ReturnTransferSetup {
        return_transfer_params,
        alias_to_param,
        return_project_inc_targets,
    }
}

/// Each such `dst` maps to its `RcStrategy`. The realize walk consumes this
/// map to emit `RcInc dst [strategy]` immediately after the Project. Without
/// this Inc, the field-walk inside `[AggFields]` decrements the projected
/// allocation to 0 BEFORE the consumer of `dst` (Return → caller's xs) reads
/// it — use-after-free.
///
/// Returns an empty map when the contract has no Project `return_alias` entries
/// — bypasses Project-instruction iteration entirely in the common case.
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
