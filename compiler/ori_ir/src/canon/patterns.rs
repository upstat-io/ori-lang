//! Canonical binding patterns, parameters, and named expressions.
//!
//! These types represent the sugar-free equivalents of parse-level patterns
//! and parameters, storing sub-patterns via [`CanArena`](super::CanArena)
//! indices instead of `Vec` allocations.

use std::fmt;

use crate::Name;

use super::ids::{CanBindingPatternId, CanBindingPatternRange, CanFieldBindingRange, CanId};

/// Canonical binding pattern — self-contained, no `ExprArena` references.
///
/// Mirrors `BindingPattern` from `ori_ir::ast` but stores sub-patterns
/// in `CanArena` via `CanBindingPatternId` instead of `Vec<BindingPattern>`.
///
/// Per-binding mutability is preserved on the `Name` variant so that
/// destructuring patterns like `let ($x, y) = ...` can enforce immutability
/// per binding rather than inheriting a single flag from `CanExpr::Let.mutable`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CanBindingPattern {
    /// Simple name binding: `let x = ...` (mutable) or `let $x = ...` (immutable).
    Name { name: Name, mutable: bool },
    /// Tuple destructuring: `let (a, b) = ...`
    Tuple(CanBindingPatternRange),
    /// Struct destructuring: `let { x, y } = ...`
    Struct { fields: CanFieldBindingRange },
    /// List destructuring: `let [head, ..tail] = ...`
    List {
        elements: CanBindingPatternRange,
        rest: Option<Name>,
    },
    /// Wildcard: `let _ = ...`
    Wildcard,
}

/// A struct field binding in canonical form: field name + sub-pattern.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanFieldBinding {
    pub name: Name,
    pub pattern: CanBindingPatternId,
}

/// Canonical function parameter — only what evaluation/codegen needs.
///
/// Replaces `Param` (which contains `MatchPattern`, `ParsedType`, `ExprId`)
/// with a minimal representation: just the name and an optional default.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanParam {
    /// Parameter name.
    pub name: Name,
    /// Default value expression. `CanId::INVALID` if no default.
    pub default: CanId,
}

/// Range of canonical parameters in `CanArena.params`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanParamRange {
    pub start: u32,
    pub len: u16,
}

impl CanParamRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanParamRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanParamRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// A named expression in canonical form (for `FunctionExp` props).
///
/// Replaces `NamedExpr` which contains `ExprId` (an `ExprArena` reference)
/// with a canonical version that uses `CanId`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanNamedExpr {
    pub name: Name,
    pub value: CanId,
}

/// Range of named expressions in `CanArena.named_exprs`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanNamedExprRange {
    pub start: u32,
    pub len: u16,
}

impl CanNamedExprRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanNamedExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanNamedExprRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}
