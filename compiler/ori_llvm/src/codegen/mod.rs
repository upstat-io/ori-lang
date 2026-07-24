//! Type-driven LLVM code generation.
//!
//! # Design
//!
//! [`TypeInfo`] centralizes type-specific representation, layout, and ABI logic
//! through exhaustive dispatch over Ori type categories.
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
//! # Function-Level IR Verification Invariant
//!
//! **Every code path that creates and finalizes a `FunctionValue`** (i.e., adds basic
//! blocks and a terminator) **MUST call `fn_val.verify(true)`** before the function is
//! considered complete, gated on the `ORI_VERIFY_ARC` flag.
//!
//! Canonical emission helpers (`emit_arc_function`, `emit_prepared_functions`,
//! `emit_prepared_lambda`) include verification internally — callers inherit it.
//! Separate emission paths (derive codegen, closure wrappers, drop gen, thunks,
//! entry point, test wrappers, panic trampolines) have their own explicit
//! `fn_val.verify()` calls.
//!
//! When adding a new function emission site, add verification at body completion.
//! See `derive_codegen::verify_derive_function` for the helper pattern.
//!
//! # Architecture Note
//!
//! [`ori_arc::ArcClassification`] owns backend-neutral classification. This
//! module consumes that classification for LLVM code generation.

// Core infrastructure
pub mod eh_model;
pub mod ir_builder;
pub mod type_info;
pub mod value_id;

// Function compilation
pub mod abi;
pub mod derive_codegen;
pub mod function_compiler;
pub mod runtime_decl;
pub mod type_registration;

// ARC IR emission
pub mod arc_emitter;

// Shared mono-dispatch discriminators (consumed by prepare + apply)

// Public re-exports
pub use ir_builder::IrBuilder;
pub use type_info::{EnumVariantInfo, TypeInfo, TypeInfoStore, TypeLayoutResolver};
pub use value_id::{BlockId, FunctionId, LLVMTypeId, TokenId, ValueId};
