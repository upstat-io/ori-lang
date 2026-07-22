//! End-to-end AOT LLVM codegen pipeline.

use super::imported_mono::ImportedSurfaces;
use super::{borrow_inference, exports, finalize};
use ori_ir::{canon::CanonResult, Name, StringInterner};
use ori_llvm::codegen::eh_model::EhModel;
use ori_llvm::codegen::function_compiler::FunctionCompiler;
use ori_llvm::codegen::ir_builder::IrBuilder;
use ori_llvm::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use ori_llvm::codegen::type_registration;
use ori_llvm::inkwell::context::Context;
use ori_llvm::SimpleCx;
use ori_repr::executable::{ExecutableProgram, UserDropBinding};
use ori_repr::monomorphize::MonoFunction;
use ori_types::{FunctionSig, Pool, TypeCheckResult};
use oric::parser::ParseOutput;
use oric::{CompilerDb, Db};
use std::mem::ManuallyDrop;

/// One producer export projected from the same closed artifact LLVM consumes.
///
/// Type indices are deliberately absent: they are local to the producer's
/// pool. Multi-module import assembly re-interns the provider's typed
/// signature into the consumer's merged pool before pairing it with these
/// frozen facts.
pub(crate) struct RealizedCallableExport {
    pub(crate) mangled_name: String,
    pub(crate) source_name: String,
    pub(crate) metadata: ori_repr::executable::ExternalCallableMetadata,
}

/// LLVM module plus producer-authored callable facts from one realization.
pub(in crate::commands) struct LlvmCodegenOutput<'ctx> {
    pub(crate) module: ori_llvm::inkwell::module::Module<'ctx>,
    pub(crate) exports: Vec<RealizedCallableExport>,
}

#[derive(Clone, Copy)]
pub(in crate::commands) struct CodegenPipelineInput<'ctx, 'a> {
    pub(in crate::commands) context: &'ctx Context,
    pub(in crate::commands) db: &'a CompilerDb,
    pub(in crate::commands) parse: &'a ParseOutput,
    pub(in crate::commands) typed: &'a TypeCheckResult,
    pub(in crate::commands) pool: &'ctx Pool,
    pub(in crate::commands) canon: &'a CanonResult,
    pub(in crate::commands) source_path: &'a str,
    pub(in crate::commands) module_name: &'a str,
    pub(in crate::commands) symbol_prefix: &'a str,
    pub(in crate::commands) import_sigs: &'a [ori_repr::monomorphize::ImportSig],
    pub(in crate::commands) imported: ImportedSurfaces<'a>,
    pub(in crate::commands) target_triple: Option<&'a str>,
    pub(in crate::commands) realization_policy: crate::realization::RealizationPolicy,
    pub(in crate::commands) imported_type_metadata: &'a [ori_types::ExportedTypeMetadata],
    pub(in crate::commands) imported_collection_surfaces: &'a [u64],
}

struct AnalysisProducts {
    prepared: crate::realization::PreparedArcBatch,
    mono_functions: Vec<MonoFunction>,
    user_drop_bindings: Vec<UserDropBinding>,
    impl_emission_names: Vec<Option<Name>>,
}

struct RealizedCodegen {
    executable: ExecutableProgram,
    function_sigs: Vec<FunctionSig>,
    mono_functions: Vec<MonoFunction>,
    impl_emission_names: Vec<Option<Name>>,
}

/// Run the codegen pipeline on a pre-checked module.
///
/// Shared by `compile_to_llvm_with_imported_monos` and
/// `compile_to_llvm_with_imports`: borrow inference, repr planning, two-pass
/// function compilation, monomorphization, impl + derive emission, and
/// main-wrapper generation.
/// `symbol_prefix`: `""` (single-file) or the module name (multi-file).
/// `import_sigs`: external symbols from other modules; `&[]` for single-file.
#[expect(
    unsafe_code,
    reason = "SimpleCx owns the LLVM context borrowed by CodegenCx for this function"
)]
pub(in crate::commands) fn run_codegen_pipeline<'ctx>(
    input: CodegenPipelineInput<'ctx, '_>,
) -> Result<LlvmCodegenOutput<'ctx>, String> {
    let interner = input.db.interner();
    let mut realization_pool = input.pool.clone();
    let scx = ManuallyDrop::new(SimpleCx::new(input.context, input.module_name));
    let realized = realize_codegen_program(&input, &mut realization_pool, interner)?;
    crate::dump_orchestrator::dump_realized_arc(&realized.executable, interner, input.source_path);

    let (codegen_errors, codegen_descriptions, exports) = {
        // SAFETY: ManuallyDrop keeps scx at a stable address through emission;
        // the detached reborrow cannot escape this block.
        let scx_ref: &'ctx SimpleCx<'ctx> = unsafe { &*std::ptr::from_ref(&*scx) };
        emit_realized_program(scx_ref, &input, &realized, interner)?
    };

    let module = finalize::finalize_module(
        &scx,
        codegen_errors,
        &codegen_descriptions,
        input.source_path,
        &realization_pool,
        interner,
    )?;
    Ok(LlvmCodegenOutput { module, exports })
}

fn realize_codegen_program(
    input: &CodegenPipelineInput<'_, '_>,
    pool: &mut Pool,
    interner: &StringInterner,
) -> Result<RealizedCodegen, String> {
    let function_sigs = oric::typeck::build_function_sigs(input.parse, input.typed);
    let AnalysisProducts {
        prepared,
        mono_functions,
        user_drop_bindings: typed_user_drop_bindings,
        impl_emission_names,
    } = prepare_analysis_products(input, pool, interner, &function_sigs)?;

    let externals = ori_repr::monomorphize::realize_imported_callables(input.import_sigs, pool)
        .map_err(|error| format!("imported callable realization failed: {error}"))?;
    let executable =
        crate::realization::realize_arc_program(crate::realization::ArcProgramRealizationInput {
            prepared,
            pool: pool.clone(),
            symbols: input.db.shared_interner(),
            facts: crate::realization::CheckedModuleFacts {
                parse: input.parse,
                types: input.typed,
                function_sigs: &function_sigs,
                interner,
                imported_repr: crate::realization::ImportedReprSurfaces {
                    type_metadata: input.imported_type_metadata,
                    collection_surfaces: input.imported_collection_surfaces,
                },
            },
            externals,
            typed_user_drop_bindings: &typed_user_drop_bindings,
            policy: input.realization_policy,
        })
        .map_err(|error| format!("closed executable realization failed: {error}"))?;

    Ok(RealizedCodegen {
        executable,
        function_sigs,
        mono_functions,
        impl_emission_names,
    })
}

fn prepare_analysis_products(
    input: &CodegenPipelineInput<'_, '_>,
    pool: &mut Pool,
    interner: &StringInterner,
    function_sigs: &[FunctionSig],
) -> Result<AnalysisProducts, String> {
    let crate::realization::DerivedMethodAnalysis {
        groups: derived_groups,
        targets: derived_targets,
    } = crate::realization::lower_non_generic_derived_methods_for_analysis(
        &input.typed.typed.accepted_derives,
        &input.typed.typed.derived_call_plans,
        interner,
        pool,
    )
    .map_err(|problems| lowering_problem("derived-method", &problems))?;
    let borrow_inference::ArcBatchLoweringResult {
        groups,
        mono_functions,
    } = lower_arc_functions(input, pool, interner, function_sigs)?;
    let crate::realization::ImplMethodAnalysis {
        groups: impl_groups,
        targets: mut method_targets,
        producer_targets: impl_producer_targets,
        user_drop_bindings,
        emission_names: impl_emission_names,
        ..
    } = crate::realization::lower_impl_methods_for_analysis(
        input.parse,
        input.typed,
        interner,
        input.canon,
        pool,
    )
    .map_err(|problems| lowering_problem("impl-method", &problems))?;

    for (key, target) in derived_targets {
        method_targets.entry(key).or_insert(target);
    }
    crate::realization::extend_mono_method_targets(
        &mut method_targets,
        &mono_functions,
        interner,
        pool,
    )
    .map_err(|problems| lowering_problem("monomorphized method target census", &problems))?;

    let mut groups = groups;
    groups.extend(impl_groups);
    groups.extend(derived_groups);
    crate::realization::CallableCensusBuilder::new(interner)
        .close_builtin_targets(&mut groups, pool)
        .map_err(|error| error.to_string())?;
    let local_generic_type_params =
        crate::realization::generic_type_param_map(&input.typed.typed.types);
    let closed = crate::realization::close_generic_mono_targets(
        crate::realization::GenericMonoClosureInput {
            groups,
            mono_functions,
            mono_instances: &input.typed.typed.mono_instances,
            function_sigs,
            local_generic_type_params: &local_generic_type_params,
            impl_sigs: &input.typed.typed.impl_sigs,
            accepted_derives: &input.typed.typed.accepted_derives,
            derived_call_plans: &input.typed.typed.derived_call_plans,
            import_sigs: input.import_sigs,
            imported_generic_templates: input.imported.generic_templates,
            re_interned_canons: input.imported.re_interned_canons,
            canon: input.canon,
            interner,
            pool,
        },
    )
    .map_err(|error| error.to_string())?;
    let groups = closed.groups;
    let mono_functions = closed.mono_functions;
    crate::realization::extend_mono_method_targets(
        &mut method_targets,
        &mono_functions,
        interner,
        pool,
    )
    .map_err(|problems| lowering_problem("final monomorphized method target census", &problems))?;
    let lowered = crate::realization::LoweredArcBatch::try_from_groups(groups, interner)
        .map_err(|error| error.to_string())?;
    let prepared = lowered
        .prepare(
            &mono_functions,
            &method_targets,
            &impl_producer_targets,
            &input.typed.typed.method_producers,
            &*pool,
            interner,
        )
        .map_err(|error| error.to_string())?;
    Ok(AnalysisProducts {
        prepared,
        mono_functions,
        user_drop_bindings,
        impl_emission_names,
    })
}

fn lower_arc_functions(
    input: &CodegenPipelineInput<'_, '_>,
    pool: &Pool,
    interner: &StringInterner,
    function_sigs: &[FunctionSig],
) -> Result<borrow_inference::ArcBatchLoweringResult, String> {
    let result = borrow_inference::lower_arc_batch(borrow_inference::ArcBatchLoweringInput {
        parse: input.parse,
        function_sigs,
        impl_sigs: &input.typed.typed.impl_sigs,
        import_sigs: input.import_sigs,
        imported: input.imported,
        canon: input.canon,
        interner,
        pool,
        mono_instances: &input.typed.typed.mono_instances,
        accepted_derives: &input.typed.typed.accepted_derives,
        derived_call_plans: &input.typed.typed.derived_call_plans,
    });
    match result {
        Ok(batch) => Ok(batch),
        Err(borrow_inference::ArcBatchLoweringFailure::ArcLowering) => Err(
            "ARC lowering rejected this module; fix the source diagnostics emitted above, then rebuild"
                .to_string(),
        ),
        Err(borrow_inference::ArcBatchLoweringFailure::MonoInventory(error)) => {
            Err(error.to_string())
        }
        Err(borrow_inference::ArcBatchLoweringFailure::CallableCensus(error)) => {
            Err(error.to_string())
        }
    }
}

fn lowering_problem<T: std::fmt::Debug>(phase: &str, problems: &[T]) -> String {
    format!(
        "{phase} ARC lowering failed with {} problem(s): {problems:?}",
        problems.len()
    )
}

fn emit_realized_program<'ctx>(
    scx: &'ctx SimpleCx<'ctx>,
    input: &CodegenPipelineInput<'ctx, '_>,
    realized: &RealizedCodegen,
    interner: &StringInterner,
) -> Result<(u32, Vec<String>, Vec<RealizedCallableExport>), String> {
    let eh_model = input
        .target_triple
        .map_or(EhModel::Itanium, EhModel::from_triple);
    let mut builder = IrBuilder::new_aot(scx, eh_model);
    let artifact_pool = realized.executable.pool();
    let repr_plan = realized.executable.repr_plan();
    let classifier = ori_arc::ArcClassifier::new(artifact_pool);
    let annotated_sigs =
        exports::artifact_annotated_signatures(&realized.executable, input.import_sigs)?;
    let store = TypeInfoStore::new_with_plan(artifact_pool, repr_plan);
    let resolver = TypeLayoutResolver::new(&store, scx, Some(interner), Some(repr_plan));
    type_registration::register_user_types(&resolver, &input.typed.typed.types);

    let mut compiler = FunctionCompiler::new(
        &mut builder,
        &store,
        &resolver,
        interner,
        artifact_pool,
        input.symbol_prefix,
        &annotated_sigs,
        &classifier,
        None,
        input.realization_policy.verify_arc,
    );
    compiler.bind_executable_program(&realized.executable);
    emit_all_functions(&mut compiler, input, realized, interner);
    let exports = exports::project_callable_exports(
        &realized.executable,
        input.parse,
        &realized.function_sigs,
        interner,
        input.symbol_prefix,
    )?;
    drop(compiler);

    Ok((
        builder.codegen_error_count() + store.type_error_count(),
        builder.codegen_error_descriptions(),
        exports,
    ))
}

fn emit_all_functions<'a, 'scx: 'ctx, 'ctx, 'tcx>(
    compiler: &mut FunctionCompiler<'a, 'scx, 'ctx, 'tcx>,
    input: &CodegenPipelineInput<'_, '_>,
    realized: &RealizedCodegen,
    interner: &StringInterner,
) {
    if !input.import_sigs.is_empty() {
        compiler.declare_imports(input.import_sigs);
    }
    compiler.declare_all(&input.parse.module.functions, &realized.function_sigs);
    compiler.declare_mono_functions(&realized.mono_functions);

    let deferred_parents = realized
        .impl_emission_names
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let artifact_remainder = compiler.declare_artifact_remainder(&deferred_parents);
    if !input.parse.module.impls.is_empty() || !input.parse.module.extends.is_empty() {
        compiler.compile_impls_from_artifact(
            &input.parse.module.impls,
            &input.parse.module.extends,
            &input.typed.typed.impl_sigs,
            input.canon,
            &input.parse.module.traits,
            &realized.impl_emission_names,
        );
    }
    compiler.bind_executable_method_targets();
    compiler.bind_user_drop_targets();

    let mut prepared =
        compiler.prepare_all_from_artifact(&input.parse.module.functions, &realized.function_sigs);
    prepared.extend(compiler.prepare_mono_from_artifact(&realized.mono_functions));
    prepared.extend(compiler.prepare_artifact_remainder_from_artifact(&artifact_remainder));
    let analyzed = compiler.compute_nounwind_set(prepared);
    compiler.emit_prepared_functions(analyzed);
    compiler.apply_posthoc_nounwind();

    let panic_name = input
        .parse
        .module
        .functions
        .iter()
        .find(|function| interner.lookup(function.name) == "panic")
        .map(|function| function.name);
    if let Some((function, signature)) = input
        .parse
        .module
        .functions
        .iter()
        .zip(&realized.function_sigs)
        .find(|(_, signature)| signature.is_main)
    {
        compiler.generate_main_wrapper(function.name, signature, panic_name);
    }
}
