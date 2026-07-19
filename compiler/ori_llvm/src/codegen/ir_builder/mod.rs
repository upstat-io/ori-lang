//! ID-based LLVM instruction builder for V2 codegen.
//!
//! `IrBuilder` wraps inkwell's builder, stores LLVM values in a `ValueArena`,
//! and exposes opaque IDs to the codegen pipeline. Operation families live in
//! focused leaf modules; this module only declares and exports them.

pub(crate) mod cfg_simplify;

mod aggregates;
mod arithmetic;
mod attributes;
mod calls;
mod checked_ops;
mod comparisons;
mod constants;
mod control_flow;
mod conversions;
mod invoke;
mod memory;
mod phi_types_blocks;
pub(crate) mod seh;
mod state;
mod type_introspection;

pub use state::IrBuilder;
pub(crate) use state::{CompilationMode, IntegerSignedness};

#[cfg(test)]
#[expect(
    clippy::approx_constant,
    clippy::doc_markdown,
    reason = "numeric builder tests intentionally use recognizable approximations and prose identifiers"
)]
mod tests;
