//! Interned type identifier.
//!
//! Per design spec A-data-structuresmd:
//! - TypeId(u32) for O(1) equality comparison
//! - Pre-interned primitive types
//! - Sharded layout for type interner
//! - All Salsa-required traits

use std::fmt;
use std::hash::{Hash, Hasher};

/// Interned type identifier.
///
/// # Layout
/// 32-bit index split into shard (4 bits) + local index (28 bits):
/// - Bits 31-28: Shard index (0-15)
/// - Bits 27-0: Local index within shard
///
/// This layout supports up to 268 million types per shard (16 shards total).
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
///
/// # Pre-interned Types
/// Primitive types are pre-interned in shard 0 with fixed local indices:
/// - INT, FLOAT, BOOL, STR, CHAR, BYTE, VOID, NEVER
/// - INFER and `SELF_TYPE` are special markers
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    // Pre-interned primitive types (all in shard 0)
    pub const INT: TypeId = TypeId(0);
    pub const FLOAT: TypeId = TypeId(1);
    pub const BOOL: TypeId = TypeId(2);
    pub const STR: TypeId = TypeId(3);
    pub const CHAR: TypeId = TypeId(4);
    pub const BYTE: TypeId = TypeId(5);
    pub const VOID: TypeId = TypeId(6);
    pub const NEVER: TypeId = TypeId(7);
    pub const INFER: TypeId = TypeId(8); // Placeholder during inference
    pub const SELF_TYPE: TypeId = TypeId(9); // Self type in trait/impl contexts

    /// First ID for compound types (after primitives).
    pub const FIRST_COMPOUND: u32 = 10;

    /// Maximum local index per shard (2^28 - 1).
    pub const MAX_LOCAL: u32 = 0x0FFF_FFFF;

    /// Number of shards for type interning.
    pub const NUM_SHARDS: usize = 16;

    /// Create a new `TypeId` from a raw index (legacy API).
    #[inline]
    pub const fn new(index: u32) -> Self {
        TypeId(index)
    }

    /// Create a `TypeId` from shard and local index.
    ///
    /// # Layout
    /// The shard occupies bits 31-28 (4 bits), and the local index
    /// occupies bits 27-0 (28 bits).
    #[inline]
    pub const fn from_shard_local(shard: u32, local: u32) -> Self {
        debug_assert!(shard < 16);
        debug_assert!(local <= Self::MAX_LOCAL);
        TypeId((shard << 28) | local)
    }

    /// Extract the shard index (bits 31-28).
    #[inline]
    pub const fn shard(self) -> usize {
        (self.0 >> 28) as usize
    }

    /// Extract the local index within the shard (bits 27-0).
    #[inline]
    pub const fn local(self) -> usize {
        (self.0 & Self::MAX_LOCAL) as usize
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Create from a raw u32 value.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        TypeId(raw)
    }

    /// Get the raw index (legacy API, same as `raw()`).
    #[inline]
    pub const fn index(self) -> u32 {
        self.0
    }

    /// Check if this is a primitive type.
    #[inline]
    pub const fn is_primitive(self) -> bool {
        // Primitives are in shard 0, indices 0-9
        self.0 < Self::FIRST_COMPOUND
    }

    /// Check if this is the inference placeholder.
    #[inline]
    pub const fn is_infer(self) -> bool {
        self.0 == Self::INFER.0
    }

    /// Check if this is the Self type (used in trait/impl contexts).
    #[inline]
    pub const fn is_self_type(self) -> bool {
        self.0 == Self::SELF_TYPE.0
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
            Self::SELF_TYPE => write!(f, "TypeId::SELF_TYPE"),
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

    #[test]
    fn test_shard_local_layout() {
        // Primitives are in shard 0
        assert_eq!(TypeId::INT.shard(), 0);
        assert_eq!(TypeId::INT.local(), 0);
        assert_eq!(TypeId::FLOAT.shard(), 0);
        assert_eq!(TypeId::FLOAT.local(), 1);

        // Create type in shard 5
        let id = TypeId::from_shard_local(5, 1000);
        assert_eq!(id.shard(), 5);
        assert_eq!(id.local(), 1000);

        // Max shard and local
        let max_id = TypeId::from_shard_local(15, TypeId::MAX_LOCAL);
        assert_eq!(max_id.shard(), 15);
        assert_eq!(max_id.local(), TypeId::MAX_LOCAL as usize);
    }

    #[test]
    fn test_raw_roundtrip() {
        let id = TypeId::from_shard_local(7, 12345);
        let raw = id.raw();
        let recovered = TypeId::from_raw(raw);
        assert_eq!(id, recovered);
        assert_eq!(recovered.shard(), 7);
        assert_eq!(recovered.local(), 12345);
    }

    #[test]
    fn test_primitives_shard_zero() {
        // All primitives should be in shard 0 for backward compatibility
        let primitives = [
            TypeId::INT,
            TypeId::FLOAT,
            TypeId::BOOL,
            TypeId::STR,
            TypeId::CHAR,
            TypeId::BYTE,
            TypeId::VOID,
            TypeId::NEVER,
            TypeId::INFER,
            TypeId::SELF_TYPE,
        ];

        for (i, &prim) in primitives.iter().enumerate() {
            assert_eq!(prim.shard(), 0, "Primitive at index {i} not in shard 0");
            assert_eq!(prim.local(), i, "Primitive at index {i} has wrong local");
        }
    }
}
