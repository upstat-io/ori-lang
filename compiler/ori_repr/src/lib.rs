//! Representation optimization IR for the Ori compiler.
//!
//! This crate provides the `MachineRepr` type and `ReprPlan` data structure
//! that records all narrowing decisions between type checking and codegen.
//! The type checker never sees machine representations; codegen never makes
//! narrowing decisions. This separation mirrors Lean 4's LCNF phase.
//!
//! # Architecture
//!
//! ```text
//! ori_types (Pool, Tag, Idx) → ori_arc (ArcFunction) → ori_repr (ReprPlan) → ori_llvm
//! ```
//!
//! `ori_repr` reads from `ori_types` and `ori_arc` but neither depends on it.

#![deny(unsafe_code)]

mod canonical;
mod enum_repr;
pub mod escape;
pub mod range;
mod repr;
mod struct_repr;

#[cfg(test)]
mod tests;

pub use canonical::canonical;
pub use enum_repr::{EnumRepr, EnumTag, VariantRepr};
pub use repr::{FloatWidth, IntWidth, MachineRepr};
pub use struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
