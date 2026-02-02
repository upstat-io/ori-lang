//! Type system for Ori.
//!
//! Per design spec 02-design-principlesmd:
//! - All types have Clone, Eq, Hash for Salsa compatibility
//! - Interned type representations for efficiency
//! - Flat structures for cache locality
//!
//! # Type Interning
//!
//! This crate provides two type representations:
//! - `Type`: The traditional boxed representation for compatibility
//! - `TypeData`/`TypeId`: The interned representation for O(1) equality
//!
//! Use `TypeInterner` to intern types and get `TypeId` handles.

mod context;
mod core;
mod data;
mod env;
mod error;
mod traverse;
mod type_interner;

// Re-export all public types
pub use context::{InferenceContext, TypeContext};
pub use core::{Type, TypeScheme, TypeSchemeId};
pub use env::TypeEnv;
pub use error::TypeError;
pub use traverse::{TypeFolder, TypeIdFolder, TypeIdVisitor, TypeVisitor};

// Type interning exports
pub use data::{TypeData, TypeVar};
pub use type_interner::{SharedTypeInterner, TypeInternError, TypeInterner};

// Size assertions to prevent accidental regressions.
// Type is used throughout type checking and stored in query results.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{Type, TypeVar};
    // Type enum: largest variant is Applied with Name (8) + Vec<Type> (24) = 32 bytes + discriminant = 40 bytes
    // Applied variant has: name: Name (u64 = 8) + args: Vec<Type> (24) = 32 bytes
    ori_ir::static_assert_size!(Type, 40);
    // TypeVar is just a u32 wrapper
    ori_ir::static_assert_size!(TypeVar, 4);
}

// Tests extracted to: compiler/oric/tests/phases/typeck/types.rs
