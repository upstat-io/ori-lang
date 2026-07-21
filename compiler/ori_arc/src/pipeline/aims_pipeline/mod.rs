//! Per-function AIMS realization pipeline.
//!
//! Interprocedural analysis freezes contracts before this module consumes them.
//! Each function normalizes IR, solves the lattice, materializes logical
//! ownership events and reuse, verifies them, rewrites control flow, and derives
//! post-merge COW/drop annotations. Physical projections consume the frozen
//! logical events through their own representation plans.
//!
//! Checkpoint diagnostics share the `ori_arc::aims::pipeline` target.

mod batch;
mod burden_emission;
mod postprocess;
mod trmc;

use ori_ir::Name;
use ori_types::TypeRegistry;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{ContractMapExt, MemoryContract};
use crate::borrow::BuiltinOwnershipSets;
use crate::ir::{ArcFunction, ArcInstr, ArcVarId, ArgOwnership};
use crate::lower::ArcProblem;
use crate::pipeline::rc_count;
use crate::ArcClassification;

pub(crate) use batch::{
    run_aims_pipeline_all_with_external_contracts, run_aims_pipeline_all_with_observer,
};

/// Receives stable function snapshots at realization checkpoints.
pub type CheckpointObserver<'a> = dyn Fn(&ArcFunction, &str /* phase */) + 'a;

/// Emits structural and ownership metrics and invokes `observer`.
///
/// The predictable `ori_arc::aims::pipeline` target supports pass bisection;
/// disabled tracing avoids metric and name computation.
pub(crate) fn trace_pipeline_checkpoint(
    func: &ArcFunction,
    phase: &str,
    interner: &ori_ir::StringInterner,
    observer: Option<&CheckpointObserver<'_>>,
) {
    // Why: Disabled tracing must not pay for count and name computation.
    if tracing::enabled!(target: "ori_arc::aims::pipeline", tracing::Level::INFO) {
        let fn_name = interner.lookup(func.name);
        let rc = rc_count::count_rc_ops(func);
        let blocks = func.blocks.len();
        let vars = func.var_types.len();
        // INVARIANT: Burden sites retain coordinates for pass bisection.
        let mut burden_sites: Vec<String> = Vec::new();
        for (b, block) in func.blocks.iter().enumerate() {
            for (i, instr) in block.body.iter().enumerate() {
                let kind = match instr {
                    ArcInstr::BurdenInc { var } => format!("bb{b}.{i}:binc%{}", var.index()),
                    ArcInstr::BurdenDec { var }
                    | ArcInstr::BurdenDecPartial { var, .. }
                    | ArcInstr::BurdenDecVariant { var } => {
                        format!("bb{b}.{i}:bdec%{}", var.index())
                    }
                    _ => continue,
                };
                burden_sites.push(kind);
            }
        }
        tracing::info!(
            target: "ori_arc::aims::pipeline",
            function = fn_name,
            phase,
            rc_incs = rc.inc,
            rc_decs = rc.dec,
            blocks,
            vars,
            burden_sites = %burden_sites.join(" "),
            "AIMS phase checkpoint"
        );
    }
    if let Some(obs) = observer {
        obs(func, phase);
    }
}

/// Shared immutable inputs for one per-function realization.
pub(crate) struct AimsPipelineConfig<'a> {
    pub classifier: &'a dyn ArcClassification,
    pub contracts: &'a FxHashMap<Name, MemoryContract>,
    /// Exact local functions whose contracts must be present.
    pub func_names: &'a FxHashSet<Name>,
    /// Local and producer-validated external callables whose exact contracts
    /// take precedence over same-spelled builtin ownership heuristics.
    pub exact_callables: &'a FxHashSet<Name>,
    pub pool: &'a ori_types::Pool,
    pub interner: &'a ori_ir::StringInterner,
    pub builtins: &'a BuiltinOwnershipSets,
    pub verify_arc: bool,
    /// Receives each stable checkpoint when snapshot capture is enabled.
    pub observer: Option<&'a CheckpointObserver<'a>>,
    /// Type information required to freeze class-ledger plans.
    pub type_registry: &'a TypeRegistry,
}

/// Result of `run_aims_pipeline` for a single function.
pub(crate) struct AimsPipelineResult {
    pub problems: Vec<ArcProblem>,
    /// Post-emission missed reuse count from `FipEvidence`. Used by the
    /// second pass to compute `may_deallocate` (> 0) and to re-verify
    /// `Bounded(n)` contracts with accurate counts.
    pub missed_reuses: usize,
    /// Whether this function was TRMC-rewritten (and the rewrite survived
    /// both structural and semantic verification). Used by the second pass
    /// to mark `has_unbounded_stack = false` on refreshed contracts.
    pub was_trmc_rewritten: bool,
}

/// Realizes one function after closed-program contracts are frozen.
///
/// Returns verification failures only when explicit ARC verification is enabled;
/// such failures represent compiler invariants, not user diagnostics.
pub(crate) fn run_aims_pipeline(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<AimsPipelineResult, Vec<crate::verify::VerifyError>> {
    // INVARIANT: Primitive facts are resolved once and otherwise validated unchanged.
    crate::aims::primitive::ensure_primitive_facts(func, config.classifier)?;

    let (norm_result, immortals, did_trmc_transform, pre_trmc_func) =
        trmc::normalize_with_trmc(func, config)?;
    trace_pipeline_checkpoint(
        func,
        "normalize_with_trmc_complete",
        config.interner,
        config.observer,
    );
    // INVARIANT: Structural rewrites preserve exact primitive-destination coverage.
    crate::aims::primitive::ensure_primitive_facts(func, config.classifier)?;

    let state_map = {
        let _span = tracing::info_span!("analyze_function").entered();
        crate::aims::intraprocedural::analyze_function(
            func,
            config.classifier,
            config.contracts,
            &norm_result.context_regions,
            immortals,
        )
    };
    trace_pipeline_checkpoint(func, "analyze_function", config.interner, config.observer);

    let (mut state_map, trmc_rewrite_survived) =
        trmc::verify_trmc_soundness(func, state_map, did_trmc_transform, pre_trmc_func, config);
    trace_pipeline_checkpoint(
        func,
        "verify_trmc_soundness",
        config.interner,
        config.observer,
    );

    freeze_yield_allocation_locality(func, &state_map);

    // INVARIANT: Converged state fixes the birth-site partition before burden emission.
    let birth_site_partition =
        crate::aims::intraprocedural::birth_site_population::compute_birth_site_partition(
            func, &state_map,
        );
    state_map.set_birth_site_partition(birth_site_partition);

    // INVARIANT: Payload analysis reads unannotated IR; class planning reads converged ownership.
    {
        let _span = tracing::debug_span!("emit_arg_ownership_prelude").entered();
        crate::aims::emit_rc::arg_ownership::emit_arg_ownership(
            func,
            config.contracts,
            config.interner,
            config.builtins,
            config.pool,
            config.exact_callables,
        )?;
    }
    trace_pipeline_checkpoint(
        func,
        "emit_arg_ownership_prelude",
        config.interner,
        config.observer,
    );

    // INVARIANT: Class-ledger planning is the sole producer of ownership events.
    crate::aims::class_ledger::apply_class_ledger_replacement(
        func,
        &state_map,
        config.contracts,
        config.type_registry,
        config.interner,
        burden_emission::burden_ops_enabled(),
    );
    crate::aims::realize::emit_survivor_remarks_all_kept(func, &state_map, config.interner);
    trace_pipeline_checkpoint(
        func,
        "class_ledger_emission",
        config.interner,
        config.observer,
    );

    burden_emission::dump_after_burden(func, config);
    burden_emission::dump_after_class_ledger_emission_compat(func, config);

    let mut result = {
        let _span = tracing::info_span!("realize_rc_reuse").entered();
        crate::aims::realize::realize_rc_reuse(
            func,
            &state_map,
            config.contracts,
            config.interner,
            config.builtins,
            config.pool,
            config.type_registry,
        )
    };
    trace_pipeline_checkpoint(func, "realize_rc_reuse", config.interner, config.observer);

    let missed_reuses = result.fip_evidence.missed_reuses;

    fip_precheck(func, config, &result)?;
    trace_pipeline_checkpoint(
        func,
        "verify_fip_contract",
        config.interner,
        config.observer,
    );

    result.synergy_metrics.canonicalize_cross_fires = state_map.count_cross_dim_states();

    postprocess::verify_and_merge(func, config)?;

    // INVARIANT: Rewrites preserve authoritative variable metadata through handoff.
    validate_metadata_checkpoint(func, config)?;

    apply_phase_2_annotations(func, &state_map, config, &mut result);

    let problems = finish_postprocess(func, config)?;

    Ok(AimsPipelineResult {
        problems,
        missed_reuses,
        was_trmc_rewritten: trmc_rewrite_survived,
    })
}

fn finish_postprocess(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<Vec<ArcProblem>, Vec<crate::verify::VerifyError>> {
    let problems = postprocess::emit_postprocess(func, config)?;
    freeze_yield_allocation_execution(func);
    freeze_yield_runtime_header_requirements(func, config);
    Ok(problems)
}

/// Freeze AIMS placement eligibility onto stable yield-allocation identities.
fn freeze_yield_allocation_locality(
    func: &mut ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
) {
    let returned_yield_results: FxHashSet<_> = func
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            crate::ir::ArcTerminator::Return { value } => Some(value),
            _ => None,
        })
        .filter_map(|returned| crate::yield_result_for_receiver_lineage(func, returned))
        .collect();
    let mut eligible = FxHashSet::default();
    for block in &func.blocks {
        for event in state_map.events_in_block(block.id) {
            if let crate::aims::intraprocedural::AimsEvent::PlacementEligibilityCandidate {
                var,
                ..
            } = event
            {
                eligible.insert(*var);
            }
        }
    }
    for fact in &mut func.yield_allocations {
        fact.locality =
            if eligible.contains(&fact.result) && !returned_yield_results.contains(&fact.result) {
                crate::ir::YieldAllocationLocality::Local
            } else {
                crate::ir::YieldAllocationLocality::Escaping
            };
    }
}

/// Freeze representation-owned execution evidence from the final CFG.
///
/// A function-entry stack slot has one physical identity. It can represent a
/// yield allocation only when the builder's defining block cannot be revisited
/// in the same invocation; otherwise separate dynamic results may overlap. This
/// projection fact does not alter AIMS locality or logical event identities.
fn freeze_yield_allocation_execution(func: &mut ArcFunction) {
    let cycle_regions = crate::graph::CycleRegions::compute(func);
    let definition_blocks: Vec<Option<usize>> = func
        .yield_allocations
        .iter()
        .map(|fact| {
            func.blocks.iter().position(|block| {
                block
                    .body
                    .iter()
                    .any(|instruction| instruction.defined_var() == Some(fact.builder))
            })
        })
        .collect();
    for (fact, definition_block) in func.yield_allocations.iter_mut().zip(definition_blocks) {
        fact.execution = match definition_block {
            Some(block) if !cycle_regions.is_in_cycle(block) => {
                crate::ir::YieldAllocationExecution::SingleExecution
            }
            _ => crate::ir::YieldAllocationExecution::RepeatedOrUnknown,
        };
    }
}

/// Clear the runtime-header requirement only for a closed, header-independent
/// primitive-scalar lineage.
///
/// This is ownership evidence, not a backend layout choice: a physical plan
/// may use compact storage only when no realized use can observe sharing state,
/// invoke a header-dependent collection operation, or require element cleanup.
fn freeze_yield_runtime_header_requirements(
    func: &mut ArcFunction,
    config: &AimsPipelineConfig<'_>,
) {
    let elidable: Vec<bool> = func
        .yield_allocations
        .iter()
        .map(|fact| runtime_header_is_elidable(func, *fact, config))
        .collect();
    for (fact, elidable) in func.yield_allocations.iter_mut().zip(elidable) {
        fact.requires_runtime_header = !elidable;
    }
}

fn runtime_header_is_elidable(
    func: &ArcFunction,
    fact: crate::ir::YieldAllocationFact,
    config: &AimsPipelineConfig<'_>,
) -> bool {
    use ori_registry::TypeTag;

    if fact.locality != crate::ir::YieldAllocationLocality::Local
        || !config.classifier.is_scalar(fact.elem_ty)
        || !matches!(
            config.classifier.builtin_type_tag(fact.elem_ty),
            Some(
                TypeTag::Int
                    | TypeTag::Float
                    | TypeTag::Bool
                    | TypeTag::Char
                    | TypeTag::Byte
                    | TypeTag::Unit
                    | TypeTag::Never
                    | TypeTag::Duration
                    | TypeTag::Size
                    | TypeTag::Ordering
            )
        )
    {
        return false;
    }

    let in_lineage = |var| {
        crate::yield_result_for_receiver_lineage(func, var)
            .is_some_and(|result| result == fact.result)
    };

    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if !instr.used_vars().iter().copied().any(&in_lineage) {
                continue;
            }
            let allowed = match instr {
                ArcInstr::Let {
                    value: crate::ir::ArcValue::Var(source),
                    ..
                } => in_lineage(*source),
                // The ARC list projection at field zero is the logical length.
                // Its scalar result carries neither the data pointer nor an RC
                // header address, so later uses cannot escape the lineage.
                ArcInstr::Project {
                    value, field: 0, ..
                } => in_lineage(*value),
                ArcInstr::RcDec { var, .. } => in_lineage(*var),
                ArcInstr::Apply {
                    dst,
                    func: callee,
                    args,
                    arg_ownership,
                    ..
                } => header_independent_lineage_call(
                    func,
                    config,
                    HeaderIndependentCall {
                        callee: *callee,
                        args,
                        arg_ownership,
                        dst: *dst,
                        position: (block_idx, instr_idx),
                    },
                    &in_lineage,
                ),
                _ => false,
            };
            if !allowed {
                return false;
            }
        }

        if !block
            .terminator
            .used_vars()
            .iter()
            .copied()
            .any(&in_lineage)
        {
            continue;
        }
        let allowed = match &block.terminator {
            crate::ir::ArcTerminator::Jump { .. } => true,
            crate::ir::ArcTerminator::Invoke {
                dst,
                func: callee,
                args,
                arg_ownership,
                ..
            } => header_independent_lineage_call(
                func,
                config,
                HeaderIndependentCall {
                    callee: *callee,
                    args,
                    arg_ownership,
                    dst: *dst,
                    position: (block_idx, block.body.len()),
                },
                &in_lineage,
            ),
            _ => false,
        };
        if !allowed {
            return false;
        }
    }

    true
}

#[derive(Clone, Copy)]
struct HeaderIndependentCall<'a> {
    callee: Name,
    args: &'a [ArcVarId],
    arg_ownership: &'a [ArgOwnership],
    dst: ArcVarId,
    position: (usize, usize),
}

fn header_independent_lineage_call(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
    call: HeaderIndependentCall<'_>,
    in_lineage: &impl Fn(ArcVarId) -> bool,
) -> bool {
    // This phase proves the operation shape and ownership behavior. The closed
    // executable artifact subsequently verifies that every admitted spelling
    // resolves to a Runtime target, preventing same-named user/imported calls
    // from authorizing compact storage.
    if !call.args.first().copied().is_some_and(in_lineage)
        || call.args.iter().skip(1).copied().any(in_lineage)
    {
        return false;
    }

    match header_independent_operation(func, config, call) {
        Some(HeaderIndependentOperation::BorrowedRead) => {
            call.arg_ownership.first() == Some(&ArgOwnership::Borrowed)
        }
        Some(HeaderIndependentOperation::StaticUniqueListSet) => {
            in_lineage(call.dst)
                && func.cow_annotations.get(call.position.0, call.position.1)
                    == crate::CowMode::StaticUnique
        }
        None => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderIndependentOperation {
    BorrowedRead,
    StaticUniqueListSet,
}

fn header_independent_operation(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
    call: HeaderIndependentCall<'_>,
) -> Option<HeaderIndependentOperation> {
    let receiver = call.args.first().copied()?;
    let callee = config.interner.try_lookup(call.callee)?;

    if ori_ir::builtin_constants::protocol::ProtocolBuiltin::from_name(callee)
        == Some(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index)
    {
        return (call.args.len()
            == ori_ir::builtin_constants::protocol::ProtocolBuiltin::Index.arg_count())
        .then_some(HeaderIndependentOperation::BorrowedRead);
    }

    let receiver_tag = config
        .pool
        .builtin_method_type_tag(func.var_type(receiver))?;
    registered_header_independent_operation(receiver_tag, callee, call.args.len())
}

fn registered_header_independent_operation(
    receiver_tag: ori_registry::TypeTag,
    callee: &str,
    arg_count: usize,
) -> Option<HeaderIndependentOperation> {
    let method = ori_registry::find_method(receiver_tag, callee)?;
    if method.kind != ori_registry::MethodKind::Instance
        || method.params.len().saturating_add(1) != arg_count
    {
        return None;
    }

    match method.runtime {
        Some(ori_registry::MethodRuntime::Length) => Some(HeaderIndependentOperation::BorrowedRead),
        Some(ori_registry::MethodRuntime::ListSet) => {
            Some(HeaderIndependentOperation::StaticUniqueListSet)
        }
        _ => None,
    }
}

pub(super) fn validate_variable_metadata(
    func: &ArcFunction,
    classifier: &dyn crate::ArcClassification,
    pool: &ori_types::Pool,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let mut errors = Vec::new();
    if func.var_metadata_state != crate::ir::VariableMetadataState::Realized {
        errors.push(crate::verify::VerifyError::VariableMetadataUnrealized);
    }
    let expected_representations = crate::ir::compute_var_reprs(func, classifier, pool);
    errors.extend(representation_metadata_errors(
        func,
        &expected_representations,
    ));

    let expected_strategies =
        crate::ir::derive_var_rc_strategies(&expected_representations, &func.var_types, pool);
    errors.extend(rc_strategy_metadata_errors(func, &expected_strategies));

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_metadata_checkpoint(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    validate_variable_metadata(func, config.classifier, config.pool)?;
    trace_pipeline_checkpoint(
        func,
        "validate_variable_metadata",
        config.interner,
        config.observer,
    );
    Ok(())
}

fn representation_metadata_errors(
    func: &ArcFunction,
    expected: &[crate::ir::ValueRepr],
) -> Vec<crate::verify::VerifyError> {
    use crate::verify::VerifyError;

    if func.var_reprs.len() == func.var_types.len() {
        expected
            .iter()
            .zip(&func.var_reprs)
            .enumerate()
            .filter(|(_, (expected, found))| expected != found)
            .map(
                |(index, (&expected, &found))| VerifyError::VariableRepresentationMismatch {
                    var: variable_id(index),
                    expected,
                    found,
                },
            )
            .collect()
    } else {
        vec![VerifyError::VariableMetadataLength {
            table: "representation",
            variables: func.var_types.len(),
            entries: func.var_reprs.len(),
        }]
    }
}

fn rc_strategy_metadata_errors(
    func: &ArcFunction,
    expected: &[Option<crate::ir::RcStrategy>],
) -> Vec<crate::verify::VerifyError> {
    use crate::verify::VerifyError;

    if func.var_rc_strategies.len() == func.var_types.len() {
        expected
            .iter()
            .zip(&func.var_rc_strategies)
            .enumerate()
            .filter(|(_, (expected, found))| expected != found)
            .map(
                |(index, (&expected, &found))| VerifyError::VariableRcStrategyMismatch {
                    var: variable_id(index),
                    expected,
                    found,
                },
            )
            .collect()
    } else {
        vec![VerifyError::VariableMetadataLength {
            table: "RC-strategy",
            variables: func.var_types.len(),
            entries: func.var_rc_strategies.len(),
        }]
    }
}

fn variable_id(index: usize) -> crate::ir::ArcVarId {
    crate::ir::ArcVarId::new(
        u32::try_from(index).unwrap_or_else(|_| panic!("variable index exceeds u32::MAX")),
    )
}

/// Installs post-merge COW and drop annotations on `func`.
fn apply_phase_2_annotations(
    func: &mut ArcFunction,
    state_map: &crate::aims::intraprocedural::AimsStateMap,
    config: &AimsPipelineConfig<'_>,
    result: &mut crate::aims::realize::RealizationResult,
) {
    {
        let _span = tracing::info_span!("realize_annotations").entered();
        let env = crate::aims::realize::AnnotationEnv {
            state_map,
            interner: config.interner,
            pool: config.pool,
            contracts: config.contracts,
            builtins: config.builtins,
            func_names: config.func_names,
        };
        crate::aims::realize::realize_annotations(func, &env, result);
    }
    trace_pipeline_checkpoint(
        func,
        "realize_annotations",
        config.interner,
        config.observer,
    );
    func.cow_annotations = std::mem::take(&mut result.cow_annotations);
    func.drop_hints = std::mem::take(&mut result.drop_hints);
}

/// Rejects structural FIP violations before the contract refresh.
///
/// Missed reuses may change `may_deallocate` and are rechecked after refresh;
/// unbounded stack or exceeded bounds are already final here.
fn fip_precheck(
    func: &ArcFunction,
    config: &AimsPipelineConfig<'_>,
    result: &crate::aims::realize::RealizationResult,
) -> Result<(), Vec<crate::verify::VerifyError>> {
    let contract = config.contracts.get_required(&func.name, "fip_precheck");
    let fip_errors =
        crate::aims::verify::fip::verify_fip_contract(func.name, contract, &result.fip_evidence);
    let mut structural_errors = Vec::new();
    for e in &fip_errors {
        use crate::aims::verify::fip::FipVerificationError;
        match e {
            FipVerificationError::CertifiedButHasMissedReuses { .. } => {
                // Why: Missed reuses feed the contract refresh before final verification.
                tracing::debug!("FIP pre-check (will recompute in second pass): {e}");
            }
            FipVerificationError::CertifiedButUnboundedStack { .. }
            | FipVerificationError::BoundedExceeded { .. } => {
                // INVARIANT: Stack and bound facts are final before emission.
                tracing::error!("FIP verification failed: {e}");
                if config.verify_arc {
                    structural_errors.push(crate::verify::VerifyError::FipStructural {
                        message: e.to_string(),
                    });
                }
            }
        }
    }
    if structural_errors.is_empty() {
        Ok(())
    } else {
        Err(structural_errors)
    }
}

#[cfg(test)]
mod header_independent_operation_tests {
    use super::{registered_header_independent_operation, HeaderIndependentOperation};

    #[test]
    fn registry_runtime_identity_classifies_all_list_aliases() {
        for name in ["len", "length"] {
            assert_eq!(
                registered_header_independent_operation(ori_registry::TypeTag::List, name, 1),
                Some(HeaderIndependentOperation::BorrowedRead),
            );
        }
        for name in ["set", "updated"] {
            assert_eq!(
                registered_header_independent_operation(ori_registry::TypeTag::List, name, 3),
                Some(HeaderIndependentOperation::StaticUniqueListSet),
            );
        }
    }

    #[test]
    fn registry_runtime_identity_rejects_wrong_shape_and_unrelated_methods() {
        assert_eq!(
            registered_header_independent_operation(ori_registry::TypeTag::List, "length", 2),
            None,
        );
        assert_eq!(
            registered_header_independent_operation(ori_registry::TypeTag::List, "push", 2),
            None,
        );
    }
}
