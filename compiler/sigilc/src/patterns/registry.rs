//! Pattern registry for looking up pattern definitions by kind.

// Arc is needed for SharedPattern - storing pattern definitions as trait objects
#![expect(clippy::disallowed_types, reason = "Arc is the implementation of SharedPattern")]

use std::collections::HashMap;
use std::sync::Arc;
use crate::ir::FunctionExpKind;
use super::PatternDefinition;

/// Shared pattern definition wrapper for storing patterns in registries.
///
/// This newtype enforces that all pattern definition sharing goes through
/// this type, preventing accidental direct `Arc<dyn PatternDefinition>` usage.
///
/// # Purpose
/// Patterns like `map`, `filter`, `fold` are stored in the PatternRegistry
/// as trait objects. SharedPattern wraps these trait objects in a clonable,
/// thread-safe wrapper that can be shared across type checking and evaluation.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Usage
/// ```ignore
/// let pattern = SharedPattern::new(MapPattern);
/// registry.register(FunctionExpKind::Map, pattern);
/// ```
#[derive(Clone)]
pub struct SharedPattern(Arc<dyn PatternDefinition>);

impl SharedPattern {
    /// Create a new shared pattern from a pattern definition.
    pub fn new<P: PatternDefinition + 'static>(pattern: P) -> Self {
        SharedPattern(Arc::new(pattern))
    }
}

impl std::ops::Deref for SharedPattern {
    type Target = dyn PatternDefinition;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

// Import all pattern types
use super::map::MapPattern;
use super::filter::FilterPattern;
use super::fold::FoldPattern;
use super::find::FindPattern;
use super::collect::CollectPattern;
use super::recurse::RecursePattern;
use super::parallel::ParallelPattern;
use super::spawn::SpawnPattern;
use super::timeout::TimeoutPattern;
use super::retry::RetryPattern;
use super::cache::CachePattern;
use super::validate::ValidatePattern;
use super::with_pattern::WithPattern;
use super::builtins::{
    AssertPattern, AssertEqPattern, AssertNePattern,
    LenPattern, IsEmptyPattern, IsSomePattern, IsNonePattern,
    IsOkPattern, IsErrPattern, PrintPattern, PanicPattern,
    ComparePattern, MinPattern, MaxPattern,
};

/// Registry mapping `FunctionExpKind` to pattern definitions.
///
/// This is the central point for pattern extensibility. Adding a new pattern
/// requires implementing `PatternDefinition` and registering it here.
pub struct PatternRegistry {
    patterns: HashMap<FunctionExpKind, SharedPattern>,
}

impl PatternRegistry {
    /// Create an empty registry (for testing or custom configurations).
    pub fn empty() -> Self {
        PatternRegistry {
            patterns: HashMap::new(),
        }
    }

    /// Create a new registry with all built-in patterns registered.
    pub fn new() -> Self {
        let mut patterns: HashMap<FunctionExpKind, SharedPattern> = HashMap::new();

        // Register all 13 function_exp patterns
        patterns.insert(FunctionExpKind::Map, SharedPattern::new(MapPattern));
        patterns.insert(FunctionExpKind::Filter, SharedPattern::new(FilterPattern));
        patterns.insert(FunctionExpKind::Fold, SharedPattern::new(FoldPattern));
        patterns.insert(FunctionExpKind::Find, SharedPattern::new(FindPattern));
        patterns.insert(FunctionExpKind::Collect, SharedPattern::new(CollectPattern));
        patterns.insert(FunctionExpKind::Recurse, SharedPattern::new(RecursePattern));
        patterns.insert(FunctionExpKind::Parallel, SharedPattern::new(ParallelPattern));
        patterns.insert(FunctionExpKind::Spawn, SharedPattern::new(SpawnPattern));
        patterns.insert(FunctionExpKind::Timeout, SharedPattern::new(TimeoutPattern));
        patterns.insert(FunctionExpKind::Retry, SharedPattern::new(RetryPattern));
        patterns.insert(FunctionExpKind::Cache, SharedPattern::new(CachePattern));
        patterns.insert(FunctionExpKind::Validate, SharedPattern::new(ValidatePattern));
        patterns.insert(FunctionExpKind::With, SharedPattern::new(WithPattern));

        // Register core patterns (function_exp with named args)
        patterns.insert(FunctionExpKind::Assert, SharedPattern::new(AssertPattern));
        patterns.insert(FunctionExpKind::AssertEq, SharedPattern::new(AssertEqPattern));
        patterns.insert(FunctionExpKind::AssertNe, SharedPattern::new(AssertNePattern));
        patterns.insert(FunctionExpKind::Len, SharedPattern::new(LenPattern));
        patterns.insert(FunctionExpKind::IsEmpty, SharedPattern::new(IsEmptyPattern));
        patterns.insert(FunctionExpKind::IsSome, SharedPattern::new(IsSomePattern));
        patterns.insert(FunctionExpKind::IsNone, SharedPattern::new(IsNonePattern));
        patterns.insert(FunctionExpKind::IsOk, SharedPattern::new(IsOkPattern));
        patterns.insert(FunctionExpKind::IsErr, SharedPattern::new(IsErrPattern));
        patterns.insert(FunctionExpKind::Print, SharedPattern::new(PrintPattern));
        patterns.insert(FunctionExpKind::Panic, SharedPattern::new(PanicPattern));
        patterns.insert(FunctionExpKind::Compare, SharedPattern::new(ComparePattern));
        patterns.insert(FunctionExpKind::Min, SharedPattern::new(MinPattern));
        patterns.insert(FunctionExpKind::Max, SharedPattern::new(MaxPattern));

        PatternRegistry { patterns }
    }

    /// Register a custom pattern.
    ///
    /// This allows injecting mock patterns for testing or adding custom patterns.
    pub fn register(&mut self, kind: FunctionExpKind, pattern: SharedPattern) {
        self.patterns.insert(kind, pattern);
    }

    /// Get the pattern definition for a given kind.
    pub fn get(&self, kind: FunctionExpKind) -> Option<SharedPattern> {
        self.patterns.get(&kind).cloned()
    }

    /// Get all registered pattern kinds.
    pub fn kinds(&self) -> impl Iterator<Item = &FunctionExpKind> {
        self.patterns.keys()
    }

    /// Get the number of registered patterns.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_all_patterns() {
        let registry = PatternRegistry::new();

        // Should have all 27 patterns (13 function_exp + 14 core patterns)
        assert_eq!(registry.len(), 27);

        // Verify each function_exp pattern is registered
        assert!(registry.get(FunctionExpKind::Map).is_some());
        assert!(registry.get(FunctionExpKind::Filter).is_some());
        assert!(registry.get(FunctionExpKind::Fold).is_some());
        assert!(registry.get(FunctionExpKind::Find).is_some());
        assert!(registry.get(FunctionExpKind::Collect).is_some());
        assert!(registry.get(FunctionExpKind::Recurse).is_some());
        assert!(registry.get(FunctionExpKind::Parallel).is_some());
        assert!(registry.get(FunctionExpKind::Spawn).is_some());
        assert!(registry.get(FunctionExpKind::Timeout).is_some());
        assert!(registry.get(FunctionExpKind::Retry).is_some());
        assert!(registry.get(FunctionExpKind::Cache).is_some());
        assert!(registry.get(FunctionExpKind::Validate).is_some());
        assert!(registry.get(FunctionExpKind::With).is_some());

        // Verify each core pattern is registered
        assert!(registry.get(FunctionExpKind::Assert).is_some());
        assert!(registry.get(FunctionExpKind::AssertEq).is_some());
        assert!(registry.get(FunctionExpKind::AssertNe).is_some());
        assert!(registry.get(FunctionExpKind::Len).is_some());
        assert!(registry.get(FunctionExpKind::IsEmpty).is_some());
        assert!(registry.get(FunctionExpKind::IsSome).is_some());
        assert!(registry.get(FunctionExpKind::IsNone).is_some());
        assert!(registry.get(FunctionExpKind::IsOk).is_some());
        assert!(registry.get(FunctionExpKind::IsErr).is_some());
        assert!(registry.get(FunctionExpKind::Print).is_some());
        assert!(registry.get(FunctionExpKind::Panic).is_some());
        assert!(registry.get(FunctionExpKind::Compare).is_some());
        assert!(registry.get(FunctionExpKind::Min).is_some());
        assert!(registry.get(FunctionExpKind::Max).is_some());
    }

    #[test]
    fn test_pattern_names() {
        let registry = PatternRegistry::new();

        assert_eq!(registry.get(FunctionExpKind::Map).unwrap().name(), "map");
        assert_eq!(registry.get(FunctionExpKind::Filter).unwrap().name(), "filter");
        assert_eq!(registry.get(FunctionExpKind::Fold).unwrap().name(), "fold");
    }

    #[test]
    fn test_required_props() {
        let registry = PatternRegistry::new();

        let map = registry.get(FunctionExpKind::Map).unwrap();
        assert!(map.required_props().contains(&"over"));
        assert!(map.required_props().contains(&"transform"));

        let fold = registry.get(FunctionExpKind::Fold).unwrap();
        assert!(fold.required_props().contains(&"over"));
        assert!(fold.required_props().contains(&"init"));
        assert!(fold.required_props().contains(&"op"));
    }
}
