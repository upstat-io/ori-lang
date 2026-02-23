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
//! - [`ArcIrEmitter`](arc_emitter::ArcIrEmitter) — ARC IR → LLVM IR emission
//! - [`ValueId`], [`BlockId`], [`FunctionId`], [`LLVMTypeId`] — Opaque IR handles
//!
//! # Module Organization
//!
//! ```text
//! codegen/
//! ├── ir_builder/          — ID-based LLVM instruction builder
//! ├── type_info.rs         — TypeInfo enum + TypeInfoStore
//! ├── value_id.rs          — Opaque ID newtypes + ValueArena
//! ├── abi/                 — ABI computation (param/return passing)
//! ├── function_compiler/   — Two-pass declare/define orchestrator
//! ├── arc_emitter/         — ARC IR → LLVM IR emission
//! ├── derive_codegen/      — Derived trait IR generation
//! ├── runtime_decl/        — Runtime function declarations
//! └── type_registration/   — Type registration for LLVM
//! ```
//!
//! # Architecture Note
//!
//! ARC classification is NOT in this module — it lives in `ori_arc::ArcClassification`
//! (no LLVM dependency). This module is purely about LLVM code generation.

// -- Core infrastructure --
pub mod ir_builder;
pub mod type_info;
pub mod value_id;

// -- Function compilation --
pub mod abi;
pub mod derive_codegen;
pub mod function_compiler;
pub mod runtime_decl;
pub mod type_registration;

// -- ARC IR emission --
pub mod arc_emitter;

// -- Public re-exports --
pub use ir_builder::IrBuilder;
pub use type_info::{EnumVariantInfo, TypeInfo, TypeInfoStore, TypeLayoutResolver};
pub use value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};
