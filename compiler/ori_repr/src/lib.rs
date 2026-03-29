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
//!
//! # Salsa Integration (§01.6)
//!
//! [`compute_repr_plan()`] is **not** a Salsa query. It is a pure function
//! that runs imperatively after type checking and ARC borrow inference:
//!
//! - **AOT path** (`codegen_pipeline.rs`): called once, result passed as
//!   `&ReprPlan` to `TypeLayoutResolver` and then to codegen.
//! - **JIT path** (`evaluator/compile.rs`): called per compilation unit,
//!   same ownership model.
//!
//! The `ReprPlan` is recomputed on every compilation. It has no interior
//! mutability (`Send + Sync` by construction), unlike `TypeInfoStore`
//! which uses `RefCell` for lazy population.

#![deny(unsafe_code)]

mod canonical;
mod enum_repr;
pub mod escape;
mod layout;
pub mod narrowing;
mod pipeline;
mod plan;
pub mod range;
mod repr;
mod struct_repr;

#[cfg(test)]
mod tests;

pub use enum_repr::{EnumRepr, EnumTag, VariantRepr};
pub use narrowing::abi::{
    AbiBoundary, CrossModuleAgreement, FunctionBoundaryInfo, WidthRequirement,
};
pub use narrowing::overflow::OverflowStrategy;
pub use pipeline::{compute_repr_plan, compute_repr_plan_with_interner};
pub use plan::{
    DecisionReason, DecisionSource, NarrowingPolicy, RcStrategy, ReprAttribute, ReprDecision,
    ReprPlan,
};
pub use range::{
    FieldSummaryTable, KnownBuiltins, RangeAnalysisConfig, RangeFixpointResult, ValueRange,
};
pub use repr::{FloatWidth, IntWidth, MachineRepr};
pub use struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
