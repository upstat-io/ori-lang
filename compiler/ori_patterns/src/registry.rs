//! Pattern registry for looking up pattern definitions by kind.

use ori_ir::FunctionExpKind;

use crate::builtins::{CatchPattern, PanicPattern, PrintPattern, TodoPattern, UnreachablePattern};
use crate::cache::CachePattern;
use crate::parallel::ParallelPattern;
use crate::recurse::RecursePattern;
use crate::spawn::SpawnPattern;
use crate::timeout::TimeoutPattern;
use crate::with_pattern::WithPattern;
use crate::{
    EvalContext, EvalResult, FusedPattern, OptionalArg, PatternDefinition, PatternExecutor,
    ScopedBinding,
};

/// Enum dispatch for all built-in patterns.
///
/// This provides static dispatch instead of trait objects for better performance:
/// - No vtable indirection
/// - Better inlining opportunities
/// - Compile-time exhaustiveness checking
///
/// All patterns are zero-sized types (ZSTs), so this enum has minimal overhead
/// (just the discriminant byte).
#[derive(Clone, Copy)]
pub enum Pattern {
    Recurse(RecursePattern),
    Parallel(ParallelPattern),
    Spawn(SpawnPattern),
    Timeout(TimeoutPattern),
    Cache(CachePattern),
    With(WithPattern),
    Print(PrintPattern),
    Panic(PanicPattern),
    Catch(CatchPattern),
    Todo(TodoPattern),
    Unreachable(UnreachablePattern),
}

impl PatternDefinition for Pattern {
    fn name(&self) -> &'static str {
        match self {
            Pattern::Recurse(p) => p.name(),
            Pattern::Parallel(p) => p.name(),
            Pattern::Spawn(p) => p.name(),
            Pattern::Timeout(p) => p.name(),
            Pattern::Cache(p) => p.name(),
            Pattern::With(p) => p.name(),
            Pattern::Print(p) => p.name(),
            Pattern::Panic(p) => p.name(),
            Pattern::Catch(p) => p.name(),
            Pattern::Todo(p) => p.name(),
            Pattern::Unreachable(p) => p.name(),
        }
    }

    fn required_props(&self) -> &'static [&'static str] {
        match self {
            Pattern::Recurse(p) => p.required_props(),
            Pattern::Parallel(p) => p.required_props(),
            Pattern::Spawn(p) => p.required_props(),
            Pattern::Timeout(p) => p.required_props(),
            Pattern::Cache(p) => p.required_props(),
            Pattern::With(p) => p.required_props(),
            Pattern::Print(p) => p.required_props(),
            Pattern::Panic(p) => p.required_props(),
            Pattern::Catch(p) => p.required_props(),
            Pattern::Todo(p) => p.required_props(),
            Pattern::Unreachable(p) => p.required_props(),
        }
    }

    fn optional_props(&self) -> &'static [&'static str] {
        match self {
            Pattern::Recurse(p) => p.optional_props(),
            Pattern::Parallel(p) => p.optional_props(),
            Pattern::Spawn(p) => p.optional_props(),
            Pattern::Timeout(p) => p.optional_props(),
            Pattern::Cache(p) => p.optional_props(),
            Pattern::With(p) => p.optional_props(),
            Pattern::Print(p) => p.optional_props(),
            Pattern::Panic(p) => p.optional_props(),
            Pattern::Catch(p) => p.optional_props(),
            Pattern::Todo(p) => p.optional_props(),
            Pattern::Unreachable(p) => p.optional_props(),
        }
    }

    fn optional_args(&self) -> &'static [OptionalArg] {
        match self {
            Pattern::Recurse(p) => p.optional_args(),
            Pattern::Parallel(p) => p.optional_args(),
            Pattern::Spawn(p) => p.optional_args(),
            Pattern::Timeout(p) => p.optional_args(),
            Pattern::Cache(p) => p.optional_args(),
            Pattern::With(p) => p.optional_args(),
            Pattern::Print(p) => p.optional_args(),
            Pattern::Panic(p) => p.optional_args(),
            Pattern::Catch(p) => p.optional_args(),
            Pattern::Todo(p) => p.optional_args(),
            Pattern::Unreachable(p) => p.optional_args(),
        }
    }

    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        match self {
            Pattern::Recurse(p) => p.scoped_bindings(),
            Pattern::Parallel(p) => p.scoped_bindings(),
            Pattern::Spawn(p) => p.scoped_bindings(),
            Pattern::Timeout(p) => p.scoped_bindings(),
            Pattern::Cache(p) => p.scoped_bindings(),
            Pattern::With(p) => p.scoped_bindings(),
            Pattern::Print(p) => p.scoped_bindings(),
            Pattern::Panic(p) => p.scoped_bindings(),
            Pattern::Catch(p) => p.scoped_bindings(),
            Pattern::Todo(p) => p.scoped_bindings(),
            Pattern::Unreachable(p) => p.scoped_bindings(),
        }
    }

    fn allows_arbitrary_props(&self) -> bool {
        match self {
            Pattern::Recurse(p) => p.allows_arbitrary_props(),
            Pattern::Parallel(p) => p.allows_arbitrary_props(),
            Pattern::Spawn(p) => p.allows_arbitrary_props(),
            Pattern::Timeout(p) => p.allows_arbitrary_props(),
            Pattern::Cache(p) => p.allows_arbitrary_props(),
            Pattern::With(p) => p.allows_arbitrary_props(),
            Pattern::Print(p) => p.allows_arbitrary_props(),
            Pattern::Panic(p) => p.allows_arbitrary_props(),
            Pattern::Catch(p) => p.allows_arbitrary_props(),
            Pattern::Todo(p) => p.allows_arbitrary_props(),
            Pattern::Unreachable(p) => p.allows_arbitrary_props(),
        }
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        match self {
            Pattern::Recurse(p) => p.evaluate(ctx, exec),
            Pattern::Parallel(p) => p.evaluate(ctx, exec),
            Pattern::Spawn(p) => p.evaluate(ctx, exec),
            Pattern::Timeout(p) => p.evaluate(ctx, exec),
            Pattern::Cache(p) => p.evaluate(ctx, exec),
            Pattern::With(p) => p.evaluate(ctx, exec),
            Pattern::Print(p) => p.evaluate(ctx, exec),
            Pattern::Panic(p) => p.evaluate(ctx, exec),
            Pattern::Catch(p) => p.evaluate(ctx, exec),
            Pattern::Todo(p) => p.evaluate(ctx, exec),
            Pattern::Unreachable(p) => p.evaluate(ctx, exec),
        }
    }

    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        match self {
            Pattern::Recurse(p) => p.can_fuse_with(next),
            Pattern::Parallel(p) => p.can_fuse_with(next),
            Pattern::Spawn(p) => p.can_fuse_with(next),
            Pattern::Timeout(p) => p.can_fuse_with(next),
            Pattern::Cache(p) => p.can_fuse_with(next),
            Pattern::With(p) => p.can_fuse_with(next),
            Pattern::Print(p) => p.can_fuse_with(next),
            Pattern::Panic(p) => p.can_fuse_with(next),
            Pattern::Catch(p) => p.can_fuse_with(next),
            Pattern::Todo(p) => p.can_fuse_with(next),
            Pattern::Unreachable(p) => p.can_fuse_with(next),
        }
    }

    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
        self_ctx: &EvalContext,
        next_ctx: &EvalContext,
    ) -> Option<FusedPattern> {
        match self {
            Pattern::Recurse(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Parallel(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Spawn(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Timeout(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Cache(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::With(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Print(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Panic(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Catch(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Todo(p) => p.fuse_with(next, self_ctx, next_ctx),
            Pattern::Unreachable(p) => p.fuse_with(next, self_ctx, next_ctx),
        }
    }
}

/// Registry mapping `FunctionExpKind` to pattern definitions.
///
/// Uses direct enum dispatch instead of trait objects for better performance:
/// - No vtable indirection
/// - Better inlining opportunities
/// - Compile-time exhaustiveness checking
///
/// All patterns are ZSTs (zero-sized types), so this struct has zero overhead.
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
    /// Returns a `Pattern` enum for static dispatch.
    pub fn get(&self, kind: FunctionExpKind) -> Pattern {
        match kind {
            FunctionExpKind::Recurse => Pattern::Recurse(RecursePattern),
            FunctionExpKind::Parallel => Pattern::Parallel(ParallelPattern),
            FunctionExpKind::Spawn => Pattern::Spawn(SpawnPattern),
            FunctionExpKind::Timeout => Pattern::Timeout(TimeoutPattern),
            FunctionExpKind::Cache => Pattern::Cache(CachePattern),
            FunctionExpKind::With => Pattern::With(WithPattern),
            FunctionExpKind::Print => Pattern::Print(PrintPattern),
            FunctionExpKind::Panic => Pattern::Panic(PanicPattern),
            FunctionExpKind::Catch => Pattern::Catch(CatchPattern),
            FunctionExpKind::Todo => Pattern::Todo(TodoPattern),
            FunctionExpKind::Unreachable => Pattern::Unreachable(UnreachablePattern),
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
            FunctionExpKind::Todo,
            FunctionExpKind::Unreachable,
        ]
        .into_iter()
    }

    /// Get the number of registered patterns.
    pub fn len(&self) -> usize {
        11
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
        assert_eq!(registry.len(), 11);

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
        let _ = registry.get(FunctionExpKind::Todo);
        let _ = registry.get(FunctionExpKind::Unreachable);
    }

    #[test]
    fn test_pattern_names() {
        let registry = PatternRegistry::new();

        assert_eq!(registry.get(FunctionExpKind::Recurse).name(), "recurse");
        assert_eq!(registry.get(FunctionExpKind::Parallel).name(), "parallel");
        assert_eq!(registry.get(FunctionExpKind::Timeout).name(), "timeout");
        assert_eq!(registry.get(FunctionExpKind::Print).name(), "print");
        assert_eq!(registry.get(FunctionExpKind::Panic).name(), "panic");
        assert_eq!(registry.get(FunctionExpKind::Todo).name(), "todo");
        assert_eq!(
            registry.get(FunctionExpKind::Unreachable).name(),
            "unreachable"
        );
    }

    #[test]
    fn test_required_props() {
        let registry = PatternRegistry::new();

        let timeout = registry.get(FunctionExpKind::Timeout);
        assert!(timeout.required_props().contains(&"operation"));
        assert!(timeout.required_props().contains(&"after"));

        let print = registry.get(FunctionExpKind::Print);
        assert!(print.required_props().contains(&"msg"));

        // todo and unreachable have no required props (reason is optional)
        let todo = registry.get(FunctionExpKind::Todo);
        assert!(todo.required_props().is_empty());

        let unreachable = registry.get(FunctionExpKind::Unreachable);
        assert!(unreachable.required_props().is_empty());
    }

    #[test]
    fn test_kinds_iterator() {
        let registry = PatternRegistry::new();
        let kinds: Vec<_> = registry.kinds().collect();
        assert_eq!(kinds.len(), 11);
        assert!(kinds.contains(&FunctionExpKind::Recurse));
        assert!(kinds.contains(&FunctionExpKind::Parallel));
        assert!(kinds.contains(&FunctionExpKind::Spawn));
        assert!(kinds.contains(&FunctionExpKind::Timeout));
        assert!(kinds.contains(&FunctionExpKind::Cache));
        assert!(kinds.contains(&FunctionExpKind::With));
        assert!(kinds.contains(&FunctionExpKind::Print));
        assert!(kinds.contains(&FunctionExpKind::Panic));
        assert!(kinds.contains(&FunctionExpKind::Catch));
        assert!(kinds.contains(&FunctionExpKind::Todo));
        assert!(kinds.contains(&FunctionExpKind::Unreachable));
    }

    #[test]
    fn test_pattern_enum_is_copy() {
        // Compile-time assertion that Pattern implements Copy
        fn assert_copy<T: Copy>() {}
        assert_copy::<Pattern>();

        // Runtime verification: can use original after copy
        let registry = PatternRegistry::new();
        let pattern = registry.get(FunctionExpKind::Print);
        let copy = pattern;
        assert_eq!(pattern.name(), copy.name());
    }

    #[test]
    fn test_pattern_enum_size() {
        // All inner patterns are ZSTs, so the enum should just be the discriminant
        assert_eq!(std::mem::size_of::<Pattern>(), 1);
    }
}
