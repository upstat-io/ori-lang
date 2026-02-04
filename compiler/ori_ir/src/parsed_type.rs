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

use crate::{Name, ParsedTypeId, ParsedTypeRange, TypeId};

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
        type_args: ParsedTypeRange,
    },

    /// A list type: `[T]`
    List(ParsedTypeId),

    /// A fixed-capacity list type: `[T, max N]`
    FixedList {
        /// Element type ID.
        elem: ParsedTypeId,
        /// Maximum capacity as a parsed integer literal.
        capacity: u64,
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

    /// Create a fixed-capacity list type with element type ID and capacity.
    #[inline]
    pub fn fixed_list(elem: ParsedTypeId, capacity: u64) -> Self {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExprArena;

    #[test]
    fn test_primitive() {
        let ty = ParsedType::primitive(TypeId::INT);
        assert!(ty.is_primitive());
        assert!(!ty.is_function());
    }

    #[test]
    fn test_named() {
        let name = Name::new(0, 1); // dummy name
        let ty = ParsedType::named(name);
        assert!(!ty.is_primitive());
        match ty {
            ParsedType::Named { name: n, type_args } => {
                assert_eq!(n, name);
                assert!(type_args.is_empty());
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_list() {
        let mut arena = ExprArena::new();
        let elem_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
        let ty = ParsedType::list(elem_id);
        match ty {
            ParsedType::List(id) => {
                assert_eq!(
                    *arena.get_parsed_type(id),
                    ParsedType::primitive(TypeId::INT)
                );
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_function() {
        let mut arena = ExprArena::new();
        let param_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
        let ret_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::BOOL));
        let params = arena.alloc_parsed_type_list([param_id]);
        let ty = ParsedType::function(params, ret_id);
        assert!(ty.is_function());
        match ty {
            ParsedType::Function { params, ret } => {
                assert_eq!(params.len(), 1);
                assert_eq!(
                    *arena.get_parsed_type(ret),
                    ParsedType::primitive(TypeId::BOOL)
                );
            }
            _ => panic!("expected Function"),
        }
    }

    #[test]
    fn test_unit() {
        let ty = ParsedType::unit();
        match ty {
            ParsedType::Tuple(elems) => {
                assert!(elems.is_empty());
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn test_equality() {
        let ty1 = ParsedType::primitive(TypeId::INT);
        let ty2 = ParsedType::primitive(TypeId::INT);
        let ty3 = ParsedType::primitive(TypeId::FLOAT);

        assert_eq!(ty1, ty2);
        assert_ne!(ty1, ty3);
    }

    #[test]
    fn test_nested_generic() {
        // Option<Result<int, str>>
        let mut arena = ExprArena::new();
        let name_option = Name::new(0, 1);
        let name_result = Name::new(0, 2);

        // Create inner type: Result<int, str>
        let int_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
        let str_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::STR));
        let inner_args = arena.alloc_parsed_type_list([int_id, str_id]);
        let inner = ParsedType::named_with_args(name_result, inner_args);

        // Create outer type: Option<Result<int, str>>
        let inner_id = arena.alloc_parsed_type(inner);
        let outer_args = arena.alloc_parsed_type_list([inner_id]);
        let ty = ParsedType::named_with_args(name_option, outer_args);

        match ty {
            ParsedType::Named { name, type_args } => {
                assert_eq!(name, name_option);
                assert_eq!(type_args.len(), 1);
                let inner_ids = arena.get_parsed_type_list(type_args);
                match arena.get_parsed_type(inner_ids[0]) {
                    ParsedType::Named {
                        name,
                        type_args: inner_args,
                    } => {
                        assert_eq!(*name, name_result);
                        assert_eq!(inner_args.len(), 2);
                    }
                    _ => panic!("expected Named"),
                }
            }
            _ => panic!("expected Named"),
        }
    }

    #[test]
    fn test_map_type() {
        let mut arena = ExprArena::new();
        let key_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::STR));
        let value_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
        let ty = ParsedType::map(key_id, value_id);
        match ty {
            ParsedType::Map { key, value } => {
                assert_eq!(
                    *arena.get_parsed_type(key),
                    ParsedType::primitive(TypeId::STR)
                );
                assert_eq!(
                    *arena.get_parsed_type(value),
                    ParsedType::primitive(TypeId::INT)
                );
            }
            _ => panic!("expected Map"),
        }
    }

    #[test]
    fn test_associated_type() {
        let mut arena = ExprArena::new();
        let base_id = arena.alloc_parsed_type(ParsedType::SelfType);
        let assoc_name = Name::new(0, 5);
        let ty = ParsedType::associated_type(base_id, assoc_name);
        match ty {
            ParsedType::AssociatedType {
                base,
                assoc_name: name,
            } => {
                assert_eq!(*arena.get_parsed_type(base), ParsedType::SelfType);
                assert_eq!(name, assoc_name);
            }
            _ => panic!("expected AssociatedType"),
        }
    }

    #[test]
    fn test_tuple_type() {
        let mut arena = ExprArena::new();
        let int_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::INT));
        let bool_id = arena.alloc_parsed_type(ParsedType::primitive(TypeId::BOOL));
        let elems = arena.alloc_parsed_type_list([int_id, bool_id]);
        let ty = ParsedType::tuple(elems);
        match ty {
            ParsedType::Tuple(range) => {
                assert_eq!(range.len(), 2);
                let ids = arena.get_parsed_type_list(range);
                assert_eq!(
                    *arena.get_parsed_type(ids[0]),
                    ParsedType::primitive(TypeId::INT)
                );
                assert_eq!(
                    *arena.get_parsed_type(ids[1]),
                    ParsedType::primitive(TypeId::BOOL)
                );
            }
            _ => panic!("expected Tuple"),
        }
    }
}
