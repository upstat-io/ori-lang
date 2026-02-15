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
mod tests;
