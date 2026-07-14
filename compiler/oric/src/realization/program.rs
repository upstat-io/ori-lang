//! Closed-program realization over explicit immutable frontend facts.

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator};
use ori_ir::canon::CanonResult;
use ori_ir::{Name, SharedInterner, StringInterner};
use ori_parse::ParseOutput;
use ori_repr::executable::{
    ExecutableProgram, ExecutableProgramParts, RealizationError, EXECUTABLE_PROGRAM_VERSION,
};
use ori_types::{FunctionSig, Idx, Pool, TypeCheckResult, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{compute_module_repr_plan, lower_impl_methods_for_analysis, ImplMethodAnalysis};

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
}

/// A typed failure in frontend-to-executable realization.
#[derive(Debug, thiserror::Error)]
pub enum ProgramRealizationError {
    /// ARC lowering rejected one or more ordinary or monomorphized bodies.
    #[error("ARC lowering produced {count} problem(s): {problems:?}")]
    ArcLowering {
        /// Number of lowering problems.
        count: usize,
        /// Structured lowering problems.
        problems: Vec<ori_arc::ArcProblem>,
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
pub fn realize_local_program(
    input: ProgramRealizationInput<'_>,
) -> Result<ExecutableProgram, ProgramRealizationError> {
    let interner = &*input.symbols;
    let function_sigs = crate::typeck::build_function_sigs(input.parse, input.types);
    let mono_functions = ori_repr::monomorphize::collect_mono_functions(
        &input.types.typed.mono_instances,
        &function_sigs,
        &input.types.typed.impl_sigs,
        &[],
        interner,
        &input.pool,
    );
    let mut arc_cache = lower_top_level_functions(
        input.parse,
        input.canon,
        interner,
        &input.pool,
        &function_sigs,
        &mono_functions,
    )?;
    ori_repr::monomorphize::rewrite_apply_targets_for_monos(
        &mut arc_cache,
        &mono_functions,
        &input.pool,
        interner,
    );

    let ImplMethodAnalysis {
        functions: impl_functions,
        targets: impl_targets,
    } = lower_impl_methods_for_analysis(
        input.parse,
        input.types,
        interner,
        input.canon,
        &input.pool,
    )
    .map_err(arc_lowering_error)?;
    let mut functions = super::collect_all_arc_functions(&arc_cache);
    functions.extend(impl_functions);
    rewrite_impl_targets(&mut functions, &impl_targets, &input.pool);

    let mut type_registry = TypeRegistry::from_typed_exports(
        input.types.typed.types.clone(),
        input.types.typed.collection_burdens.clone(),
    );
    ori_types::register_resolved_collection_burdens(&input.pool, &mut type_registry);
    let repr_plan = compute_module_repr_plan(
        &input.pool,
        &functions,
        input.narrowing_policy,
        input.types,
        Some(interner),
        &[],
        &[],
        false,
    );
    run_aims(&mut functions, &input.pool, interner, &type_registry)?;
    let main = interner.try_intern("main")?;

    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols: input.symbols,
        pool: input.pool,
        functions,
        main,
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
    mono_functions: &[ori_repr::monomorphize::MonoFunction],
) -> Result<FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>, ProgramRealizationError> {
    let mut cache = FxHashMap::default();
    let mut problems = Vec::new();
    for (function, signature) in parse.module.functions.iter().zip(function_sigs) {
        if signature.is_generic() {
            continue;
        }
        let lowered = crate::arc_lowering::lower_to_arc(
            function.name,
            signature,
            function.name,
            canon,
            interner,
            pool,
            &mut problems,
            None,
        );
        cache.insert(lowered.0.name, lowered);
    }
    lower_monomorphized_functions(
        &mut cache,
        mono_functions,
        canon,
        interner,
        pool,
        &mut problems,
    );
    if problems.is_empty() {
        Ok(cache)
    } else {
        Err(arc_lowering_error(problems))
    }
}

fn lower_monomorphized_functions(
    cache: &mut FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>,
    mono_functions: &[ori_repr::monomorphize::MonoFunction],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    problems: &mut Vec<ori_arc::ArcProblem>,
) {
    for mono in mono_functions {
        let lowered = match mono.receiver_type_name {
            Some(type_name) => crate::arc_lowering::lower_impl_method_to_arc(
                mono.mangled_name,
                &mono.sig,
                mono.original_name,
                type_name,
                canon,
                interner,
                pool,
                problems,
                Some(&mono.body_type_map),
            ),
            None => crate::arc_lowering::lower_to_arc(
                mono.mangled_name,
                &mono.sig,
                mono.original_name,
                canon,
                interner,
                pool,
                problems,
                Some(&mono.body_type_map),
            ),
        };
        cache.insert(lowered.0.name, lowered);
    }
}

fn rewrite_impl_targets(
    functions: &mut [ArcFunction],
    targets: &FxHashMap<(Idx, Name), Name>,
    pool: &Pool,
) {
    let function_names: FxHashSet<Name> = functions.iter().map(|function| function.name).collect();
    for function in functions {
        let var_types = &function.var_types;
        for block in &mut function.blocks {
            for instruction in &mut block.body {
                if let ArcInstr::Apply { func, args, .. } = instruction {
                    rewrite_impl_target(func, args, var_types, targets, &function_names, pool);
                }
            }
            if let ArcTerminator::Invoke { func, args, .. } = &mut block.terminator {
                rewrite_impl_target(func, args, var_types, targets, &function_names, pool);
            }
        }
    }
}

fn rewrite_impl_target(
    target: &mut Name,
    arguments: &[ori_arc::ArcVarId],
    var_types: &[Idx],
    impl_targets: &FxHashMap<(Idx, Name), Name>,
    function_names: &FxHashSet<Name>,
    pool: &Pool,
) {
    if function_names.contains(target) {
        return;
    }
    let Some(receiver) = arguments.first() else {
        return;
    };
    let Some(&receiver_type) = var_types.get(receiver.index()) else {
        return;
    };
    let key = (pool.resolve_fully(receiver_type), *target);
    if let Some(&qualified) = impl_targets.get(&key) {
        *target = qualified;
    }
}

fn run_aims(
    functions: &mut [ArcFunction],
    pool: &Pool,
    interner: &StringInterner,
    type_registry: &TypeRegistry,
) -> Result<(), ProgramRealizationError> {
    let classifier = ori_arc::ArcClassifier::new(pool);
    let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
    let problems = ori_arc::run_arc_pipeline_all(
        functions,
        &classifier,
        interner,
        pool,
        &builtins,
        type_registry,
        true,
    )
    .map_err(|errors| ProgramRealizationError::ArcVerification {
        count: errors.len(),
        errors,
    })?;
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ProgramRealizationError::Aims {
            count: problems.len(),
            problems,
        })
    }
}

fn arc_lowering_error(problems: Vec<ori_arc::ArcProblem>) -> ProgramRealizationError {
    ProgramRealizationError::ArcLowering {
        count: problems.len(),
        problems,
    }
}
