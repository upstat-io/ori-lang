//! Statement Types
//!
//! Statement node and variants for block expressions.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use std::fmt;

use super::patterns::BindingPattern;
use crate::{ExprId, ParsedType, Span, Spanned};

/// Statement node.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Stmt { kind, span }
    }
}

impl fmt::Debug for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {:?}", self.kind, self.span)
    }
}

impl Spanned for Stmt {
    fn span(&self) -> Span {
        self.span
    }
}

/// Statement kinds.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum StmtKind {
    /// Expression statement
    Expr(ExprId),

    /// Let binding (also available as expression)
    Let {
        pattern: BindingPattern,
        ty: Option<ParsedType>,
        init: ExprId,
        mutable: bool,
    },
}
