//! Backend-neutral AIMS realization entry points.
//!
//! Production consumers submit one closed program to
//! [`realize_closed_program`]. Interprocedural contracts, logical ownership
//! events, reuse decisions, and callable facts are frozen together; physical
//! backends consume that result without re-running or approximating the
//! calculus locally.

use std::collections::HashMap;
use std::hash::BuildHasher;

use ori_ir::Name;
use ori_types::{Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::aims::contract::{FreshSelfAllocationFacts, FunctionEffectFacts, MemoryContract};
use crate::aims::realize::rl31_disjoint::ParamDisjointnessFacts;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::{ArcClassification, ClosureAdapterPlan, FunctionCallableFacts, RetainPlanTable};

mod aims_pipeline;
pub(crate) mod callable_boundary;
pub(crate) mod rc_count;

pub use aims_pipeline::CheckpointObserver;
pub use callable_boundary::{CallableBoundaryError, CallableBoundaryFacts};

#[cfg(test)]
mod tests;

/// Final output of one whole-program AIMS realization.
///
/// The contracts are the same map updated by the post-emission second pass;
/// consumers must not recompute interprocedural facts from the realized IR.
#[derive(Debug)]
pub struct ArcPipelineBatchOutcome {
    /// Semantic lowering problems collected across all realized functions.
    pub problems: Vec<ArcProblem>,
    /// Final backend-neutral contracts keyed by stable semantic function name.
    pub contracts: FxHashMap<Name, MemoryContract>,
    /// Final RL-30 facts from the same realized bodies and contracts.
    pub function_effects: FxHashMap<Name, FunctionEffectFacts>,
    /// Final RL-29 facts from the same realized return contracts.
    pub fresh_return_facts: FxHashMap<Name, FreshSelfAllocationFacts>,
    /// Final RL-31 facts from the same realized signatures and type pool.
    pub param_disjointness: FxHashMap<Name, ParamDisjointnessFacts>,
    /// Exact closure-call bridge plans frozen from the same realized target
    /// signatures. Residual indirect arguments remain borrowed; physical
    /// projections only encode these actions.
    pub closure_adapters: FxHashMap<Name, ClosureAdapterPlan>,
    /// Closed logical ownership-credit graph referenced by closure adapter
    /// actions. Physical projections map these stable identities through their
    /// own representation plans.
    pub retain_plans: RetainPlanTable,
    /// Semantic callable signatures parallel to every realized function's SSA
    /// register table. Backends consume these facts without querying `Pool`.
    pub callable_facts: FxHashMap<Name, FunctionCallableFacts>,
}

/// Immutable inputs for one closed-program AIMS realization.
///
/// This context is the backend-neutral analysis seam. It contains semantic
/// services and producer-frozen external contracts, but no executor, layout,
/// ABI, opcode, or runtime representation choice.
pub struct ArcPipelineContext<'a, S> {
    /// Type ownership classifier used by the logical calculus.
    pub classifier: &'a dyn ArcClassification,
    /// Stable name interner shared by the closed program.
    pub interner: &'a ori_ir::StringInterner,
    /// Canonical type pool for semantic type queries.
    pub pool: &'a Pool,
    /// Registry-authored builtin ownership contracts.
    pub builtins: &'a BuiltinOwnershipSets,
    /// Canonical registry for type-directed logical facts.
    pub type_registry: &'a TypeRegistry,
    /// Exact language-prescribed callable roles projected from frontend-owned
    /// semantic bindings.
    pub callable_boundaries: &'a CallableBoundaryFacts,
    /// Whether to enforce the expensive verification layer in this run.
    pub verify_arc: bool,
    /// Immutable contracts imported from other compiled units.
    pub external_contracts: &'a HashMap<Name, MemoryContract, S>,
}

/// Realize one closed program through the backend-neutral AIMS calculus.
///
/// `external_contracts` contains immutable producer-authored facts for calls
/// whose bodies live in another compiled unit. The returned facts cover local
/// realized bodies exactly; no physical backend may fill gaps conservatively.
pub fn realize_closed_program<S: BuildHasher>(
    functions: &mut [ArcFunction],
    context: &ArcPipelineContext<'_, S>,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    aims_pipeline::run_aims_pipeline_all_with_external_contracts(functions, context)
}

/// Realize one closed program and report whole-batch pipeline checkpoints.
///
/// The observer is diagnostic only; it cannot select a different calculus or
/// bypass interprocedural closure. `ownership_applied` is reported before the
/// first per-function transformation, followed by the normal phase names.
pub fn realize_closed_program_with_observer<'a, S: BuildHasher>(
    functions: &mut [ArcFunction],
    context: &ArcPipelineContext<'a, S>,
    observer: &'a CheckpointObserver<'a>,
) -> Result<ArcPipelineBatchOutcome, Vec<crate::verify::VerifyError>> {
    aims_pipeline::run_aims_pipeline_all_with_observer(functions, context, observer)
}

/// Run ARC IR verification if enabled.
///
/// Active under `debug_assertions` or when the caller passes `verify: true`
/// (typically from `ORI_VERIFY_ARC=1` read in `oric`).
///
/// Under explicit verification mode (`verify=true`), returns `Err` with the
/// list of verification errors — these are ICEs that must halt compilation.
/// Under debug-assertions-only mode, logs warnings and returns `Ok`.
pub(crate) fn run_verify(
    func: &ArcFunction,
    phase: &str,
    verify: bool,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let enabled = verify || cfg!(debug_assertions);
    if !enabled {
        return Ok(());
    }

    let errors = crate::verify::check_function(func);
    if errors.is_empty() {
        return Ok(());
    }

    if verify {
        // Explicit verification mode: hard error.
        return Err(errors);
    }

    // debug_assertions only: warn but continue.
    for e in &errors {
        tracing::warn!(phase, "ARC IR verification: {e}");
    }
    Ok(())
}

/// Run AIMS-specific consistency checks (contract vs IR).
///
/// Verifies that AIMS analysis results are consistent with the actual IR.
/// For example, parameters with `Cardinality::Absent` should have no uses.
///
/// Under explicit verification mode (`verify=true`), returns `Err` with
/// AIMS-specific verification errors. Under debug-assertions-only mode,
/// logs warnings and returns `Ok`.
pub(crate) fn run_aims_verify(
    func: &ArcFunction,
    contract: &crate::aims::contract::MemoryContract,
    phase: &str,
    verify: bool,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let enabled = verify || cfg!(debug_assertions);
    if !enabled {
        return Ok(());
    }

    let errors = crate::verify::check_function_with_contract(func, contract);
    // Only report AIMS-specific errors (structural errors already reported by run_verify).
    let aims_errors: Vec<_> = errors
        .into_iter()
        .filter(|e| matches!(e, crate::verify::VerifyError::AbsentParamHasUses { .. }))
        .collect();

    if aims_errors.is_empty() {
        return Ok(());
    }

    if verify {
        // Explicit verification mode: hard error.
        // Note: check_absent_param_no_uses already filters to only LIVE blocks
        // (forward-reachable from entry AND backward-reachable to Return).
        // Dead-code references (e.g., after `if true then panic`) are excluded
        // by the live_blocksanalysis. Any error reaching here is a genuine
        // contract/IR inconsistency on a live path.
        return Err(aims_errors);
    }

    // debug_assertions only: warn but continue.
    for e in &aims_errors {
        tracing::warn!(phase, "AIMS consistency: {e}");
    }
    Ok(())
}
