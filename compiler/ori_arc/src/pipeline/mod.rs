//! ARC pipeline entry points.
//!
//! Contains [`run_arc_pipeline`] (single function), [`run_arc_pipeline_all`]
//! (batch with ownership application), and [`compute_aims_contracts`]
//! (interprocedural contract computation).
//!
//! All functions use the AIMS unified lattice pipeline (interprocedural
//! contracts → per-variable state map → RC/reuse emission).

use ori_ir::Name;
use ori_types::{Pool, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

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
/// The `uniqueness_summaries` parameter is unused — it exists for API
/// compatibility during the transition period. The AIMS pipeline derives
/// all ownership information from `aims_contracts`.
///
/// `sigs` and `type_registry` are bundled into `AimsPipelineConfig` for
/// downstream consumption by `infer_derived_ownership` and `emit_burden_ops`.
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
    type_registry: &TypeRegistry,
    verify_arc: bool,
) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>> {
    let _ = uniqueness_summaries;
    let builtins = BuiltinOwnershipSets::new(interner);
    // Single-function entry: codegen may compile functions OUTSIDE the
    // interprocedural batch (test-wrapper bodies, impl methods, derived
    // methods lowered after `analyze_program`). IC-1 makes no claim about a
    // function not in the analyzed set, so its own contract may be absent.
    // Seed a conservative contract for `func` when absent — the sound upper
    // bound `analyze_program` would assign an unanalyzed function — so the
    // pipeline's own-contract lookups (trmc, fip) have a real entry. Callee
    // lookups for analyzed functions still hit their precise contracts.
    let contracts = ensure_own_contract(aims_contracts, func);
    let func_names: FxHashSet<Name> = contracts.keys().copied().collect();
    let config = aims_pipeline::AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        func_names: &func_names,
        pool,
        interner,
        builtins: &builtins,
        verify_arc,
        observer: None,
        sigs,
        type_registry,
    };
    Ok(aims_pipeline::run_aims_pipeline(func, &config)?.problems)
}

/// Return a contracts map guaranteed to contain `func`'s own contract.
///
/// When `func.name` is already present (the batch / analyzed case), the
/// borrowed map is cloned unchanged. When absent (codegen of a function
/// outside the interprocedural batch — test bodies, impl methods, derives),
/// a conservative contract for `func`'s arity is inserted. Conservative is the
/// sound upper bound: all-Owned params, `may_share`, conservative effects —
/// identical to what `analyze_program` assigns a function it could not refine.
fn ensure_own_contract(
    aims_contracts: &FxHashMap<Name, MemoryContract>,
    func: &ArcFunction,
) -> FxHashMap<Name, MemoryContract> {
    let mut contracts = aims_contracts.clone();
    contracts
        .entry(func.name)
        .or_insert_with(|| MemoryContract::conservative(func.params.len()));
    contracts
}

/// Run the full ARC optimization pipeline on a single function with a
/// checkpoint observer.
///
/// Used by snapshot tests to capture per-pass ARC IR. The observer
/// receives `(&ArcFunction, phase_name)` at each pipeline boundary.
/// Otherwise identical to [`run_arc_pipeline`].
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline entry point bundles all context"
)]
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn run_arc_pipeline_with_observer<'a>(
    func: &mut ArcFunction,
    classifier: &'a dyn ArcClassification,
    sigs: &'a FxHashMap<Name, AnnotatedSig>,
    pool: &'a Pool,
    interner: &'a ori_ir::StringInterner,
    uniqueness_summaries: &FxHashMap<Name, UniquenessSummary>,
    aims_contracts: &'a FxHashMap<Name, MemoryContract>,
    type_registry: &'a TypeRegistry,
    verify_arc: bool,
    observer: &'a aims_pipeline::CheckpointObserver<'a>,
) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>> {
    let _ = uniqueness_summaries;
    let builtins = BuiltinOwnershipSets::new(interner);
    // Seed `func`'s own conservative contract when absent — see
    // `run_arc_pipeline` for the out-of-batch-function rationale.
    let contracts = ensure_own_contract(aims_contracts, func);
    let func_names: FxHashSet<Name> = contracts.keys().copied().collect();
    let config = aims_pipeline::AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        func_names: &func_names,
        pool,
        interner,
        builtins: &builtins,
        verify_arc,
        observer: Some(observer),
        sigs,
        type_registry,
    };
    Ok(aims_pipeline::run_aims_pipeline(func, &config)?.problems)
}

/// Run the full ARC pipeline on all functions, including ownership application.
///
/// This is the batch entry point for the entire ARC optimization pass:
/// 1. Compute AIMS interprocedural contracts (`MemoryContract` per function)
/// 2. Apply ownership annotations to function parameters
/// 3. Run the per-function AIMS pipeline for each function
///
/// `sigs` and `type_registry` are bundled into `AimsPipelineConfig` for
/// downstream consumption (see `run_arc_pipeline` docs).
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline entry point bundles all context"
)]
#[expect(clippy::implicit_hasher, reason = "callee functions require FxHashMap")]
pub fn run_arc_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    sigs: &FxHashMap<Name, AnnotatedSig>,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    type_registry: &TypeRegistry,
    verify_arc: bool,
) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>> {
    aims_pipeline::run_aims_pipeline_all(
        functions,
        classifier,
        sigs,
        interner,
        pool,
        builtins,
        type_registry,
        verify_arc,
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
pub use aims_pipeline::CheckpointObserver;
pub(crate) mod rc_count;

#[cfg(test)]
mod tests;

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
