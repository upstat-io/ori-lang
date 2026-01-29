//! Pattern registry for looking up pattern definitions by kind.

use ori_ir::FunctionExpKind;

use crate::builtins::{CatchPattern, PanicPattern, PrintPattern};
use crate::cache::CachePattern;
use crate::parallel::ParallelPattern;
use crate::recurse::RecursePattern;
use crate::spawn::SpawnPattern;
use crate::timeout::TimeoutPattern;
use crate::with_pattern::WithPattern;
use crate::PatternDefinition;

// Static pattern instances for 'static lifetime references
static RECURSE: RecursePattern = RecursePattern;
static PARALLEL: ParallelPattern = ParallelPattern;
static SPAWN: SpawnPattern = SpawnPattern;
static TIMEOUT: TimeoutPattern = TimeoutPattern;
static CACHE: CachePattern = CachePattern;
static WITH: WithPattern = WithPattern;
static PRINT: PrintPattern = PrintPattern;
static PANIC: PanicPattern = PanicPattern;
static CATCH: CatchPattern = CatchPattern;

/// Registry mapping `FunctionExpKind` to pattern definitions.
///
/// Uses direct enum dispatch instead of HashMap lookup.
/// All patterns are ZSTs (zero-sized types) with static lifetime,
/// so this struct has zero overhead and avoids borrow issues.
pub struct PatternRegistry {
    // Marker field to prevent external construction
    _private: (),
}

impl PatternRegistry {
    /// Create a new registry with all compiler patterns registered.
    pub fn new() -> Self {
        PatternRegistry { _private: () }
    }

    /// Get the pattern definition for a given kind.
    ///
    /// Returns a static reference to avoid borrow issues with the registry.
    pub fn get(&self, kind: FunctionExpKind) -> &'static dyn PatternDefinition {
        match kind {
            FunctionExpKind::Recurse => &RECURSE,
            FunctionExpKind::Parallel => &PARALLEL,
            FunctionExpKind::Spawn => &SPAWN,
            FunctionExpKind::Timeout => &TIMEOUT,
            FunctionExpKind::Cache => &CACHE,
            FunctionExpKind::With => &WITH,
            FunctionExpKind::Print => &PRINT,
            FunctionExpKind::Panic => &PANIC,
            FunctionExpKind::Catch => &CATCH,
        }
    }

    /// Get all registered pattern kinds.
    pub fn kinds(&self) -> impl Iterator<Item = FunctionExpKind> {
        [
            FunctionExpKind::Recurse,
            FunctionExpKind::Parallel,
            FunctionExpKind::Spawn,
            FunctionExpKind::Timeout,
            FunctionExpKind::Cache,
            FunctionExpKind::With,
            FunctionExpKind::Print,
            FunctionExpKind::Panic,
            FunctionExpKind::Catch,
        ]
        .into_iter()
    }

    /// Get the number of registered patterns.
    pub fn len(&self) -> usize {
        9
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        false
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
        assert_eq!(registry.len(), 9);

        // Verify each pattern is accessible (all FunctionExpKind variants are covered)
        let _ = registry.get(FunctionExpKind::Recurse);
        let _ = registry.get(FunctionExpKind::Parallel);
        let _ = registry.get(FunctionExpKind::Spawn);
        let _ = registry.get(FunctionExpKind::Timeout);
        let _ = registry.get(FunctionExpKind::Cache);
        let _ = registry.get(FunctionExpKind::With);
        let _ = registry.get(FunctionExpKind::Print);
        let _ = registry.get(FunctionExpKind::Panic);
        let _ = registry.get(FunctionExpKind::Catch);
    }

    #[test]
    fn test_pattern_names() {
        let registry = PatternRegistry::new();

        assert_eq!(registry.get(FunctionExpKind::Recurse).name(), "recurse");
        assert_eq!(registry.get(FunctionExpKind::Parallel).name(), "parallel");
        assert_eq!(registry.get(FunctionExpKind::Timeout).name(), "timeout");
        assert_eq!(registry.get(FunctionExpKind::Print).name(), "print");
        assert_eq!(registry.get(FunctionExpKind::Panic).name(), "panic");
    }

    #[test]
    fn test_required_props() {
        let registry = PatternRegistry::new();

        let timeout = registry.get(FunctionExpKind::Timeout);
        assert!(timeout.required_props().contains(&"operation"));
        assert!(timeout.required_props().contains(&"after"));

        let print = registry.get(FunctionExpKind::Print);
        assert!(print.required_props().contains(&"msg"));
    }

    #[test]
    fn test_kinds_iterator() {
        let registry = PatternRegistry::new();
        let kinds: Vec<_> = registry.kinds().collect();
        assert_eq!(kinds.len(), 9);
        assert!(kinds.contains(&FunctionExpKind::Recurse));
        assert!(kinds.contains(&FunctionExpKind::Parallel));
        assert!(kinds.contains(&FunctionExpKind::Spawn));
        assert!(kinds.contains(&FunctionExpKind::Timeout));
        assert!(kinds.contains(&FunctionExpKind::Cache));
        assert!(kinds.contains(&FunctionExpKind::With));
        assert!(kinds.contains(&FunctionExpKind::Print));
        assert!(kinds.contains(&FunctionExpKind::Panic));
        assert!(kinds.contains(&FunctionExpKind::Catch));
    }
}
