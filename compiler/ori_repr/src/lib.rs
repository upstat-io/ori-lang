//! Representation optimization IR for the Ori compiler.
//!
//! This crate currently provides the compiled-shaped `MachineRepr` and
//! `ReprPlan` carriers used by LLVM, while also owning the migration seam for
//! backend-neutral executable and representation evidence. The type checker
//! never sees machine representations, and a physical emitter never invents
//! narrowing decisions. Historical influence: the Lean 4 LCNF
//! phase-separation shape.
//!
//! # Architecture
//!
//! ```text
//! ori_arc + ori_types → ori_repr::ExecutableProgram + ReprEvidence
//!                                  ├─ VmLayoutPlan → ori_vm
//!                                  └─ CompiledLayoutPlan(TargetSpec)
//!                                             ├─ ori_llvm
//!                                             ├─ ori_backend native
//!                                             └─ direct compiled WebAssembly
//! ```
//!
//! `ori_repr` reads from `ori_types` and `ori_arc` but neither depends on it.
//!
//! # Salsa Integration
//!
//! [`compute_repr_plan()`] is **not** a Salsa query. It is a pure function
//! that runs imperatively after type checking and ARC borrow inference:
//!
//! The following are shipped LLVM migration paths, not the production crate
//! boundary:
//!
//! - **LLVM AOT path** (`codegen_pipeline.rs`): called once, result passed as
//!   `&ReprPlan` to `TypeLayoutResolver` and then to codegen.
//! - **LLVM JIT path** (`evaluator/compile.rs`): called per compilation unit,
//!   same ownership model.
//!
//! The `ReprPlan` is recomputed on every compilation. It has no interior
//! mutability (`Send + Sync` by construction), unlike `TypeInfoStore`
//! which uses `RefCell` for lazy population.

#![deny(unsafe_code)]

mod backend;
mod canonical;
mod enum_repr;
pub mod escape;
pub mod executable;
mod layout;
pub mod monomorphize;
pub mod narrowing;
mod pipeline;
mod plan;
mod primitive;
pub mod range;
mod repr;
mod struct_repr;
mod unconstrained_fns;

#[cfg(test)]
mod tests;

pub use backend::{BackendError, CodegenBackend, RealizedProgram};
pub use canonical::canonical_enum_for_type;
pub use enum_repr::{min_tag_width, EnumRepr, EnumTag, VariantRepr};
pub use layout::{compute_enum_layout_info, slot_count, slot_padded_size, EnumLayoutInfo};
pub use narrowing::abi::{
    AbiBoundary, CrossModuleAgreement, FunctionBoundaryInfo, WidthRequirement,
};
pub use narrowing::overflow::OverflowStrategy;
pub use pipeline::{compute_repr_plan, compute_repr_plan_with_interner};
pub use plan::{
    CompiledAllocationDecision, CompiledAllocationMechanism, DecisionReason, DecisionSource,
    NarrowingPolicy, RcStrategy, ReprAttribute, ReprDecision, ReprPlan, MAX_LOCAL_YIELD_BYTES,
};
pub use primitive::{
    binary_primitive_strategy, unary_primitive_strategy, BuiltinType, PrimitiveStrategy,
};
pub use range::{
    FieldSummaryTable, KnownBuiltins, RangeAnalysisConfig, RangeFixpointResult, ValueRange,
};
pub use repr::{FloatWidth, IntWidth, MachineRepr};
pub use struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
pub use unconstrained_fns::collect_unconstrained_fn_names;
