//! Registration passes for module type checking.
//!
//! These passes run before signature collection to populate the registries
//! with type definitions, traits, and implementations.
//!
//! # Pass Order
//!
//! - **Pass 0a**: Built-in types (Ordering, `TraceEntry`, format types)
//! - **Pass 0b**: User-defined types (struct, enum, newtype)
//! - **Pass 0c**: Traits and implementations
//! - **Pass 0d**: Derived implementations (`#[derive(...)]`)
//! - **Pass 0e**: Constants
//!
//! # Cross-Reference
//!
//! - Trait features: `plans/roadmap/section-03-traits.md`
//! - Module checker design: `plans/types_v2/section-08b-module-checker.md`

mod builtin_types;
mod consts;
mod derived;
mod impls;
mod traits;
mod type_resolution;
mod user_types;

// Re-export public entry points for api/mod.rs
pub use builtin_types::register_builtin_types;
pub use consts::register_consts;
pub use derived::register_derived_impls;
pub use impls::register_impls;
pub use traits::register_traits;
pub use user_types::register_user_types;

// Re-export for check/mod.rs (foreign module trait registration)
pub(super) use traits::register_imported_traits;

// Re-export shared type resolution for bodies/mod.rs and signatures/tests.rs
pub(super) use type_resolution::{resolve_parsed_type_simple, resolve_type_with_self};

// Re-exports for tests — internal functions accessed by registration/tests.rs
#[cfg(test)]
use derived::{build_derived_methods, register_derived_impl};
#[cfg(test)]
use traits::compute_object_safety_violations;
#[cfg(test)]
use type_resolution::{parsed_type_contains_self, resolve_type_with_params};

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[expect(clippy::expect_used, reason = "Tests use expect for clarity")]
mod tests;
