//! Pattern registry for managing pattern definitions.
//!
//! The registry provides a central location for looking up pattern
//! definitions by keyword and for pattern discovery.

use std::sync::OnceLock;
use rustc_hash::FxHashMap;
use super::definition::PatternDefinition;
use super::builtins;

/// Global pattern registry singleton.
static GLOBAL_REGISTRY: OnceLock<PatternRegistry> = OnceLock::new();

/// Registry of pattern definitions.
pub struct PatternRegistry {
    /// Map from keyword to pattern definition.
    patterns: FxHashMap<&'static str, Box<dyn PatternDefinition>>,
}

impl PatternRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        PatternRegistry {
            patterns: FxHashMap::default(),
        }
    }

    /// Register a pattern definition.
    pub fn register<P: PatternDefinition>(&mut self, pattern: P) {
        let keyword = pattern.keyword();
        self.patterns.insert(keyword, Box::new(pattern));
    }

    /// Look up a pattern by keyword.
    pub fn get(&self, keyword: &str) -> Option<&dyn PatternDefinition> {
        self.patterns.get(keyword).map(|p| p.as_ref())
    }

    /// Check if a keyword is a registered pattern.
    pub fn is_pattern(&self, keyword: &str) -> bool {
        self.patterns.contains_key(keyword)
    }

    /// Get all registered pattern keywords.
    pub fn keywords(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.patterns.keys().copied()
    }

    /// Get the number of registered patterns.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Get all registered patterns.
    pub fn patterns(&self) -> impl Iterator<Item = &dyn PatternDefinition> + '_ {
        self.patterns.values().map(|p| p.as_ref())
    }
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the global pattern registry with all built-in patterns registered.
pub fn global_registry() -> &'static PatternRegistry {
    GLOBAL_REGISTRY.get_or_init(|| {
        let mut registry = PatternRegistry::new();
        register_builtins(&mut registry);
        registry
    })
}

/// Register all built-in patterns.
fn register_builtins(registry: &mut PatternRegistry) {
    // Sequential and control flow
    registry.register(builtins::RunPattern);
    registry.register(builtins::TryPattern);
    registry.register(builtins::MatchPattern);

    // Data transformation
    registry.register(builtins::MapPattern);
    registry.register(builtins::FilterPattern);
    registry.register(builtins::FoldPattern);
    registry.register(builtins::FindPattern);
    registry.register(builtins::CollectPattern);

    // Recursion and iteration
    registry.register(builtins::RecursePattern);

    // Concurrency
    registry.register(builtins::ParallelPattern);
    registry.register(builtins::TimeoutPattern);
    registry.register(builtins::RetryPattern);

    // Caching and validation
    registry.register(builtins::CachePattern);
    registry.register(builtins::ValidatePattern);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_registration() {
        let registry = global_registry();
        assert!(registry.is_pattern("map"));
        assert!(registry.is_pattern("filter"));
        assert!(registry.is_pattern("fold"));
        assert!(!registry.is_pattern("unknown"));
    }

    #[test]
    fn test_registry_lookup() {
        let registry = global_registry();
        let map = registry.get("map").unwrap();
        assert_eq!(map.keyword(), "map");
    }

    #[test]
    fn test_registry_keywords() {
        let registry = global_registry();
        let keywords: Vec<_> = registry.keywords().collect();
        assert!(keywords.contains(&"map"));
        assert!(keywords.contains(&"filter"));
        assert!(keywords.contains(&"fold"));
        assert_eq!(keywords.len(), 14); // All 14 patterns
    }

    #[test]
    fn test_all_patterns_have_params() {
        let registry = global_registry();
        for pattern in registry.patterns() {
            // Every pattern should have at least a description
            assert!(!pattern.description().is_empty());
        }
    }
}
