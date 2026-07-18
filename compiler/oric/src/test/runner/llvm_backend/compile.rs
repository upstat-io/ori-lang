//! LLVM lowering, compilation, and execution for a filtered test batch.

use ori_llvm::evaluator::{
    CompiledTestModule, ImportedFunctionForCodegen, LLVMEvalError, OwnedLLVMEvaluator,
};
use ori_types::{ExportedTypeMetadata, FunctionSig, Pool, TypeCheckResult};

use super::super::arc_lowering::{lower_jit_arc_program, JitArcLowering, JitArcLoweringInput};
use super::super::{TestRunner, TestRunnerConfig};
use super::{imports, LlvmTestFile};

pub(super) struct CompilationFailure {
    pub(super) summary: String,
    pub(super) test_result: String,
}

struct CompilationInput<'a, 'test, 'import> {
    db: &'a crate::db::CompilerDb,
    parse: &'a crate::parser::ParseOutput,
    typed: &'a TypeCheckResult,
    tests: &'a [&'test crate::ir::TestDef],
    canon: &'a ori_ir::canon::SharedCanonResult,
    interner: &'a crate::ir::StringInterner,
    pool: &'a Pool,
    function_sigs: &'a [FunctionSig],
    imported_functions: &'a [ImportedFunctionForCodegen<'import>],
    imported_type_metadata: &'a [ExportedTypeMetadata],
    imported_collection_surfaces: &'a [u64],
    lowering: JitArcLowering,
}

pub(super) fn compile_and_run(
    file: LlvmTestFile<'_>,
    tests: &[&crate::ir::TestDef],
    config: &TestRunnerConfig,
) -> Result<Vec<super::TestResult>, CompilationFailure> {
    let LlvmTestFile {
        db,
        path,
        parse,
        typed,
        pool,
        canon,
        interner,
    } = file;
    let imports::PreparedImports {
        modules,
        type_results,
        pool: mut merged_pool,
        canons,
        function_refs,
        signatures: imported_signatures,
        renamed_functions,
        mono_functions,
        generic_templates,
        target_maps,
        root_targets,
    } = imports::prepare(db, path, parse, typed, pool, interner).map_err(|error| {
        let message = format!("LLVM JIT import closure failed: {error}");
        CompilationFailure {
            summary: message.clone(),
            test_result: message,
        }
    })?;
    let imported_functions = imports::for_codegen(
        &modules,
        &function_refs,
        &renamed_functions,
        &imported_signatures,
        &canons,
    );
    let imported_function_modules = function_refs
        .iter()
        .map(|reference| reference.module_index)
        .collect::<Vec<_>>();
    let function_sigs = crate::typeck::build_function_sigs(parse, typed);
    let type_metadata = type_results
        .iter()
        .flat_map(|result| result.typed.exported_type_metadata.iter().cloned())
        .collect::<Vec<_>>();
    let collection_surfaces = type_results
        .iter()
        .flat_map(|result| result.typed.exported_collection_surfaces.iter().copied())
        .collect::<Vec<_>>();

    let lowering = lower_program(JitArcLoweringInput {
        parse,
        typed,
        tests,
        function_sigs: &function_sigs,
        canon,
        interner,
        pool: &mut merged_pool,
        imported_functions: &imported_functions,
        imported_mono_fns: &mono_functions,
        imported_generic_templates: &generic_templates,
        re_interned_canons: &canons,
        imported_function_modules: &imported_function_modules,
        imported_target_maps: &target_maps,
        root_import_targets: &root_targets,
        import_sigs: &[],
    })?;
    let imported_functions = imported_functions
        .into_iter()
        .filter(|imported| lowering.reachable_imports.contains(&imported.function.name))
        .collect::<Vec<_>>();
    let evaluator = OwnedLLVMEvaluator::new();
    let compiled = compile_program(
        &evaluator,
        CompilationInput {
            db,
            parse,
            typed,
            tests,
            canon,
            interner,
            pool: &merged_pool,
            function_sigs: &function_sigs,
            imported_functions: &imported_functions,
            imported_type_metadata: &type_metadata,
            imported_collection_surfaces: &collection_surfaces,
            lowering,
        },
    )?;

    Ok(run_compiled_tests(&compiled, tests, interner, config))
}

fn lower_program(
    input: JitArcLoweringInput<'_, '_, '_>,
) -> Result<JitArcLowering, CompilationFailure> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        lower_jit_arc_program(input)
    }));
    match result {
        Ok(Ok(lowering)) => Ok(lowering),
        Ok(Err(error)) => {
            let message = format!("LLVM JIT ARC preparation failed: {error}");
            Err(CompilationFailure {
                summary: message.clone(),
                test_result: message,
            })
        }
        Err(panic_info) => {
            let message = format!(
                "LLVM backend error during ARC lowering: {}",
                super::super::panic_message(panic_info.as_ref())
            );
            Err(CompilationFailure {
                summary: message.clone(),
                test_result: message,
            })
        }
    }
}

fn compile_program<'llvm>(
    evaluator: &'llvm OwnedLLVMEvaluator,
    input: CompilationInput<'_, '_, '_>,
) -> Result<CompiledTestModule<'llvm>, CompilationFailure> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compile_program_inner(evaluator, input)
    }));
    match result {
        Ok(Ok(compiled)) => Ok(compiled),
        Ok(Err(error)) => Err(CompilationFailure {
            summary: error.message.clone(),
            test_result: format!("LLVM compilation failed: {}", error.message),
        }),
        Err(panic_info) => {
            let message = format!(
                "LLVM backend error: {}",
                super::super::panic_message(panic_info.as_ref())
            );
            Err(CompilationFailure {
                summary: message.clone(),
                test_result: message,
            })
        }
    }
}

fn compile_program_inner<'llvm>(
    evaluator: &'llvm OwnedLLVMEvaluator,
    input: CompilationInput<'_, '_, '_>,
) -> Result<CompiledTestModule<'llvm>, LLVMEvalError> {
    let CompilationInput {
        db,
        parse,
        typed,
        tests,
        canon,
        interner,
        pool,
        function_sigs,
        imported_functions,
        imported_type_metadata,
        imported_collection_surfaces,
        lowering,
    } = input;
    let JitArcLowering {
        prepared_batch,
        mono_functions,
        user_drop_bindings: typed_user_drop_bindings,
        impl_emission_names,
        reachable_imports: _,
    } = lowering;
    let mut type_registry = ori_types::TypeRegistry::from_typed_exports(
        typed.typed.types.clone(),
        typed.typed.collection_burdens.clone(),
    );
    ori_types::register_resolved_collection_burdens(pool, &mut type_registry);
    let policy = if ori_repr::NarrowingPolicy::env_disabled() {
        ori_repr::NarrowingPolicy::Disabled
    } else {
        ori_repr::NarrowingPolicy::Aggressive
    };
    let repr_plan =
        crate::realization::compute_module_repr_plan(crate::realization::ModuleReprInput {
            pool,
            arc_functions: prepared_batch.functions(),
            narrowing_policy: policy,
            type_result: typed,
            interner: Some(interner),
            imported_type_metadata,
            imported_collection_surfaces,
            has_analysis_only_functions: typed
                .typed
                .impl_sigs
                .iter()
                .any(|entry| !entry.sig.is_generic()),
        });
    let user_drop_bindings = crate::realization::collect_user_drop_bindings(
        &type_registry,
        &typed_user_drop_bindings,
        pool,
    )
    .map_err(|error| {
        LLVMEvalError::new(format!("user-drop callable realization failed: {error}"))
    })?;
    let roots = prepared_batch.parent_roots();
    let cli_entry = parse
        .module
        .functions
        .iter()
        .zip(function_sigs)
        .find_map(|(function, signature)| signature.is_main.then_some(function.name));
    let executable =
        crate::realization::realize_arc_program(crate::realization::ArcProgramRealizationInput {
            prepared: prepared_batch,
            pool: pool.clone(),
            symbols: db.shared_interner(),
            roots,
            cli_entry,
            externals: Vec::new(),
            user_drop_bindings,
            repr_plan,
            type_registry,
            verify_arc: std::env::var(crate::debug_flags::ORI_VERIFY_ARC)
                .is_ok_and(|value| value != "0"),
        })
        .map_err(|error| {
            LLVMEvalError::new(format!("closed JIT executable realization failed: {error}"))
        })?;

    evaluator.compile_module_with_tests(
        &parse.module,
        tests,
        canon,
        interner,
        function_sigs,
        &typed.typed.types,
        &typed.typed.impl_sigs,
        imported_functions,
        &mono_functions,
        &executable,
        &impl_emission_names,
    )
}

fn run_compiled_tests(
    compiled: &CompiledTestModule<'_>,
    tests: &[&crate::ir::TestDef],
    interner: &crate::ir::StringInterner,
    config: &TestRunnerConfig,
) -> Vec<super::TestResult> {
    tests
        .iter()
        .map(|test| {
            TestRunner::protocol_start(test.name, config, interner);
            let result = TestRunner::run_single_test_from_compiled(compiled, test, interner);
            let result = match test.fail_expected {
                Some(expected) => TestRunner::apply_fail_wrapper(result, expected, interner),
                None => result,
            };
            TestRunner::protocol_result(&result, config, interner);
            result
        })
        .collect()
}
