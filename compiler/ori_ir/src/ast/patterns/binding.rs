//! Binding and Match Patterns
//!
//! Patterns for destructuring in let expressions and match expressions.
//!
//! # Arena Allocation
//!
//! `MatchPattern` uses arena allocation via `MatchPatternId` and `MatchPatternRange`
//! instead of `Box<T>` and `Vec<T>`. This provides:
//! - Better cache locality (patterns stored contiguously)
//! - Reduced allocation overhead (one arena vs many small allocations)
//! - Efficient equality/hashing for Salsa queries
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{ExprId, MatchPatternId, MatchPatternRange, Name, Span, Spanned};

/// Binding pattern for let expressions.
///
/// Per spec (§05-variables.md): `$` prefix marks individual bindings as immutable.
/// `mutable` defaults to `true`; `$` sets it to `false`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum BindingPattern {
    /// Simple name binding: `let x = ...` (mutable) or `let $x = ...` (immutable).
    Name { name: Name, mutable: bool },
    /// Tuple destructuring: `let (a, b) = ...`
    Tuple(Vec<BindingPattern>),
    /// Struct destructuring: `let { x, y } = ...`
    Struct { fields: Vec<FieldBinding> },
    /// List destructuring: `let [head, ..tail] = ...`
    List {
        elements: Vec<BindingPattern>,
        rest: Option<Name>,
    },
    /// Wildcard: `let _ = ...`
    Wildcard,
}

/// A single field binding in a struct destructuring pattern.
///
/// Handles both shorthand `{ x }` / `{ $x }` and explicit `{ x: px }` / `{ x: $px }`.
/// Per grammar: `field_binding = [ "$" ] identifier [ ":" binding_pattern ]`.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FieldBinding {
    /// The struct field name being destructured.
    pub name: Name,
    /// Whether the shorthand binding is mutable (`true` = no `$`, `false` = `$` prefix).
    /// Only meaningful when `pattern` is `None` (shorthand form).
    /// When `pattern` is `Some(...)`, mutability is tracked on the sub-pattern.
    pub mutable: bool,
    /// Optional explicit binding pattern. `None` means shorthand: `{ x }` binds field `x`
    /// to variable `x`.
    pub pattern: Option<BindingPattern>,
}

/// Match pattern for match expressions.
///
/// # Arena Allocation
///
/// Nested patterns are stored via arena allocation using `MatchPatternId`
/// (single patterns) and `MatchPatternRange` (lists of patterns). This replaces
/// the previous `Box<MatchPattern>` and `Vec<MatchPattern>` approach.
///
/// To create or access nested patterns, use:
/// - `arena.alloc_match_pattern(pattern)` → `MatchPatternId`
/// - `arena.get_match_pattern(id)` → `&MatchPattern`
/// - `arena.alloc_match_pattern_list(patterns)` → `MatchPatternRange`
/// - `arena.get_match_pattern_list(range)` → `&[MatchPatternId]`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum MatchPattern {
    /// Wildcard: _
    Wildcard,
    /// Binding: x
    Binding(Name),
    /// Literal: 42, "hello", true
    Literal(ExprId),
    /// Variant: Some(x), Ok(value), Click(x, y)
    ///
    /// For unit variants, `inner` is empty.
    /// For single-field variants, `inner` has one element.
    /// For multi-field variants, `inner` has multiple elements.
    Variant {
        name: Name,
        inner: MatchPatternRange,
    },
    /// Struct: `{ x, y }` or `{ x, .. }` (with rest).
    ///
    /// Uses `Vec` for fields because each field is a tuple `(Name, Option<MatchPatternId>)`,
    /// and flattening this would lose the name-pattern association.
    /// When `rest` is true, the pattern matches structs with additional fields
    /// beyond those explicitly listed (the `..` syntax).
    Struct {
        fields: Vec<(Name, Option<MatchPatternId>)>,
        rest: bool,
    },
    /// Tuple: (a, b)
    Tuple(MatchPatternRange),
    /// List: [a, b, ..rest]
    List {
        elements: MatchPatternRange,
        rest: Option<Name>,
    },
    /// Range: 1..10
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },
    /// Or pattern: A | B
    Or(MatchPatternRange),
    /// At pattern: x @ Some(_)
    At { name: Name, pattern: MatchPatternId },
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
