//! Parsed type representation for the Ori compiler.
//!
//! `ParsedType` captures the full structure of type annotations as parsed,
//! before resolution by the type checker. This enables proper handling of:
//! - Generic types like `Option<T>`, `Result<T, E>`
//! - Compound types like `[T]`, `{K: V}`, `(T, U) -> R`
//! - User-defined types
//!
//! # Design
//!
//! Unlike `TypeId` which only captures primitive types, `ParsedType` preserves
//! the full structure of type expressions. The type checker then resolves
//! `ParsedType` into the internal `Type` representation.
//!
//! # Arena Allocation
//!
//! Recursive type references use arena-allocated IDs:
//! - `List(ParsedTypeId)` — element type ID
//! - `Function { ret: ParsedTypeId }` — return type ID
//! - `Map { key: ParsedTypeId, value: ParsedTypeId }` — key and value type IDs
//! - `AssociatedType { base: ParsedTypeId }` — base type ID
//! - `Named { type_args: ParsedTypeRange }` — type argument IDs
//! - `Tuple(ParsedTypeRange)` — element type IDs
//! - `Function { params: ParsedTypeRange }` — parameter type IDs
//!
//! This design eliminates per-type heap allocations, improves cache locality,
//! and enables bulk deallocation when the arena is dropped.
//!
//! To construct recursive types, use `ExprArena::alloc_parsed_type()` and
//! `ExprArena::alloc_parsed_type_list()`.
//!
//! # Salsa Compatibility
//!
//! All types derive Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{ExprId, Name, ParsedTypeId, ParsedTypeRange, TypeId};

/// A parsed type expression, preserving full structure.
///
/// This is used in AST nodes where type annotations appear:
/// - Parameter types: `(x: int)` → `Primitive(TypeId::INT)`
/// - Return types: `-> Option<str>` → `Named { name: "Option", type_args: [int_id] }`
/// - Field types: `name: str` → `Primitive(TypeId::STR)`
///
/// The type checker resolves these into the internal `Type` representation.
///
/// # Construction
///
/// Simple types can be constructed directly:
/// ```text
/// ParsedType::Primitive(TypeId::INT)
/// ParsedType::Infer
/// ParsedType::SelfType
/// ```
///
/// Recursive types require arena allocation:
/// ```text
/// // For [int]:
/// let elem_id = arena.alloc_parsed_type(ParsedType::Primitive(TypeId::INT));
/// let list_ty = ParsedType::List(elem_id);
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParsedType {
    /// A primitive type: int, float, bool, str, char, byte, void, Never
    Primitive(TypeId),

    /// A named type with optional type arguments.
    /// Examples: `MyType`, `Option<int>`, `Result<T, E>`
    Named {
        /// The type name (interned).
        name: Name,
        /// Generic type arguments (range into arena), empty if non-generic.
        ///
        /// May contain both type arguments (`ParsedType::Named`, `Primitive`, etc.)
        /// and const arguments (`ParsedType::ConstExpr`) — consumers discriminate
        /// by variant. Example: `Array<int, $N>` has `[Named("int"), ConstExpr($N)]`.
        type_args: ParsedTypeRange,
    },

    /// A list type: `[T]`
    List(ParsedTypeId),

    /// A fixed-capacity list type: `[T, max N]`
    FixedList {
        /// Element type ID.
        elem: ParsedTypeId,
        /// Maximum capacity as a const expression (literal `42` or generic `$N`).
        capacity: ExprId,
    },

    /// A tuple type: `(T, U)` or unit `()`
    Tuple(ParsedTypeRange),

    /// A function type: `(T, U) -> R`
    Function {
        /// Parameter types (range into arena).
        params: ParsedTypeRange,
        /// Return type ID.
        ret: ParsedTypeId,
    },

    /// A map type: `{K: V}`
    Map {
        /// Key type ID.
        key: ParsedTypeId,
        /// Value type ID.
        value: ParsedTypeId,
    },

    /// Type inference marker (used internally).
    Infer,

    /// The `Self` type in trait/impl contexts.
    SelfType,

    /// An associated type projection: `Self.Item` or `T.Item`
    /// Represents a type accessed via `.` on another type.
    AssociatedType {
        /// The base type ID (e.g., `Self` or a type variable).
        base: ParsedTypeId,
        /// The associated type name (e.g., `Item`).
        assoc_name: Name,
    },

    /// A const expression in a type position: `$N`, `$N + 1`, `42`.
    /// Used in generic type arguments: `Array<int, $N>`, `Buffer<T, $N + 1>`.
    ConstExpr(ExprId),

    /// Bounded trait object: `Printable + Hashable`.
    /// Each element in the range is a trait type (typically `Named`).
    /// Requires at least two bounds (single bound is just a `Named` type).
    TraitBounds(ParsedTypeRange),
}

impl ParsedType {
    /// Create a primitive type.
    #[inline]
    pub fn primitive(id: TypeId) -> Self {
        ParsedType::Primitive(id)
    }

    /// Create a named type without type arguments.
    #[inline]
    pub fn named(name: Name) -> Self {
        ParsedType::Named {
            name,
            type_args: ParsedTypeRange::EMPTY,
        }
    }

    /// Create a named type with type arguments (already allocated in arena).
    #[inline]
    pub fn named_with_args(name: Name, type_args: ParsedTypeRange) -> Self {
        ParsedType::Named { name, type_args }
    }

    /// Create a list type with element type ID (already allocated in arena).
    #[inline]
    pub fn list(elem: ParsedTypeId) -> Self {
        ParsedType::List(elem)
    }

    /// Create a fixed-capacity list type with element type ID and capacity expression.
    #[inline]
    pub fn fixed_list(elem: ParsedTypeId, capacity: ExprId) -> Self {
        ParsedType::FixedList { elem, capacity }
    }

    /// Create a tuple type with element type IDs (already allocated in arena).
    #[inline]
    pub fn tuple(elems: ParsedTypeRange) -> Self {
        ParsedType::Tuple(elems)
    }

    /// Create a unit type (empty tuple).
    #[inline]
    pub fn unit() -> Self {
        ParsedType::Tuple(ParsedTypeRange::EMPTY)
    }

    /// Create a function type with param and return type IDs (already allocated in arena).
    #[inline]
    pub fn function(params: ParsedTypeRange, ret: ParsedTypeId) -> Self {
        ParsedType::Function { params, ret }
    }

    /// Create a map type with key and value type IDs (already allocated in arena).
    #[inline]
    pub fn map(key: ParsedTypeId, value: ParsedTypeId) -> Self {
        ParsedType::Map { key, value }
    }

    /// Create an associated type projection with base type ID (already allocated in arena).
    #[inline]
    pub fn associated_type(base: ParsedTypeId, assoc_name: Name) -> Self {
        ParsedType::AssociatedType { base, assoc_name }
    }

    /// Create a const expression in a type position.
    #[inline]
    pub fn const_expr(expr: ExprId) -> Self {
        ParsedType::ConstExpr(expr)
    }

    /// Check if this is the Infer marker.
    #[inline]
    pub fn is_infer(&self) -> bool {
        matches!(self, ParsedType::Infer)
    }

    /// Check if this is a primitive type.
    #[inline]
    pub fn is_primitive(&self) -> bool {
        matches!(self, ParsedType::Primitive(_))
    }

    /// Check if this is a function type.
    #[inline]
    pub fn is_function(&self) -> bool {
        matches!(self, ParsedType::Function { .. })
    }

    /// Check if this is a const expression in a type position.
    #[inline]
    pub fn is_const_expr(&self) -> bool {
        matches!(self, ParsedType::ConstExpr(_))
    }

    /// Create a bounded trait object type with bounds (already allocated in arena).
    #[inline]
    pub fn trait_bounds(bounds: ParsedTypeRange) -> Self {
        ParsedType::TraitBounds(bounds)
    }

    /// Check if this is a bounded trait object.
    #[inline]
    pub fn is_trait_bounds(&self) -> bool {
        matches!(self, ParsedType::TraitBounds(_))
    }
}

#[cfg(test)]
mod tests;
