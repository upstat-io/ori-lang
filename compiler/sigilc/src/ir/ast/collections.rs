//! Collection Literal Types
//!
//! Map entries, field initializers, and call arguments.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

use crate::ir::{Name, Span, ExprId, Spanned};

/// Map entry in a map literal.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MapEntry {
    pub key: ExprId,
    pub value: ExprId,
    pub span: Span,
}

impl Spanned for MapEntry {
    fn span(&self) -> Span {
        self.span
    }
}

/// Field initializer in a struct literal.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FieldInit {
    pub name: Name,
    pub value: Option<ExprId>,
    pub span: Span,
}

impl Spanned for FieldInit {
    fn span(&self) -> Span {
        self.span
    }
}

/// Named argument for function calls.
///
/// Single-param functions can use positional (name is None).
/// Multi-param functions require named arguments.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct CallArg {
    pub name: Option<Name>,
    pub value: ExprId,
    pub span: Span,
}

impl Spanned for CallArg {
    fn span(&self) -> Span {
        self.span
    }
}
