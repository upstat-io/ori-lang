//! LLVM Backend for Ori
//!
//! This crate provides native code generation via LLVM, using the V2 codegen
//! architecture: `TypeInfoStore` → `IrBuilder` → `FunctionCompiler` → `ExprLowerer`.
//!
//! # Debug Environment Variables
//!
//! - `ORI_DEBUG_LLVM`: Print LLVM IR to stderr before JIT compilation.
//!   Useful for debugging codegen issues. Any non-empty value enables this.
//!   Example: `ORI_DEBUG_LLVM=1 cargo test`
//!
//! - `RUST_LOG=ori_llvm=debug`: Enable debug-level tracing output.
//!   Example: `RUST_LOG=ori_llvm=debug cargo test`
//!
//! # Key Types
//!
//! - [`SimpleCx`](context::SimpleCx): Minimal LLVM context (module + types)
//! - [`IrBuilder`](codegen::IrBuilder): ID-based LLVM instruction builder
//! - [`FunctionCompiler`](codegen::function_compiler::FunctionCompiler): Two-pass compilation
//! - [`ExprLowerer`](codegen::ExprLowerer): AST → LLVM IR lowering
//! - [`TypeInfoStore`](codegen::TypeInfoStore): Type information cache
//! - [`LLVMEvaluator`](evaluator::LLVMEvaluator): JIT evaluation

// Crate-level lint configuration for codegen-specific patterns
#![allow(
    // LLVM uses u32 for struct/array indices, we use usize in Rust
    clippy::cast_possible_truncation,
    // Ori uses i64 for integers, conversions to usize are intentional
    clippy::cast_sign_loss,
    // usize to i64 is safe on 64-bit (our target), acceptable wrap on 32-bit
    clippy::cast_possible_wrap,
    // Codegen functions thread through context, arena, types, locals, etc.
    clippy::too_many_arguments,
    // Internal functions - panics are invariant violations
    clippy::missing_panics_doc,
    // Most Result returns are for LLVM builder operations
    clippy::missing_errors_doc,
    // Compile functions return Option to propagate compilation failures
    clippy::unnecessary_wraps,
)]

// -- V2 codegen pipeline --
pub mod codegen;
pub mod context;

// -- Evaluator (JIT) --
pub mod evaluator;

// -- Runtime bindings --
pub mod runtime;

// -- AOT compilation --
pub mod aot;

// -- Re-exports --
pub use context::SimpleCx;
pub use inkwell;

#[cfg(test)]
mod tests;

use std::sync::Once;

static TRACING_INIT: Once = Once::new();
static FATAL_HANDLER_INIT: Once = Once::new();

/// Install a custom LLVM fatal error handler that logs instead of aborting.
///
/// By default, LLVM calls `abort()` on fatal errors (e.g., "unable to allocate
/// function return"), which kills the entire process. This replaces that handler
/// with one that logs the error. Note: the handler cannot prevent abort since
/// panicking across `extern "C"` boundaries is not allowed.
///
/// Safe to call multiple times — only the first call takes effect.
pub fn install_fatal_error_handler() {
    FATAL_HANDLER_INIT.call_once(|| {
        // SAFETY: `LLVMInstallFatalErrorHandler` is called once during
        // initialization with a valid function pointer.
        unsafe {
            llvm_sys::error_handling::LLVMInstallFatalErrorHandler(Some(llvm_fatal_error_handler));
        }
    });
}

/// LLVM fatal error callback that logs the error.
///
/// Cannot unwind (extern "C"), so we log and let LLVM abort.
extern "C" fn llvm_fatal_error_handler(reason: *const std::os::raw::c_char) {
    let msg = if reason.is_null() {
        "unknown LLVM fatal error".to_string()
    } else {
        // SAFETY: LLVM guarantees a valid C string pointer in the callback.
        unsafe { std::ffi::CStr::from_ptr(reason) }
            .to_string_lossy()
            .into_owned()
    };
    eprintln!("LLVM fatal error (aborting): {msg}");
}

/// Initialize tracing for debug output.
///
/// Call this once at startup. Safe to call multiple times.
/// Enable with `RUST_LOG=ori_llvm=debug` or `RUST_LOG=ori_llvm=trace`.
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};

        // Only initialize if RUST_LOG is set
        if std::env::var("RUST_LOG").is_ok() {
            let filter = EnvFilter::from_default_env();
            tracing_subscriber::registry()
                .with(fmt::layer().with_target(true).with_level(true))
                .with(filter)
                .init();
        }
    });
}
