//! Unified method registry for type-level method resolution.
//!
//! Built-in method resolution for the type checker is handled by
//! [`resolve_builtin_method()`](crate::infer::expr::methods) using
//! direct string matching, with the method manifest in
//! [`TYPECK_BUILTIN_METHODS`](crate::TYPECK_BUILTIN_METHODS).
//!
//! This registry is reserved for future use when method resolution
//! needs to integrate inherent impls and trait impls with builtin methods.

use ori_ir::Name;

use super::traits::TraitRegistry;

/// Unified method registry.
///
/// Currently a thin wrapper around trait-based method lookup.
/// Built-in method resolution happens in `resolve_builtin_method()`.
#[derive(Clone, Debug, Default)]
pub struct MethodRegistry;

impl MethodRegistry {
    /// Create a new method registry.
    pub fn new() -> Self {
        Self
    }

    /// Look up a method from trait implementations.
    ///
    /// Delegates to `TraitRegistry::lookup_method()`. Built-in method
    /// resolution is not handled here — it's done by `resolve_builtin_method()`
    /// in `ori_types::infer::expr::methods`.
    pub fn lookup_trait_method<'a>(
        &self,
        receiver_ty: crate::Idx,
        method_name: Name,
        trait_registry: &'a TraitRegistry,
    ) -> Option<super::traits::MethodLookup<'a>> {
        trait_registry.lookup_method(receiver_ty, method_name)
    }
}
