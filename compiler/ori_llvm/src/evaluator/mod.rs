//! LLVM-based evaluator for running Ori code.
//!
//! This provides a JIT-based evaluator that compiles Ori code to LLVM IR
//! and executes it natively, as an alternative to the tree-walking interpreter.
//!
//! # V2 Architecture
//!
//! The evaluator is a pure physical projection component. It receives one
//! closed executable artifact from `oric` and does not lower canonical IR or
//! run ownership analysis.
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
use ori_types::FunctionSig;

/// Env: `ORI_DUMP_AFTER_LLVM` — dumps LLVM IR after code generation, debug-only.
/// Env: `ORI_DEBUG_LLVM` — also requests an LLVM IR dump, debug-only.
///
/// Cannot use `oric::dbg_do!` here because `ori_llvm` doesn't depend on `oric`.
fn llvm_dump_requested() -> bool {
    std::env::var("ORI_DUMP_AFTER_LLVM").is_ok_and(|v| v != "0")
        || std::env::var("ORI_DEBUG_LLVM").is_ok_and(|v| v != "0")
}

/// A single imported function ready for LLVM compilation.
///
/// Pairs a function AST with its type-checked signature and the canonical IR
/// from its source module. All `Idx` values in `sig` and `canon` have been
/// re-interned into the main compilation pool before construction — the caller
/// uses [`ori_types::re_intern_type`] and [`ori_ir::canon::CanArena::remap_types`] to ensure
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
/// dropped after engine creation because it becomes a non-owning shell.
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
    /// On Linux/macOS the test wrapper has a built-in catch-all `landingpad`
    /// that catches uncaught panics via LLVM Itanium EH. After the wrapper
    /// returns, `did_panic()` detects whether an exception was caught at
    /// the wrapper boundary (test failure) vs normal completion (test pass).
    ///
    /// On Windows, LLVM's JIT cannot compile Itanium-style `landingpad` for
    /// MSVC targets, so the wrapper is a plain function and uses
    /// `jit_run_protected` (C++ try/catch) for panic recovery.
    ///
    /// # Safety
    ///
    /// The test function must exist in the compiled module and have signature `() -> void`.
    #[expect(
        unsafe_code,
        reason = "LLVM JIT symbol lookup and invocation cross the FFI boundary after validating the generated wrapper signature"
    )]
    pub fn run_test(&self, test_name: Name) -> LLVMEvalResult {
        let live_before = runtime::ori_rc_live_count();

        let wrapper_name = self.test_wrappers.get(&test_name).ok_or_else(|| {
            LLVMEvalError::new(format!("Test wrapper not found for test: {test_name:?}"))
        })?;

        // SAFETY: Generated test wrappers have the validated `() -> void` signature.
        let test_fn = unsafe {
            self.engine
                .get_function::<unsafe extern "C" fn()>(wrapper_name)
                .map_err(|e| LLVMEvalError::new(format!("Test function not found: {e}")))?
        };

        #[cfg(target_os = "windows")]
        {
            let raw_fn: unsafe extern "C" fn() = unsafe { test_fn.as_raw() };
            if let Err(msg) = unsafe { runtime::jit_run_protected(raw_fn) } {
                runtime::ori_report_uncaught_panic();
                return Err(LLVMEvalError::new(msg));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            runtime::reset_panic_state();
            // SAFETY: Generated test wrappers have the validated `() -> void` signature.
            unsafe { test_fn.call() };

            if runtime::did_panic() {
                let msg =
                    runtime::get_panic_message().unwrap_or_else(|| "unknown panic".to_string());
                return Err(LLVMEvalError::new(msg));
            }
        }

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
pub struct OwnedLLVMEvaluator {
    context: Context,
}

impl OwnedLLVMEvaluator {
    /// Create an evaluator. The bound executable artifact owns all semantic
    /// type identities used during physical projection.
    #[must_use]
    pub fn new() -> Self {
        OwnedLLVMEvaluator {
            context: Context::create(),
        }
    }
}

impl Default for OwnedLLVMEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::llvm_dump_requested;

    #[test]
    fn llvm_dump_environment_variables_enable_dumping() {
        const CHILD: &str = "LLVM_DUMP_ENV_TEST_CHILD";
        if let Some(variable) = std::env::var_os(CHILD) {
            assert!(
                llvm_dump_requested(),
                "{} must request an LLVM dump",
                variable.to_string_lossy()
            );
            return;
        }

        for variable in ["ORI_DUMP_AFTER_LLVM", "ORI_DEBUG_LLVM"] {
            let output = std::process::Command::new(
                std::env::current_exe().expect("test executable path must be available"),
            )
            .arg("llvm_dump_environment_variables_enable_dumping")
            .arg("--nocapture")
            .env(CHILD, variable)
            .env(variable, "1")
            .output()
            .expect("LLVM dump behavior child must start");
            assert!(
                output.status.success(),
                "LLVM dump behavior child for {variable} failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }
}
