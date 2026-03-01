//! JIT compilation pipeline for `OwnedLLVMEvaluator`.
//!
//! Extracted from `evaluator/mod.rs` to keep the module under 500 lines.
//! Contains the `compile_module_with_tests` method which orchestrates the
//! full V2 codegen pipeline: type infrastructure → function compilation →
//! test wrapper generation → IR verification → JIT engine creation.

use std::mem::ManuallyDrop;

use rustc_hash::FxHashMap;
use tracing::{debug, instrument};

use ori_ir::ast::{Module, TestDef};
use ori_ir::canon::CanonResult;
use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, TypeEntry};

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::codegen::type_registration;
use crate::context::SimpleCx;

use super::runtime_mappings;
use super::{llvm_dump_requested, CompiledTestModule, ImportedFunctionForCodegen, LLVMEvalError};

impl super::OwnedLLVMEvaluator<'_> {
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
    /// - `impl_sigs`: Impl method signatures as (`Name`, `FunctionSig`) pairs
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
        impl_sigs: &[(Name, FunctionSig)],
        imported_functions: &[ImportedFunctionForCodegen<'_>],
        mono_instances: &[ori_types::MonoInstance],
        annotated_sigs: &FxHashMap<Name, ori_arc::AnnotatedSig>,
        mut arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
    ) -> Result<CompiledTestModule<'a>, LLVMEvalError> {
        use inkwell::OptimizationLevel;

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

            // 2. Type infrastructure
            let store = TypeInfoStore::new(self.pool);
            let resolver = TypeLayoutResolver::new(&store, scx_ref, Some(interner));

            // 3. IR builder
            let mut builder = IrBuilder::new(scx_ref);

            // 4. Runtime functions: declared lazily via builder.runtime_fn()
            // (no eager declare_runtime() call needed)

            // 5. Register user-defined types
            type_registration::register_user_types(&resolver, user_types);

            // 5b. ARC classifier for type classification during codegen
            let classifier = ori_arc::ArcClassifier::new(self.pool);

            // 5c. Collect monomorphized generic functions (needed for declaration)
            let mono_functions = crate::monomorphize::collect_mono_functions(
                mono_instances,
                function_sigs,
                interner,
                self.pool,
            );

            // 5d. Interprocedural uniqueness analysis (COW check elimination).
            let uniqueness_summaries = {
                let all_funcs: Vec<ori_arc::ArcFunction> = arc_cache
                    .values()
                    .flat_map(|(parent, lambdas)| std::iter::once(parent).chain(lambdas.iter()))
                    .cloned()
                    .collect();
                ori_arc::run_uniqueness_analysis(&all_funcs, &classifier, interner)
            };

            // 6. Two-pass function compilation
            debug!("declaring functions (phase 1)");
            let mut fc = FunctionCompiler::new(
                &mut builder,
                &store,
                &resolver,
                interner,
                self.pool,
                "",
                annotated_sigs,
                &classifier,
                None, // No debug info for JIT
                uniqueness_summaries,
            );
            fc.declare_all(&module.functions, function_sigs);

            // 6b. Declare imported functions (phase 1)
            // Imported functions must be declared before function body emission
            // so that call sites in the main module can resolve references to them.
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

            // 6c. Declare monomorphized generic functions (phase 1)
            if !mono_functions.is_empty() {
                debug!(
                    count = mono_functions.len(),
                    "declaring monomorphized functions"
                );
                fc.declare_mono_functions(&mono_functions);
            }

            // 7. Compile impl methods (declare + define)
            // Impl methods still lower inline — they use type-qualified canon
            // lookup paths and are not pre-lowered for borrow inference.
            if !module.impls.is_empty() {
                debug!("compiling impl methods");
                fc.compile_impls(&module.impls, impl_sigs, canon, &module.traits);
            }

            // 7b. Compile derived trait methods
            if module.types.iter().any(|t| !t.derives.is_empty()) {
                debug!("compiling derived trait methods");
                fc.compile_derives(module, user_types);
            }

            // 8. Two-pass function compilation for sound nounwind analysis:
            //    a) Lower all functions to ARC IR (no LLVM emission)
            //    b) Build complete nounwind set via fixed-point analysis
            //    c) Emit LLVM IR using the complete nounwind set
            //
            // This ensures monomorphized callee nounwind status is available
            // when analyzing callers, preventing unnecessary invoke+landingpad.
            debug!("preparing function bodies (phase 2a, ARC pipeline)");
            let mut prepared =
                fc.prepare_all_cached(&module.functions, function_sigs, canon, &mut arc_cache);

            // 8b. Prepare imported function bodies
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
                        &mut arc_cache,
                    ));
                }
            }

            // 8c. Prepare monomorphized function bodies
            if !mono_functions.is_empty() {
                debug!(
                    count = mono_functions.len(),
                    "preparing monomorphized function bodies"
                );
                prepared.extend(fc.prepare_mono_cached(&mono_functions, canon, &mut arc_cache));
            }

            // 8d. Build complete nounwind set and emit LLVM IR
            fc.compute_nounwind_set(&prepared);
            fc.emit_prepared_functions(prepared);

            // 9. Compile test wrappers
            debug!("compiling test wrappers");
            let wrappers = fc.compile_tests(tests, canon);

            // Drop fc to release &mut builder borrow
            drop(fc);

            let errors = builder.codegen_error_count();
            let descriptions = builder.codegen_error_descriptions();
            (wrappers, errors, descriptions)
            // builder, resolver, store dropped here
        };

        // Bail out early if codegen produced type-mismatch errors.
        // Feeding malformed IR to LLVM's verifier or JIT can cause
        // heap corruption (SIGABRT) that kills the entire process.
        if codegen_errors > 0 {
            // Drop scx to free the LLVM Module while the Context (owned by
            // self) is still alive. Previously this was leaked (ManuallyDrop
            // suppressed drop), but that caused the Module's LLVM-internal
            // pointers to dangle when the Context was freed — accumulating
            // leaked modules across many files eventually corrupted LLVM's heap.
            // SAFETY: The Module was created from self.context which is still
            // alive, so LLVMDisposeModule can safely clean up.
            drop(ManuallyDrop::into_inner(scx));
            let details = if codegen_error_descriptions.is_empty() {
                String::new()
            } else {
                format!(":\n  - {}", codegen_error_descriptions.join("\n  - "))
            };
            return Err(LLVMEvalError::new(format!(
                "LLVM codegen had {codegen_errors} type-mismatch error(s) — skipping verification/JIT{details}",
            )));
        }

        // 10. Debug: print IR if requested (supports both new and legacy flag)
        if llvm_dump_requested() {
            eprintln!("=== LLVM IR for compiled module ===");
            eprintln!("{}", scx.llmod.print_to_string());
            eprintln!("=== END IR ===");
        }

        // 11. Verify IR
        if let Err(msg) = scx.llmod.verify() {
            // Drop scx to free the Module while Context is alive (see codegen_errors note).
            drop(ManuallyDrop::into_inner(scx));
            return Err(LLVMEvalError::new(format!(
                "LLVM IR verification failed: {msg}"
            )));
        }

        // 11.5. RC audit (gated on ORI_AUDIT_CODEGEN=1)
        if crate::verify::audit_requested() {
            let audit_report = crate::verify::audit_module(&scx.llmod);
            audit_report.emit_to_stderr();
            if audit_report.has_errors() {
                let n = audit_report.error_count();
                drop(ManuallyDrop::into_inner(scx));
                return Err(LLVMEvalError::new(format!("RC audit found {n} error(s)")));
            }
        }

        // 12. Create JIT execution engine
        // SAFETY: Same detached-reference pattern as above — see step 1 comment.
        debug!("creating JIT execution engine");
        let engine = unsafe {
            let module = &*std::ptr::addr_of!(scx.llmod);
            let eng = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .map_err(|e| LLVMEvalError::new(e.to_string()))?;
            runtime_mappings::add_runtime_mappings_to_engine(&eng, module)?;
            eng
        };

        Ok(CompiledTestModule {
            engine,
            test_wrappers,
        })
    }
}
