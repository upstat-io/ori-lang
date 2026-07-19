//! Closed-program realization over explicit immutable frontend facts.

mod error_mapping;
mod impl_targets;

use ori_arc::ArcFunction;
use ori_ir::canon::CanonResult;
use ori_ir::{Name, SharedInterner, StringInterner};
use ori_parse::ParseOutput;
use ori_repr::executable::{
    ExecutableProgram, ExecutableProgramParts, ExternalCallable, RealizationError, UserDropBinding,
    EXECUTABLE_PROGRAM_VERSION,
};
use ori_types::{FunctionSig, Idx, Pool, TypeCheckResult, TypeRegistry};
use rustc_hash::FxHashMap;

use super::{
    close_generic_mono_targets, compute_module_repr_plan, extend_mono_method_targets,
    generic_type_param_map, lower_impl_methods_for_analysis,
    lower_non_generic_derived_methods_for_analysis, ArcFunctionGroup, CallableCensusBuilder,
    CallableCensusError, GenericMonoClosureInput, ImplMethodAnalysis, LoweredArcBatch,
    ModuleReprInput,
};
use crate::realization::arc_batch::MethodTargetMap;
use error_mapping::{arc_lowering_error, map_arc_batch_error, map_generic_mono_closure_error};
pub(crate) use impl_targets::rewrite_impl_targets;

/// Explicit inputs to backend-neutral local-module realization.
pub struct ProgramRealizationInput<'a> {
    /// Parsed module and arena.
    pub parse: &'a ParseOutput,
    /// Type-checked module metadata.
    pub types: &'a TypeCheckResult,
    /// Canonical IR shared with the independent evaluator oracle.
    pub canon: &'a CanonResult,
    /// Type pool used by every downstream artifact.
    pub pool: Pool,
    /// Shared symbol storage retained without a compiler database.
    pub symbols: SharedInterner,
    /// Representation policy selected by the outer compiler driver.
    pub narrowing_policy: ori_repr::NarrowingPolicy,
    /// Run the optional ARC consistency oracle while freezing the artifact.
    pub verify_arc: bool,
}

/// Complete pre-AIMS ARC batch and immutable facts for one executable unit.
pub(crate) struct ArcProgramRealizationInput {
    /// The only prepared body inventory and its exact parent/lambda topology.
    pub prepared: super::PreparedArcBatch,
    /// Shared type pool used by all bodies and imported callable facts.
    pub pool: Pool,
    /// Immutable symbol storage retained by the resulting artifact.
    pub symbols: SharedInterner,
    /// Explicit nonempty externally reachable roots.
    pub roots: Vec<Name>,
    /// Distinguished standalone-process entry, when present.
    pub cli_entry: Option<Name>,
    /// Producer-frozen callables linked from other compiled units.
    pub externals: Vec<ExternalCallable>,
    /// Exact burden-to-impl bindings for user-defined drop operations.
    pub user_drop_bindings: Vec<UserDropBinding>,
    /// Representation plan computed from this exact body batch.
    pub repr_plan: ori_repr::ReprPlan,
    /// Closed type and burden metadata used by realization and projections.
    pub type_registry: TypeRegistry,
    /// Run the optional ARC consistency oracle while freezing the artifact.
    pub verify_arc: bool,
}

#[derive(Clone, Copy)]
struct TopLevelMonoSources<'a> {
    functions: &'a [ori_repr::monomorphize::MonoFunction],
    accepted_derives: &'a [ori_types::AcceptedDerivedImpl],
    derived_call_plans: &'a [ori_types::DerivedCallPlan],
}

/// A typed failure in frontend-to-executable realization.
#[derive(Debug, thiserror::Error)]
pub enum ProgramRealizationError {
    /// Raw declarations could not form one semantic callable seed inventory.
    #[error(transparent)]
    CallableCensus(#[from] CallableCensusError),
    /// ARC lowering rejected one or more ordinary or monomorphized bodies.
    #[error("ARC lowering produced {count} problem(s): {problems:?}")]
    ArcLowering {
        /// Number of lowering problems.
        count: usize,
        /// Structured lowering problems.
        problems: Vec<ori_arc::ArcProblem>,
    },
    /// Shared pre-AIMS lambda specialization could not make every body concrete.
    #[error("lambda specialization produced {count} error(s): {errors:?}")]
    LambdaSpecialization {
        /// Number of parent/lambda batches that could not be specialized.
        count: usize,
        /// Structured specialization failures.
        errors: Vec<ori_arc::LambdaSpecializationError>,
    },
    /// A user-defined operator lacked one exact callable identity.
    #[error("operator-call resolution produced {count} error(s): {errors:?}")]
    OperatorCallResolution {
        /// Number of unresolved operator sites.
        count: usize,
        /// Structured resolution failures.
        errors: Vec<ori_arc::OperatorCallResolutionError>,
    },
    /// A source-selected method handle did not resolve against typed producer
    /// metadata before callable closure.
    #[error(
        "selected-method producer resolution produced {count} error(s): {errors:?}. This is an internal compiler error; report this complete message"
    )]
    SelectedMethodProducerResolution {
        /// Number of invalid selected call sites.
        count: usize,
        /// Exact invalid handle/conflict descriptions.
        errors: Vec<String>,
    },
    /// The pre-AIMS generic callable census could not reach a closed inventory.
    #[error("generic target census failed: {message}")]
    GenericMonoClosure {
        /// Exact closure failure retained without erasing its actionable context.
        message: String,
    },
    /// Two lowering sources claimed the same parent callable identity.
    #[error(
        "ARC batch contains duplicate parent callable `{parent}` because multiple lowering sources claimed one executable body. Run with `ORI_LOG=oric::realization::arc_batch=debug` and report this compiler error"
    )]
    DuplicateArcParent { parent: String },
    /// One body identity appeared in more than one parent/lambda position.
    #[error(
        "ARC batch body `{body}` appears under both `{first_parent}` and `{second_parent}`; every executable body must belong to exactly one family. Run with `ORI_LOG=oric::realization::arc_batch=debug` and report this compiler error"
    )]
    DuplicateArcBody {
        body: String,
        first_parent: String,
        second_parent: String,
    },
    /// More than one realized impl body claimed the same user-drop operation.
    #[error("user-drop target resolution for type {ty:?} found {targets} callable bodies")]
    AmbiguousUserDropTarget {
        /// Canonical type carrying the user-drop burden.
        ty: Idx,
        /// Number of candidate qualified impl bodies.
        targets: usize,
    },
    /// A type declares a user-drop burden but no exact realized implementation
    /// body was bound before AIMS.
    #[error("user-drop target resolution for type {ty:?} found no callable body")]
    MissingUserDropTarget { ty: Idx },
    /// A typed impl role claimed user-drop semantics for a type whose burden
    /// has no such logical operation.
    #[error("user-drop impl role for type {ty:?} has no matching burden identity")]
    UnexpectedUserDropRole { ty: Idx },
    /// The typed impl role and burden registry disagree on logical identity.
    #[error(
        "user-drop impl role for type {ty:?} carries logical identity {found:?}, expected {expected:?}"
    )]
    UserDropLogicalIdentityMismatch {
        ty: Idx,
        expected: ori_registry::burden::FnSym,
        found: ori_registry::burden::FnSym,
    },
    /// ARC verification rejected post-AIMS IR.
    #[error("post-AIMS verification produced {count} error(s): {errors:?}")]
    ArcVerification {
        /// Number of verification failures.
        count: usize,
        /// Structured verifier failures.
        errors: Vec<ori_arc::verify::VerifyError>,
    },
    /// AIMS completed but reported semantic lowering problems.
    #[error("post-AIMS realization produced {count} problem(s): {problems:?}")]
    Aims {
        /// Number of AIMS problems.
        count: usize,
        /// Structured AIMS problems.
        problems: Vec<ori_arc::ArcProblem>,
    },
    /// The immutable string interner could not allocate the entry-point name.
    #[error(transparent)]
    Intern(#[from] ori_ir::InternError),
    /// Closed-program validation failed.
    #[error(transparent)]
    Executable(#[from] RealizationError),
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

pub fn realize_local_program(
    mut input: ProgramRealizationInput<'_>,
) -> Result<ExecutableProgram, ProgramRealizationError> {
    let interner = &*input.symbols;
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
            &mut input.pool,
            interner,
        )
        .map_err(map_arc_batch_error)?;
    let mut type_registry = TypeRegistry::from_typed_exports(
        input.types.typed.types.clone(),
        input.types.typed.collection_burdens.clone(),
    );
    ori_types::register_resolved_collection_burdens(&input.pool, &mut type_registry);
    let repr_plan = compute_module_repr_plan(ModuleReprInput {
        pool: &input.pool,
        arc_functions: prepared.functions(),
        narrowing_policy: input.narrowing_policy,
        type_result: input.types,
        interner: Some(interner),
        imported_type_metadata: &[],
        imported_collection_surfaces: &[],
        has_analysis_only_functions: false,
    });
    let user_drop_bindings =
        collect_user_drop_bindings(&type_registry, &typed_user_drop_bindings, &input.pool)?;
    let main = interner.try_intern("main")?;
    realize_arc_program(ArcProgramRealizationInput {
        prepared,
        pool: input.pool,
        symbols: input.symbols,
        roots: vec![main],
        cli_entry: Some(main),
        externals: Vec::new(),
        user_drop_bindings,
        repr_plan,
        type_registry,
        verify_arc: input.verify_arc,
    })
}

/// Run the backend-neutral calculus exactly once and close its artifact.
pub(crate) fn realize_arc_program(
    input: ArcProgramRealizationInput,
) -> Result<ExecutableProgram, ProgramRealizationError> {
    let ArcProgramRealizationInput {
        prepared,
        pool,
        symbols,
        roots,
        cli_entry,
        externals,
        user_drop_bindings,
        repr_plan,
        type_registry,
        verify_arc,
    } = input;
    let (mut functions, function_families, method_targets) = prepared.into_executable_parts();

    // External ownership/effect policy is producer-owned. Reject any stale,
    // incomplete, or signature-mismatched transport record before its
    // contract can seed the AIMS fixed point.
    ori_repr::executable::validate_external_callables(&externals, &pool)?;
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

pub(crate) fn collect_user_drop_bindings(
    registry: &TypeRegistry,
    typed_bindings: &[UserDropBinding],
    pool: &Pool,
) -> Result<Vec<UserDropBinding>, ProgramRealizationError> {
    let mut expected = FxHashMap::default();
    for entry in registry.iter() {
        let Some(logical) = registry
            .burden(entry.idx)
            .and_then(|burden| burden.user_drop)
        else {
            continue;
        };
        expected.insert(pool.resolve_fully(entry.idx), (entry.idx, logical));
    }

    let mut seen = FxHashMap::default();
    let mut bindings = Vec::with_capacity(typed_bindings.len());
    for &binding in typed_bindings {
        if !pool.is_valid_idx(binding.ty()) {
            return Err(RealizationError::InvalidUserDropType { ty: binding.ty() }.into());
        }
        let canonical = pool.resolve_fully(binding.ty());
        let Some(&(expected_ty, expected_logical)) = expected.get(&canonical) else {
            return Err(ProgramRealizationError::UnexpectedUserDropRole { ty: binding.ty() });
        };
        if binding.logical() != expected_logical {
            return Err(ProgramRealizationError::UserDropLogicalIdentityMismatch {
                ty: expected_ty,
                expected: expected_logical,
                found: binding.logical(),
            });
        }
        let count = seen.entry(canonical).or_insert(0usize);
        *count += 1;
        if *count > 1 {
            return Err(ProgramRealizationError::AmbiguousUserDropTarget {
                ty: expected_ty,
                targets: *count,
            });
        }
        bindings.push(UserDropBinding::new(
            expected_ty,
            expected_logical,
            binding.target(),
        ));
    }
    if let Some((_, &(ty, _))) = expected
        .iter()
        .find(|(canonical, _)| !seen.contains_key(canonical))
    {
        return Err(ProgramRealizationError::MissingUserDropTarget { ty });
    }
    bindings.sort_by_key(|binding| binding.ty().raw());
    Ok(bindings)
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

fn run_aims(
    functions: &mut [ArcFunction],
    pool: &Pool,
    interner: &StringInterner,
    type_registry: &TypeRegistry,
    external_contracts: &FxHashMap<Name, ori_arc::MemoryContract>,
    callable_boundaries: &ori_arc::CallableBoundaryFacts,
    verify_arc: bool,
) -> Result<ori_arc::ArcPipelineBatchOutcome, ProgramRealizationError> {
    let classifier = ori_arc::ArcClassifier::new(pool);
    let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
    let outcome = ori_arc::realize_closed_program(
        functions,
        &ori_arc::ArcPipelineContext {
            classifier: &classifier,
            interner,
            pool,
            builtins: &builtins,
            type_registry,
            callable_boundaries,
            verify_arc,
            external_contracts,
        },
    )
    .map_err(|errors| ProgramRealizationError::ArcVerification {
        count: errors.len(),
        errors,
    })?;
    if outcome.problems.is_empty() {
        Ok(outcome)
    } else {
        Err(ProgramRealizationError::Aims {
            count: outcome.problems.len(),
            problems: outcome.problems,
        })
    }
}

#[cfg(test)]
mod tests;
