//! Codegen pipeline implementation for AOT compilation.
//!
//! Contains the heavy implementation details:
//! - LLVM codegen orchestration (`run_codegen_pipeline`)
//!
//! Helpers extracted into sibling modules:
//! - `borrow_inference`: complete pre-AIMS ARC batch lowering
//! - `repr_setup`: repr plan computation and impl-method ARC lowering for analysis
//! - `pc2_hooks`: PC-2 invariant check helper for AOT pre-mono / mono sites
//! - `finalize`: ARC phase dumps + post-codegen diagnostics-and-verify
//!
//! Called from `compile_common::compile_to_llvm_with_imported_monos` and
//! `compile_common::compile_to_llvm_with_imports`.

#[cfg(feature = "llvm")]
mod borrow_inference;
#[cfg(feature = "llvm")]
mod finalize;
#[cfg(feature = "llvm")]
pub(crate) mod imported_mono;
#[cfg(feature = "llvm")]
mod pc2_hooks;

#[cfg(feature = "llvm")]
use imported_mono::ImportedSurfaces;
#[cfg(feature = "llvm")]
use ori_ir::canon::CanonResult;
#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;
#[cfg(feature = "llvm")]
use ori_types::{Pool, TypeCheckResult};
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db};

/// One producer export projected from the same closed artifact LLVM consumes.
///
/// Type indices are deliberately absent: they are local to the producer's
/// pool. Multi-module import assembly re-interns the provider's typed
/// signature into the consumer's merged pool before pairing it with these
/// frozen facts.
#[cfg(feature = "llvm")]
pub(crate) struct RealizedCallableExport {
    pub(crate) mangled_name: String,
    pub(crate) source_name: String,
    pub(crate) metadata: ori_repr::executable::ExternalCallableMetadata,
}

/// LLVM module plus producer-authored callable facts from one realization.
#[cfg(feature = "llvm")]
pub struct LlvmCodegenOutput<'ctx> {
    pub(crate) module: ori_llvm::inkwell::module::Module<'ctx>,
    pub(crate) exports: Vec<RealizedCallableExport>,
}

/// Run the codegen pipeline on a pre-checked module.
///
/// Shared by `compile_to_llvm_with_imported_monos` and
/// `compile_to_llvm_with_imports`: borrow inference, repr planning, two-pass
/// function compilation, monomorphization, impl + derive emission, and
/// main-wrapper generation.
/// `symbol_prefix`: `""` (single-file) or the module name (multi-file).
/// `import_sigs`: external symbols from other modules; `&[]` for single-file.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "private pipeline helper — params match the data flow from both public compile functions"
)]
#[allow(unsafe_code, reason = "LLVM C API requires unsafe FFI calls")]
#[expect(
    clippy::too_many_lines,
    reason = "sequential pipeline — further splitting would fragment the compilation flow"
)]
pub(super) fn run_codegen_pipeline<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &'ctx Pool,
    canon: &CanonResult,
    source_path: &str,
    module_name: &str,
    symbol_prefix: &str,
    import_sigs: &[ori_repr::monomorphize::ImportSig],
    imported: ImportedSurfaces<'_>,
    target_triple: Option<&str>,
    narrowing_policy: ori_repr::NarrowingPolicy,
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
) -> Result<LlvmCodegenOutput<'ctx>, String> {
    use ori_llvm::codegen::eh_model::EhModel;
    use ori_llvm::codegen::function_compiler::FunctionCompiler;
    use ori_llvm::codegen::ir_builder::IrBuilder;
    use ori_llvm::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use ori_llvm::codegen::type_registration;
    use ori_llvm::SimpleCx;

    use std::mem::ManuallyDrop;

    let interner = db.interner();
    // Pre-AIMS specialization structurally materializes compound type
    // identities. Own that append-only pool evolution inside realization; the
    // frontend pool remains immutable and every physical projection consumes
    // the pool retained by the closed artifact.
    let mut realization_pool = pool.clone();
    let pool = &mut realization_pool;

    // SAFETY: FunctionCompiler's lifetimes tie the block's borrow of `scx` to
    // the return lifetime; ManuallyDrop + raw-pointer reborrow detaches it.
    // Sound: `scx` lives for the whole function and the borrows end at the block.
    let scx = ManuallyDrop::new(SimpleCx::new(context, module_name));

    let (codegen_errors, codegen_descriptions, exports) = {
        // SAFETY: `scx_ref` is a detached reborrow of `scx`, which `ManuallyDrop`
        // holds alive for this whole function; the reborrow's lifetime ends
        // inside this block, so the detachment is sound.
        let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

        let eh_model = target_triple.map_or(EhModel::Itanium, EhModel::from_triple);
        let mut builder = IrBuilder::new_aot(scx_ref, eh_model);

        // Runtime functions are declared lazily on first `builder.runtime_fn(name)` use.
        let function_sigs = oric::typeck::build_function_sigs(parse_result, type_result);
        let derived_analysis = crate::realization::lower_non_generic_derived_methods_for_analysis(
            &type_result.typed.accepted_derives,
            &type_result.typed.derived_call_plans,
            interner,
            pool,
        )
        .map_err(|problems| {
            format!(
                "derived-method ARC lowering failed with {} problem(s): {problems:?}",
                problems.len()
            )
        })?;
        let borrow_inference::ArcBatchLoweringResult {
            groups,
            mono_functions,
        } = match borrow_inference::lower_arc_batch(
            parse_result,
            &function_sigs,
            &type_result.typed.impl_sigs,
            import_sigs,
            imported,
            canon,
            interner,
            pool,
            &type_result.typed.mono_instances,
            &type_result.typed.accepted_derives,
            &type_result.typed.derived_call_plans,
        ) {
            Ok(result) => result,
            Err(borrow_inference::ArcBatchLoweringFailure::ArcLowering) => {
                // Why: diagnostics are already emitted; continuing with an
                // empty cache would recurse while resolving missing callees.
                return Err(
                    "ARC lowering rejected this module; fix the source diagnostics emitted above, then rebuild"
                        .to_string(),
                );
            }
            Err(borrow_inference::ArcBatchLoweringFailure::MonoInventory(error)) => {
                return Err(error.to_string());
            }
            Err(borrow_inference::ArcBatchLoweringFailure::CallableCensus(error)) => {
                return Err(error.to_string());
            }
        };

        // Why: impl methods join the analysis set so interprocedural range
        // analysis sees their call sites; they are lowered once (with
        // type-qualified analysis names) and reused for both the repr plan
        // and the as-compiled impl-method contract pre-pass.
        let crate::realization::ImplMethodAnalysis {
            groups: impl_groups,
            targets: mut impl_qualified_by_recv,
            user_drop_bindings: typed_user_drop_bindings,
            emission_names: impl_emission_names,
            ..
        } = crate::realization::lower_impl_methods_for_analysis(
            parse_result,
            type_result,
            interner,
            canon,
            pool,
        )
        .map_err(|problems| {
            format!(
                "impl-method ARC lowering failed with {} problem(s): {problems:?}",
                problems.len()
            )
        })?;

        for (key, target) in derived_analysis.targets {
            impl_qualified_by_recv.entry(key).or_insert(target);
        }
        crate::realization::extend_mono_method_targets(
            &mut impl_qualified_by_recv,
            &mono_functions,
            interner,
            pool,
        )
        .map_err(|problems| {
            format!(
                "monomorphized method target census failed with {} problem(s): {problems:?}",
                problems.len()
            )
        })?;

        let mut lowered_batch =
            crate::realization::LoweredArcBatch::try_from_groups(groups, interner)
                .map_err(|error| error.to_string())?;
        for group in impl_groups {
            lowered_batch
                .insert(group, interner)
                .map_err(|error| error.to_string())?;
        }
        for group in derived_analysis.groups {
            lowered_batch
                .insert(group, interner)
                .map_err(|error| error.to_string())?;
        }
        let prepared_batch = lowered_batch
            .prepare(&mono_functions, &impl_qualified_by_recv, pool, interner)
            .map_err(|error| error.to_string())?;

        // Preparation materializes exact compound identities for specialized
        // lambdas. Rebuild the registry only after that append-only pool
        // evolution so AIMS sees burdens for the exact prepared batch.
        let mut type_registry = ori_types::TypeRegistry::from_typed_exports(
            type_result.typed.types.clone(),
            type_result.typed.collection_burdens.clone(),
        );
        // Imported bodies can materialize collection types absent from their
        // signatures. Fill those exact burdens from the closed realization pool.
        ori_types::register_resolved_collection_burdens(pool, &mut type_registry);
        // Why: only non-generic impl methods enter the analysis set — generics
        // are skipped by the ARC lowering loop.
        let has_impl_methods = type_result
            .typed
            .impl_sigs
            .iter()
            .any(|entry| !entry.sig.is_generic());
        let repr_plan = crate::realization::compute_module_repr_plan(
            pool,
            prepared_batch.functions(),
            narrowing_policy,
            type_result,
            Some(interner),
            imported_type_metadata,
            imported_collection_surfaces,
            has_impl_methods,
        );

        let externals = ori_repr::monomorphize::realize_imported_callables(import_sigs, pool)
            .map_err(|error| format!("imported callable realization failed: {error}"))?;
        let user_drop_bindings = crate::realization::collect_user_drop_bindings(
            &type_registry,
            &typed_user_drop_bindings,
            pool,
        )
        .map_err(|error| format!("user-drop callable realization failed: {error}"))?;
        let roots = prepared_batch.parent_roots();
        let cli_entry = parse_result
            .module
            .functions
            .iter()
            .zip(&function_sigs)
            .find_map(|(function, signature)| signature.is_main.then_some(function.name));
        let executable = crate::realization::realize_arc_program(
            crate::realization::ArcProgramRealizationInput {
                prepared: prepared_batch,
                pool: pool.clone(),
                symbols: db.shared_interner(),
                roots,
                cli_entry,
                externals,
                user_drop_bindings,
                repr_plan,
                type_registry,
                verify_arc: std::env::var(crate::debug_flags::ORI_VERIFY_ARC)
                    .is_ok_and(|value| value != "0"),
            },
        )
        .map_err(|error| format!("closed executable realization failed: {error}"))?;

        let artifact_pool = executable.pool();
        let artifact_repr_plan = executable.repr_plan();
        let artifact_classifier = ori_arc::ArcClassifier::new(artifact_pool);
        finalize::dump_arc_phases(&executable, interner, source_path);
        let annotated_sigs = artifact_annotated_signatures(&executable, import_sigs)?;
        let store = TypeInfoStore::new_with_plan(artifact_pool, artifact_repr_plan);
        let resolver =
            TypeLayoutResolver::new(&store, scx_ref, Some(interner), Some(artifact_repr_plan));

        type_registration::register_user_types(&resolver, &type_result.typed.types);

        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            artifact_pool,
            symbol_prefix,
            &annotated_sigs,
            &artifact_classifier,
            None, // Debug info wiring deferred to AOT pipeline integration
            std::env::var(crate::debug_flags::ORI_VERIFY_ARC).is_ok_and(|v| v != "0"),
        );
        fc.bind_executable_program(&executable);

        if !import_sigs.is_empty() {
            fc.declare_imports(import_sigs);
        }
        fc.declare_all(&parse_result.module.functions, &function_sigs);
        fc.declare_mono_functions(&mono_functions);

        // Impl methods keep their qualified physical symbols. Every other
        // family not claimed by source/mono/import declarations is projected
        // generically from the closed executable artifact.
        let deferred_artifact_parents: Vec<_> =
            impl_emission_names.iter().flatten().copied().collect();
        let artifact_remainder = fc.declare_artifact_remainder(&deferred_artifact_parents);

        // Impl emission keeps its source-level ABI/symbol path, but every body
        // and callable fact is loaded by its qualified artifact identity.
        if !parse_result.module.impls.is_empty() {
            fc.compile_impls_from_artifact(
                &parse_result.module.impls,
                &type_result.typed.impl_sigs,
                canon,
                &parse_result.module.traits,
                &impl_emission_names,
            );
        }
        fc.bind_user_drop_targets();

        if parse_result
            .module
            .types
            .iter()
            .any(|t| !t.derives.is_empty())
        {
            fc.compile_derives(&parse_result.module, &type_result.typed.types);
        }

        // Why: two-pass (prepare everything, then emit) so the nounwind set is
        // complete before any LLVM IR is emitted.
        let mut prepared =
            fc.prepare_all_from_artifact(&parse_result.module.functions, &function_sigs);

        prepared.extend(fc.prepare_mono_from_artifact(&mono_functions));
        prepared.extend(fc.prepare_artifact_remainder_from_artifact(&artifact_remainder));

        fc.compute_nounwind_set(&prepared);
        fc.emit_prepared_functions(prepared);

        // Why: inlined Apply callees (e.g. built-in @length) can leave no invoke
        // instructions, so nounwind is re-derived post-hoc.
        fc.apply_posthoc_nounwind();

        let panic_name = parse_result
            .module
            .functions
            .iter()
            .find(|f| interner.lookup(f.name) == "panic")
            .map(|f| f.name);
        if let Some((func, sig)) = parse_result
            .module
            .functions
            .iter()
            .zip(function_sigs.iter())
            .find(|(_, sig)| sig.is_main)
        {
            fc.generate_main_wrapper(func.name, sig, panic_name);
        }

        let exports = project_callable_exports(
            &executable,
            parse_result,
            &function_sigs,
            interner,
            symbol_prefix,
        )?;

        // Why: AOT must check soft codegen errors (as the JIT path does) to
        // avoid emitting crashing binaries.
        (
            builder.codegen_error_count() + store.type_error_count(),
            builder.codegen_error_descriptions(),
            exports,
        )
    };

    // SAFETY: the compilation block's borrows have ended; ManuallyDrop only
    // suppressed the borrow checker. `into_inner()` would move SimpleCx's other
    // fields while the ManuallyDrop exists, so `finalize_module` clones the module.
    let module = finalize::finalize_module(
        &scx,
        codegen_errors,
        &codegen_descriptions,
        source_path,
        pool,
        interner,
    )?;
    Ok(LlvmCodegenOutput { module, exports })
}

#[cfg(feature = "llvm")]
fn artifact_annotated_signatures(
    program: &ori_repr::executable::ExecutableProgram,
    imports: &[ori_repr::monomorphize::ImportSig],
) -> Result<rustc_hash::FxHashMap<ori_ir::Name, ori_arc::AnnotatedSig>, String> {
    use ori_arc::aims::lattice::{AccessClass, Consumption};
    use ori_arc::{AnnotatedParam, Ownership};

    let mut signatures = rustc_hash::FxHashMap::default();
    for function in program.functions() {
        let function_id = program.function_id(function.name).ok_or_else(|| {
            format!(
                "validated executable has no stable identity for {:?}",
                function.name
            )
        })?;
        signatures.insert(
            function.name,
            program
                .function_contract(function_id)
                .to_annotated_sig(&function.params, function.return_type),
        );
    }
    for import in imports {
        let external_id = program.external_function_id(import.name).ok_or_else(|| {
            format!(
                "validated executable has no external callable for imported alias {:?}",
                import.name
            )
        })?;
        let external = program.external_function(external_id);
        let params = import
            .sig
            .param_names
            .iter()
            .copied()
            .zip(external.parameter_types().iter().copied())
            .zip(&external.contract().params)
            .map(|((name, ty), contract)| AnnotatedParam {
                name,
                ty,
                ownership: if contract.consumption == Consumption::Dead
                    || contract.access == AccessClass::Borrowed
                {
                    Ownership::Borrowed
                } else {
                    Ownership::Owned
                },
            })
            .collect();
        signatures.insert(
            import.name,
            ori_arc::AnnotatedSig {
                params,
                return_type: external.return_type(),
            },
        );
    }
    Ok(signatures)
}

#[cfg(feature = "llvm")]
fn project_callable_exports(
    program: &ori_repr::executable::ExecutableProgram,
    parse: &ParseOutput,
    signatures: &[ori_types::FunctionSig],
    interner: &ori_ir::StringInterner,
    symbol_prefix: &str,
) -> Result<Vec<RealizedCallableExport>, String> {
    let mangler = ori_llvm::aot::Mangler::new();
    let mut exports = Vec::new();
    for (function, signature) in parse.module.functions.iter().zip(signatures) {
        if !function.visibility.is_public() || signature.is_generic() {
            continue;
        }
        let function_id = program.function_id(function.name).ok_or_else(|| {
            format!(
                "closed executable omitted public callable '{}'",
                interner.lookup(function.name)
            )
        })?;
        let source_name = interner.lookup(function.name).to_string();
        let mangled_name = mangler.mangle_function(symbol_prefix, &source_name);
        let metadata = program.export_callable_metadata(function_id, &mangled_name);
        exports.push(RealizedCallableExport {
            mangled_name,
            source_name,
            metadata,
        });
    }
    Ok(exports)
}
