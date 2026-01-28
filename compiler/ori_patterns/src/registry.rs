//! Pattern registry for looking up pattern definitions by kind.

// Arc is needed for SharedPattern - storing pattern definitions as trait objects
#![expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedPattern"
)]

use std::collections::HashMap;
use std::sync::Arc;

use ori_ir::FunctionExpKind;

use crate::builtins::{CatchPattern, PanicPattern, PrintPattern};
use crate::cache::CachePattern;
use crate::parallel::ParallelPattern;
use crate::recurse::RecursePattern;
use crate::spawn::SpawnPattern;
use crate::timeout::TimeoutPattern;
use crate::with_pattern::WithPattern;
use crate::PatternDefinition;

/// Shared pattern definition wrapper for storing patterns in registries.
///
/// This newtype enforces that all pattern definition sharing goes through
/// this type, preventing accidental direct `Arc<dyn PatternDefinition>` usage.
///
/// # Purpose
/// Patterns like `map`, `filter`, `fold` are stored in the `PatternRegistry`
/// as trait objects. `SharedPattern` wraps these trait objects in a clonable,
/// thread-safe wrapper that can be shared across type checking and evaluation.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Usage
///
/// ```text
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

    /// Create a new registry with all compiler patterns registered.
    pub fn new() -> Self {
        let mut patterns: HashMap<FunctionExpKind, SharedPattern> = HashMap::new();

        // Compiler patterns (require special syntax or static analysis)
        patterns.insert(FunctionExpKind::Recurse, SharedPattern::new(RecursePattern));
        patterns.insert(
            FunctionExpKind::Parallel,
            SharedPattern::new(ParallelPattern),
        );
        patterns.insert(FunctionExpKind::Spawn, SharedPattern::new(SpawnPattern));
        patterns.insert(FunctionExpKind::Timeout, SharedPattern::new(TimeoutPattern));
        patterns.insert(FunctionExpKind::Cache, SharedPattern::new(CachePattern));
        patterns.insert(FunctionExpKind::With, SharedPattern::new(WithPattern));

        // Fundamental built-ins (I/O, control flow, error recovery)
        patterns.insert(FunctionExpKind::Print, SharedPattern::new(PrintPattern));
        patterns.insert(FunctionExpKind::Panic, SharedPattern::new(PanicPattern));
        patterns.insert(FunctionExpKind::Catch, SharedPattern::new(CatchPattern));

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
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_all_patterns() {
        let registry = PatternRegistry::new();
        assert_eq!(registry.len(), 9);

        // Verify each pattern is registered
        assert!(registry.get(FunctionExpKind::Recurse).is_some());
        assert!(registry.get(FunctionExpKind::Parallel).is_some());
        assert!(registry.get(FunctionExpKind::Spawn).is_some());
        assert!(registry.get(FunctionExpKind::Timeout).is_some());
        assert!(registry.get(FunctionExpKind::Cache).is_some());
        assert!(registry.get(FunctionExpKind::With).is_some());
        assert!(registry.get(FunctionExpKind::Print).is_some());
        assert!(registry.get(FunctionExpKind::Panic).is_some());
        assert!(registry.get(FunctionExpKind::Catch).is_some());
    }

    #[test]
    fn test_pattern_names() {
        let registry = PatternRegistry::new();

        assert_eq!(
            registry.get(FunctionExpKind::Recurse).unwrap().name(),
            "recurse"
        );
        assert_eq!(
            registry.get(FunctionExpKind::Parallel).unwrap().name(),
            "parallel"
        );
        assert_eq!(
            registry.get(FunctionExpKind::Timeout).unwrap().name(),
            "timeout"
        );
        assert_eq!(
            registry.get(FunctionExpKind::Print).unwrap().name(),
            "print"
        );
        assert_eq!(
            registry.get(FunctionExpKind::Panic).unwrap().name(),
            "panic"
        );
    }

    #[test]
    fn test_required_props() {
        let registry = PatternRegistry::new();

        let timeout = registry.get(FunctionExpKind::Timeout).unwrap();
        assert!(timeout.required_props().contains(&"operation"));
        assert!(timeout.required_props().contains(&"after"));

        let print = registry.get(FunctionExpKind::Print).unwrap();
        assert!(print.required_props().contains(&"msg"));
    }
}
