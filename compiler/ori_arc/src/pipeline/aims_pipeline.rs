//! AIMS pipeline implementation.
//!
//! Replaces the sequential analysis passes (borrow inference, liveness,
//! uniqueness, RC insertion, reset/reuse, RC elimination) with the unified
//! AIMS analysis + emission pipeline.
//!
//! # Pipeline (Section 06.2)
//!
//! **Interprocedural** (once across all functions):
//! 1. `aims::analyze_program()` — compute `MemoryContract` per function
//! 2. `aims::apply_ownership()` — populate `ArcParam.ownership`
//!
//! **Per-function** (steps 3–14):
//! 3. `compute_var_reprs()` — fill `ValueRepr` per variable
//! 4. `aims::emit_arg_ownership()` — populate `arg_ownership` on Apply/Invoke
//! 5. `aims::analyze_function()` — backward dataflow → converged state map
//! 6. `aims::emit_rc_ops()` — insert `RcInc`/`RcDec` from state map
//! 7. `aims::emit_reuse()` — insert Reset/Reuse from state map
//! 9. `verify()` — ARC IR sanity check
//! 10. `detect_tail_calls()` + `rewrite_tail_calls()`
//! 11. `merge_blocks()` — CFG cleanup
//! 11a. COW annotations — derived from state map (post-merge)
//! 12. Drop hints — derived from state map (post-merge)
//! 13. `verify()` — final sanity check
//! 14. FBIP enforcement — read-only diagnostic

use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::aims::contract::{MemoryContract, ParamContract};
use crate::aims::lattice::AccessClass;
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::ArcFunction;
use crate::lower::ArcProblem;
use crate::ownership::Ownership;
use crate::ArcClassification;

/// Configuration for the AIMS per-function pipeline.
///
/// Bundles the shared parameters that `run_aims_pipeline` needs, avoiding
/// the 7-parameter signature anti-pattern from the old pipeline.
pub(crate) struct AimsPipelineConfig<'a> {
    pub classifier: &'a dyn ArcClassification,
    pub contracts: &'a FxHashMap<Name, MemoryContract>,
    pub pool: &'a Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
}

/// Run the AIMS pipeline on a single function (steps 3–14).
///
/// Called from within `run_arc_pipeline` when the `aims` feature is active.
/// Interprocedural contracts must already be computed and passed via `config`.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Vec<ArcProblem> {
    // Step 3: compute value representations.
    func.var_reprs = crate::ir::compute_var_reprs(func, config.classifier, config.pool);

    // Step 4: populate arg_ownership on Apply/Invoke instructions.
    crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
        func,
        config.contracts,
        config.interner,
        config.builtins,
        config.pool,
    );

    // Step 5: intraprocedural analysis → converged state map.
    let state_map = crate::aims::intraprocedural::analyze_function(
        func,
        config.classifier,
        config.contracts,
        &[], // context_regions: empty in Stage 1
    );

    // Step 6: RC emission from state map.
    let _rc_result = crate::aims::emit_rc::emit_rc_ops(
        func,
        &state_map,
        config.contracts,
        config.classifier,
        config.pool,
    );

    // Step 7: reuse emission from state map.
    let _reuse_result = crate::aims::emit_reuse::emit_reuse(func, &state_map, config.pool);

    // Step 9: ARC IR verification after emission.
    super::run_verify(func, "after AIMS emission", config.verify_arc);

    // Step 10: tail call detection + loop lowering.
    func.tail_calls = crate::tail_call::detect_tail_calls(func);
    crate::tail_call::rewrite_tail_calls(func);

    // Step 11: block merge (CFG cleanup).
    crate::block_merge::merge_blocks(func);

    // Step 11a: COW annotations (post-merge, derived from state map).
    func.cow_annotations =
        crate::aims::emit_rc::cow::compute_aims_cow_annotations(func, &state_map, config.interner);

    // Step 12: drop hints (post-merge, derived from state map).
    func.drop_hints =
        crate::aims::emit_rc::drop_hints::compute_aims_drop_hints(func, &state_map, config.pool);

    // Step 13: final verification.
    super::run_verify(func, "after AIMS pipeline", config.verify_arc);

    // Step 14: FBIP enforcement.
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

/// Run the AIMS pipeline on all functions (batch entry point).
///
/// Called from within `run_arc_pipeline_all` when the `aims` feature is active.
///
/// 1. Compute interprocedural contracts via `aims::analyze_program()`
/// 2. Apply ownership to function parameters
/// 3. Run per-function pipeline for each function
#[cfg_attr(
    feature = "aims-shadow",
    expect(dead_code, reason = "used in pure AIMS mode, not shadow mode")
)]
pub(crate) fn run_aims_pipeline_all(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
    interner: &ori_ir::StringInterner,
    pool: &Pool,
    builtins: &BuiltinOwnershipSets,
    verify_arc: bool,
) -> Vec<ArcProblem> {
    // Step 1: interprocedural analysis → MemoryContract per function.
    let contracts =
        crate::aims::interprocedural::analyze_program(functions, classifier, builtins, interner);

    // Step 2: apply ownership to function parameters.
    apply_aims_ownership(functions, &contracts);

    // Steps 3–14: per-function pipeline.
    let config = AimsPipelineConfig {
        classifier,
        contracts: &contracts,
        pool,
        interner,
        builtins,
        verify_arc,
    };

    let mut all_problems = Vec::new();
    let mut total_rc = super::rc_count::RcOpCount::default();
    for func in functions.iter_mut() {
        let problems = run_aims_pipeline(func, &config);
        all_problems.extend(problems);
        let rc = super::rc_count::count_rc_ops(func);
        total_rc.inc += rc.inc;
        total_rc.dec += rc.dec;
    }

    tracing::debug!(
        functions = functions.len(),
        rc_inc = total_rc.inc,
        rc_dec = total_rc.dec,
        rc_total = total_rc.total(),
        "AIMS pipeline RC operation totals"
    );

    all_problems
}

/// Apply AIMS ownership annotations to function parameters.
///
/// Sets `ArcParam.ownership` on each function from its `MemoryContract`.
/// Replaces `borrow::apply_borrows()` in the old pipeline.
pub(crate) fn apply_aims_ownership(
    functions: &mut [ArcFunction],
    contracts: &FxHashMap<Name, MemoryContract>,
) {
    for func in functions {
        let Some(contract) = contracts.get(&func.name) else {
            continue;
        };
        for (param, pc) in func.params.iter_mut().zip(&contract.params) {
            param.ownership = param_contract_to_ownership(*pc);
        }
    }
}

/// Convert a `ParamContract` access class to the `Ownership` enum used by
/// `ArcParam`. This bridges the AIMS contract representation with the
/// existing ARC IR parameter ownership field.
fn param_contract_to_ownership(pc: ParamContract) -> Ownership {
    match pc.access {
        AccessClass::Borrowed => Ownership::Borrowed,
        AccessClass::Owned => Ownership::Owned,
    }
}
