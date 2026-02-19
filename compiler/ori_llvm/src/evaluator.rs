//! LLVM-based evaluator for running Ori code.
//!
//! This provides a JIT-based evaluator that compiles Ori code to LLVM IR
//! and executes it natively, as an alternative to the tree-walking interpreter.
//!
//! # V2 Architecture
//!
//! The evaluator uses the V2 codegen pipeline:
//! 1. `TypeInfoStore` + `TypeLayoutResolver` for LLVM type computation
//! 2. `IrBuilder` for ID-based instruction emission
//! 3. `FunctionCompiler` for two-pass declare-then-define compilation
//! 4. `ExprLowerer` for AST → LLVM IR lowering
//!
//! The legacy `ModuleCompiler` → `CodegenCx` → `Builder` path is still
//! available via `LLVMEvaluator` for backward compatibility during migration.

use std::mem::ManuallyDrop;

use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use rustc_hash::FxHashMap;
use tracing::{debug, instrument};

use ori_ir::ast::{Function, Module, TestDef};
use ori_ir::canon::CanonResult;
use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, Pool, TypeEntry};

/// A single imported function ready for LLVM compilation.
///
/// Pairs a function AST with its type-checked signature and the canonical IR
/// from its source module. Created by the caller after resolving, filtering,
/// and type-checking imported modules.
pub struct ImportedFunctionForCodegen<'a> {
    /// The function AST from the imported module.
    pub function: &'a Function,
    /// Type-checked signature for this function.
    pub sig: &'a FunctionSig,
    /// Canonical IR for this function's source module.
    pub canon: &'a CanonResult,
}

use crate::codegen::function_compiler::FunctionCompiler;
use crate::codegen::ir_builder::IrBuilder;
use crate::codegen::runtime_decl;
use crate::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
use crate::codegen::type_registration;
use crate::context::SimpleCx;
use crate::runtime;

/// Result type for LLVM evaluation.
pub type LLVMEvalResult = Result<LLVMValue, LLVMEvalError>;

/// Values that can be returned from LLVM evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum LLVMValue {
    /// Void/unit value
    Void,
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Bool(bool),
}

/// Error during LLVM evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LLVMEvalError {
    pub message: String,
}

impl LLVMEvalError {
    pub fn new(message: impl Into<String>) -> Self {
        LLVMEvalError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LLVMEvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LLVMEvalError {}

// ---------------------------------------------------------------------------
// CompiledTestModule
// ---------------------------------------------------------------------------

/// A compiled module with JIT engine ready for test execution.
///
/// All functions and tests are compiled once, then tests can be run multiple times
/// from the same engine. This avoids the O(n²) recompilation problem where each test
/// would otherwise recompile all module functions.
///
/// # Lifetime
///
/// The `'ll` lifetime ties to the LLVM `Context` (owned by `OwnedLLVMEvaluator`).
/// The `ExecutionEngine` takes C-level ownership of the module via
/// `LLVMCreateJITCompilerForModule`, so the Rust `Module` wrapper can be safely
/// dropped after engine creation (it becomes a shell — see inkwell's
/// `Module::owned_by_ee` field).
pub struct CompiledTestModule<'ll> {
    /// The JIT execution engine (owns the compiled machine code).
    engine: ExecutionEngine<'ll>,
    /// Test wrapper function names for lookup.
    /// Maps test `Name` to the wrapper function name string (e.g., `__test_my_test`).
    test_wrappers: FxHashMap<Name, String>,
}

impl CompiledTestModule<'_> {
    /// Run a single test from this compiled module.
    ///
    /// Uses `setjmp`/`longjmp` to recover from panics in JIT-compiled code.
    /// When JIT code calls `ori_panic` or `ori_panic_cstr`, it `longjmp`s back
    /// here instead of calling `exit(1)`, preserving the test runner process.
    ///
    /// # Safety
    ///
    /// The test function must exist in the compiled module and have signature `() -> void`.
    #[allow(
        unsafe_code,
        reason = "JIT execution requires unsafe FFI: get_function, setjmp, and call"
    )]
    pub fn run_test(&self, test_name: Name) -> LLVMEvalResult {
        // Reset panic state before running
        runtime::reset_panic_state();

        // Look up the wrapper function name
        let wrapper_name = self.test_wrappers.get(&test_name).ok_or_else(|| {
            LLVMEvalError::new(format!("Test wrapper not found for test: {test_name:?}"))
        })?;

        // Get function pointer
        // SAFETY: We compiled this test wrapper with signature () -> void
        let test_fn = unsafe {
            self.engine
                .get_function::<unsafe extern "C" fn()>(wrapper_name)
                .map_err(|e| LLVMEvalError::new(format!("Test function not found: {e}")))?
        };

        // Set up setjmp/longjmp recovery for JIT panics
        let mut jmp_buf = runtime::JmpBuf::new();
        let buf_ptr: *mut runtime::JmpBuf = &raw mut jmp_buf;
        runtime::enter_jit_mode(buf_ptr);

        // SAFETY: jmp_buf is stack-allocated and valid for the duration of this call.
        // setjmp returns 0 on direct call, non-zero when longjmp fires.
        let longjmp_fired = unsafe { runtime::jit_setjmp(buf_ptr) } != 0;

        if longjmp_fired {
            // longjmp returned us here — JIT code hit a panic
            runtime::leave_jit_mode();
            let msg = runtime::get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            return Err(LLVMEvalError::new(msg));
        }

        // Normal path: execute the test
        // SAFETY: test_fn has signature () -> void, compiled by us
        unsafe { test_fn.call() };

        runtime::leave_jit_mode();

        // Check if panic occurred via assertions (ori_assert sets state without longjmp)
        if runtime::did_panic() {
            let msg = runtime::get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            Err(LLVMEvalError::new(msg))
        } else {
            Ok(LLVMValue::Void)
        }
    }
}

// ---------------------------------------------------------------------------
// OwnedLLVMEvaluator (V2 pipeline)
// ---------------------------------------------------------------------------

/// LLVM-based evaluator that owns its context.
///
/// Uses the V2 codegen pipeline (`TypeInfoStore` → `IrBuilder` → `FunctionCompiler`).
pub struct OwnedLLVMEvaluator<'tcx> {
    context: Context,
    /// Type pool for resolving compound types (List, Map, etc.)
    pool: &'tcx Pool,
}

impl<'tcx> OwnedLLVMEvaluator<'tcx> {
    /// Create an evaluator with a type pool for compound type resolution.
    #[must_use]
    pub fn with_pool(pool: &'tcx Pool) -> Self {
        OwnedLLVMEvaluator {
            context: Context::create(),
            pool,
        }
    }

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
    #[instrument(skip_all, level = "debug", fields(
        functions = module.functions.len(),
        tests = tests.len(),
        imports = imported_functions.len(),
    ))]
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

        let (test_wrappers, codegen_errors) = {
            // SAFETY: Detached reference to scx — see comment above.
            let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

            // 2. Type infrastructure
            let store = TypeInfoStore::new(self.pool);
            let resolver = TypeLayoutResolver::new(&store, scx_ref);

            // 3. IR builder
            let mut builder = IrBuilder::new(scx_ref);

            // 4. Declare runtime functions
            runtime_decl::declare_runtime(&mut builder);

            // 5. Register user-defined types
            type_registration::register_user_types(&resolver, user_types);

            // 6. Two-pass function compilation
            debug!("declaring functions (phase 1)");
            let mut fc = FunctionCompiler::new(
                &mut builder,
                &store,
                &resolver,
                interner,
                self.pool,
                "",
                None,
                None,
                None, // No debug info for JIT
            );
            fc.declare_all(&module.functions, function_sigs);

            // 6b. Declare imported functions (phase 1)
            // Imported functions must be declared before any define_all so that
            // call sites in the main module can resolve references to them.
            if !imported_functions.is_empty() {
                debug!(
                    count = imported_functions.len(),
                    "declaring imported functions"
                );
                for imp_fn in imported_functions {
                    // declare_all skips generics internally, but we use it
                    // with single-element slices for each imported function
                    fc.declare_all(
                        std::slice::from_ref(imp_fn.function),
                        std::slice::from_ref(imp_fn.sig),
                    );
                }
            }

            // 7. Compile impl methods (declare + define)
            if !module.impls.is_empty() {
                debug!("compiling impl methods");
                fc.compile_impls(&module.impls, impl_sigs, canon, &module.traits);
            }

            // 7b. Compile derived trait methods
            if module.types.iter().any(|t| !t.derives.is_empty()) {
                debug!("compiling derived trait methods");
                fc.compile_derives(module, user_types);
            }

            // 8. Define all function bodies (phase 2)
            debug!("defining function bodies (phase 2)");
            fc.define_all(&module.functions, function_sigs, canon);

            // 8b. Define imported function bodies (phase 2)
            // Bodies are compiled into the same LLVM module so the JIT engine
            // can resolve calls without a linker.
            if !imported_functions.is_empty() {
                debug!("defining imported function bodies (phase 2)");
                for imp_fn in imported_functions {
                    fc.define_all(
                        std::slice::from_ref(imp_fn.function),
                        std::slice::from_ref(imp_fn.sig),
                        imp_fn.canon,
                    );
                }
            }

            // 9. Compile test wrappers
            debug!("compiling test wrappers");
            let wrappers = fc.compile_tests(tests, canon);

            // Drop fc to release &mut builder borrow
            drop(fc);

            let errors = builder.codegen_error_count();
            (wrappers, errors)
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
            return Err(LLVMEvalError::new(format!(
                "LLVM codegen had {codegen_errors} type-mismatch error(s) — skipping verification/JIT",
            )));
        }

        // 10. Debug: print IR if requested
        if std::env::var("ORI_DEBUG_LLVM").is_ok() {
            eprintln!("=== LLVM IR for compiled module ===");
            eprintln!("{}", scx.llmod.print_to_string().to_string());
            eprintln!("=== END IR ===");
        }

        // 11. Verify IR
        if let Err(msg) = scx.llmod.verify() {
            // Drop scx to free the Module while Context is alive (see codegen_errors note).
            drop(ManuallyDrop::into_inner(scx));
            return Err(LLVMEvalError::new(format!(
                "LLVM IR verification failed: {}",
                msg.to_string()
            )));
        }

        // 12. Create JIT execution engine
        // SAFETY: Same detached-reference pattern as above — see step 1 comment.
        debug!("creating JIT execution engine");
        let engine = unsafe {
            let module = &*std::ptr::addr_of!(scx.llmod);
            let eng = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .map_err(|e| LLVMEvalError::new(e.to_string()))?;
            add_runtime_mappings_to_engine(&eng, module)?;
            eng
        };

        Ok(CompiledTestModule {
            engine,
            test_wrappers,
        })
    }
}

// ---------------------------------------------------------------------------
// Runtime mappings
// ---------------------------------------------------------------------------

/// Runtime functions declared in `runtime_decl` that are intentionally NOT
/// in the JIT mapping table. These are only used in AOT compilation.
#[cfg(test)]
pub(crate) const AOT_ONLY_RUNTIME_FUNCTIONS: &[&str] = &[
    // ori_run_main wraps @main with catch_unwind — JIT compiles tests directly
    "ori_run_main",
];

/// Names of all runtime functions registered in the JIT mapping table.
///
/// Used by sync tests to verify declarations and JIT mappings stay aligned.
pub(crate) const JIT_MAPPED_RUNTIME_FUNCTIONS: &[&str] = &[
    "ori_print",
    "ori_print_int",
    "ori_print_float",
    "ori_print_bool",
    "ori_panic",
    "ori_panic_cstr",
    "ori_assert",
    "ori_assert_eq_int",
    "ori_assert_eq_bool",
    "ori_assert_eq_float",
    "ori_list_alloc_data",
    "ori_list_free_data",
    "ori_list_new",
    "ori_list_free",
    "ori_list_len",
    "ori_compare_int",
    "ori_min_int",
    "ori_max_int",
    "ori_str_concat",
    "ori_str_eq",
    "ori_str_ne",
    "ori_str_compare",
    "ori_str_hash",
    "ori_str_next_char",
    "ori_assert_eq_str",
    "ori_str_from_int",
    "ori_str_from_bool",
    "ori_str_from_float",
    "ori_format_int",
    "ori_format_float",
    "ori_format_str",
    "ori_format_bool",
    "ori_format_char",
    "ori_rc_alloc",
    "ori_rc_inc",
    "ori_rc_dec",
    "ori_rc_free",
    "ori_args_from_argv",
    "ori_register_panic_handler",
    "rust_eh_personality",
];

/// Add runtime function mappings to an execution engine.
///
/// Maps declared function names to actual Rust function addresses so the
/// JIT engine can resolve calls to runtime functions.
fn add_runtime_mappings_to_engine(
    engine: &ExecutionEngine<'_>,
    module: &inkwell::module::Module<'_>,
) -> Result<(), LLVMEvalError> {
    let mappings: &[(&str, usize)] = &[
        ("ori_print", runtime::ori_print as *const () as usize),
        (
            "ori_print_int",
            runtime::ori_print_int as *const () as usize,
        ),
        (
            "ori_print_float",
            runtime::ori_print_float as *const () as usize,
        ),
        (
            "ori_print_bool",
            runtime::ori_print_bool as *const () as usize,
        ),
        ("ori_panic", runtime::ori_panic as *const () as usize),
        (
            "ori_panic_cstr",
            runtime::ori_panic_cstr as *const () as usize,
        ),
        ("ori_assert", runtime::ori_assert as *const () as usize),
        (
            "ori_assert_eq_int",
            runtime::ori_assert_eq_int as *const () as usize,
        ),
        (
            "ori_assert_eq_bool",
            runtime::ori_assert_eq_bool as *const () as usize,
        ),
        (
            "ori_assert_eq_float",
            runtime::ori_assert_eq_float as *const () as usize,
        ),
        (
            "ori_list_alloc_data",
            runtime::ori_list_alloc_data as *const () as usize,
        ),
        (
            "ori_list_free_data",
            runtime::ori_list_free_data as *const () as usize,
        ),
        ("ori_list_new", runtime::ori_list_new as *const () as usize),
        (
            "ori_list_free",
            runtime::ori_list_free as *const () as usize,
        ),
        ("ori_list_len", runtime::ori_list_len as *const () as usize),
        (
            "ori_compare_int",
            runtime::ori_compare_int as *const () as usize,
        ),
        ("ori_min_int", runtime::ori_min_int as *const () as usize),
        ("ori_max_int", runtime::ori_max_int as *const () as usize),
        (
            "ori_str_concat",
            runtime::ori_str_concat as *const () as usize,
        ),
        ("ori_str_eq", runtime::ori_str_eq as *const () as usize),
        ("ori_str_ne", runtime::ori_str_ne as *const () as usize),
        (
            "ori_str_compare",
            runtime::ori_str_compare as *const () as usize,
        ),
        ("ori_str_hash", runtime::ori_str_hash as *const () as usize),
        (
            "ori_str_next_char",
            runtime::ori_str_next_char as *const () as usize,
        ),
        (
            "ori_assert_eq_str",
            runtime::ori_assert_eq_str as *const () as usize,
        ),
        (
            "ori_str_from_int",
            runtime::ori_str_from_int as *const () as usize,
        ),
        (
            "ori_str_from_bool",
            runtime::ori_str_from_bool as *const () as usize,
        ),
        (
            "ori_str_from_float",
            runtime::ori_str_from_float as *const () as usize,
        ),
        // Format functions (§3.16 Formattable trait)
        (
            "ori_format_int",
            runtime::format::ori_format_int as *const () as usize,
        ),
        (
            "ori_format_float",
            runtime::format::ori_format_float as *const () as usize,
        ),
        (
            "ori_format_str",
            runtime::format::ori_format_str as *const () as usize,
        ),
        (
            "ori_format_bool",
            runtime::format::ori_format_bool as *const () as usize,
        ),
        (
            "ori_format_char",
            runtime::format::ori_format_char as *const () as usize,
        ),
        ("ori_rc_alloc", runtime::ori_rc_alloc as *const () as usize),
        ("ori_rc_inc", runtime::ori_rc_inc as *const () as usize),
        ("ori_rc_dec", runtime::ori_rc_dec as *const () as usize),
        ("ori_rc_free", runtime::ori_rc_free as *const () as usize),
        (
            "ori_args_from_argv",
            runtime::ori_args_from_argv as *const () as usize,
        ),
        (
            "ori_register_panic_handler",
            runtime::ori_register_panic_handler as *const () as usize,
        ),
        // Exception handling personality function — required by any function
        // containing `invoke`/`landingpad`. Not in the dynamic symbol table,
        // so MCJIT's dlsym-based resolution can't find it automatically.
        ("rust_eh_personality", rust_eh_personality_addr()),
    ];

    // Verify the mapping array stays in sync with JIT_MAPPED_RUNTIME_FUNCTIONS.
    debug_assert_eq!(
        mappings.len(),
        JIT_MAPPED_RUNTIME_FUNCTIONS.len(),
        "JIT mapping array and JIT_MAPPED_RUNTIME_FUNCTIONS constant have different lengths"
    );

    for &(name, addr) in mappings {
        if let Some(func) = module.get_function(name) {
            engine.add_global_mapping(&func, addr);
        }
        // Silently skip functions not declared in this module — they may not
        // be needed if no code calls them.
    }

    Ok(())
}

/// Get the address of `rust_eh_personality` for JIT symbol mapping.
///
/// This function is defined in the Rust standard library and handles
/// DWARF-based exception handling (Itanium ABI). It's present in the
/// host binary but not exported in the dynamic symbol table, so the
/// LLVM MCJIT can't resolve it via `dlsym`. We provide it explicitly.
fn rust_eh_personality_addr() -> usize {
    extern "C" {
        fn rust_eh_personality();
    }
    rust_eh_personality as *const () as usize
}
