//! Sugar-free, type-annotated IR shared by evaluation and ARC lowering.
//!
//! # Representation contract
//!
//! [`CanExpr`] is distinct from source `ExprKind`, so named arguments,
//! templates, and spread syntax cannot survive canonicalization. Calls are
//! positional, constants are folded, matches reference compiled decision
//! trees, and canonical nodes use their own index space.

mod arena;
pub mod consumers;
mod expr;
pub mod hash;
mod ids;
mod patterns;
mod pools;
pub mod prelude;
mod result;
mod support;
pub mod tree;

pub use prelude::*;

#[cfg(test)]
mod tests;
