//! Pattern registry for looking up pattern definitions by kind.

use std::collections::HashMap;
use std::sync::Arc;
use crate::ir::FunctionExpKind;
use super::PatternDefinition;

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

/// Registry mapping `FunctionExpKind` to pattern definitions.
///
/// This is the central point for pattern extensibility. Adding a new pattern
/// requires implementing `PatternDefinition` and registering it here.
pub struct PatternRegistry {
    patterns: HashMap<FunctionExpKind, Arc<dyn PatternDefinition>>,
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
        let mut patterns: HashMap<FunctionExpKind, Arc<dyn PatternDefinition>> = HashMap::new();

        // Register all 13 function_exp patterns
        patterns.insert(FunctionExpKind::Map, Arc::new(MapPattern));
        patterns.insert(FunctionExpKind::Filter, Arc::new(FilterPattern));
        patterns.insert(FunctionExpKind::Fold, Arc::new(FoldPattern));
        patterns.insert(FunctionExpKind::Find, Arc::new(FindPattern));
        patterns.insert(FunctionExpKind::Collect, Arc::new(CollectPattern));
        patterns.insert(FunctionExpKind::Recurse, Arc::new(RecursePattern));
        patterns.insert(FunctionExpKind::Parallel, Arc::new(ParallelPattern));
        patterns.insert(FunctionExpKind::Spawn, Arc::new(SpawnPattern));
        patterns.insert(FunctionExpKind::Timeout, Arc::new(TimeoutPattern));
        patterns.insert(FunctionExpKind::Retry, Arc::new(RetryPattern));
        patterns.insert(FunctionExpKind::Cache, Arc::new(CachePattern));
        patterns.insert(FunctionExpKind::Validate, Arc::new(ValidatePattern));
        patterns.insert(FunctionExpKind::With, Arc::new(WithPattern));

        PatternRegistry { patterns }
    }

    /// Register a custom pattern.
    ///
    /// This allows injecting mock patterns for testing or adding custom patterns.
    pub fn register(&mut self, kind: FunctionExpKind, pattern: Arc<dyn PatternDefinition>) {
        self.patterns.insert(kind, pattern);
    }

    /// Get the pattern definition for a given kind.
    pub fn get(&self, kind: FunctionExpKind) -> Option<Arc<dyn PatternDefinition>> {
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

        // Should have all 13 function_exp patterns
        assert_eq!(registry.len(), 13);

        // Verify each pattern is registered
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
