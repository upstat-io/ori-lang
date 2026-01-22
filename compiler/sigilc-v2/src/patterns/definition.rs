//! Pattern definition trait.
//!
//! Each pattern implements this trait to provide unified type checking,
//! evaluation, and documentation.

use crate::intern::{Name, TypeId};
use crate::syntax::{PatternArgsId, Span, ExprArena};
use super::param::ParamSpec;

/// Result type for pattern operations.
pub type PatternResult<T> = Result<T, PatternError>;

/// Error from pattern operations.
#[derive(Clone, Debug)]
pub struct PatternError {
    /// Error message.
    pub message: String,
    /// Source span where error occurred.
    pub span: Option<Span>,
}

impl PatternError {
    /// Create a new pattern error.
    pub fn new(message: impl Into<String>) -> Self {
        PatternError {
            message: message.into(),
            span: None,
        }
    }

    /// Create a pattern error with span.
    pub fn with_span(message: impl Into<String>, span: Span) -> Self {
        PatternError {
            message: message.into(),
            span: Some(span),
        }
    }
}

/// Unified interface for pattern definitions.
///
/// Each pattern (map, filter, fold, etc.) implements this trait to provide
/// all the behavior needed by the compiler in one place.
pub trait PatternDefinition: Send + Sync + 'static {
    /// The keyword that identifies this pattern (e.g., "map", "fold").
    fn keyword(&self) -> &'static str;

    /// Parameter specifications for this pattern.
    fn params(&self) -> &'static [ParamSpec];

    /// Short description of the pattern.
    fn description(&self) -> &'static str;

    /// Extended help text.
    fn help(&self) -> &'static str {
        self.description()
    }

    /// Usage examples.
    fn examples(&self) -> &'static [&'static str] {
        &[]
    }

    /// Check if this pattern requires a specific capability.
    fn required_capability(&self) -> Option<&'static str> {
        None
    }

    /// Check if this pattern can be fused with another.
    fn can_fuse_with(&self, _other: &'static str) -> bool {
        false
    }
}

/// Context for pattern type inference.
pub struct PatternTypeContext<'a> {
    /// Expression arena.
    pub arena: &'a ExprArena,
    /// Function to infer expression types.
    pub infer_fn: &'a dyn Fn(crate::syntax::ExprId) -> TypeId,
    /// Function to look up named argument.
    pub get_named_arg: &'a dyn Fn(Name) -> Option<crate::syntax::ExprId>,
}

/// Context for pattern evaluation.
pub struct PatternEvalContext<'a> {
    /// Expression arena.
    pub arena: &'a ExprArena,
    /// Function to evaluate an expression.
    pub eval_fn: &'a dyn Fn(crate::syntax::ExprId) -> crate::eval::Value,
    /// Function to look up named argument.
    pub get_named_arg: &'a dyn Fn(Name) -> Option<crate::syntax::ExprId>,
}
