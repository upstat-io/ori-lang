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
//! - Trait features:
//! - Module checker design: `ori_types/src/check/mod.rs`

mod builtin_types;
pub(crate) mod burden_compute;
mod consts;
mod derived;
mod extern_types;
mod impls;
mod traits;
mod type_resolution;
mod user_types;

// Re-export public entry points for api/mod.rs
pub use builtin_types::register_builtin_types;
pub use consts::register_consts;
pub use derived::register_derived_impls;
pub use extern_types::register_extern_burdens;
pub(super) use impls::register_imported_impls;
pub use impls::{register_builtin_extensions, register_impls};
pub use traits::{register_object_safety_violations, register_traits};
pub use user_types::register_user_types;

// Re-export for check/mod.rs (foreign module trait registration)
pub(super) use traits::register_imported_traits;

// Re-export shared type resolution for bodies/mod.rs and signatures/tests.rs
pub(super) use type_resolution::resolve_type_with_method_generics;
// Test-only re-export: production callers resolve impl/method self-types through
// `resolve_type_with_method_generics` (overlay-aware); only registration/
// signature unit tests exercise the bare simple resolver directly.
#[cfg(test)]
pub(super) use type_resolution::resolve_parsed_type_simple;

// Re-exports for tests — internal functions accessed by registration/tests.rs
#[cfg(test)]
use derived::{build_derived_methods, register_derived_impl};
#[cfg(test)]
use traits::compute_object_safety_violations;
#[cfg(test)]
use type_resolution::{
    parsed_type_contains_self, resolve_type_with_params, resolve_type_with_self,
};

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[expect(clippy::expect_used, reason = "Tests use expect for clarity")]
mod tests;
