//! Binding and Match Patterns
//!
//! Patterns for destructuring in let expressions and match expressions.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{ExprId, Name, Span, Spanned};

/// Binding pattern for let expressions.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum BindingPattern {
    /// Simple name binding: let x = ...
    Name(Name),
    /// Tuple destructuring: let (a, b) = ...
    Tuple(Vec<BindingPattern>),
    /// Struct destructuring: let { x, y } = ...
    Struct {
        fields: Vec<(Name, Option<BindingPattern>)>,
    },
    /// List destructuring: let [head, ..tail] = ...
    List {
        elements: Vec<BindingPattern>,
        rest: Option<Name>,
    },
    /// Wildcard: let _ = ...
    Wildcard,
}

/// Match pattern for match expressions.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum MatchPattern {
    /// Wildcard: _
    Wildcard,
    /// Binding: x
    Binding(Name),
    /// Literal: 42, "hello", true
    Literal(ExprId),
    /// Variant: Some(x), Ok(value)
    Variant {
        name: Name,
        inner: Option<Box<MatchPattern>>,
    },
    /// Struct: { x, y }
    Struct {
        fields: Vec<(Name, Option<MatchPattern>)>,
    },
    /// Tuple: (a, b)
    Tuple(Vec<MatchPattern>),
    /// List: [a, b, ..rest]
    List {
        elements: Vec<MatchPattern>,
        rest: Option<Name>,
    },
    /// Range: 1..10
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },
    /// Or pattern: A | B
    Or(Vec<MatchPattern>),
    /// At pattern: x @ Some(_)
    At {
        name: Name,
        pattern: Box<MatchPattern>,
    },
}

/// Match arm.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<ExprId>,
    pub body: ExprId,
    pub span: Span,
}

impl Spanned for MatchArm {
    fn span(&self) -> Span {
        self.span
    }
}
