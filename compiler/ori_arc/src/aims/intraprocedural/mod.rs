//! Intraprocedural backward dataflow analysis for AIMS.
//!
//! Computes [`AimsState`](super::lattice::AimsState) for every variable at
//! every block boundary within a single function. The analysis direction is
//! BACKWARD: we discover how each value WILL be used (future demand) to
//! decide what RC operations to emit.
//!
//! # Entry point
//!
//! [`analyze_function`] runs the backward dataflow to fixed-point convergence,
//! returning an [`AimsStateMap`] that downstream passes (RC emission, reuse
//! emission, COW annotation) consume.
//!
//! # Module structure
//!
//! - [`state_map`] — [`AimsStateMap`] data structure + sparse events
//! - [`block`] — per-block backward analysis (instructions, terminators,
//!   control flow merge via `alt_join`, pattern match via `Project` transfer)
//!
//! # References
//!
//! - GHC demand analysis backward pass (`compiler/GHC/Core/Opt/DmdAnal.hs`)
//! - Lean 4 RC insertion (`src/Lean/Compiler/IR/RC.lean`)
//! - `ori_arc` liveness (`compiler/ori_arc/src/liveness/mod.rs`)

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

pub mod block;
pub mod state_map;

pub use state_map::{AimsEvent, AimsStateMap, InvokeEdgeState};

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};
use crate::ArcClassification;

use super::contract::{ContextRegion, MemoryContract};
use super::lattice::{AimsState, Consumption, Locality, ReuseCtorKind, ShapeClass, Uniqueness};
use super::transfer::transfer_def;

/// Run backward dataflow analysis on a single function.
///
/// Iterates blocks in postorder (successors before predecessors in the
/// backward direction) until fixed-point convergence. Returns the
/// converged [`AimsStateMap`] with per-block entry/exit states.
///
/// # Parameters
///
/// - `func` — the ARC IR function to analyze
/// - `classifier` — type classification (scalar filtering)
/// - `sigs` — interprocedural contracts (from Section 03; empty in Stage 1)
/// - `context_regions` — TRMC metadata (from Stage 3; empty in Stage 1)
///
/// # Convergence
///
/// The lattice has finite chain height 15. Convergence is guaranteed in
/// at most `15 × |variables| × |blocks|` iterations. If exceeded, a
/// `tracing::warn!` is emitted and remaining variables are widened to TOP.
///
/// This is mathematically stronger than GHC's demand analysis, which uses an
/// empirical `n > 10` iteration cutoff in `dmdFix` with `reuseEnv` demand
/// stabilization for recursive bindings. AIMS derives its bound from the
/// product lattice's finite chain height (15 = sum of per-dimension heights),
/// giving a provable upper bound rather than an empirical safety net. GHC
/// needs `reuseEnv` and weak-demand splitting to improve convergence in lazy
/// contexts; AIMS's strict evaluation model avoids these because all demands
/// are strict by definition.
/// (See: Literature Review §09 — GHC Demand Analysis)
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the project-wide hasher; no generic needed"
)]
pub fn analyze_function(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, MemoryContract>,
    // Reserved for Stage 3 (TRMC context regions). Empty slice in Stages 1–2.
    _context_regions: &[ContextRegion],
    immortals: Vec<bool>,
) -> AimsStateMap {
    let mut state_map = AimsStateMap::new(func);

    // Set immortal variables — excluded from analysis and emission.
    state_map.set_immortals(immortals);

    // Mark scalar variables — excluded from analysis entirely.
    for (var_idx, &ty) in func.var_types.iter().enumerate() {
        if classifier.is_scalar(ty) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR var counts fit in u32"
            )]
            state_map.set_permanent_scalar(ArcVarId::new(var_idx as u32));
        }
    }

    // Compute postorder for backward traversal: successors appear before
    // predecessors, so demand from successors is available when we compute
    // a block's exit/entry state.
    let postorder = crate::graph::compute_postorder(func);

    // Collect invoke definitions: Invoke { dst, normal, .. } defines `dst`
    // at the entry of the `normal` successor only (not unwind). We need to
    // remove these from the normal successor's entry state.
    let invoke_defs = crate::graph::collect_invoke_defs(func);

    let iteration_limit = AimsState::iteration_limit(func.var_types.len(), func.blocks.len());
    let mut iteration = 0;

    loop {
        state_map.reset_changed();
        iteration += 1;

        // Process blocks in postorder (successors first for backward analysis).
        for &block_idx in &postorder {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "ARC IR block counts fit in u32"
            )]
            let block_id = ArcBlockId::new(block_idx as u32);

            // Compute the block's exit state from successor entry states.
            let exit_state = block::compute_block_exit_state(func, block_id, &state_map);
            state_map.update_block_exit(block_id, exit_state);

            // Record per-edge demand for Invoke terminators: normal
            // and unwind successors carry different variable sets.
            let block = &func.blocks[block_idx];
            if let ArcTerminator::Invoke { normal, unwind, .. } = &block.terminator {
                let normal_entry = state_map
                    .block_entry_states(*normal)
                    .cloned()
                    .unwrap_or_default();
                let unwind_entry = state_map
                    .block_entry_states(*unwind)
                    .cloned()
                    .unwrap_or_default();
                state_map.set_invoke_edge_state(
                    block_id,
                    InvokeEdgeState {
                        normal: normal_entry,
                        unwind: unwind_entry,
                    },
                );
            }

            // Compute the block's entry state by walking instructions backward.
            // Also accumulates block-level effects (Section 09.2).
            let result =
                block::compute_block_entry_state(func, block_id, &state_map, sigs, &invoke_defs);
            state_map.accumulate_effect(result.effects);
            state_map.update_block_entry(block_id, result.entry_state);
        }

        if state_map.is_converged() {
            break;
        }

        // Non-convergence safety net.
        if iteration >= iteration_limit {
            tracing::warn!(
                func = ?func.name,
                iterations = iteration,
                limit = iteration_limit,
                "AIMS analysis did not converge within bound — widening to TOP. \
                 This indicates a bug in transfer functions."
            );
            widen_to_top(&mut state_map, func);
            break;
        }
    }

    // Post-convergence: verify canonical fixed point and detect
    // cross-dimension chaining (Section 09.5 Convergence Feedback).
    verify_canonical_fixed_point(&mut state_map, func);

    tracing::debug!(
        func = ?func.name,
        iterations = iteration,
        blocks = func.blocks.len(),
        vars = func.var_types.len(),
        cross_dimension = state_map.cross_dimension_detected(),
        "AIMS intraprocedural analysis converged"
    );

    // Post-convergence: populate borrow source side table.
    // In SSA form each variable has exactly one definition, so the borrow
    // source is per-variable (not per-point). Walk all instructions and
    // record BorrowSource for Project (and any future borrow-producing
    // instructions).
    populate_borrow_sources(&mut state_map, func);

    // Post-convergence: populate sparse event table with reusable allocation
    // candidates and local-allocation eligibility. These are derived from the
    // converged state and instruction types, making the state map a
    // self-contained fact source for downstream passes.
    populate_sparse_events(&mut state_map, func);

    // Effect summary is now accumulated during analysis (Section 09.2),
    // not post-convergence. Block-level effects are OR'd into the state map
    // in the convergence loop above via `result.effects`.

    // Post-convergence: populate per-variable shape from definitions.
    // Shape is a definition-site property that doesn't propagate through
    // backward demand analysis. This side table makes shape available at
    // all program points for reuse detection, COW, and FIP.
    // Section 09.2 Shape Activation.
    populate_var_shapes(&mut state_map, func);

    // Post-convergence: detect TRMC candidates and set ContextHole shape.
    // Must run after populate_var_shapes (which sets initial shape) and
    // after convergence (needs final uniqueness/locality/effect data).
    // Section 09.2 Shape Activation — ContextHole detection.
    detect_trmc_candidates(&mut state_map, func);

    // Post-convergence: compute FIP token balance from converged state.
    // Counts Construct allocations vs consumed reusable-shaped deaths.
    // Section 09.2 Effect Activation.
    populate_fip_balance(&mut state_map, func);

    state_map
}

/// Populate the borrow source side table after analysis converges.
///
/// Each variable defined by a `Project` instruction gets
/// `BorrowSource::exact_field(source_var, field)`. Block params that receive values
/// from multiple predecessors are left without a source (implicitly
/// `Unknown` if queried). In ARC IR's SSA form, each variable has exactly
/// one definition point, so this is per-variable, not per-point.
fn populate_borrow_sources(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for block in &func.blocks {
        for instr in &block.body {
            // Use the converged state to compute the transfer result.
            let get_state = |v: ArcVarId| -> AimsState {
                if state_map.is_scalar(v) {
                    return AimsState::SCALAR;
                }
                state_map.var_state_at_block_entry(block.id, v)
            };

            if let Some(def) = transfer_def(instr, &get_state) {
                if let (Some(dst), Some(source)) = (instr.defined_var(), def.borrow_source) {
                    state_map.set_borrow_source(dst, source);
                }
            }
        }
    }
}

/// Populate the sparse event table after analysis converges.
///
/// Records two categories of events:
///
/// 1. **Reusable allocation candidates** (`AimsEvent::ReusableAllocation`):
///    `Construct` instructions with reusable constructor kinds (`Struct`,
///    `EnumVariant`) on non-scalar destinations. These mark allocation sites
///    that the `ReusePlanner` can match against death events for cross-block
///    reuse.
///
/// 2. **Local-allocation eligibility** (`AimsEvent::LocalAllocCandidate`):
///    Variables whose converged exit state shows `Locality::FunctionLocal` or
///    `BlockLocal`, indicating they never escape the function and may be
///    eligible for stack allocation in a future optimization pass.
fn populate_sparse_events(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for (instr_idx, instr) in block.body.iter().enumerate() {
            // Reusable allocation candidates: Construct with reusable ctor.
            if let ArcInstr::Construct { dst, ctor, .. } = instr {
                if !state_map.is_scalar(*dst)
                    && matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
                {
                    state_map.record_event(AimsEvent::ReusableAllocation {
                        block: blk,
                        instr: instr_idx,
                        var: *dst,
                    });
                }
            }

            // Local-allocation eligibility: variables with local exit state.
            if let Some(dst) = instr.defined_var() {
                if state_map.is_scalar(dst) {
                    continue;
                }
                let exit_state = state_map.var_state_at_block_exit(blk, dst);
                if matches!(
                    exit_state.locality,
                    Locality::FunctionLocal | Locality::BlockLocal
                ) {
                    state_map.record_event(AimsEvent::LocalAllocCandidate {
                        block: blk,
                        instr: instr_idx,
                        var: dst,
                    });
                }
            }
        }
    }
}

/// Populate the per-variable shape map from definition instructions.
///
/// Shape is a property of how a value was produced (its definition instruction),
/// not of backward demand. The backward analysis only carries shape within a
/// block's backward walk (set at `Construct`, killed at definition removal).
/// Cross-block, shape is lost because `add_backward_demand` initializes from
/// `BOTTOM` (which has `NonReusable` shape).
///
/// This post-convergence step makes shape available everywhere via
/// [`AimsStateMap::var_shape`], enabling:
/// - Death event filtering in reuse detection
/// - Cross-dimensional uniqueness proof (Once+ReusableCtor → static reuse)
/// - COW annotation for `CollectionBuffer` non-parameters
/// - TRMC candidate detection (`ContextHole`)
///
/// Section 09.2 Shape Activation.
fn populate_var_shapes(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for block in &func.blocks {
        for instr in &block.body {
            let Some(dst) = instr.defined_var() else {
                continue;
            };
            if state_map.is_excluded(dst) {
                continue;
            }
            let shape = match instr {
                ArcInstr::Construct { ctor, .. } | ArcInstr::Reuse { ctor, .. } => match ctor {
                    CtorKind::Struct(_) => ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                    CtorKind::EnumVariant { .. } => {
                        ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant)
                    }
                    CtorKind::ListLiteral | CtorKind::SetLiteral | CtorKind::MapLiteral => {
                        ShapeClass::CollectionBuffer
                    }
                    CtorKind::Tuple | CtorKind::Closure { .. } => ShapeClass::NonReusable,
                },
                ArcInstr::CollectionReuse { .. } => ShapeClass::CollectionBuffer,
                _ => ShapeClass::NonReusable,
            };
            state_map.set_var_shape(dst, shape);
        }
    }
}

/// Detect TRMC (Tail Recursive Modulo Constructor) candidates.
///
/// A `Construct` instruction is a TRMC candidate when:
/// 1. It has `ReusableCtor` shape (struct or enum variant)
/// 2. At least one of its field arguments was defined by a recursive call
///    (`Apply` or `Invoke` where callee == current function)
/// 3. **Soundness** (Lemma 2, Leijen & Lorenzen JFP 2025):
///    The constructor destination is `Unique` — no other references exist
///    at the mutation point, so in-place hole fill is safe.
///
/// Locality is deliberately NOT checked: TRMC constructors are typically
/// returned (`HeapEscaping`), which is expected — the whole point is
/// building the result in place and returning it.
///
/// Function-level `may_share` is NOT checked: the `HeapEscaping → may_share`
/// accumulation rule makes ANY returned Construct trigger `may_share`, which
/// would block all TRMC detection. The per-variable `Unique` guarantee is
/// the actual soundness condition (refcount == 1 at the mutation point).
///
/// When all conditions hold, the constructor's shape is upgraded to
/// `ContextHole`, enabling Stage 3 TRMC normalization to rewrite the
/// recursive call into an in-place fill of the constructor's hole.
///
/// Section 09.2 Shape Activation — `ContextHole` detection.
fn detect_trmc_candidates(state_map: &mut AimsStateMap, func: &ArcFunction) {
    // Collect variables defined by recursive calls (callee == func.name).
    let recursive_defs = collect_recursive_call_defs(func);
    if recursive_defs.is_empty() {
        return;
    }

    // Scan for Construct instructions with a recursive-call argument.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        for instr in &block.body {
            let ArcInstr::Construct {
                dst, ctor, args, ..
            } = instr
            else {
                continue;
            };

            // Only struct/enum constructors are TRMC candidates.
            if !matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. }) {
                continue;
            }

            if state_map.is_excluded(*dst) {
                continue;
            }

            // Check if any field argument was produced by a recursive call.
            let has_recursive_arg = args.iter().any(|arg| recursive_defs.contains(arg));
            if !has_recursive_arg {
                continue;
            }

            // Soundness (Lemma 2, Leijen & Lorenzen JFP 2025):
            // The constructor must be uniquely owned at the mutation point.
            // This ensures the in-place hole fill doesn't corrupt other viewers.
            //
            // Locality is NOT checked: TRMC constructors are typically returned
            // (HeapEscaping), which is expected — the whole point of TRMC is to
            // build the result in place and return it.
            //
            // Function-level may_share is NOT checked: it's too conservative
            // (any returned Construct triggers may_share via HeapEscaping → may_share
            // rule). The per-variable Unique guarantee is the actual soundness
            // condition — refcount == 1 at the mutation point.
            let state = state_map.var_state_at_block_exit(blk, *dst);
            if state.uniqueness != Uniqueness::Unique {
                continue;
            }

            // All conditions met — upgrade shape to ContextHole.
            state_map.set_var_shape(*dst, ShapeClass::ContextHole);
            tracing::debug!(
                func = ?func.name,
                var = dst.raw(),
                block = blk.raw(),
                "TRMC candidate detected: ContextHole shape set"
            );
        }
    }
}

/// Collect variables defined by recursive calls to the current function.
///
/// Scans both `Apply` instructions (body) and `Invoke` terminators for
/// calls where the callee name matches `func.name`.
fn collect_recursive_call_defs(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut defs = FxHashSet::default();
    let self_name = func.name;

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Apply {
                dst, func: callee, ..
            } = instr
            {
                if *callee == self_name {
                    defs.insert(*dst);
                }
            }
        }
        // Invoke terminators also define a dst from a call.
        if let ArcTerminator::Invoke {
            dst, func: callee, ..
        } = &block.terminator
        {
            if *callee == self_name {
                defs.insert(*dst);
            }
        }
    }

    defs
}

// populate_effect_summary() removed — effects are now accumulated during
// analysis in compute_block_entry_state() (Section 09.2 Effect Activation).

/// Compute FIP token balance from the converged state map.
///
/// Counts two quantities:
/// 1. **Construct allocations**: non-scalar `Construct` instructions with reusable
///    constructor kinds (`Struct`, `EnumVariant`). Each one needs a memory slot.
/// 2. **Consumed reusable-shaped values**: function parameters whose converged entry
///    state shows `Dead` or `Unrestricted` consumption with `ReusableCtor` shape.
///    Each consumed parameter provides a "reuse token" — its memory slot can be
///    recycled by a Construct of compatible type.
///
/// The balance determines FIP classification:
/// - `consumed >= constructs` → token-balanced → `FipContract::Certified` candidate
/// - `consumed < constructs` → `FipContract::Bounded(constructs - consumed)` candidate
///
/// Also records `AllocCreditBalance` events at Switch terminators for per-branch
/// FIP checking (`FIPTree` DMATCH! rule).
///
/// Section 09.2 Effect Activation.
fn populate_fip_balance(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let construct_count = count_reusable_constructs(state_map, func);
    let consumed_count = count_consumed_reusable_params(state_map, func);

    // Per-branch balance at Switch terminators (FIPTree DMATCH! rule).
    record_per_branch_balance(state_map, func);

    state_map.set_fip_balance(construct_count, consumed_count);

    if construct_count > 0 || consumed_count > 0 {
        tracing::debug!(
            construct_count,
            consumed_count,
            token_balanced = consumed_count >= construct_count,
            net_allocation = construct_count.saturating_sub(consumed_count),
            "FIP token balance computed"
        );
    }
}

/// Count `Construct` instructions with reusable constructor kinds (Struct, `EnumVariant`)
/// on non-scalar, non-immortal destinations.
fn count_reusable_constructs(state_map: &AimsStateMap, func: &ArcFunction) -> u32 {
    let mut count: u32 = 0;
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, ctor, .. } = instr {
                if !state_map.is_excluded(*dst)
                    && matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
                {
                    count = count.saturating_add(1);
                }
            }
        }
    }
    count
}

/// Count consumed function parameters with reusable shape.
///
/// A parameter consumed by the function (Dead/Unrestricted consumption in the
/// entry block's entry state) with `ReusableCtor` shape provides a "reuse token".
fn count_consumed_reusable_params(state_map: &AimsStateMap, func: &ArcFunction) -> u32 {
    let mut count: u32 = 0;
    let entry_block = ArcBlockId::new(0);
    for (param_idx, _) in func.params.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR var counts fit in u32"
        )]
        let var = ArcVarId::new(param_idx as u32);
        if state_map.is_excluded(var) {
            continue;
        }
        let state = state_map.var_state_at_block_entry(entry_block, var);
        let is_consumed = matches!(
            state.consumption,
            Consumption::Dead | Consumption::Unrestricted
        );
        if is_consumed && matches!(state.shape, ShapeClass::ReusableCtor(_)) {
            count = count.saturating_add(1);
        }
    }
    count
}

/// Record per-branch allocation credit balance at Switch terminators.
///
/// For each Switch successor, computes the per-block allocation vs death count
/// and records an `AllocCreditBalance` event. FIP certification requires each
/// branch to independently maintain non-negative credit balance (`FIPTree` DMATCH! rule).
fn record_per_branch_balance(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, block) in func.blocks.iter().enumerate() {
        let ArcTerminator::Switch { cases, default, .. } = &block.terminator else {
            continue;
        };

        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let blk = ArcBlockId::new(block_idx as u32);

        // Collect successor block IDs: case targets + default.
        let successors: Vec<ArcBlockId> = cases
            .iter()
            .map(|(_, target)| *target)
            .chain(std::iter::once(*default))
            .collect();

        for (succ_idx, target) in successors.iter().enumerate() {
            let balance = compute_block_fip_balance(state_map, func, *target);
            state_map.record_event(AimsEvent::AllocCreditBalance {
                block: blk,
                successor_idx: succ_idx,
                balance,
            });
        }
    }
}

/// Compute the FIP allocation balance for a single block.
///
/// Returns `allocs - deaths`: positive means the block needs more tokens
/// than it provides, zero is balanced, negative means surplus.
fn compute_block_fip_balance(
    state_map: &AimsStateMap,
    func: &ArcFunction,
    block_id: ArcBlockId,
) -> i32 {
    let block = &func.blocks[block_id.index()];
    let mut allocs: i32 = 0;
    let mut deaths: i32 = 0;

    for instr in &block.body {
        if let ArcInstr::Construct { dst, ctor, .. } = instr {
            if !state_map.is_excluded(*dst)
                && matches!(ctor, CtorKind::Struct(_) | CtorKind::EnumVariant { .. })
            {
                allocs = allocs.saturating_add(1);
            }
        }

        if let Some(dst) = instr.defined_var() {
            if state_map.is_excluded(dst) {
                continue;
            }
            let exit_state = state_map.var_state_at_block_exit(block_id, dst);
            let is_consumed = matches!(
                exit_state.consumption,
                Consumption::Dead | Consumption::Unrestricted
            );
            if is_consumed && matches!(exit_state.shape, ShapeClass::ReusableCtor(_)) {
                deaths = deaths.saturating_add(1);
            }
        }
    }

    allocs.saturating_sub(deaths)
}

/// Verify that all converged states are at a canonical fixed point.
///
/// Runs [`AimsState::canonicalize_with_feedback`] on every block entry/exit
/// state. Converged states should already be canonical (`rounds == 0`).
/// If any state is NOT canonical (`rounds > 0`), this indicates a bug in
/// the analysis — some path didn't call `canonicalize()` after a state
/// update. If cross-dimension chaining is detected (`rounds > 1`), the
/// `cross_dimension_detected` flag is set.
///
/// With current rules (Section 09.3), this should always pass.
///
/// Section 09.5 Convergence Feedback.
fn verify_canonical_fixed_point(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let mut max_rounds: u8 = 0;

    for (block_idx, _) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);

        // Check entry states.
        if let Some(entry) = state_map.block_entry_states(block_id) {
            for (var, state) in entry {
                let mut copy = *state;
                let feedback = copy.canonicalize_with_feedback();
                if feedback.rounds > max_rounds {
                    max_rounds = feedback.rounds;
                }
                debug_assert_eq!(
                    copy, *state,
                    "converged state is not canonical: block={block_idx}, var={var:?}"
                );
            }
        }

        // Check exit states.
        if let Some(exit) = state_map.block_exit_states(block_id) {
            for (var, state) in exit {
                let mut copy = *state;
                let feedback = copy.canonicalize_with_feedback();
                if feedback.rounds > max_rounds {
                    max_rounds = feedback.rounds;
                }
                debug_assert_eq!(
                    copy, *state,
                    "converged state is not canonical: block={block_idx}, var={var:?}"
                );
            }
        }
    }

    if max_rounds > 0 {
        tracing::warn!(
            func = ?func.name,
            max_rounds,
            "converged state was not canonical — analysis bug"
        );
    }

    if max_rounds > 1 {
        state_map.set_cross_dimension_detected();
        tracing::warn!(
            func = ?func.name,
            max_rounds,
            "cross-dimension canonicalize chaining detected in converged states"
        );
    }
}

/// Widen all non-converged variables to TOP (safety net for non-convergence).
fn widen_to_top(state_map: &mut AimsStateMap, func: &ArcFunction) {
    for (block_idx, _block) in func.blocks.iter().enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let block_id = ArcBlockId::new(block_idx as u32);

        let mut entry = state_map
            .block_entry_states(block_id)
            .cloned()
            .unwrap_or_default();
        for state in entry.values_mut() {
            *state = AimsState::TOP;
        }
        state_map.update_block_entry(block_id, entry);

        let mut exit = state_map
            .block_exit_states(block_id)
            .cloned()
            .unwrap_or_default();
        for state in exit.values_mut() {
            *state = AimsState::TOP;
        }
        state_map.update_block_exit(block_id, exit);
    }
}
