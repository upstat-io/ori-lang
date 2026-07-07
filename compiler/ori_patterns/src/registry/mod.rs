//! Pattern registry for looking up pattern definitions by kind.

use ori_ir::FunctionExpKind;

use crate::builtins::{CatchPattern, PanicPattern, PrintPattern, TodoPattern, UnreachablePattern};
use crate::cache::CachePattern;
use crate::channel::ChannelPattern;
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
    Channel(ChannelPattern),
}

/// Forward a `PatternDefinition` method call to the wrapped concrete pattern,
/// matching every `Pattern` variant. Adding a variant requires one new arm here
/// rather than an arm in each forwarding method.
macro_rules! dispatch_to_variant {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        match $self {
            Pattern::Recurse(p) => p.$method($($arg),*),
            Pattern::Parallel(p) => p.$method($($arg),*),
            Pattern::Spawn(p) => p.$method($($arg),*),
            Pattern::Timeout(p) => p.$method($($arg),*),
            Pattern::Cache(p) => p.$method($($arg),*),
            Pattern::With(p) => p.$method($($arg),*),
            Pattern::Print(p) => p.$method($($arg),*),
            Pattern::Panic(p) => p.$method($($arg),*),
            Pattern::Catch(p) => p.$method($($arg),*),
            Pattern::Todo(p) => p.$method($($arg),*),
            Pattern::Unreachable(p) => p.$method($($arg),*),
            Pattern::Channel(p) => p.$method($($arg),*),
        }
    };
}

impl PatternDefinition for Pattern {
    fn name(&self) -> &'static str {
        dispatch_to_variant!(self, name)
    }

    fn required_props(&self) -> &'static [&'static str] {
        dispatch_to_variant!(self, required_props)
    }

    fn optional_props(&self) -> &'static [&'static str] {
        dispatch_to_variant!(self, optional_props)
    }

    fn optional_args(&self) -> &'static [OptionalArg] {
        dispatch_to_variant!(self, optional_args)
    }

    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        dispatch_to_variant!(self, scoped_bindings)
    }

    fn allows_arbitrary_props(&self) -> bool {
        dispatch_to_variant!(self, allows_arbitrary_props)
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        dispatch_to_variant!(self, evaluate, ctx, exec)
    }

    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        dispatch_to_variant!(self, can_fuse_with, next)
    }

    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
        self_ctx: &EvalContext,
        next_ctx: &EvalContext,
    ) -> Option<FusedPattern> {
        dispatch_to_variant!(self, fuse_with, next, self_ctx, next_ctx)
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
            FunctionExpKind::Channel
            | FunctionExpKind::ChannelIn
            | FunctionExpKind::ChannelOut
            | FunctionExpKind::ChannelAll => Pattern::Channel(ChannelPattern),
        }
    }

    /// Get all registered pattern kinds.
    ///
    /// Derives from `FunctionExpKind::ALL` (the canonical inventory) so a new
    /// variant is registered here automatically rather than via a parallel list.
    pub fn kinds(&self) -> impl Iterator<Item = FunctionExpKind> {
        FunctionExpKind::ALL.into_iter()
    }

    /// Get the number of registered patterns.
    pub fn len(&self) -> usize {
        self.kinds().count()
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
mod tests;
