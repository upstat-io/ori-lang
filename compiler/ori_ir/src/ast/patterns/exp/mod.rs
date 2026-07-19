//! Named Expression Constructs (`function_exp`)
//!
//! Contains patterns like recurse, parallel, spawn, timeout, cache, with.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{ExprId, Name, ParsedTypeRange, Span, Spanned};

use super::super::ranges::NamedExprRange;

/// Named expression for `function_exp`.
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

/// Kind of `function_exp`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionExpKind {
    // Compiler patterns (require special syntax or static analysis)
    Recurse,
    Parallel,
    Spawn,
    Timeout,
    Cache,
    With,
    // Fundamental built-ins (I/O, control flow, error recovery)
    Print,
    Panic,
    Catch,
    // Developer convenience (diverge with diagnostics)
    Todo,
    Unreachable,
    // Channel constructors (parsed from identifier, not lexer keywords)
    Channel,
    ChannelIn,
    ChannelOut,
    ChannelAll,
}

impl FunctionExpKind {
    /// Every `function_exp` kind, in declaration order. The canonical inventory:
    /// consumers (e.g. `PatternRegistry::kinds`) derive from this rather than
    /// re-listing the variants.
    pub const ALL: [Self; 15] = [
        Self::Recurse,
        Self::Parallel,
        Self::Spawn,
        Self::Timeout,
        Self::Cache,
        Self::With,
        Self::Print,
        Self::Panic,
        Self::Catch,
        Self::Todo,
        Self::Unreachable,
        Self::Channel,
        Self::ChannelIn,
        Self::ChannelOut,
        Self::ChannelAll,
    ];

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
            FunctionExpKind::Catch => "catch",
            FunctionExpKind::Todo => "todo",
            FunctionExpKind::Unreachable => "unreachable",
            FunctionExpKind::Channel => "channel",
            FunctionExpKind::ChannelIn => "channel_in",
            FunctionExpKind::ChannelOut => "channel_out",
            FunctionExpKind::ChannelAll => "channel_all",
        }
    }
}

/// Named expression construct (`function_exp`).
///
/// Contains named expressions (`name: value`).
/// Requires named property syntax - positional not allowed.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionExp {
    pub kind: FunctionExpKind,
    pub props: NamedExprRange,
    /// Optional generic type arguments (e.g., `channel<int>(...)`).
    /// `ParsedTypeRange::EMPTY` when no type args are present.
    pub type_args: ParsedTypeRange,
    pub span: Span,
}

impl Spanned for FunctionExp {
    fn span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests;
