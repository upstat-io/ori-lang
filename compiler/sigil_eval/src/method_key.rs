//! Method lookup key for type/method name pairs.
//!
//! Provides a type-safe key for method registry lookups,
//! improving code clarity and enabling better error messages.

use std::fmt;

/// Key for looking up methods in registries.
///
/// Combines a type name and method name into a single hashable key.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MethodKey {
    /// The type name (e.g., "Point", "int", "[int]")
    pub type_name: String,
    /// The method name (e.g., "distance", "double")
    pub method_name: String,
}

impl MethodKey {
    /// Create a new method key.
    #[inline]
    pub fn new(type_name: impl Into<String>, method_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            method_name: method_name.into(),
        }
    }

    /// Create a method key from string slices (allocates new Strings).
    #[inline]
    pub fn from_strs(type_name: &str, method_name: &str) -> Self {
        Self {
            type_name: type_name.to_string(),
            method_name: method_name.to_string(),
        }
    }
}

impl fmt::Display for MethodKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.type_name, self.method_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_method_key_equality() {
        let k1 = MethodKey::new("Point", "distance");
        let k2 = MethodKey::new("Point", "distance");
        let k3 = MethodKey::new("Point", "other");

        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn test_method_key_as_hashmap_key() {
        let mut map: HashMap<MethodKey, u32> = HashMap::new();
        map.insert(MethodKey::new("Point", "distance"), 1);
        map.insert(MethodKey::new("Point", "scale"), 2);

        assert_eq!(map.get(&MethodKey::new("Point", "distance")), Some(&1));
        assert_eq!(map.get(&MethodKey::new("Point", "scale")), Some(&2));
        assert_eq!(map.get(&MethodKey::new("Point", "missing")), None);
    }

    #[test]
    fn test_method_key_display() {
        let key = MethodKey::new("Point", "distance");
        assert_eq!(format!("{key}"), "Point::distance");
    }

    #[test]
    fn test_method_key_from_strs() {
        let type_name = "Point";
        let method_name = "distance";
        let key = MethodKey::from_strs(type_name, method_name);
        assert_eq!(key.type_name, "Point");
        assert_eq!(key.method_name, "distance");
    }
}
