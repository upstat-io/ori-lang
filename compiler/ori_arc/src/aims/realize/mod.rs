//! Unified realization — one decision surface for all AIMS outputs.
//!
//! Replaces the four separate emission passes (`emit_rc_ops`, `emit_reuse`,
//! `compute_aims_cow_annotations`, `compute_aims_drop_hints`) with a
//! two-phase realization that reads the converged [`AimsStateMap`] through
//! unified decision functions.
//!
//! # Architecture
//!
//! - **Phase 1** ([`realize_rc_reuse`]): pre-merge. Forward walk calling
//!   `decide()` for RC and reuse decisions. Calls edge cleanup at the end.
//! - **Phase 2** ([`realize_annotations`]): post-merge. Walks post-merge IR
//!   using ArcVarId-keyed state lookups for COW and drop hint decisions.
//!
//! Both phases share the same [`AimsStateMap`] and decision surface.
//! No output owns an independent decision procedure.
//!
//! # References
//!
//! - Section 10 of the AIMS plan (`plans/aims/section-10-unified-realization.md`)
//! - Perceus (Reinking et al., PLDI 2021): unified RC + reuse
//! - FP² (Marshall et al., ESOP 2022): FIP-guided reuse decisions

pub mod decide;
#[cfg(test)]
mod tests;

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::aims::emit_reuse::FipGateRecord;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::uniqueness::drop_hints::DropHints;
use crate::uniqueness::CowAnnotations;

/// Result of the unified realization — all outputs in one struct.
///
/// Phase 1 (`realize_rc_reuse`) populates `rc_ops_inserted`,
/// `reuse_ops_inserted`, and `fip_evidence`. Phase 2
/// (`realize_annotations`) populates `cow_annotations` and `drop_hints`.
#[derive(Debug)]
pub struct RealizationResult {
    /// RC operations inserted (`RcInc` + `RcDec` count).
    pub rc_ops_inserted: usize,
    /// Reuse operations inserted (Reset + Reuse + `IsShared` count).
    pub reuse_ops_inserted: usize,
    /// COW annotations computed in Phase 2, keyed by `(block_idx, instr_idx)`.
    pub cow_annotations: CowAnnotations,
    /// Drop hints computed in Phase 2, keyed by `(block_idx, instr_idx)`.
    pub drop_hints: DropHints,
    /// FIP diagnostic evidence (missed reuses, gate records).
    /// NOT the authoritative FIP classification — that is
    /// `MemoryContract.fip`, owned by interprocedural analysis.
    pub fip_evidence: FipEvidence,
}

/// FIP diagnostic evidence accumulated during realization.
///
/// This is NOT the authoritative FIP classification. `MemoryContract.fip`
/// is authoritative (computed by `extract_contract()` in interprocedural
/// analysis). Realization consumes the contract and emits evidence that
/// verification can cross-check against it.
#[derive(Debug, Default)]
pub struct FipEvidence {
    /// FIP gate records from reuse emission.
    pub fip_gates: Vec<FipGateRecord>,
    /// Missed reuse opportunities in FIP functions.
    pub missed_reuses: usize,
}

/// Phase 1: RC and reuse emission (pre-merge).
///
/// Reads the converged [`AimsStateMap`], emits `RcInc`/`RcDec` and
/// `Reset`/`Reuse` operations, populates `arg_ownership` on
/// `Apply`/`Invoke` instructions, and calls edge cleanup.
///
/// Returns a partial [`RealizationResult`] — `cow_annotations` and
/// `drop_hints` are empty (populated by Phase 2 after `merge_blocks`).
///
/// # Pipeline position
///
/// Runs AFTER `analyze_function()` (step 4) and BEFORE `verify()` (step 6).
/// Replaces old steps 4 (`emit_arg_ownership`), 6 (`emit_rc_ops`), and
/// 7 (`emit_reuse`).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn realize_rc_reuse(
    func: &mut ArcFunction,
    state_map: &AimsStateMap,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &ori_ir::StringInterner,
    builtins: &BuiltinOwnershipSets,
    pool: &Pool,
) -> RealizationResult {
    // Sub-step A: emit arg_ownership (previously standalone step 4).
    // This is an emission artifact (Option C disposition), not an analysis
    // input — zero production reads in intraprocedural/ or transfer/.
    {
        let _span = tracing::debug_span!("realize_arg_ownership").entered();
        crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
            func, contracts, interner, builtins, pool,
        );
    }

    // Sub-step B: emit RC operations (previously step 6).
    let rc_ops_inserted = {
        let _span = tracing::debug_span!("realize_rc").entered();
        let rc_result = crate::aims::emit_rc::emit_rc_ops(func, state_map, pool);
        // Count RC ops inserted.
        let count = count_rc_ops(func);
        // local_alloc_candidates consumed here (v1: hints only, not yet used).
        let _ = rc_result.local_alloc_candidates;
        count
    };

    // Sub-step C: emit reuse operations (previously step 7).
    let (reuse_ops_inserted, fip_evidence) = {
        let _span = tracing::debug_span!("realize_reuse").entered();
        let reuse_result = crate::aims::emit_reuse::emit_reuse(func, state_map, pool, contracts);
        if !reuse_result.fip_gates.is_empty() {
            tracing::debug!(
                function = func.name.raw(),
                fip_gates = reuse_result.fip_gates.len(),
                "FIP gate records captured during realization"
            );
        }
        let ops = reuse_result.static_reuses
            + reuse_result.dynamic_reuses
            + reuse_result.cross_block_reuses;
        let evidence = FipEvidence {
            fip_gates: reuse_result.fip_gates,
            missed_reuses: reuse_result.missed_reuses,
        };
        (ops, evidence)
    };

    RealizationResult {
        rc_ops_inserted,
        reuse_ops_inserted,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        fip_evidence,
    }
}

/// Phase 2: COW and drop hint annotations (post-merge).
///
/// Reads the [`AimsStateMap`] via ArcVarId-keyed lookups on the post-merge
/// IR. Computes `cow_annotations` and `drop_hints`, completing the
/// [`RealizationResult`].
///
/// # Pipeline position
///
/// Runs AFTER `merge_blocks()` (step 9). Replaces old steps 11a
/// (`compute_aims_cow_annotations`) and 12 (`compute_aims_drop_hints`).
///
/// # Panics
///
/// Does NOT panic on failure — logs `tracing::error!` and leaves
/// annotations empty (functionally correct but suboptimal).
pub fn realize_annotations(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    result: &mut RealizationResult,
) {
    // Sub-step D: COW annotations (previously step 11a).
    result.cow_annotations = {
        let _span = tracing::debug_span!("realize_cow").entered();
        crate::aims::emit_rc::cow::compute_aims_cow_annotations(func, state_map, interner)
    };

    // Sub-step E: drop hints (previously step 12).
    result.drop_hints = {
        let _span = tracing::debug_span!("realize_drop_hints").entered();
        crate::aims::emit_rc::drop_hints::compute_aims_drop_hints(func, state_map, pool)
    };
}

/// Count RC operations (`RcInc` + `RcDec`) in a function.
fn count_rc_ops(func: &ArcFunction) -> usize {
    use crate::ir::ArcInstr;
    func.blocks
        .iter()
        .flat_map(|b| &b.body)
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .count()
}
