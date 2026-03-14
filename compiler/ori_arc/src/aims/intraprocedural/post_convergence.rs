//! Post-convergence passes for intraprocedural analysis.
//!
//! After the backward dataflow analysis converges, these passes populate
//! side tables in the [`AimsStateMap`] using the converged state:
//!
//! - [`populate_borrow_sources`] — borrow source tracking for Project instructions
//! - [`populate_sparse_events`] — reusable allocations + local-alloc candidates
//! - [`populate_var_shapes`] — per-variable shape from definitions
//! - [`detect_trmc_candidates`] — TRMC `ContextHole` detection
//! - [`populate_context_events`] — ContextOpen/ContextClose from normalize metadata
//! - [`super::fip_balance::populate_fip_balance`] — FIP token balance (in `fip_balance.rs`)
//! - [`super::fip_balance::populate_fip_gate_events`] — FIP gate events (in `fip_balance.rs`)
//!
//! These passes are called from [`super::analyze_function`] after convergence.

use rustc_hash::FxHashSet;

use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId, CtorKind};

use super::super::contract::ContextRegion;
use super::super::lattice::{AimsState, Locality, ReuseCtorKind, ShapeClass, Uniqueness};
use super::super::transfer::transfer_def;
use super::state_map::{AimsEvent, AimsStateMap};

/// Populate the borrow source side table after analysis converges.
///
/// Each variable defined by a `Project` instruction gets
/// `BorrowSource::exact_field(source_var, field)`. Block params that receive values
/// from multiple predecessors are left without a source (implicitly
/// `Unknown` if queried). In ARC IR's SSA form, each variable has exactly
/// one definition point, so this is per-variable, not per-point.
pub(crate) fn populate_borrow_sources(state_map: &mut AimsStateMap, func: &ArcFunction) {
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
pub(crate) fn populate_sparse_events(state_map: &mut AimsStateMap, func: &ArcFunction) {
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
pub(crate) fn populate_var_shapes(state_map: &mut AimsStateMap, func: &ArcFunction) {
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
/// # Soundness gates (Section 13.2)
///
/// TRMC requires TWO soundness gates:
///
/// 1. **Per-variable uniqueness (enforced):** The context variable must have
///    `Uniqueness::Unique` at the mutation point. Checked below.
///
/// 2. **Effect purity (logged, not enforced):** In principle,
///    `may_resume_nonlinearly` (derived from `EffectSummary.may_share`)
///    guards against non-linear effect handler resumption capturing the
///    context variable. However, the current `HeapEscaping → may_share`
///    accumulation rule makes ALL TRMC candidates trigger `may_share`,
///    so enforcing this gate blocks all TRMC. The correct formulation
///    depends on effect-handler semantics (not yet implemented). Until
///    then, gate 1 alone is sound because no mechanism for non-linear
///    resumption exists in Ori v1. See `contract/mod.rs` `ContextBehavior` doc.
///
/// Locality is deliberately NOT checked: TRMC constructors are typically
/// returned (`HeapEscaping`), which is expected — the whole point is
/// building the result in place and returning it.
///
/// When all conditions hold, the constructor's shape is upgraded to
/// `ContextHole`, enabling Stage 3 TRMC normalization to rewrite the
/// recursive call into an in-place fill of the constructor's hole.
///
/// Section 09.2 Shape Activation — `ContextHole` detection.
pub(crate) fn detect_trmc_candidates(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    may_share: bool,
) {
    // Collect variables defined by recursive calls (callee == func.name).
    // Uses the shared helper from normalize/detect.rs (Section 12.4a unification).
    let recursive_sites = crate::aims::normalize::collect_recursive_call_sites(func);
    let recursive_defs: FxHashSet<ArcVarId> = recursive_sites.into_keys().collect();
    if recursive_defs.is_empty() {
        return;
    }

    // Soundness gate 2 (Section 13.2): Effect purity — logged, not enforced.
    // In Ori v1, no effect handlers exist, so non-linear resumption cannot
    // occur. When effect handlers are implemented, this must be enforced
    // (or refined to exclude self-sharing from returned Constructs).
    // See contract/mod.rs ContextBehavior doc for the full design rationale.
    if may_share {
        tracing::trace!(
            func = ?func.name,
            "TRMC effect gate: may_share=true (logged, not enforced — \
             no effect handlers in v1)"
        );
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

/// Record `ContextOpen`/`ContextClose` events from normalize-provided context regions.
///
/// For each [`ContextRegion`] where the context variable has `ContextHole` shape
/// (set by `detect_trmc_candidates`) and is `Unique` at the block exit, records
/// paired events in the sparse event table.
///
/// # Soundness gates (Section 13.2)
///
/// Same two-gate model as `detect_trmc_candidates`:
///
/// 1. **Per-variable uniqueness (enforced):** The context variable must be
///    `Unique` at the open block exit. Double-checked here even though
///    `detect_trmc_candidates` already verified it at `ContextHole` marking.
///
/// 2. **Effect purity (logged, not enforced):** `may_share` is logged for
///    diagnostics but not enforced. See `detect_trmc_candidates` doc for
///    the full rationale.
///
/// Stage 3: `context_regions` produced by `aims::normalize::normalize_function()`.
pub(crate) fn populate_context_events(
    state_map: &mut AimsStateMap,
    func: &ArcFunction,
    context_regions: &[ContextRegion],
    may_share: bool,
) {
    if context_regions.is_empty() {
        return;
    }

    // Soundness gate 2 (Section 13.2): Effect purity — logged, not enforced.
    if may_share {
        tracing::trace!(
            func = ?func.name,
            "TRMC context events: may_share=true (logged, not enforced — \
             no effect handlers in v1)"
        );
    }

    for region in context_regions {
        // Skip regions for excluded (scalar/immortal) variables.
        if state_map.is_excluded(region.context_var) {
            continue;
        }

        // Soundness gate: the context variable must have ContextHole shape
        // (set by detect_trmc_candidates, which already checks uniqueness).
        if !matches!(
            state_map.var_shape(region.context_var),
            ShapeClass::ContextHole
        ) {
            tracing::trace!(
                func = ?func.name,
                var = region.context_var.raw(),
                shape = ?state_map.var_shape(region.context_var),
                "skipping context event: not ContextHole shape"
            );
            continue;
        }

        // Double-check uniqueness at the open block exit.
        let state = state_map.var_state_at_block_exit(region.open_block, region.context_var);
        if state.uniqueness != Uniqueness::Unique {
            tracing::trace!(
                func = ?func.name,
                var = region.context_var.raw(),
                uniqueness = ?state.uniqueness,
                "skipping context event: not Unique"
            );
            continue;
        }

        // Record paired ContextOpen/ContextClose events.
        state_map.record_event(AimsEvent::ContextOpen {
            block: region.open_block,
            instr: region.open_instr,
            var: region.context_var,
        });
        state_map.record_event(AimsEvent::ContextClose {
            block: region.close_block,
            instr: region.close_instr,
            var: region.hole_var,
        });

        tracing::debug!(
            func = ?func.name,
            context_var = region.context_var.raw(),
            hole_var = region.hole_var.raw(),
            hole_field = region.hole_field,
            "recorded TRMC context events (open + close)"
        );
    }
}
