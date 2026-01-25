//! Named Expression Constructs (function_exp)
//!
//! Contains patterns like recurse, parallel, spawn, timeout, cache, with.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

use crate::{Name, Span, ExprId, Spanned};
use super::super::ranges::NamedExprRange;

/// Named expression for function_exp.
///
/// Represents: `name: expr`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NamedExpr {
    pub name: Name,
    pub value: ExprId,
    pub span: Span,
}

impl Spanned for NamedExpr {
    fn span(&self) -> Span {
        self.span
    }
}

/// Kind of function_exp.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionExpKind {
    // Compiler patterns (require special syntax or static analysis)
    Recurse,
    Parallel,
    Spawn,
    Timeout,
    Cache,
    With,
    // Fundamental built-ins (I/O and control flow)
    Print,
    Panic,
}

impl FunctionExpKind {
    pub fn name(self) -> &'static str {
        match self {
            FunctionExpKind::Recurse => "recurse",
            FunctionExpKind::Parallel => "parallel",
            FunctionExpKind::Spawn => "spawn",
            FunctionExpKind::Timeout => "timeout",
            FunctionExpKind::Cache => "cache",
            FunctionExpKind::With => "with",
            FunctionExpKind::Print => "print",
            FunctionExpKind::Panic => "panic",
        }
    }
}

/// Named expression construct (function_exp).
///
/// Contains named expressions (`name: value`).
/// Requires named property syntax - positional not allowed.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionExp {
    pub kind: FunctionExpKind,
    pub props: NamedExprRange,
    pub span: Span,
}

impl Spanned for FunctionExp {
    fn span(&self) -> Span {
        self.span
    }
}
