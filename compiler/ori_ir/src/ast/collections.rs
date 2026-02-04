//! Collection Literal Types
//!
//! Map entries, field initializers, and call arguments.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{ExprId, Name, Span, Spanned};

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

/// An element in a struct literal, either a field or a spread.
///
/// Supports both regular field initialization and spread syntax:
/// - `Point { x: 1, y: 2 }` - regular fields
/// - `Point { ...base }` - spread an existing struct
/// - `Point { ...base, x: 10 }` - spread with overrides (later wins)
///
/// Named `StructLitField` to distinguish from `StructField` (type definitions).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum StructLitField {
    /// A regular field initializer: `field: value` or `field` (shorthand).
    Field(FieldInit),
    /// A spread expression: `...expr` copies fields from another struct.
    Spread {
        /// The expression to spread (must be same struct type).
        expr: ExprId,
        /// Source span of the spread including `...`.
        span: Span,
    },
}

impl Spanned for StructLitField {
    fn span(&self) -> Span {
        match self {
            StructLitField::Field(init) => init.span,
            StructLitField::Spread { span, .. } => *span,
        }
    }
}

/// An element in a list literal, either a value or a spread.
///
/// Supports both regular values and spread syntax:
/// - `[1, 2, 3]` - regular values
/// - `[...a, 4, ...b]` - spread with other values
///
/// Spread elements are expanded at runtime, concatenating the spread
/// expression's elements into the resulting list.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ListElement {
    /// A regular expression: `expr`.
    Expr {
        /// The expression value.
        expr: ExprId,
        /// Source span of the element.
        span: Span,
    },
    /// A spread expression: `...expr` expands an iterable.
    Spread {
        /// The expression to spread (must be iterable with same element type).
        expr: ExprId,
        /// Source span including the `...`.
        span: Span,
    },
}

impl Spanned for ListElement {
    fn span(&self) -> Span {
        match self {
            ListElement::Expr { span, .. } | ListElement::Spread { span, .. } => *span,
        }
    }
}

/// An element in a map literal, either an entry or a spread.
///
/// Supports both regular entries and spread syntax:
/// - `{"a": 1, "b": 2}` - regular entries
/// - `{...base, "c": 3}` - spread with other entries
///
/// Spread elements are expanded at runtime, merging the spread
/// expression's entries into the resulting map. Later values win.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum MapElement {
    /// A regular entry: `key: value`.
    Entry(MapEntry),
    /// A spread expression: `...expr` copies entries from another map.
    Spread {
        /// The expression to spread (must be map with compatible types).
        expr: ExprId,
        /// Source span including the `...`.
        span: Span,
    },
}

impl Spanned for MapElement {
    fn span(&self) -> Span {
        match self {
            MapElement::Entry(entry) => entry.span,
            MapElement::Spread { span, .. } => *span,
        }
    }
}

/// Named argument for function calls.
///
/// Single-param functions can use positional (name is None).
/// Multi-param functions require named arguments.
/// Spread syntax `...expr` expands an iterable into multiple arguments.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct CallArg {
    pub name: Option<Name>,
    pub value: ExprId,
    /// If true, this argument is a spread: `...expr`.
    /// The value will be expanded into multiple arguments at runtime.
    pub is_spread: bool,
    pub span: Span,
}

impl Spanned for CallArg {
    fn span(&self) -> Span {
        self.span
    }
}
