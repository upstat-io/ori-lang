//! Method resolution chain for extensible method dispatch.
//!
//! This module implements a Chain of Responsibility pattern for method resolution,
//! allowing extensible method dispatch without modifying the core dispatch logic.
//!
//! # Resolution Order
//!
//! Methods are resolved in the following priority order:
//! 1. User-defined and derived methods via `UserRegistryResolver` (priority 0)
//!    - User-defined methods from impl blocks (checked first)
//!    - Derived methods from `#[derive(...)]` (checked second)
//! 2. Collection methods requiring evaluator (priority 1)
//! 3. Built-in methods in `MethodRegistry` (priority 2)
//!
//! # Architecture
//!
//! Each resolver implements the `MethodResolver` trait and handles a specific
//! category of methods. The `MethodDispatcher` chains these resolvers and
//! tries them in priority order until one handles the method call.

mod builtin;
mod collection;
mod user_registry;

pub use builtin::BuiltinMethodResolver;
pub use collection::CollectionMethodResolver;
pub use user_registry::UserRegistryResolver;

use crate::{DerivedMethodInfo, UserMethod, Value};
use ori_ir::Name;

/// Result of method resolution - identifies what kind of method was found.
#[derive(Clone, Debug)]
pub enum MethodResolution {
    /// User-defined method from an impl block.
    User(UserMethod),
    /// Derived method from `#[derive(...)]` attribute.
    Derived(DerivedMethodInfo),
    /// Collection method that needs evaluator access (map, filter, fold, etc.).
    Collection(CollectionMethod),
    /// Built-in method handled by `MethodRegistry`.
    Builtin,
    /// Method not found by this resolver.
    NotFound,
}

/// Collection method types that require evaluator access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionMethod {
    /// [T].map(transform: T -> U) -> [U]
    Map,
    /// [T].filter(predicate: T -> bool) -> [T]
    Filter,
    /// [T].fold(initial: U, op: (U, T) -> U) -> U
    Fold,
    /// [T].find(predicate: T -> bool) -> Option<T>
    Find,
    /// Range<T>.`collect()` -> [T]
    Collect,
    /// {K: V}.map(transform: (K, V) -> (K2, V2)) -> {K2: V2}
    MapEntries,
    /// {K: V}.filter(predicate: (K, V) -> bool) -> {K: V}
    FilterEntries,
    /// [T].any(predicate: T -> bool) -> bool
    Any,
    /// [T].all(predicate: T -> bool) -> bool
    All,
}

impl CollectionMethod {
    /// Try to identify a collection method by name.
    ///
    /// Returns Some(method) if the name matches a known collection method.
    /// Currently used primarily for testing; the `CollectionMethodResolver`
    /// performs matching directly.
    #[cfg(test)]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "map" => Some(Self::Map),
            "filter" => Some(Self::Filter),
            "fold" => Some(Self::Fold),
            "find" => Some(Self::Find),
            "collect" => Some(Self::Collect),
            "any" => Some(Self::Any),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

/// Trait for method resolvers in the chain of responsibility.
///
/// Each resolver handles a specific category of methods and returns
/// a `MethodResolution` indicating what was found.
///
/// Uses interned `Name` values for efficient lookup without allocation.
pub trait MethodResolver {
    /// Try to resolve a method call.
    ///
    /// # Arguments
    /// * `receiver` - The value the method is called on
    /// * `type_name` - The concrete type name of the receiver (interned)
    /// * `method_name` - The name of the method being called (interned)
    ///
    /// # Returns
    /// * `MethodResolution::NotFound` if this resolver doesn't handle this method
    /// * Other `MethodResolution` variant if the method was found
    fn resolve(&self, receiver: &Value, type_name: Name, method_name: Name) -> MethodResolution;

    /// Get the priority of this resolver (lower = higher priority).
    fn priority(&self) -> u8;

    /// Get a human-readable name for this resolver (for debugging).
    /// Used in tests and for tracing/logging when debugging method resolution.
    fn name(&self) -> &'static str;
}

/// Enum-based resolver kind for the fixed set of method resolvers.
///
/// Uses an enum instead of `Box<dyn MethodResolver>` because:
/// - Fixed set of 3 resolver types (no user extension needed)
/// - Eliminates virtual dispatch overhead
/// - Enables exhaustive matching at compile time
/// - Better cache locality (no heap indirection)
pub enum MethodResolverKind {
    /// User-defined and derived methods (priority 0)
    UserRegistry(UserRegistryResolver),
    /// Collection methods requiring evaluator (priority 1)
    Collection(CollectionMethodResolver),
    /// Built-in methods on primitives (priority 2)
    Builtin(BuiltinMethodResolver),
}

impl MethodResolverKind {
    /// Try to resolve a method call.
    pub fn resolve(
        &self,
        receiver: &Value,
        type_name: Name,
        method_name: Name,
    ) -> MethodResolution {
        match self {
            Self::UserRegistry(r) => r.resolve(receiver, type_name, method_name),
            Self::Collection(r) => r.resolve(receiver, type_name, method_name),
            Self::Builtin(r) => r.resolve(receiver, type_name, method_name),
        }
    }

    /// Get the priority of this resolver (lower = higher priority).
    pub fn priority(&self) -> u8 {
        match self {
            Self::UserRegistry(r) => r.priority(),
            Self::Collection(r) => r.priority(),
            Self::Builtin(r) => r.priority(),
        }
    }

    /// Get a human-readable name for this resolver (for debugging).
    pub fn name(&self) -> &'static str {
        match self {
            Self::UserRegistry(r) => r.name(),
            Self::Collection(r) => r.name(),
            Self::Builtin(r) => r.name(),
        }
    }
}

/// Method dispatcher that chains multiple resolvers.
///
/// Tries resolvers in priority order (lowest priority number first)
/// until one returns a match or all have been tried.
pub struct MethodDispatcher {
    resolvers: Vec<MethodResolverKind>,
}

impl MethodDispatcher {
    /// Create a new dispatcher with the given resolvers.
    ///
    /// Resolvers are sorted by priority (lowest first).
    pub fn new(mut resolvers: Vec<MethodResolverKind>) -> Self {
        resolvers.sort_by_key(MethodResolverKind::priority);
        Self { resolvers }
    }

    /// Try to resolve a method using the resolver chain.
    ///
    /// Returns the first successful resolution, or `NotFound` if no resolver handles it.
    pub fn resolve(
        &self,
        receiver: &Value,
        type_name: Name,
        method_name: Name,
    ) -> MethodResolution {
        for resolver in &self.resolvers {
            let result = resolver.resolve(receiver, type_name, method_name);
            if !matches!(result, MethodResolution::NotFound) {
                return result;
            }
        }
        MethodResolution::NotFound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_method_from_name() {
        assert_eq!(
            CollectionMethod::from_name("map"),
            Some(CollectionMethod::Map)
        );
        assert_eq!(
            CollectionMethod::from_name("filter"),
            Some(CollectionMethod::Filter)
        );
        assert_eq!(
            CollectionMethod::from_name("fold"),
            Some(CollectionMethod::Fold)
        );
        assert_eq!(
            CollectionMethod::from_name("find"),
            Some(CollectionMethod::Find)
        );
        assert_eq!(
            CollectionMethod::from_name("collect"),
            Some(CollectionMethod::Collect)
        );
        assert_eq!(
            CollectionMethod::from_name("any"),
            Some(CollectionMethod::Any)
        );
        assert_eq!(
            CollectionMethod::from_name("all"),
            Some(CollectionMethod::All)
        );
        assert_eq!(CollectionMethod::from_name("unknown"), None);
    }

    #[test]
    fn test_dispatcher_priority_ordering() {
        use crate::SharedMutableRegistry;
        use crate::UserMethodRegistry;
        use ori_ir::SharedInterner;

        let interner = SharedInterner::default();
        let registry = SharedMutableRegistry::new(UserMethodRegistry::new());

        // Create resolvers in wrong order
        let resolvers = vec![
            MethodResolverKind::Builtin(BuiltinMethodResolver::new()), // priority 2
            MethodResolverKind::UserRegistry(UserRegistryResolver::new(registry)), // priority 0
            MethodResolverKind::Collection(CollectionMethodResolver::new(&interner)), // priority 1
        ];

        let dispatcher = MethodDispatcher::new(resolvers);

        // Verify they're sorted by priority (0, 1, 2)
        assert_eq!(dispatcher.resolvers[0].priority(), 0);
        assert_eq!(dispatcher.resolvers[1].priority(), 1);
        assert_eq!(dispatcher.resolvers[2].priority(), 2);
        assert_eq!(dispatcher.resolvers[0].name(), "UserRegistryResolver");
        assert_eq!(dispatcher.resolvers[1].name(), "CollectionMethodResolver");
        assert_eq!(dispatcher.resolvers[2].name(), "BuiltinMethodResolver");
    }

    #[test]
    fn test_resolver_kind_dispatch() {
        use ori_ir::SharedInterner;

        let interner = SharedInterner::default();

        // Test that MethodResolverKind correctly dispatches to underlying resolvers
        let builtin = MethodResolverKind::Builtin(BuiltinMethodResolver::new());
        assert_eq!(builtin.priority(), 2);
        assert_eq!(builtin.name(), "BuiltinMethodResolver");

        let collection = MethodResolverKind::Collection(CollectionMethodResolver::new(&interner));
        assert_eq!(collection.priority(), 1);
        assert_eq!(collection.name(), "CollectionMethodResolver");
    }
}
