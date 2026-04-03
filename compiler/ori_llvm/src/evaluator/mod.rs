//! LLVM-based evaluator for running Ori code.
//!
//! This provides a JIT-based evaluator that compiles Ori code to LLVM IR
//! and executes it natively, as an alternative to the tree-walking interpreter.
//!
//! # V2 Architecture
//!
//! The evaluator is a pure codegen component — it receives pre-computed
//! borrow inference results and pre-lowered ARC functions from the caller
//! (`oric`). It does NOT perform ARC lowering or borrow inference itself.
//!
//! Pipeline:
//! 1. `TypeInfoStore` + `TypeLayoutResolver` for LLVM type computation
//! 2. `IrBuilder` for ID-based instruction emission
//! 3. `FunctionCompiler` for two-pass declare-then-define compilation
//! 4. `ArcIrEmitter` for ARC IR → LLVM IR lowering (with RC lifecycle)

mod compile;
mod runtime_mappings;

#[cfg(test)]
pub(crate) use runtime_mappings::jit_symbol_mappings;

use inkwell::context::Context;
use inkwell::execution_engine::ExecutionEngine;
use rustc_hash::FxHashMap;

use ori_ir::ast::Function;
use ori_ir::canon::CanonResult;
use ori_ir::Name;
use ori_types::{FunctionSig, Pool};

/// Check if LLVM IR dumping is requested (new or legacy env var).
///
/// Cannot use `oric::dbg_do!` here because `ori_llvm` doesn't depend on `oric`.
/// This is a raw env var check — the zero-overhead debug-only gating happens at
/// the `oric` call sites instead.
fn llvm_dump_requested() -> bool {
    std::env::var("ORI_DUMP_AFTER_LLVM").is_ok_and(|v| v != "0")
        || std::env::var("ORI_DEBUG_LLVM").is_ok_and(|v| v != "0")
}

/// A single imported function ready for LLVM compilation.
///
/// Pairs a function AST with its type-checked signature and the canonical IR
/// from its source module. All `Idx` values in `sig` and `canon` have been
/// re-interned into the main compilation pool before construction — the caller
/// uses [`ori_types::re_intern_type`] and [`CanArena::remap_types`] to ensure
/// single-pool consistency.
pub struct ImportedFunctionForCodegen<'a> {
    /// The function AST from the imported module.
    pub function: &'a Function,
    /// Type-checked signature, re-interned into the main compilation pool.
    pub sig: FunctionSig,
    /// Canonical IR with types remapped to the main compilation pool.
    pub canon: &'a CanonResult,
}

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
    /// The test wrapper function has a built-in catch-all `landingpad` that
    /// catches uncaught panics via LLVM's exception handling mechanism. This
    /// ensures `catch(expr:)` inside tests works correctly — panics propagate
    /// through `_Unwind_RaiseException`, caught by `invoke`/`landingpad` at
    /// both the `catch(expr:)` scope and the test wrapper boundary.
    ///
    /// After the wrapper returns, we check `did_panic()` to detect whether
    /// an exception was caught at the wrapper boundary (test failure) vs
    /// normal completion (test pass).
    ///
    /// # Safety
    ///
    /// The test function must exist in the compiled module and have signature `() -> void`.
    #[allow(
        unsafe_code,
        reason = "JIT execution requires unsafe FFI: get_function and call"
    )]
    pub fn run_test(&self, test_name: Name) -> LLVMEvalResult {
        // Snapshot live RC count before test for leak detection
        let live_before = runtime::ori_rc_live_count();

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

        // Reset panic state before running
        runtime::reset_panic_state();

        // Call the wrapper directly. The wrapper's catch-all landingpad handles
        // uncaught panics — no setjmp/longjmp needed.
        // SAFETY: test_fn has signature () -> void, compiled by us
        unsafe { test_fn.call() };

        // Check if the test panicked (caught by the wrapper's landingpad)
        if runtime::did_panic() {
            let msg = runtime::get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
            return Err(LLVMEvalError::new(msg));
        }

        // Check for ARC leaks: compare live count after test to snapshot before
        let live_after = runtime::ori_rc_live_count();
        let leaked = live_after - live_before;
        if leaked > 0 {
            return Err(LLVMEvalError::new(format!(
                "ARC leak: {leaked} allocation(s) not freed"
            )));
        }

        Ok(LLVMValue::Void)
    }
}

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
}
