//! Post-emission processing: verification, tail calls, block merging, FBIP.
//!
//! Contains steps 6–9 (`verify_and_merge`), steps 11–12 (`emit_postprocess`),
//! and the FBIP enforcement check.

use super::AimsPipelineConfig;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;

/// Verify, AIMS-verify, detect tail calls, merge blocks (steps 6–9).
pub(crate) fn verify_and_merge(func: &mut ArcFunction, config: &AimsPipelineConfig<'_>) {
    {
        let _span = tracing::info_span!("verify_post_emission").entered();
        crate::pipeline::run_verify(func, "after AIMS emission", config.verify_arc);
    }
    super::trace_pipeline_checkpoint(func, "verify_post_emission", config.interner);
    if let Some(contract) = config.contracts.get(&func.name) {
        let _span = tracing::info_span!("aims_verify").entered();
        crate::pipeline::run_aims_verify(func, contract, "after AIMS emission", config.verify_arc);
    }
    super::trace_pipeline_checkpoint(func, "aims_verify", config.interner);
    {
        let _span = tracing::info_span!("tail_calls").entered();
        func.tail_calls = crate::tail_call::detect_tail_calls(func);
        crate::tail_call::rewrite_tail_calls(func);
    }
    super::trace_pipeline_checkpoint(func, "tail_calls", config.interner);
    {
        let _span = tracing::info_span!("unwind_cleanup").entered();
        crate::aims::emit_rc::unwind_cleanup::add_invoke_unwind_cleanup(func, config.interner);
    }
    super::trace_pipeline_checkpoint(func, "unwind_cleanup", config.interner);
    {
        let _span = tracing::info_span!("merge_blocks").entered();
        crate::block_merge::merge_blocks(func);
    }
    super::trace_pipeline_checkpoint(func, "merge_blocks", config.interner);
}

/// Post-emission steps: final verify + FBIP (steps 11–12).
pub(crate) fn emit_postprocess(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Vec<ArcProblem> {
    {
        let _span = tracing::info_span!("verify_final").entered();
        crate::pipeline::run_verify(func, "after AIMS pipeline", config.verify_arc);
    }
    super::trace_pipeline_checkpoint(func, "verify_final", config.interner);

    let problems = check_fbip(func, config);
    super::trace_pipeline_checkpoint(func, "fbip_enforcement", config.interner);
    problems
}

/// Check FBIP enforcement and auto-FBIP detection (step 12).
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
