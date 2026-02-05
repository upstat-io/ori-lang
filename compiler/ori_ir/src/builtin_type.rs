//! Builtin type identification enum.
//!
//! Provides a single enum that represents all built-in types in Ori, enabling
//! consistent type identification across compiler backends (typeck, eval, llvm).
//!
//! # Design
//!
//! `BuiltinType` serves as a bridge between:
//! - `TypeId` constants (used in IR and LLVM backend)
//! - `Type` enum variants (used in typeck)
//! - String names (used in error messages and debugging)
//!
//! # Usage
//!
//! ```ignore
//! use ori_ir::{BuiltinType, TypeId};
//!
//! // Convert from TypeId
//! if let Some(builtin) = BuiltinType::from_type_id(type_id) {
//!     match builtin {
//!         BuiltinType::Int => println!("It's an int!"),
//!         BuiltinType::Duration => println!("It's a duration!"),
//!         _ => {}
//!     }
//! }
//!
//! // Get display name
//! assert_eq!(BuiltinType::Duration.name(), "Duration");
//! ```

use crate::TypeId;

/// Enum representing all built-in types in Ori.
///
/// This provides a unified way to identify built-in types across all compiler
/// phases without relying on string comparisons or magic constants.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum BuiltinType {
    // Primitives
    /// 64-bit signed integer
    Int,
    /// 64-bit floating point (IEEE 754)
    Float,
    /// Boolean
    Bool,
    /// UTF-8 string
    Str,
    /// Unicode codepoint
    Char,
    /// Unsigned byte (0-255)
    Byte,
    /// Unit type `()`
    Unit,
    /// Never type (diverging)
    Never,

    // Special types
    /// Duration (nanoseconds)
    Duration,
    /// Size (bytes)
    Size,
    /// Ordering (Less | Equal | Greater)
    Ordering,

    // Generic containers (identified by structure, not TypeId)
    /// List type `[T]`
    List,
    /// Map type `{K: V}`
    Map,
    /// Option type `Option<T>`
    Option,
    /// Result type `Result<T, E>`
    Result,
    /// Range type `Range<T>`
    Range,
    /// Set type `Set<T>`
    Set,
    /// Channel type `Channel<T>`
    Channel,
}

impl BuiltinType {
    /// Convert from a `TypeId` to a `BuiltinType`.
    ///
    /// Returns `Some` for pre-interned primitive types (indices 0-11),
    /// `None` for ERROR, markers (INFER, `SELF_TYPE`), and compound types.
    ///
    /// # Note
    ///
    /// Container types like List, Map, Option, etc. cannot be identified from
    /// `TypeId` alone since they require type arguments. Use `from_type_data`
    /// or pattern matching on `Type`/`TypeData` for those.
    #[must_use]
    pub const fn from_type_id(id: TypeId) -> Option<Self> {
        match id.raw() {
            0 => Some(Self::Int),   // TypeId::INT
            1 => Some(Self::Float), // TypeId::FLOAT
            2 => Some(Self::Bool),  // TypeId::BOOL
            3 => Some(Self::Str),   // TypeId::STR
            4 => Some(Self::Char),  // TypeId::CHAR
            5 => Some(Self::Byte),  // TypeId::BYTE
            6 => Some(Self::Unit),  // TypeId::UNIT
            7 => Some(Self::Never), // TypeId::NEVER
            // 8 = ERROR (no BuiltinType variant)
            9 => Some(Self::Duration),  // TypeId::DURATION
            10 => Some(Self::Size),     // TypeId::SIZE
            11 => Some(Self::Ordering), // TypeId::ORDERING
            _ => None,
        }
    }

    /// Get the `TypeId` for this builtin type.
    ///
    /// Returns `Some` for types with pre-interned `TypeId` constants,
    /// `None` for container types (which require type arguments).
    #[must_use]
    pub const fn type_id(self) -> Option<TypeId> {
        match self {
            Self::Int => Some(TypeId::INT),
            Self::Float => Some(TypeId::FLOAT),
            Self::Bool => Some(TypeId::BOOL),
            Self::Str => Some(TypeId::STR),
            Self::Char => Some(TypeId::CHAR),
            Self::Byte => Some(TypeId::BYTE),
            Self::Unit => Some(TypeId::UNIT),
            Self::Never => Some(TypeId::NEVER),
            Self::Duration => Some(TypeId::DURATION),
            Self::Size => Some(TypeId::SIZE),
            Self::Ordering => Some(TypeId::ORDERING),
            // Container types don't have fixed TypeIds
            Self::List
            | Self::Map
            | Self::Option
            | Self::Result
            | Self::Range
            | Self::Set
            | Self::Channel => None,
        }
    }

    /// Get the display name for this builtin type.
    ///
    /// Returns the canonical name as it appears in Ori source code.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Char => "char",
            Self::Byte => "byte",
            Self::Unit => "()",
            Self::Never => "Never",
            Self::Duration => "Duration",
            Self::Size => "Size",
            Self::Ordering => "Ordering",
            Self::List => "List",
            Self::Map => "Map",
            Self::Option => "Option",
            Self::Result => "Result",
            Self::Range => "Range",
            Self::Set => "Set",
            Self::Channel => "Channel",
        }
    }

    /// Check if this is a primitive type (not a container).
    #[must_use]
    pub const fn is_primitive(self) -> bool {
        matches!(
            self,
            Self::Int
                | Self::Float
                | Self::Bool
                | Self::Str
                | Self::Char
                | Self::Byte
                | Self::Unit
                | Self::Never
                | Self::Duration
                | Self::Size
                | Self::Ordering
        )
    }

    /// Check if this is a container type (requires type arguments).
    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(
            self,
            Self::List
                | Self::Map
                | Self::Option
                | Self::Result
                | Self::Range
                | Self::Set
                | Self::Channel
        )
    }

    /// Check if this is a numeric type.
    #[must_use]
    pub const fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Byte)
    }

    /// Check if this type supports the Comparable trait.
    #[must_use]
    pub const fn is_comparable(self) -> bool {
        matches!(
            self,
            Self::Int
                | Self::Float
                | Self::Bool
                | Self::Str
                | Self::Char
                | Self::Byte
                | Self::Duration
                | Self::Size
                | Self::Ordering
        )
    }
}

impl std::fmt::Display for BuiltinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_type_id() {
        assert_eq!(
            BuiltinType::from_type_id(TypeId::INT),
            Some(BuiltinType::Int)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::FLOAT),
            Some(BuiltinType::Float)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::BOOL),
            Some(BuiltinType::Bool)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::STR),
            Some(BuiltinType::Str)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::CHAR),
            Some(BuiltinType::Char)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::BYTE),
            Some(BuiltinType::Byte)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::UNIT),
            Some(BuiltinType::Unit)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::NEVER),
            Some(BuiltinType::Never)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::DURATION),
            Some(BuiltinType::Duration)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::SIZE),
            Some(BuiltinType::Size)
        );
        assert_eq!(
            BuiltinType::from_type_id(TypeId::ORDERING),
            Some(BuiltinType::Ordering)
        );

        // ERROR, INFER, SELF_TYPE, and compound types return None
        assert_eq!(BuiltinType::from_type_id(TypeId::ERROR), None);
        assert_eq!(BuiltinType::from_type_id(TypeId::INFER), None);
        assert_eq!(BuiltinType::from_type_id(TypeId::SELF_TYPE), None);
        assert_eq!(BuiltinType::from_type_id(TypeId::from_raw(100)), None);
    }

    #[test]
    fn test_type_id_roundtrip() {
        for builtin in [
            BuiltinType::Int,
            BuiltinType::Float,
            BuiltinType::Bool,
            BuiltinType::Str,
            BuiltinType::Char,
            BuiltinType::Byte,
            BuiltinType::Unit,
            BuiltinType::Never,
            BuiltinType::Duration,
            BuiltinType::Size,
            BuiltinType::Ordering,
        ] {
            let Some(type_id) = builtin.type_id() else {
                panic!("primitive {builtin:?} should have TypeId");
            };
            let Some(recovered) = BuiltinType::from_type_id(type_id) else {
                panic!("should recover builtin from TypeId");
            };
            assert_eq!(builtin, recovered);
        }
    }

    #[test]
    fn test_container_types_no_type_id() {
        for builtin in [
            BuiltinType::List,
            BuiltinType::Map,
            BuiltinType::Option,
            BuiltinType::Result,
            BuiltinType::Range,
            BuiltinType::Set,
            BuiltinType::Channel,
        ] {
            assert!(builtin.type_id().is_none());
            assert!(builtin.is_container());
            assert!(!builtin.is_primitive());
        }
    }

    #[test]
    fn test_names() {
        assert_eq!(BuiltinType::Int.name(), "int");
        assert_eq!(BuiltinType::Duration.name(), "Duration");
        assert_eq!(BuiltinType::Ordering.name(), "Ordering");
        assert_eq!(BuiltinType::Unit.name(), "()");
    }

    #[test]
    fn test_is_numeric() {
        assert!(BuiltinType::Int.is_numeric());
        assert!(BuiltinType::Float.is_numeric());
        assert!(BuiltinType::Byte.is_numeric());
        assert!(!BuiltinType::Bool.is_numeric());
        assert!(!BuiltinType::Str.is_numeric());
    }

    #[test]
    fn test_is_comparable() {
        assert!(BuiltinType::Int.is_comparable());
        assert!(BuiltinType::Duration.is_comparable());
        assert!(BuiltinType::Ordering.is_comparable());
        assert!(!BuiltinType::Unit.is_comparable());
        assert!(!BuiltinType::Never.is_comparable());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", BuiltinType::Int), "int");
        assert_eq!(format!("{}", BuiltinType::Duration), "Duration");
    }
}
