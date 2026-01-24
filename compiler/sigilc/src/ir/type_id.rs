//! Interned type identifier.
//!
//! Per design spec A-data-structuresmd:
//! - TypeId(u32) for O(1) equality comparison
//! - Pre-interned primitive types
//! - All Salsa-required traits

use std::fmt;
use std::hash::{Hash, Hasher};

/// Interned type identifier.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, PartialEq, Hash, Debug
///
/// # Pre-interned Types
/// Primitive types are pre-interned with fixed IDs:
/// - INT, FLOAT, BOOL, STR, CHAR, BYTE, VOID, NEVER, INFER
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    // Pre-interned primitive types
    pub const INT: TypeId = TypeId(0);
    pub const FLOAT: TypeId = TypeId(1);
    pub const BOOL: TypeId = TypeId(2);
    pub const STR: TypeId = TypeId(3);
    pub const CHAR: TypeId = TypeId(4);
    pub const BYTE: TypeId = TypeId(5);
    pub const VOID: TypeId = TypeId(6);
    pub const NEVER: TypeId = TypeId(7);
    pub const INFER: TypeId = TypeId(8); // Placeholder during inference

    /// First ID for compound types (after primitives).
    pub const FIRST_COMPOUND: u32 = 9;

    /// Create a new TypeId.
    #[inline]
    pub const fn new(index: u32) -> Self {
        TypeId(index)
    }

    /// Get the raw index.
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }

    /// Check if this is a primitive type.
    #[inline]
    pub const fn is_primitive(self) -> bool {
        self.0 < Self::FIRST_COMPOUND
    }

    /// Check if this is the inference placeholder.
    #[inline]
    pub const fn is_infer(self) -> bool {
        self.0 == Self::INFER.0
    }
}

impl Hash for TypeId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::INT => write!(f, "TypeId::INT"),
            Self::FLOAT => write!(f, "TypeId::FLOAT"),
            Self::BOOL => write!(f, "TypeId::BOOL"),
            Self::STR => write!(f, "TypeId::STR"),
            Self::CHAR => write!(f, "TypeId::CHAR"),
            Self::BYTE => write!(f, "TypeId::BYTE"),
            Self::VOID => write!(f, "TypeId::VOID"),
            Self::NEVER => write!(f, "TypeId::NEVER"),
            Self::INFER => write!(f, "TypeId::INFER"),
            _ => write!(f, "TypeId({})", self.0),
        }
    }
}

impl Default for TypeId {
    fn default() -> Self {
        Self::INFER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_types() {
        assert!(TypeId::INT.is_primitive());
        assert!(TypeId::FLOAT.is_primitive());
        assert!(TypeId::VOID.is_primitive());
        assert!(TypeId::INFER.is_primitive());
    }

    #[test]
    fn test_compound_types() {
        let compound = TypeId::new(TypeId::FIRST_COMPOUND);
        assert!(!compound.is_primitive());
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TypeId::INT);
        set.insert(TypeId::INT); // duplicate
        set.insert(TypeId::FLOAT);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_infer() {
        assert!(TypeId::INFER.is_infer());
        assert!(!TypeId::INT.is_infer());
    }
}
