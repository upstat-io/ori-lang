//! LLVM Backend for Ori
//!
//! This crate provides native code generation via LLVM, following patterns
//! from Rust's `rustc_codegen_llvm`.
//!
//! # Clippy Configuration
//!
//! This crate intentionally allows certain clippy lints that are common in
//! low-level codegen code:
//! - Cast warnings: LLVM APIs use specific integer widths (u32 for indices, i64 for values)
//! - Too many arguments: Codegen functions naturally thread through many context values
//! - Missing panic docs: Internal panics are invariant violations, not API concerns

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
//!
//! # Architecture
//!
//! The crate is organized following Rust's two-tier codegen architecture:
//!
//! - **Context hierarchy** (`context.rs`): `SimpleCx` → `CodegenCx`
//! - **Builder** (`builder.rs`): Instruction generation separated from context
//! - **Traits** (`traits.rs`): Backend abstraction for future extensibility
//! - **Declare** (`declare.rs`): Two-phase codegen (predefine/define)
//!
//! # Key Types
//!
//! - [`CodegenCx`](context::CodegenCx): Full codegen context with caches
//! - [`Builder`](builder::Builder): LLVM instruction builder + expression compilation
//! - [`ModuleCompiler`](module::ModuleCompiler): High-level module compilation
//!
//! # Debugging
//!
//! Enable tracing with environment variables:
//! - `RUST_LOG=ori_llvm=debug` - Debug level tracing
//! - `RUST_LOG=ori_llvm=trace` - Trace level (very verbose)
//! - `RUST_LOG=ori_llvm::functions=trace` - Trace specific module
//!
//! # Example
//!
//!     use ori_llvm::{CodegenCx, Builder};
//!     use inkwell::context::Context;
//!
//!     let context = Context::create();
//!     let interner = StringInterner::new();
//!     let cx = CodegenCx::new(&context, &interner, "my_module");
//!
//!     // Declare runtime functions
//!     cx.declare_runtime_functions();
//!
//!     // Declare a function
//!     let func = cx.declare_fn(name, &[TypeId::INT], TypeId::INT);
//!     let entry = cx.llcx().append_basic_block(func, "entry");
//!
//!     // Build instructions and compile expressions
//!     let bx = Builder::build(&cx, entry);
//!     let result = bx.compile_expr(body, arena, expr_types, &mut locals, func, None);
//!     bx.ret(result.unwrap());

// -- Public modules (new architecture) --
pub mod builder;
pub mod context;
pub mod declare;
pub mod traits;

// -- Existing public modules --
pub mod evaluator;
pub mod module;
pub mod runtime;

// Re-export key types from new architecture
pub use builder::Builder;
pub use context::{CodegenCx, SimpleCx, TypeCache};
pub use traits::{BackendTypes, BuilderMethods, CodegenMethods, TypeMethods};

// Re-export from existing modules
pub use evaluator::FunctionSig;

// -- Private codegen modules (expression compilation on Builder) --
mod collections;
mod control_flow;
mod functions;
mod matching;
mod operators;
mod types;

#[cfg(test)]
mod tests;

use inkwell::values::PhiValue;
use std::sync::Once;

static TRACING_INIT: Once = Once::new();

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

/// Loop context for break/continue.
#[derive(Clone)]
pub struct LoopContext<'ctx> {
    /// Block to jump to on continue.
    pub header: inkwell::basic_block::BasicBlock<'ctx>,
    /// Block to jump to on break.
    pub exit: inkwell::basic_block::BasicBlock<'ctx>,
    /// Phi node for break values (if any). TODO: use for break-with-value.
    pub break_phi: Option<PhiValue<'ctx>>,
}
