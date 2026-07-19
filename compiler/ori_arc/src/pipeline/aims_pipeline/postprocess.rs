//! Verification and control-flow finalization after ownership-event emission.

use super::AimsPipelineConfig;
use crate::aims::contract::ContractMapExt;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;

/// Verifies ownership facts, lowers tail calls and unwind cleanup, then merges blocks.
///
/// Returns `Err` if verification fails under explicit verification mode
/// (`ORI_VERIFY_ARC=1`). The error contains the list of ICE-level
/// verification failures that must halt compilation.
pub(crate) fn verify_and_merge(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    {
        let _span = tracing::info_span!("verify_post_emission").entered();
        crate::pipeline::run_verify(func, "after AIMS emission", config.verify_arc)?;
    }
    super::trace_pipeline_checkpoint(
        func,
        "verify_post_emission",
        config.interner,
        config.observer,
    );
    {
        let contract = config
            .contracts
            .get_required(&func.name, "aims_verify_postprocess");
        let _span = tracing::info_span!("aims_verify").entered();
        crate::pipeline::run_aims_verify(func, contract, "after AIMS emission", config.verify_arc)?;
    }
    super::trace_pipeline_checkpoint(func, "aims_verify", config.interner, config.observer);
    {
        let _span = tracing::info_span!("tail_calls").entered();
        func.tail_calls = crate::tail_call::detect_tail_calls(func);
        crate::tail_call::rewrite_tail_calls(func);
    }
    super::trace_pipeline_checkpoint(func, "tail_calls", config.interner, config.observer);
    {
        let _span = tracing::info_span!("unwind_cleanup").entered();
        crate::aims::emit_rc::unwind_cleanup::add_invoke_unwind_cleanup(func, config.interner);
    }
    super::trace_pipeline_checkpoint(func, "unwind_cleanup", config.interner, config.observer);
    {
        let _span = tracing::info_span!("merge_blocks").entered();
        crate::block_merge::merge_blocks(func);
    }
    super::trace_pipeline_checkpoint(func, "merge_blocks", config.interner, config.observer);
    crate::aims::validate_primitive_facts(func, config.classifier)?;
    super::trace_pipeline_checkpoint(
        func,
        "validate_primitive_facts_post_merge",
        config.interner,
        config.observer,
    );
    Ok(())
}

/// Performs final verification and produces user-facing FBIP diagnostics.
///
/// Returns `Err` if the final verification fails under explicit verification
/// mode. On success, returns the list of user-facing FBIP diagnostics.
pub(crate) fn emit_postprocess(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>> {
    {
        let _span = tracing::info_span!("verify_final").entered();
        crate::pipeline::run_verify(func, "after AIMS pipeline", config.verify_arc)?;
    }
    super::trace_pipeline_checkpoint(func, "verify_final", config.interner, config.observer);

    // INVARIANT: Every burden-emitting variable has zero net credit at function exits.
    run_burden_balance(func, config.verify_arc, config.interner)?;
    super::trace_pipeline_checkpoint(
        func,
        "verify_burden_balance",
        config.interner,
        config.observer,
    );

    let problems = check_fbip(func, config);
    super::trace_pipeline_checkpoint(func, "fbip_enforcement", config.interner, config.observer);
    Ok(problems)
}

/// Enforces VF-1 under explicit or debug verification.
///
/// Explicit verification returns errors; debug verification logs warnings.
fn run_burden_balance(
    func: &ArcFunction,
    verify: bool,
    interner: &ori_ir::StringInterner,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let enabled = verify || cfg!(debug_assertions);
    if !enabled {
        return Ok(());
    }
    let errors = crate::aims::verify::burden_balance::verify_burden_balance(func);
    if errors.is_empty() {
        return Ok(());
    }
    let verify_errors: Vec<crate::verify::VerifyError> = errors
        .into_iter()
        .map(crate::verify::VerifyError::BurdenImbalance)
        .collect();
    if verify {
        return Err(verify_errors);
    }
    let fn_name = interner.lookup(func.name);
    for e in &verify_errors {
        tracing::warn!(
            phase = "verify_burden_balance",
            function = fn_name,
            "ARC IR verification: {e}"
        );
    }
    Ok(())
}

/// Produces explicit and inferred FBIP diagnostics.
fn check_fbip(func: &ArcFunction, config: &AimsPipelineConfig<'_>) -> Vec<ArcProblem> {
    let mut problems = Vec::new();
    if func.is_fbip {
        let func_name = config.interner.lookup(func.name);
        let func_span = func
            .spans
            .first()
            .and_then(|block_spans| block_spans.first().copied().flatten())
            .unwrap_or(ori_ir::Span::DUMMY);
        if let Some(problem) =
            crate::fbip::check_fbip_enforcement(func, config.classifier, func_name, func_span)
        {
            problems.push(problem);
        }
    }

    if crate::fbip::is_auto_fbip(func) {
        let func_name = config.interner.lookup(func.name);
        tracing::debug!(
            function = func_name,
            cow_ops = func.cow_annotations.len(),
            "auto FBIP: all COW operations are StaticUnique"
        );
    }

    problems
}
