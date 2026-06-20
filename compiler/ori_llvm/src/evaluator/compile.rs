//! JIT compilation pipeline for `OwnedLLVMEvaluator`.
//!
//! Contains the `compile_module_with_tests` method which orchestrates the
//! full V2 codegen pipeline: type infrastructure → function compilation →
//! test wrapper generation → IR verification → JIT engine creation.

use std::mem::ManuallyDrop;

use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{debug, instrument};

use ori_ir::ast::{Module, TestDef};
use ori_ir::canon::CanonResult;
use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, TypeEntry};

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::function_compiler::MonoTargetMaps;
use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::codegen::type_registration;
use crate::context::SimpleCx;

use super::runtime_mappings;
use super::{llvm_dump_requested, CompiledTestModule, ImportedFunctionForCodegen, LLVMEvalError};

impl<'tcx> super::OwnedLLVMEvaluator<'tcx> {
    /// Compile an entire module with all its tests using the V2 pipeline.
    ///
    /// This is the recommended way to run multiple tests from the same module.
    /// It compiles all functions and test wrappers ONCE, then returns a
    /// `CompiledTestModule` that can run individual tests without recompilation.
    ///
    /// # Performance
    ///
    /// For a module with N functions and M tests:
    /// - Old approach: O(M × N) function compilations (each test recompiles all)
    /// - This approach: O(N + M) function compilations (compile once, run many)
    ///
    /// # Arguments
    ///
    /// - `module`: The parsed module containing functions and type declarations
    /// - `tests`: The tests to compile wrappers for
    /// - `canon`: Canonical IR for this module
    /// - `interner`: String interner for name resolution
    /// - `function_sigs`: Function signatures from type checker (aligned with module.functions)
    /// - `user_types`: User-defined type entries from type checker
    /// - `impl_sigs`: Impl signatures as (`Idx`, `Name`, `FunctionSig`) triples, where `Idx` is the owning impl receiver type
    /// - `imported_functions`: Individual imported functions to compile into
    ///   this JIT module so calls to them resolve correctly
    /// - `mono_instances`: Monomorphized generic function instances
    /// - `annotated_sigs`: Pre-computed borrow inference results from the caller
    /// - `arc_cache`: Pre-lowered ARC functions (consumed during define phase)
    #[instrument(skip_all, level = "debug", fields(
        functions = module.functions.len(),
        tests = tests.len(),
        imports = imported_functions.len(),
    ))]
    #[expect(
        clippy::too_many_arguments,
        reason = "JIT compilation pipeline — all params are required data flow inputs"
    )]
    pub fn compile_module_with_tests<'a>(
        &'a self,
        module: &Module,
        tests: &[&TestDef],
        canon: &CanonResult,
        interner: &StringInterner,
        function_sigs: &[FunctionSig],
        user_types: &[TypeEntry],
        collection_burdens: &[(ori_types::Idx, ori_types::burden::UserBurdenSpec)],
        impl_sigs: &[(ori_types::Idx, Name, FunctionSig)],
        imported_functions: &[ImportedFunctionForCodegen<'_>],
        mono_instances: &[ori_types::MonoInstance],
        annotated_sigs: &FxHashMap<Name, ori_arc::AnnotatedSig>,
        mut arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
        narrowing_policy: Option<ori_repr::NarrowingPolicy>,
        imported_type_metadata: &[ori_types::ExportedTypeMetadata],
        imported_collection_surfaces: &[u64],
        trait_impl_fn_names: &[(ori_types::Idx, Name)],
        imported_mono_functions: Vec<crate::monomorphize::MonoFunction>,
    ) -> Result<CompiledTestModule<'a>, LLVMEvalError> {
        // --- V2 pipeline ---

        // 1. Create LLVM module context.
        //
        // We use ManuallyDrop + raw-pointer reborrow to work around a borrow
        // checker limitation: FunctionCompiler's lifetime parameters tie the
        // compilation block's borrow of `scx` to the return lifetime, preventing
        // us from creating the ExecutionEngine afterward. The raw-pointer
        // roundtrip (`scx_ref`) creates a detached reference whose borrow
        // doesn't leak out of the block. This is sound because:
        //
        // - `scx` lives for the entire function (ManuallyDrop suppresses drop)
        // - The compilation block's borrows genuinely end at the block boundary
        // - `create_jit_execution_engine` takes C-level ownership of the module
        //   (the Rust `Module` becomes a shell — see inkwell's `owned_by_ee`)
        //   and returns `ExecutionEngine<'ctx>` tied to the Context lifetime
        let scx = ManuallyDrop::new(SimpleCx::new(&self.context, "test_module"));

        let (test_wrappers, codegen_errors, codegen_error_descriptions) = {
            // SAFETY: Detached reference to scx — see comment above.
            let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

            self.compile_all_functions(
                scx_ref,
                module,
                tests,
                canon,
                interner,
                function_sigs,
                user_types,
                collection_burdens,
                impl_sigs,
                imported_functions,
                mono_instances,
                annotated_sigs,
                &mut arc_cache,
                narrowing_policy,
                imported_type_metadata,
                imported_collection_surfaces,
                trait_impl_fn_names,
                imported_mono_functions,
            )
        };

        Self::finalize_jit(
            scx,
            test_wrappers,
            codegen_errors,
            &codegen_error_descriptions,
        )
    }

    /// Compile all functions, impls, derives, and test wrappers into the LLVM module.
    ///
    /// Returns `(test_wrappers, codegen_error_count, codegen_error_descriptions)`.
    #[expect(
        clippy::too_many_arguments,
        reason = "JIT compilation — all params are required data flow inputs"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "sequential JIT pipeline — splitting would fragment the compilation flow"
    )]
    fn compile_all_functions(
        &self,
        scx_ref: &'tcx SimpleCx<'tcx>,
        module: &Module,
        tests: &[&TestDef],
        canon: &CanonResult,
        interner: &StringInterner,
        function_sigs: &[FunctionSig],
        user_types: &[TypeEntry],
        collection_burdens: &[(ori_types::Idx, ori_types::burden::UserBurdenSpec)],
        impl_sigs: &[(ori_types::Idx, Name, FunctionSig)],
        imported_functions: &[ImportedFunctionForCodegen<'_>],
        mono_instances: &[ori_types::MonoInstance],
        annotated_sigs: &FxHashMap<Name, ori_arc::AnnotatedSig>,
        arc_cache: &mut FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
        narrowing_policy: Option<ori_repr::NarrowingPolicy>,
        imported_type_metadata: &[ori_types::ExportedTypeMetadata],
        imported_collection_surfaces: &[u64],
        trait_impl_fn_names: &[(ori_types::Idx, Name)],
        imported_mono_functions: Vec<crate::monomorphize::MonoFunction>,
    ) -> (FxHashMap<Name, String>, u32, Vec<String>) {
        // Type infrastructure
        let classifier = ori_arc::ArcClassifier::new(self.pool);

        // Compute representation plan (canonical reprs only).
        let all_arc_funcs = ori_arc::collect_all_arc_functions(arc_cache);
        let policy = narrowing_policy.unwrap_or_else(|| {
            if ori_repr::NarrowingPolicy::env_disabled() {
                ori_repr::NarrowingPolicy::Disabled
            } else {
                ori_repr::NarrowingPolicy::Aggressive
            }
        });
        // Extract #repr attributes from user types for the repr plan.
        let repr_attrs: Vec<(ori_types::Idx, ori_ir::ReprAttrKind)> = user_types
            .iter()
            .filter_map(|te| te.repr.map(|r| (te.idx, r)))
            .collect();
        // Extract public type indices for ABI-safe narrowing.
        let mut pub_type_indices: Vec<ori_types::Idx> = user_types
            .iter()
            .filter(|te| te.visibility == ori_types::Visibility::Public)
            .map(|te| te.idx)
            .collect();
        // Mark collection wrapper types from public function signatures as
        // public, recursively walking into nested types.
        for sig in function_sigs {
            if sig.is_public {
                for &param_ty in &sig.param_types {
                    ori_types::walk_collection_types(self.pool, param_ty, &mut |idx| {
                        if !pub_type_indices.contains(&idx) {
                            pub_type_indices.push(idx);
                        }
                    });
                }
                ori_types::walk_collection_types(self.pool, sig.return_type, &mut |idx| {
                    if !pub_type_indices.contains(&idx) {
                        pub_type_indices.push(idx);
                    }
                });
            }
        }
        // Collect unconstrained function names (pub + trait impl).
        // Uses trait_impl_fn_names (not all impl_sigs) per
        let unconstrained_fn_names = crate::collect_unconstrained_fn_names(
            function_sigs,
            trait_impl_fn_names,
            Some(interner),
        );
        let repr_plan = ori_repr::compute_repr_plan_with_interner(
            self.pool,
            &all_arc_funcs,
            policy,
            &repr_attrs,
            Some(interner),
            &pub_type_indices,
            imported_type_metadata,
            imported_collection_surfaces,
            &unconstrained_fn_names,
            // JIT also has analysis-only impl methods.
            impl_sigs.iter().any(|(_, _, sig)| !sig.is_generic()),
        );
        let store = TypeInfoStore::new_with_plan(self.pool, &repr_plan);
        let resolver = TypeLayoutResolver::new(&store, scx_ref, Some(interner), Some(&repr_plan));
        let mut builder = IrBuilder::new_jit(scx_ref);
        type_registration::register_user_types(&resolver, user_types);

        let mut mono_functions = crate::monomorphize::collect_mono_functions(
            mono_instances,
            function_sigs,
            impl_sigs,
            // JIT: single-module scope; cross-module imports are an AOT concern.
            &[],
            interner,
            self.pool,
        );
        mono_functions.extend(imported_mono_functions);

        // PC-2 contract check (PC-2) — diagnostic localization for
        // JIT pre-mono IR. Non-load-bearing: the primary seam in
        // process_arc_function owns record_codegen_error(); this site only
        // attributes diagnostics to the caller-pre-populated arc_cache.
        //
        // Empty exempt set: lower_and_infer_borrows skips generics in
        // arc_lowering.rs; arc_cache contains only non-generic bodies +
        // imported monomorphized instances — both categories have empty
        // scheme_var_ids.
        let exempt: FxHashSet<u32> = FxHashSet::default();
        for (_fn_name, (arc_fn, lambdas)) in arc_cache.iter() {
            if let Err(err) =
                ori_arc::assert_no_unresolved_type_vars(self.pool, arc_fn, interner, &exempt)
            {
                tracing::error!(
                    contract_violation = true,
                    error = ?err,
                    site = "jit_pre_mono",
                    "Tag::Var in JIT pre-mono ARC IR"
                );
            }
            for lambda in lambdas {
                if let Err(err) =
                    ori_arc::assert_no_unresolved_type_vars(self.pool, lambda, interner, &exempt)
                {
                    tracing::error!(
                        contract_violation = true,
                        error = ?err,
                        site = "jit_pre_mono_lambda",
                        "Tag::Var in JIT pre-mono lambda ARC IR"
                    );
                }
            }
        }

        // INVARIANT: every reachable mono lands in arc_cache before the
        // interprocedural pass below (PL-5: no-stale-summary). Without this,
        // analyze_program sees an arc_cache missing every mono and produces
        // CONSERVATIVE contracts for every generic call site.
        if !mono_functions.is_empty() {
            crate::codegen::function_compiler::pre_lower_monos_to_arc_cache(
                &mono_functions,
                canon,
                interner,
                self.pool,
                arc_cache,
            );
            // INVARIANT: Apply / Invoke targets in cached ArcFunctions resolve
            // to mangled mono names before AIMS contract lookup. Without this,
            // forwarder bodies (e.g., wrap calling @id) reference generic names
            // that miss `analyze_program`'s mono-keyed contract map, breaking
            // transitive transfers_through_return propagation across hops.
            crate::codegen::function_compiler::rewrite_apply_targets_for_monos(
                arc_cache,
                &mono_functions,
                self.pool,
                interner,
            );
        }

        let (uniqueness_summaries, aims_contracts) =
            Self::run_interprocedural_analyses(arc_cache, &classifier, interner);

        // Reconstruct the TypeRegistry from the type-checker exports so the
        // Phase-5 burden walker resolves collection / closure UserBurdenSpec.
        // Mirrors the AOT codegen_pipeline reconstruction (eval/LLVM parity).
        // Spec: Annex E §AIMS.
        let type_registry = ori_types::TypeRegistry::from_typed_exports(
            user_types.to_vec(),
            collection_burdens.to_vec(),
        );

        // Two-pass function compilation
        debug!("declaring functions (phase 1)");
        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            self.pool,
            &type_registry,
            "",
            annotated_sigs,
            &classifier,
            None, // No debug info for JIT
            uniqueness_summaries,
            aims_contracts,
            std::env::var("ORI_VERIFY_ARC").is_ok_and(|v| v != "0"),
        );
        fc.declare_all(&module.functions, function_sigs);

        // Declare imported functions
        if !imported_functions.is_empty() {
            debug!(
                count = imported_functions.len(),
                "declaring imported functions"
            );
            for imp_fn in imported_functions {
                fc.declare_all(
                    std::slice::from_ref(imp_fn.function),
                    std::slice::from_ref(&imp_fn.sig),
                );
            }
        }

        // Declare monomorphized generic functions
        if !mono_functions.is_empty() {
            debug!(
                count = mono_functions.len(),
                "declaring monomorphized functions"
            );
            fc.declare_mono_functions(&mono_functions);
        }

        // Compile impl methods
        if !module.impls.is_empty() {
            debug!("compiling impl methods");
            fc.compile_impls(&module.impls, impl_sigs, canon, &module.traits);
        }

        // Compile derived trait methods
        if module.types.iter().any(|t| !t.derives.is_empty()) {
            debug!("compiling derived trait methods");
            fc.compile_derives(module, user_types);
        }

        // Prepare bodies (ARC pipeline), compute nounwind set, emit LLVM IR
        debug!("preparing function bodies (phase 2a, ARC pipeline)");
        let mut prepared =
            fc.prepare_all_cached(&module.functions, function_sigs, canon, arc_cache);

        if !imported_functions.is_empty() {
            debug!(
                count = imported_functions.len(),
                "preparing imported function bodies"
            );
            for imp_fn in imported_functions {
                prepared.extend(fc.prepare_all_cached(
                    std::slice::from_ref(imp_fn.function),
                    std::slice::from_ref(&imp_fn.sig),
                    imp_fn.canon,
                    arc_cache,
                ));
            }
        }

        if !mono_functions.is_empty() {
            debug!(
                count = mono_functions.len(),
                "preparing monomorphized function bodies"
            );
            prepared.extend(fc.prepare_mono_cached(&mono_functions, canon, arc_cache));
        }

        fc.compute_nounwind_set(&prepared);
        fc.emit_prepared_functions(prepared);

        // Compile test wrappers
        debug!("compiling test wrappers");
        // Test bodies are lowered directly here (NOT via `arc_cache`), so they
        // miss the `rewrite_apply_targets_for_monos` pass that rewrites generic
        // call targets to mangled mono names. Without it, a test body's
        // `Invoke @assert_eq(generic)` misses the mono-keyed AIMS contract
        // (`@assert_eq$m$..`), so a borrowed transfer-through-return arg is
        // mis-excluded from the dead-collection release scan and leaks. Pass the
        // mono maps so test bodies get the same rewrite as `arc_cache` functions
        // (eval/AOT contract parity).
        let mono_target_maps =
            (!mono_functions.is_empty()).then(|| MonoTargetMaps::build(&mono_functions, self.pool));
        let wrappers = fc.compile_tests(tests, canon, mono_target_maps.as_ref());

        // Post-hoc nounwind: catch impl methods and test wrappers that were
        // compiled before the two-pass analysis but contain no invoke instructions.
        fc.apply_posthoc_nounwind();
        drop(fc);

        let errors = builder.codegen_error_count() + store.type_error_count();
        let descriptions = builder.codegen_error_descriptions();
        (wrappers, errors, descriptions)
    }

    /// Run interprocedural analyses: uniqueness (COW) and AIMS contracts (ownership).
    fn run_interprocedural_analyses(
        arc_cache: &FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
        classifier: &ori_arc::ArcClassifier,
        interner: &StringInterner,
    ) -> (
        FxHashMap<Name, ori_arc::UniquenessSummary>,
        FxHashMap<Name, ori_arc::MemoryContract>,
    ) {
        let all_funcs = ori_arc::collect_all_arc_functions(arc_cache);
        let uniqueness_summaries =
            ori_arc::run_uniqueness_analysis(&all_funcs, classifier, interner);

        let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
        let mut all_funcs_mut = all_funcs;
        let aims_contracts =
            ori_arc::compute_aims_contracts(&mut all_funcs_mut, classifier, interner, &builtins);

        (uniqueness_summaries, aims_contracts)
    }

    /// Validate compiled IR and create the JIT execution engine.
    ///
    /// Checks for codegen errors, optionally dumps IR, verifies the module,
    /// runs the RC audit, registers runtime symbols, and creates the engine.
    fn finalize_jit<'a>(
        scx: ManuallyDrop<SimpleCx<'a>>,
        test_wrappers: FxHashMap<Name, String>,
        codegen_errors: u32,
        codegen_error_descriptions: &[String],
    ) -> Result<CompiledTestModule<'a>, LLVMEvalError> {
        use inkwell::OptimizationLevel;

        // Bail out early if codegen produced errors (type mismatches,
        // ARC IR verification failures, LLVM IR verification failures).
        // Feeding malformed IR to LLVM's verifier or JIT can cause
        // heap corruption (SIGABRT) that kills the entire process.
        if codegen_errors > 0 {
            // SAFETY: The Module was created from a Context that is still alive,
            // so LLVMDisposeModule can safely clean up.
            drop(ManuallyDrop::into_inner(scx));
            let details = if codegen_error_descriptions.is_empty() {
                String::new()
            } else {
                format!(":\n  - {}", codegen_error_descriptions.join("\n  - "))
            };
            return Err(LLVMEvalError::new(format!(
                "LLVM codegen had {codegen_errors} error(s) — skipping verification/JIT{details}",
            )));
        }

        // Debug: print IR if requested
        if llvm_dump_requested() {
            eprintln!("LLVM IR:");
            eprintln!("{}", scx.llmod.print_to_string());
        }

        // Verify IR
        if let Err(msg) = scx.llmod.verify() {
            drop(ManuallyDrop::into_inner(scx));
            return Err(LLVMEvalError::new(format!(
                "LLVM IR verification failed: {msg}"
            )));
        }

        // RC audit (gated on ORI_AUDIT_CODEGEN=1)
        if crate::verify::audit_requested() {
            let audit_report = crate::verify::audit_module(&scx.llmod);
            audit_report.emit_to_stderr();
            if audit_report.has_errors() {
                let n = audit_report.error_count();
                drop(ManuallyDrop::into_inner(scx));
                return Err(LLVMEvalError::new(format!("RC audit found {n} error(s)")));
            }
        }

        // Register runtime symbols + create JIT execution engine.
        // Symbols must be registered BEFORE engine creation so MCJIT's
        // RuntimeDyld can resolve them during module compilation.
        runtime_mappings::ensure_runtime_symbols_registered();

        // SAFETY: Detached reference to scx.llmod — the Module was created
        // from a Context that is still alive. See compile_module_with_tests.
        debug!("creating JIT execution engine");
        let engine = unsafe {
            let module = &*std::ptr::addr_of!(scx.llmod);
            module
                .create_jit_execution_engine(OptimizationLevel::None)
                .map_err(|e| LLVMEvalError::new(e.to_string()))?
        };

        Ok(CompiledTestModule {
            engine,
            test_wrappers,
        })
    }
}
