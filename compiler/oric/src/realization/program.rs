//! Closed-program realization over explicit immutable frontend facts.

mod aims;
mod error;
mod error_mapping;
mod impl_targets;
mod inputs;
mod user_drop;

use ori_ir::canon::CanonResult;
use ori_ir::{Name, SharedInterner, StringInterner};
use ori_parse::ParseOutput;
use ori_repr::executable::{
    ExecutableProgram, ExecutableProgramParts, UserDropBinding, ValidatedExternalCallables,
    EXECUTABLE_PROGRAM_VERSION,
};
use ori_types::{FunctionSig, Pool, TypeCheckResult, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::realization::arc_batch::MethodTargetMap;

use super::{
    close_generic_mono_targets, compute_module_repr_plan, extend_mono_method_targets,
    generic_type_param_map, lower_impl_methods_for_analysis,
    lower_non_generic_derived_methods_for_analysis, ArcFunctionGroup, CallableCensusBuilder,
    GenericMonoClosureInput, ImplMethodAnalysis, LoweredArcBatch, ModuleReprInput,
};
use aims::run_aims;
use error_mapping::{arc_lowering_error, map_arc_batch_error, map_generic_mono_closure_error};

pub use error::ProgramRealizationError;
pub(crate) use impl_targets::rewrite_impl_targets;
pub use inputs::RealizationPolicy;
pub(crate) use inputs::{CheckedModuleFacts, ImportedReprSurfaces};
use user_drop::collect_user_drop_bindings;

/// Explicit inputs to backend-neutral local-module realization.
#[derive(Debug)]
pub struct ProgramRealizationInput<'a> {
    /// Parsed module and arena.
    pub parse: &'a ParseOutput,
    /// Type-checked module metadata.
    pub types: &'a TypeCheckResult,
    /// Canonical IR shared with the independent evaluator oracle.
    pub canon: &'a CanonResult,
    /// Type pool for the realized artifact.
    pub pool: Pool,
    /// Shared symbol storage retained without a compiler database.
    pub symbols: SharedInterner,
    /// Narrowing and ARC-oracle policy resolved by the outer compiler driver.
    pub policy: RealizationPolicy,
}

/// Complete pre-AIMS ARC batch and immutable facts for one executable unit.
pub(crate) struct ArcProgramRealizationInput<'a> {
    /// The only prepared body inventory and its exact parent/lambda topology.
    pub prepared: super::PreparedArcBatch,
    /// Type pool for bodies and imported callable facts.
    pub pool: Pool,
    /// Immutable symbol storage retained by the resulting artifact.
    pub symbols: SharedInterner,
    /// Checked frontend facts the derived plans are computed from.
    pub facts: CheckedModuleFacts<'a>,
    /// Producer-frozen callables linked from other compiled units.
    pub externals: ValidatedExternalCallables,
    /// Typed user-drop bindings collected while lowering this batch.
    pub typed_user_drop_bindings: &'a [UserDropBinding],
    /// Narrowing and ARC-oracle policy resolved by the calling driver.
    pub policy: RealizationPolicy,
}

/// Immutable plans the shared entry derives before running the calculus.
struct FrozenProgramPlans {
    repr_plan: ori_repr::ReprPlan,
    type_registry: TypeRegistry,
    user_drop_bindings: Vec<UserDropBinding>,
}

#[derive(Clone, Copy)]
struct TopLevelMonoSources<'a> {
    functions: &'a [ori_repr::monomorphize::MonoFunction],
    accepted_derives: &'a [ori_types::AcceptedDerivedImpl],
    derived_call_plans: &'a [ori_types::DerivedCallPlan],
}

/// Realize one checked module into the closed artifact executable backends consume.
/// One-shot lowered assembly of every module-local callable group before the
/// generic-mono fixed point closes over it.
struct LoweredCallableAssembly {
    groups: Vec<ArcFunctionGroup>,
    mono_functions: Vec<ori_repr::monomorphize::MonoFunction>,
    impl_targets: MethodTargetMap,
    impl_producer_targets: FxHashMap<ori_types::MethodProducer, Name>,
    typed_user_drop_bindings: Vec<UserDropBinding>,
    function_sigs: Vec<FunctionSig>,
}

/// Lower impl methods, derived methods, and top-level functions into one
/// combined callable-group set, merging their method-target dispositions.
fn lower_all_callable_groups(
    input: &ProgramRealizationInput<'_>,
) -> Result<LoweredCallableAssembly, ProgramRealizationError> {
    let interner = &*input.symbols;
    let function_sigs = crate::typeck::build_function_sigs(input.parse, input.types);
    let mono_functions = ori_repr::monomorphize::collect_mono_functions(
        &input.types.typed.mono_instances,
        &function_sigs,
        &input.types.typed.impl_sigs,
        &input.types.typed.accepted_derives,
        &[],
        interner,
        &input.pool,
    );
    let ImplMethodAnalysis {
        groups: impl_groups,
        targets: mut impl_targets,
        producer_targets: impl_producer_targets,
        user_drop_bindings: typed_user_drop_bindings,
        ..
    } = lower_impl_methods_for_analysis(
        input.parse,
        input.types,
        interner,
        input.canon,
        &input.pool,
    )
    .map_err(arc_lowering_error)?;
    let derived = lower_non_generic_derived_methods_for_analysis(
        &input.types.typed.accepted_derives,
        &input.types.typed.derived_call_plans,
        interner,
        &input.pool,
    )
    .map_err(arc_lowering_error)?;
    let groups = lower_top_level_functions(
        input.parse,
        input.canon,
        interner,
        &input.pool,
        &function_sigs,
        TopLevelMonoSources {
            functions: &mono_functions,
            accepted_derives: &input.types.typed.accepted_derives,
            derived_call_plans: &input.types.typed.derived_call_plans,
        },
    )?;
    for (key, target) in derived.targets {
        impl_targets.entry(key).or_insert(target);
    }
    extend_mono_method_targets(&mut impl_targets, &mono_functions, interner, &input.pool)
        .map_err(arc_lowering_error)?;
    let mut groups = groups;
    groups.extend(impl_groups);
    groups.extend(derived.groups);
    CallableCensusBuilder::new(interner).close_builtin_targets(&mut groups, &input.pool)?;
    Ok(LoweredCallableAssembly {
        groups,
        mono_functions,
        impl_targets,
        impl_producer_targets,
        typed_user_drop_bindings,
        function_sigs,
    })
}

#[must_use = "success or failure must be handled"]
pub fn realize_local_program(
    mut input: ProgramRealizationInput<'_>,
) -> Result<ExecutableProgram, ProgramRealizationError> {
    let symbols = input.symbols.clone();
    let interner = &*symbols;
    let LoweredCallableAssembly {
        groups,
        mono_functions,
        mut impl_targets,
        impl_producer_targets,
        typed_user_drop_bindings,
        function_sigs,
    } = lower_all_callable_groups(&input)?;
    let local_generic_type_params = generic_type_param_map(&input.types.typed.types);
    let closed = close_generic_mono_targets(GenericMonoClosureInput {
        groups,
        mono_functions,
        mono_instances: &input.types.typed.mono_instances,
        function_sigs: &function_sigs,
        local_generic_type_params: &local_generic_type_params,
        impl_sigs: &input.types.typed.impl_sigs,
        accepted_derives: &input.types.typed.accepted_derives,
        derived_call_plans: &input.types.typed.derived_call_plans,
        import_sigs: &[],
        imported_generic_templates: &[],
        re_interned_canons: &[],
        canon: input.canon,
        interner,
        pool: &mut input.pool,
    })
    .map_err(map_generic_mono_closure_error)?;
    let groups = closed.groups;
    let mono_functions = closed.mono_functions;
    extend_mono_method_targets(&mut impl_targets, &mono_functions, interner, &input.pool)
        .map_err(arc_lowering_error)?;
    let batch = LoweredArcBatch::try_from_groups(groups, interner).map_err(map_arc_batch_error)?;
    let prepared = batch
        .prepare(
            &mono_functions,
            &impl_targets,
            &impl_producer_targets,
            &input.types.typed.method_producers,
            &input.pool,
            interner,
        )
        .map_err(map_arc_batch_error)?;
    realize_arc_program(ArcProgramRealizationInput {
        prepared,
        pool: input.pool,
        symbols: input.symbols,
        facts: CheckedModuleFacts {
            parse: input.parse,
            types: input.types,
            function_sigs: &function_sigs,
            interner,
            imported_repr: ImportedReprSurfaces::default(),
        },
        externals: ValidatedExternalCallables::empty(),
        typed_user_drop_bindings: &typed_user_drop_bindings,
        policy: input.policy,
    })
}

/// Derive the registry, representation plan, and user-drop bindings every
/// driver would otherwise assemble independently.
fn derive_program_plans(
    prepared: &super::PreparedArcBatch,
    pool: &Pool,
    facts: CheckedModuleFacts<'_>,
    typed_user_drop_bindings: &[UserDropBinding],
    policy: RealizationPolicy,
) -> Result<FrozenProgramPlans, ProgramRealizationError> {
    let mut type_registry = TypeRegistry::from_typed_exports(
        facts.types.typed.types.clone(),
        facts.types.typed.collection_burdens.clone(),
    );
    ori_types::register_resolved_collection_burdens(pool, &mut type_registry);
    let repr_plan = compute_module_repr_plan(ModuleReprInput {
        pool,
        arc_functions: prepared.functions(),
        narrowing_policy: policy.narrowing,
        type_result: facts.types,
        interner: Some(facts.interner),
        imported_type_metadata: facts.imported_repr.type_metadata,
        imported_collection_surfaces: facts.imported_repr.collection_surfaces,
        has_analysis_only_functions: facts.has_analysis_only_functions(),
    });
    let user_drop_bindings =
        collect_user_drop_bindings(&type_registry, typed_user_drop_bindings, pool)?;
    Ok(FrozenProgramPlans {
        repr_plan,
        type_registry,
        user_drop_bindings,
    })
}

/// Assemble the derived plans, run the backend-neutral calculus exactly once,
/// and close its artifact.
///
/// This is the sole `ExecutableProgram::validate` edge. Every driver reaches it
/// with the same checked frontend facts, so no caller can select its own roots,
/// entry point, registry, representation plan, or drop bindings.
#[must_use = "success or failure must be handled"]
pub(crate) fn realize_arc_program(
    input: ArcProgramRealizationInput<'_>,
) -> Result<ExecutableProgram, ProgramRealizationError> {
    let ArcProgramRealizationInput {
        prepared,
        pool,
        symbols,
        facts,
        externals,
        typed_user_drop_bindings,
        policy,
    } = input;
    let FrozenProgramPlans {
        mut repr_plan,
        type_registry,
        user_drop_bindings,
    } = derive_program_plans(&prepared, &pool, facts, typed_user_drop_bindings, policy)?;
    let roots = prepared.parent_roots();
    let cli_entry = facts.cli_entry();
    let verify_arc = policy.verify_arc;
    let (mut functions, function_families, method_targets) = prepared.into_executable_parts();

    // External ownership/effect policy is producer-owned. Reject any stale,
    // incomplete, or signature-mismatched transport record before its
    // contract can seed the AIMS fixed point.
    let external_contracts = externals
        .iter()
        .map(|external| (external.name(), external.contract().clone()))
        .collect();
    let callable_boundaries = ori_arc::CallableBoundaryFacts::from_user_drop_targets(
        user_drop_bindings
            .iter()
            .map(|binding| (binding.target(), binding.ty(), binding.logical())),
    )
    .map_err(|error| ProgramRealizationError::ArcVerification {
        count: 1,
        errors: vec![error.into()],
    })?;
    let aims = run_aims(
        &mut functions,
        &pool,
        &symbols,
        &type_registry,
        &external_contracts,
        &callable_boundaries,
        verify_arc,
    )?;
    repr_plan.freeze_yield_allocations(&functions);
    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols,
        pool,
        functions,
        function_families,
        contracts: aims.contracts,
        function_effects: aims.function_effects,
        fresh_return_facts: aims.fresh_return_facts,
        param_disjointness: aims.param_disjointness,
        closure_adapters: aims.closure_adapters,
        retain_plans: aims.retain_plans,
        callable_facts: aims.callable_facts,
        roots,
        cli_entry,
        externals,
        method_targets,
        user_drop_bindings,
        repr_plan,
        type_registry,
    })
    .map_err(ProgramRealizationError::from)
}

fn lower_top_level_functions(
    parse: &ParseOutput,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    function_sigs: &[FunctionSig],
    monos: TopLevelMonoSources<'_>,
) -> Result<Vec<ArcFunctionGroup>, ProgramRealizationError> {
    let mut groups = Vec::new();
    let mut problems = Vec::new();
    let source_seeds = CallableCensusBuilder::new(interner)
        .source_functions(&parse.module.functions, function_sigs)?;
    for seed in source_seeds {
        let function = seed.function;
        let signature = seed.signature;
        if signature.requires_specialization() {
            continue;
        }
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon,
            interner,
            pool,
            problems: &mut problems,
        };
        let lowered = crate::arc_lowering::lower_to_arc(
            function.name,
            signature,
            function.name,
            &mut context,
            None,
        );
        groups.push(lowered.into());
    }
    let mut mono_context = crate::arc_lowering::ArcLoweringContext {
        canon,
        interner,
        pool,
        problems: &mut problems,
    };
    groups.extend(super::lower_mono_functions_for_analysis(
        monos.functions,
        monos.accepted_derives,
        monos.derived_call_plans,
        &mut mono_context,
    ));
    if problems.is_empty() {
        Ok(groups)
    } else {
        Err(arc_lowering_error(problems))
    }
}

#[cfg(test)]
mod tests;
