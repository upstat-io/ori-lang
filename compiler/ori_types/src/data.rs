//! Internal type representation for the type interner.
//!
//! `TypeData` is the internal representation stored in the `TypeInterner`.
//! External code works with `TypeId` (u32 indices) for O(1) equality.

use ori_ir::{Name, TypeId};

/// Type variable for inference (also stored in `TypeData`).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeVar(pub u32);

impl TypeVar {
    /// Create a new type variable.
    pub fn new(id: u32) -> Self {
        TypeVar(id)
    }
}

/// Internal type representation stored in the interner.
///
/// Unlike the external `Type` enum which uses `Box<Type>` for recursive types,
/// `TypeData` uses `TypeId` for children, enabling O(1) type equality.
///
/// # Design
///
/// - Primitives are pre-interned with fixed `TypeId` values
/// - Compound types store `TypeId` children, not Box<Type>
/// - Type variables use TypeVar(u32) for inference
/// - Error is a special sentinel for error recovery
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeData {
    // Primitives (pre-interned at fixed indices)
    /// Integer type (64-bit signed)
    Int,
    /// Floating point type (64-bit IEEE 754)
    Float,
    /// Boolean type
    Bool,
    /// String type (UTF-8)
    Str,
    /// Character type (Unicode codepoint)
    Char,
    /// Byte type (u8)
    Byte,
    /// Unit type ()
    Unit,
    /// Never type (diverging)
    Never,
    /// Duration type (30s, 100ms)
    Duration,
    /// Size type (4kb, 10mb)
    Size,
    /// Ordering type (Less | Equal | Greater)
    Ordering,

    // Compound types with TypeId children
    /// Function type: (params) -> return
    Function {
        /// Parameter types
        params: Box<[TypeId]>,
        /// Return type
        ret: TypeId,
    },

    /// Tuple type: (T, U, V)
    Tuple(Box<[TypeId]>),

    /// List type: [T]
    List(TypeId),

    /// Map type: {K: V}
    Map {
        /// Key type
        key: TypeId,
        /// Value type
        value: TypeId,
    },

    /// Set type: Set<T>
    Set(TypeId),

    /// Option type: Option<T>
    Option(TypeId),

    /// Result type: Result<T, E>
    Result {
        /// Success type
        ok: TypeId,
        /// Error type
        err: TypeId,
    },

    /// Range type: Range<T>
    Range(TypeId),

    /// Channel type: Channel<T>
    Channel(TypeId),

    // Named and generic types
    /// User-defined type reference (non-generic or unapplied generic)
    Named(Name),

    /// Applied generic type: the base type name with concrete type arguments.
    /// For example, `Box<int>` is `Applied { name: "Box", args: [INT] }`.
    Applied {
        /// The generic type name
        name: Name,
        /// The type arguments
        args: Box<[TypeId]>,
    },

    // Inference and error
    /// Type variable for inference
    Var(TypeVar),

    /// Error type (for error recovery)
    Error,

    // Associated types
    /// Associated type projection (e.g., `Self.Item`, `T.Item`).
    Projection {
        /// The base type (e.g., Self, or a type variable)
        base: TypeId,
        /// The trait that defines the associated type
        trait_name: Name,
        /// The associated type name (e.g., Item)
        assoc_name: Name,
    },

    // Module namespaces
    /// Module namespace type: created by module alias imports like `use std.http as http`.
    /// Contains a mapping from exported item names to their types.
    ///
    /// # Invariant
    ///
    /// Items **must** be sorted by `Name` (ascending order) to enable O(log n) lookup.
    ModuleNamespace {
        /// Mapping from exported item names to their types (as `TypeId`s).
        /// **Invariant:** Sorted by `Name` in ascending order.
        items: Box<[(Name, TypeId)]>,
    },
}

impl TypeData {
    /// Check if this is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            TypeData::Int
                | TypeData::Float
                | TypeData::Bool
                | TypeData::Str
                | TypeData::Char
                | TypeData::Byte
                | TypeData::Unit
                | TypeData::Never
                | TypeData::Duration
                | TypeData::Size
                | TypeData::Ordering
        )
    }

    /// Check if this is the error type.
    pub fn is_error(&self) -> bool {
        matches!(self, TypeData::Error)
    }

    /// Check if this is a type variable.
    pub fn is_var(&self) -> bool {
        matches!(self, TypeData::Var(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_check() {
        assert!(TypeData::Int.is_primitive());
        assert!(TypeData::Float.is_primitive());
        assert!(TypeData::Duration.is_primitive());
        assert!(!TypeData::List(TypeId::INT).is_primitive());
        assert!(!TypeData::Error.is_primitive());
    }

    #[test]
    fn test_error_check() {
        assert!(TypeData::Error.is_error());
        assert!(!TypeData::Int.is_error());
    }

    #[test]
    fn test_var_check() {
        assert!(TypeData::Var(TypeVar::new(0)).is_var());
        assert!(!TypeData::Int.is_var());
    }

    #[test]
    fn test_typedata_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(TypeData::Int);
        set.insert(TypeData::Int); // duplicate
        set.insert(TypeData::Bool);

        assert_eq!(set.len(), 2);
    }
}
