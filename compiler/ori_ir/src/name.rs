//! Interned string identifier.
//!
//! Provides compact 32-bit interned identifiers with all Salsa-required traits.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Interned string identifier.
///
/// Layout: 32-bit index split into shard (4 bits) + local index (28 bits)
/// - Bits 31-28: Shard index (0-15)
/// - Bits 27-0: Local index within shard
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Ord, `PartialOrd`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct Name(u32);

impl Name {
    /// Pre-interned empty string.
    pub const EMPTY: Name = Name(0);

    /// Maximum local index per shard.
    pub const MAX_LOCAL: u32 = 0x0FFF_FFFF;

    /// Number of shards.
    pub const NUM_SHARDS: usize = 16;

    /// Create from shard and local index.
    #[inline]
    pub const fn new(shard: u32, local: u32) -> Self {
        debug_assert!(shard < 16);
        debug_assert!(local <= Self::MAX_LOCAL);
        Name((shard << 28) | local)
    }

    /// Extract shard index.
    #[inline]
    pub const fn shard(self) -> usize {
        (self.0 >> 28) as usize
    }

    /// Extract local index.
    #[inline]
    pub const fn local(self) -> usize {
        (self.0 & Self::MAX_LOCAL) as usize
    }

    /// Get raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Create from raw u32 value.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Name(raw)
    }
}

impl Hash for Name {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Name(shard={}, local={})", self.shard(), self.local())
    }
}

impl Default for Name {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_layout() {
        let name = Name::new(5, 1000);
        assert_eq!(name.shard(), 5);
        assert_eq!(name.local(), 1000);
    }

    #[test]
    fn test_name_empty() {
        assert_eq!(Name::EMPTY.shard(), 0);
        assert_eq!(Name::EMPTY.local(), 0);
    }

    #[test]
    fn test_name_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Name::new(0, 1));
        set.insert(Name::new(0, 1)); // duplicate
        set.insert(Name::new(0, 2));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_name_ord() {
        let a = Name::new(0, 1);
        let b = Name::new(0, 2);
        assert!(a < b);
    }
}
