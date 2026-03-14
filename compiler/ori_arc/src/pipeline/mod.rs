//! ARC pipeline entry points.
//!
//! Contains [`run_arc_pipeline`] (single function), [`run_arc_pipeline_all`]
//! (batch with ownership application), and [`compute_aims_contracts`]
//! (interprocedural contract computation).
//!
//! All functions use the AIMS unified lattice pipeline (interprocedural
//! contracts → per-variable state map → RC/reuse emission).

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::MemoryContract;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::AnnotatedSig;
use crate::uniqueness::UniquenessSummary;
use crate::ArcClassification;

/// Run the full ARC optimization pipeline on a single function.
///
/// Uses the AIMS unified lattice pipeline: backward dataflow analysis
/// produces a per-variable state map, then RC/reuse emission reads the
/// state map to place operations.
///
/// The `uniqueness_summaries` and `sigs` parameters are unused — they
/// exist for API compatibility during the transition period. The AIMS
/// pipeline derives all ownership information from `aims_contracts`.
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline entry point bundles all context"
)]
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn run_arc_pipeline(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    pool: &Pool,
    interner: &ori_ir::StringInterner,
    uniqueness_summaries: &FxHashMap<Name, UniquenessSummary>,
    aims_contracts: &FxHashMap<Name, MemoryContract>,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    let _ = (sigs, uniqueness_summaries);
    let builtins = BuiltinOwnershipSets::new(interner);
    let config = aims_pipeline::AimsPipelineConfig {
        classifier,
        contracts: aims_contracts,
        pool,
        interner,
        builtins: &builtins,
        verify_arc,
    };
    aims_pipeline::run_aims_pipeline(func, &config).problems
}

/// Run the full ARC pipeline on all functions, including ownership application.
///
/// This is the batch entry point for the entire ARC optimization pass:
/// 1. Compute AIMS interprocedural contracts (`MemoryContract` per function)
/// 2. Apply ownership annotations to function parameters
/// 3. Run the per-function AIMS pipeline for each function
///
/// The `sigs` parameter is unused — it exists for API compatibility during
/// the transition period.
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    let _ = sigs;
    aims_pipeline::run_aims_pipeline_all(
        functions, classifier, interner, pool, builtins, verify_arc,
    )
}

/// Compute AIMS interprocedural contracts and apply param ownership.
///
/// Runs the AIMS interprocedural analysis ([`aims::analyze_program`]) to produce
/// a [`MemoryContract`] for each function, then applies the resulting ownership
/// annotations to function parameters.
///
/// The returned contracts map must be passed to [`run_arc_pipeline`] for each
/// function so that callsite `arg_ownership` is correctly annotated from contract
/// data.
pub fn compute_aims_contracts(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
    builtins: &BuiltinOwnershipSets,
) -> FxHashMap<Name, MemoryContract> {
    let contracts =
        crate::aims::interprocedural::analyze_program(functions, classifier, builtins, interner);
    aims_pipeline::apply_aims_ownership(functions, &contracts);
    contracts
}

/// Run interprocedural uniqueness analysis on all functions.
///
/// Returns an empty map — the AIMS pipeline computes uniqueness internally
/// via its 7-dimensional state lattice. This function exists for API
/// compatibility during the transition period.
pub fn run_uniqueness_analysis(
    functions: &[ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
) -> FxHashMap<Name, UniquenessSummary> {
    let _ = (functions, classifier, interner);
    FxHashMap::default()
}

mod aims_pipeline;
pub(crate) mod rc_count;

/// Run ARC IR verification if enabled.
///
/// Active under `debug_assertions` or when the caller passes `verify: true`
/// (typically from `ORI_VERIFY_ARC=1` read in `oric`).
/// Logs warnings for each error but does not panic — this is diagnostic,
/// not blocking.
fn run_verify(func: &ArcFunction, phase: &str, verify: bool) {
    let enabled = verify || cfg!(debug_assertions);
    if !enabled {
        return;
    }

    let errors = crate::verify::check_function(func);
    for e in &errors {
        tracing::warn!(phase, "ARC IR verification: {e}");
    }
}

/// Run AIMS-specific consistency checks (contract vs IR).
///
/// Verifies that AIMS analysis results are consistent with the actual IR.
/// For example, parameters with `Cardinality::Absent` should have no uses.
fn run_aims_verify(
    func: &ArcFunction,
    contract: &crate::aims::contract::MemoryContract,
    phase: &str,
    verify: bool,
) {
    let enabled = verify || cfg!(debug_assertions);
    if !enabled {
        return;
    }

    let errors = crate::verify::check_function_with_contract(func, contract);
    // Only report AIMS-specific errors (structural errors already reported by run_verify).
    for e in &errors {
        if matches!(e, crate::verify::VerifyError::AbsentParamHasUses { .. }) {
            tracing::warn!(phase, "AIMS consistency: {e}");
        }
    }
}
