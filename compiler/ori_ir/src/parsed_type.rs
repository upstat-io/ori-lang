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
//! # Salsa Compatibility
//!
//! All types derive Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use crate::{Name, TypeId};

/// A parsed type expression, preserving full structure.
///
/// This is used in AST nodes where type annotations appear:
/// - Parameter types: `(x: int)` → `Primitive(TypeId::INT)`
/// - Return types: `-> Option<str>` → `Named { name: "Option", type_args: [Primitive(STR)] }`
/// - Field types: `name: str` → `Primitive(TypeId::STR)`
///
/// The type checker resolves these into the internal `Type` representation.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParsedType {
    /// A primitive type: int, float, bool, str, char, byte, void, Never
    Primitive(TypeId),

    /// A named type with optional type arguments.
    /// Examples: `MyType`, `Option<int>`, `Result<T, E>`
    Named {
        /// The type name (interned).
        name: Name,
        /// Generic type arguments, empty if non-generic.
        type_args: Vec<ParsedType>,
    },

    /// A list type: `[T]`
    List(Box<ParsedType>),

    /// A tuple type: `(T, U)` or unit `()`
    Tuple(Vec<ParsedType>),

    /// A function type: `(T, U) -> R`
    Function {
        /// Parameter types.
        params: Vec<ParsedType>,
        /// Return type.
        ret: Box<ParsedType>,
    },

    /// A map type: `{K: V}`
    Map {
        /// Key type.
        key: Box<ParsedType>,
        /// Value type.
        value: Box<ParsedType>,
    },

    /// Type inference marker (used internally).
    Infer,

    /// The `Self` type in trait/impl contexts.
    SelfType,

    /// An associated type projection: `Self.Item` or `T.Item`
    /// Represents a type accessed via `.` on another type.
    AssociatedType {
        /// The base type (e.g., `Self` or a type variable).
        base: Box<ParsedType>,
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
            type_args: Vec::new(),
        }
    }

    /// Create a named type with type arguments.
    #[inline]
    pub fn named_with_args(name: Name, type_args: Vec<ParsedType>) -> Self {
        ParsedType::Named { name, type_args }
    }

    /// Create a list type.
    #[inline]
    pub fn list(elem: ParsedType) -> Self {
        ParsedType::List(Box::new(elem))
    }

    /// Create a tuple type.
    #[inline]
    pub fn tuple(elems: Vec<ParsedType>) -> Self {
        ParsedType::Tuple(elems)
    }

    /// Create a unit type (empty tuple).
    #[inline]
    pub fn unit() -> Self {
        ParsedType::Tuple(Vec::new())
    }

    /// Create a function type.
    #[inline]
    pub fn function(params: Vec<ParsedType>, ret: ParsedType) -> Self {
        ParsedType::Function {
            params,
            ret: Box::new(ret),
        }
    }

    /// Create a map type.
    #[inline]
    pub fn map(key: ParsedType, value: ParsedType) -> Self {
        ParsedType::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    /// Create an associated type projection.
    #[inline]
    pub fn associated_type(base: ParsedType, assoc_name: Name) -> Self {
        ParsedType::AssociatedType {
            base: Box::new(base),
            assoc_name,
        }
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
        let ty = ParsedType::list(ParsedType::primitive(TypeId::INT));
        match ty {
            ParsedType::List(elem) => {
                assert_eq!(*elem, ParsedType::primitive(TypeId::INT));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_function() {
        let ty = ParsedType::function(
            vec![ParsedType::primitive(TypeId::INT)],
            ParsedType::primitive(TypeId::BOOL),
        );
        assert!(ty.is_function());
        match ty {
            ParsedType::Function { params, ret } => {
                assert_eq!(params.len(), 1);
                assert_eq!(*ret, ParsedType::primitive(TypeId::BOOL));
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
        let name_option = Name::new(0, 1);
        let name_result = Name::new(0, 2);

        let inner = ParsedType::named_with_args(
            name_result,
            vec![
                ParsedType::primitive(TypeId::INT),
                ParsedType::primitive(TypeId::STR),
            ],
        );

        let ty = ParsedType::named_with_args(name_option, vec![inner]);

        match ty {
            ParsedType::Named { name, type_args } => {
                assert_eq!(name, name_option);
                assert_eq!(type_args.len(), 1);
                match &type_args[0] {
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
}
