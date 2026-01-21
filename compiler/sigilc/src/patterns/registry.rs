// Pattern registry for the Sigil compiler
//
// Provides a global registry of pattern handlers that can be used
// across all compiler phases.

use super::core::PatternDefinition;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Global pattern registry.
///
/// This singleton holds all registered pattern definitions and provides
/// lookup by keyword.
pub struct PatternRegistry {
    patterns: HashMap<&'static str, Box<dyn PatternDefinition>>,
}

impl PatternRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        PatternRegistry {
            patterns: HashMap::new(),
        }
    }

    /// Register a pattern definition.
    ///
    /// # Panics
    /// Panics if a pattern with the same keyword is already registered.
    pub fn register<P: PatternDefinition>(&mut self, pattern: P) {
        let keyword = pattern.keyword();
        if self.patterns.contains_key(keyword) {
            panic!("Pattern '{}' is already registered", keyword);
        }
        self.patterns.insert(keyword, Box::new(pattern));
    }

    /// Look up a pattern definition by keyword.
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
}

impl Default for PatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton registry
static GLOBAL_REGISTRY: OnceLock<RwLock<PatternRegistry>> = OnceLock::new();

/// Get a reference to the global pattern registry.
///
/// The registry is lazily initialized with all built-in patterns
/// on first access.
pub fn global_registry() -> &'static RwLock<PatternRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| {
        let mut registry = PatternRegistry::new();
        register_builtins(&mut registry);
        RwLock::new(registry)
    })
}

/// Register all built-in pattern definitions.
fn register_builtins(registry: &mut PatternRegistry) {
    use super::builtins::*;

    registry.register(FoldPattern);
    registry.register(MapPattern);
    registry.register(FilterPattern);
    registry.register(CollectPattern);
    registry.register(RecursePattern);
    registry.register(IteratePattern);
    registry.register(TransformPattern);
    registry.register(CountPattern);
    registry.register(ParallelPattern);
}

/// Helper function to check types for a pattern expression.
///
/// This looks up the pattern definition and delegates to its type checking.
pub fn check_pattern_types(
    pattern: &crate::ast::PatternExpr,
    ctx: &crate::types::context::TypeContext,
) -> Result<crate::ast::TypeExpr, String> {
    crate::types::check_pattern::check_pattern_expr(pattern, ctx)
}

/// Helper function to evaluate a pattern expression.
///
/// This looks up the pattern definition and delegates to its evaluation.
pub fn eval_pattern(
    pattern: &crate::ast::PatternExpr,
    env: &crate::eval::value::Environment,
) -> Result<crate::eval::value::Value, String> {
    crate::eval::patterns::eval_pattern(pattern, env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = PatternRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_global_registry_has_builtins() {
        let registry = global_registry().read().unwrap();
        assert!(registry.is_pattern("fold"));
        assert!(registry.is_pattern("map"));
        assert!(registry.is_pattern("filter"));
        assert!(registry.is_pattern("recurse"));
        assert!(!registry.is_pattern("unknown_pattern"));
    }
}
