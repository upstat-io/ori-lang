//! LLVM Backend for Ori
//!
//! This crate provides native code generation via LLVM, using the V2 codegen
//! architecture: `TypeInfoStore` → `IrBuilder` → `FunctionCompiler` → `ArcIrEmitter`.
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
//! - [`TypeInfoStore`](codegen::TypeInfoStore): Type information cache
//! - [`LLVMEvaluator`](evaluator::LLVMEvaluator): JIT evaluation

#![warn(clippy::allow_attributes_without_reason)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "LLVM uses u32 indices and Ori uses i64 — casts between usize/u32/i64 are intentional"
)]
#![allow(
    clippy::too_many_arguments,
    reason = "codegen functions thread through context, arena, types, locals, etc."
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::unnecessary_wraps,
    reason = "internal codegen functions — panics are invariant violations, Results wrap LLVM ops"
)]
// Match workspace-level allows (ori_llvm is excluded from workspace)
#![allow(
    clippy::similar_names,
    reason = "workspace equivalent — codegen uses similar names intentionally"
)]
#![allow(
    clippy::cognitive_complexity,
    reason = "workspace equivalent — codegen match arms are complex"
)]
#![allow(clippy::module_name_repetitions, reason = "workspace equivalent")]
#![allow(clippy::must_use_candidate, reason = "workspace equivalent")]

// -- V2 codegen pipeline --
pub mod codegen;
pub mod context;

// -- Monomorphization --
pub mod monomorphize;

// -- Evaluator (JIT) --
pub mod evaluator;

// -- Runtime bindings --
pub mod runtime;

// -- AOT compilation --
pub mod aot;

// -- Verification --
pub mod verify;

// -- Initialization --
mod init;

// -- Re-exports --
pub use context::SimpleCx;
pub use init::{init_tracing, install_fatal_error_handler};
pub use inkwell;

/// Collect unconstrained function identities (pub or trait impl).
///
/// Returns `(Option<Idx>, Name)` pairs: `None` for pub top-level functions,
/// `Some(self_type)` for trait impl methods (TPR-03-042 disambiguation).
pub fn collect_unconstrained_fn_names(
    function_sigs: &[ori_types::FunctionSig],
    trait_impl_fn_names: &[(ori_types::Idx, ori_ir::Name)],
    interner: Option<&ori_ir::StringInterner>,
) -> Vec<(Option<ori_types::Idx>, ori_ir::Name)> {
    let mut names = Vec::new();
    for sig in function_sigs {
        if sig.is_public {
            names.push((None, sig.name));
        }
    }
    for &(self_type, name) in trait_impl_fn_names {
        names.push((Some(self_type), name));
        // Also register the qualified name used by analysis-only ARC functions
        // (TPR-03-043). Format matches codegen_pipeline's ARC lowering.
        if let Some(interner) = interner {
            let method_str = interner.lookup(name);
            let qualified = interner.intern(&format!("__impl_{}_{method_str}", self_type.raw()));
            names.push((None, qualified));
        }
    }
    names
}

#[cfg(test)]
mod tests;
