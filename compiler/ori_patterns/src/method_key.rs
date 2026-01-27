//! Method lookup key for type/method name pairs.
//!
//! Provides a type-safe key for method registry lookups,
//! improving code clarity and enabling better error messages.
//!
//! Uses interned `Name` values for zero-allocation lookups.

use ori_ir::{Name, SharedInterner};

/// Key for looking up methods in registries.
///
/// Combines a type name and method name into a single hashable key.
/// Uses interned `Name` values for efficient comparison and hashing.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct MethodKey {
    /// The type name (e.g., "Point", "int", "[int]")
    pub type_name: Name,
    /// The method name (e.g., "distance", "double")
    pub method_name: Name,
}

impl MethodKey {
    /// Create a new method key from interned names.
    #[inline]
    pub const fn new(type_name: Name, method_name: Name) -> Self {
        Self {
            type_name,
            method_name,
        }
    }

    /// Format the method key for display (requires interner).
    #[inline]
    pub fn display<'a>(&self, interner: &'a SharedInterner) -> MethodKeyDisplay<'a> {
        MethodKeyDisplay {
            type_name: interner.lookup(self.type_name),
            method_name: interner.lookup(self.method_name),
        }
    }
}

/// Helper for displaying a `MethodKey` with resolved names.
pub struct MethodKeyDisplay<'a> {
    type_name: &'a str,
    method_name: &'a str,
}

impl std::fmt::Display for MethodKeyDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.type_name, self.method_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_method_key_equality() {
        let interner = SharedInterner::default();
        let point = interner.intern("Point");
        let distance = interner.intern("distance");
        let other = interner.intern("other");

        let k1 = MethodKey::new(point, distance);
        let k2 = MethodKey::new(point, distance);
        let k3 = MethodKey::new(point, other);

        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn test_method_key_as_hashmap_key() {
        let interner = SharedInterner::default();
        let point = interner.intern("Point");
        let distance = interner.intern("distance");
        let scale = interner.intern("scale");
        let missing = interner.intern("missing");

        let mut map: HashMap<MethodKey, u32> = HashMap::new();
        map.insert(MethodKey::new(point, distance), 1);
        map.insert(MethodKey::new(point, scale), 2);

        assert_eq!(map.get(&MethodKey::new(point, distance)), Some(&1));
        assert_eq!(map.get(&MethodKey::new(point, scale)), Some(&2));
        assert_eq!(map.get(&MethodKey::new(point, missing)), None);
    }

    #[test]
    fn test_method_key_display() {
        let interner = SharedInterner::default();
        let point = interner.intern("Point");
        let distance = interner.intern("distance");

        let key = MethodKey::new(point, distance);
        assert_eq!(format!("{}", key.display(&interner)), "Point::distance");
    }

    #[test]
    fn test_method_key_is_copy() {
        let interner = SharedInterner::default();
        let point = interner.intern("Point");
        let distance = interner.intern("distance");

        let key = MethodKey::new(point, distance);
        let key_copy = key; // This should work since MethodKey is Copy
        assert_eq!(key, key_copy);
    }
}
