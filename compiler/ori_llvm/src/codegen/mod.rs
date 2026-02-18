//! V2 Codegen Module
//!
//! Modular, type-driven code generation following the LLVM V2 architecture.
//!
//! # Design
//!
//! The V2 codegen architecture centralizes type-specific logic behind `TypeInfo`,
//! an enum with one variant per Ori type category. This replaces the scattered
//! type matching in the current `compute_llvm_type` with exhaustive enum dispatch.
//!
//! Key types:
//! - [`TypeInfo`] — LLVM-specific type information (representation, layout, ABI)
//! - [`TypeInfoStore`] — Lazily-populated `Idx → TypeInfo` cache backed by Pool
//! - [`IrBuilder`] — ID-based instruction builder wrapping inkwell
//! - [`Scope`] — Persistent-map variable scoping with O(1) clone
//! - [`ExprLowerer`] — Expression lowering coordinator (Section 03)
//! - [`ValueId`], [`BlockId`], [`FunctionId`], [`LLVMTypeId`] — Opaque IR handles
//!
//! # Module Organization
//!
//! ```text
//! codegen/
//! ├── ir_builder/          — ID-based LLVM instruction builder (Section 02)
//! ├── scope.rs            — Persistent-map variable scoping
//! ├── type_info.rs        — TypeInfo enum + TypeInfoStore (Section 01)
//! ├── value_id.rs         — Opaque ID newtypes + ValueArena
//! ├── expr_lowerer.rs     — ExprLowerer struct + dispatch (Section 03)
//! ├── lower_literals.rs   — Literals, identifiers, constants
//! ├── lower_operators.rs  — Binary/unary ops, cast, short-circuit
//! ├── lower_control_flow.rs — If, loop, for, block, break, continue, match
//! ├── lower_error_handling.rs — Ok, Err, Some, None, Try
//! ├── lower_collections.rs — List, map, tuple, struct, range, field, index
//! ├── lower_calls.rs      — Call, MethodCall, Lambda
//! └── lower_constructs.rs — FunctionSeq, FunctionExp, SelfRef, Await
//! ```
//!
//! # Architecture Note
//!
//! ARC classification is NOT in this module — it lives in `ori_arc::ArcClassification`
//! (no LLVM dependency). This module is purely about LLVM code generation.

// -- Core infrastructure (Sections 01–02) --
pub mod ir_builder;
pub mod scope;
pub mod type_info;
pub mod value_id;

// -- Function compilation (Section 04) --
pub mod abi;
pub mod derive_codegen;
pub mod function_compiler;
pub mod runtime_decl;
pub mod type_registration;

// -- ARC IR emission (Tier 2 — Section 07.2) --
pub mod arc_emitter;

// -- Expression lowering (Section 03) --
pub mod expr_lowerer;
mod lower_builtin_methods;
mod lower_calls;
mod lower_collection_methods;
mod lower_collections;
mod lower_constructs;
mod lower_control_flow;
mod lower_error_handling;
mod lower_literals;
mod lower_operators;

// -- Public re-exports --
pub use expr_lowerer::ExprLowerer;
pub use ir_builder::IrBuilder;
pub use scope::{Scope, ScopeBinding};
pub use type_info::{EnumVariantInfo, TypeInfo, TypeInfoStore, TypeLayoutResolver};
pub use value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};
