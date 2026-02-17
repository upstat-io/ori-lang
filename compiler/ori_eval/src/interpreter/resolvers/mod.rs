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

#[expect(
    clippy::disallowed_types,
    reason = "Arc for immutable resolver list shared across child interpreters"
)]
use std::sync::Arc;

use crate::{UserMethod, Value};
use ori_ir::DerivedMethodInfo;
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

    // Iterator methods (adapters + consumers)
    /// `Iterator<T>.next() -> (T?, Iterator<T>)`
    IterNext,
    /// `Iterator<T>.map(transform: T -> U) -> Iterator<U>`
    IterMap,
    /// `Iterator<T>.filter(predicate: T -> bool) -> Iterator<T>`
    IterFilter,
    /// `Iterator<T>.take(count: int) -> Iterator<T>`
    IterTake,
    /// `Iterator<T>.skip(count: int) -> Iterator<T>`
    IterSkip,
    /// `Iterator<T>.fold(initial: U, op: (U, T) -> U) -> U`
    IterFold,
    /// `Iterator<T>.count() -> int`
    IterCount,
    /// `Iterator<T>.find(predicate: T -> bool) -> T?`
    IterFind,
    /// `Iterator<T>.any(predicate: T -> bool) -> bool`
    IterAny,
    /// `Iterator<T>.all(predicate: T -> bool) -> bool`
    IterAll,
    /// `Iterator<T>.for_each(f: T -> void) -> void`
    IterForEach,
    /// `Iterator<T>.collect() -> [T]` (default)
    IterCollect,
    /// `Iterator<T>.__collect_set() -> Set<T>` (type-directed via Collect trait)
    IterCollectSet,
    /// `Iterator<T>.enumerate() -> Iterator<(int, T)>`
    IterEnumerate,
    /// `Iterator<T>.zip(other: Iterator<U>) -> Iterator<(T, U)>`
    IterZip,
    /// `Iterator<T>.chain(other: Iterator<T>) -> Iterator<T>`
    IterChain,
    /// `Iterator<Iterator<T>>.flatten() -> Iterator<T>`
    IterFlatten,
    /// `Iterator<T>.flat_map(f: T -> Iterator<U>) -> Iterator<U>`
    IterFlatMap,
    /// `Iterator<T>.cycle() -> Iterator<T>`
    IterCycle,
    /// `Iterator<T>.next_back() -> (T?, Iterator<T>)` (double-ended only)
    IterNextBack,
    /// `Iterator<T>.rev() -> Iterator<T>` (double-ended only)
    IterRev,
    /// `Iterator<T>.last() -> T?` (double-ended only)
    IterLast,
    /// `Iterator<T>.rfind(predicate: T -> bool) -> T?` (double-ended only)
    IterRFind,
    /// `Iterator<T>.rfold(initial: U, op: (U, T) -> U) -> U` (double-ended only)
    IterRFold,
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

    /// Check if this is an iterator method variant.
    pub fn is_iterator_method(self) -> bool {
        matches!(
            self,
            Self::IterNext
                | Self::IterMap
                | Self::IterFilter
                | Self::IterTake
                | Self::IterSkip
                | Self::IterEnumerate
                | Self::IterZip
                | Self::IterChain
                | Self::IterFlatten
                | Self::IterFlatMap
                | Self::IterCycle
                | Self::IterNextBack
                | Self::IterRev
                | Self::IterLast
                | Self::IterRFind
                | Self::IterRFold
                | Self::IterFold
                | Self::IterCount
                | Self::IterFind
                | Self::IterAny
                | Self::IterAll
                | Self::IterForEach
                | Self::IterCollect
                | Self::IterCollectSet
        )
    }

    /// All `Iter*` variants paired with their Ori method name.
    ///
    /// Single source of truth for consistency tests — any new iterator method
    /// variant added to the enum must be added here, and the consistency test
    /// will verify it is also wired into the resolver and dispatcher.
    #[cfg(test)]
    pub fn all_iterator_variants() -> &'static [(&'static str, CollectionMethod)] {
        &[
            ("next", Self::IterNext),
            ("map", Self::IterMap),
            ("filter", Self::IterFilter),
            ("take", Self::IterTake),
            ("skip", Self::IterSkip),
            ("enumerate", Self::IterEnumerate),
            ("zip", Self::IterZip),
            ("chain", Self::IterChain),
            ("flatten", Self::IterFlatten),
            ("flat_map", Self::IterFlatMap),
            ("cycle", Self::IterCycle),
            ("fold", Self::IterFold),
            ("count", Self::IterCount),
            ("find", Self::IterFind),
            ("any", Self::IterAny),
            ("all", Self::IterAll),
            ("for_each", Self::IterForEach),
            ("collect", Self::IterCollect),
            ("next_back", Self::IterNextBack),
            ("rev", Self::IterRev),
            ("last", Self::IterLast),
            ("rfind", Self::IterRFind),
            ("rfold", Self::IterRFold),
            (
                ori_ir::builtin_constants::iterator::COLLECT_SET_METHOD,
                Self::IterCollectSet,
            ),
        ]
    }
}

/// All Iterator method names recognized by the eval `CollectionMethodResolver`.
///
/// Used by cross-crate consistency tests in `oric` to verify that every Iterator
/// method registered in typeck has a corresponding eval resolver entry, and vice
/// versa. Sorted alphabetically.
pub const ITERATOR_METHOD_NAMES: &[&str] = &[
    "all",
    "any",
    "chain",
    "collect",
    "count",
    "cycle",
    "enumerate",
    "filter",
    "find",
    "flat_map",
    "flatten",
    "fold",
    "for_each",
    "last",
    "map",
    "next",
    "next_back",
    "rev",
    "rfind",
    "rfold",
    "skip",
    "take",
    "zip",
];

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
#[derive(Clone)]
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
///
/// Uses `Arc<Vec<...>>` internally so that cloning is O(1). The resolver
/// list is immutable after construction, making shared ownership correct.
#[derive(Clone)]
#[expect(
    clippy::disallowed_types,
    reason = "Arc for immutable resolver list shared across child interpreters"
)]
pub struct MethodDispatcher {
    resolvers: Arc<Vec<MethodResolverKind>>,
}

impl MethodDispatcher {
    /// Create a new dispatcher with the given resolvers.
    ///
    /// Resolvers are sorted by priority (lowest first) and wrapped in `Arc`
    /// so that child interpreters can share the dispatcher without deep-cloning
    /// the resolver state (e.g., `BuiltinMethodResolver`'s `FxHashSet`).
    #[expect(
        clippy::disallowed_types,
        reason = "Arc for immutable resolver list shared across child interpreters"
    )]
    pub fn new(mut resolvers: Vec<MethodResolverKind>) -> Self {
        resolvers.sort_by_key(MethodResolverKind::priority);
        Self {
            resolvers: Arc::new(resolvers),
        }
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
        for resolver in self.resolvers.iter() {
            let result = resolver.resolve(receiver, type_name, method_name);
            if !matches!(result, MethodResolution::NotFound) {
                return result;
            }
        }
        MethodResolution::NotFound
    }
}

#[cfg(test)]
mod tests;
