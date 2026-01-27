//! Derived trait definitions.
//!
//! This module contains the `DerivedTrait` enum and `DerivedMethodInfo` struct
//! that are used by both the type checker and the evaluator. By placing them
//! in `ori_ir`, we avoid a circular dependency between `ori_typeck` and `ori_eval`.

use crate::Name;

/// A derived trait that can be auto-implemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DerivedTrait {
    /// Eq trait - structural equality
    Eq,
    /// Clone trait - field-by-field cloning
    Clone,
    /// Hashable trait - hash computation
    Hashable,
    /// Printable trait - string representation
    Printable,
    /// Default trait - default value construction
    Default,
}

impl DerivedTrait {
    /// Parse a trait name string into a `DerivedTrait`.
    pub fn from_name(s: &str) -> Option<DerivedTrait> {
        match s {
            "Eq" => Some(DerivedTrait::Eq),
            "Clone" => Some(DerivedTrait::Clone),
            "Hashable" => Some(DerivedTrait::Hashable),
            "Printable" => Some(DerivedTrait::Printable),
            "Default" => Some(DerivedTrait::Default),
            _ => None,
        }
    }

    /// Get the method name for this derived trait.
    pub fn method_name(&self) -> &'static str {
        match self {
            DerivedTrait::Eq => "eq",
            DerivedTrait::Clone => "clone",
            DerivedTrait::Hashable => "hash",
            DerivedTrait::Printable => "to_string",
            DerivedTrait::Default => "default",
        }
    }
}

/// Information about a derived method.
///
/// Unlike user-defined methods, derived methods don't have expression bodies.
/// Instead, they operate on struct/enum field information.
#[derive(Clone, Debug)]
pub struct DerivedMethodInfo {
    /// The trait being derived.
    pub trait_kind: DerivedTrait,
    /// Field names for struct types (in order).
    pub field_names: Vec<Name>,
}

impl DerivedMethodInfo {
    /// Create a new derived method info for a struct.
    pub fn new(trait_kind: DerivedTrait, field_names: Vec<Name>) -> Self {
        DerivedMethodInfo {
            trait_kind,
            field_names,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derived_trait_from_name() {
        assert_eq!(DerivedTrait::from_name("Eq"), Some(DerivedTrait::Eq));
        assert_eq!(DerivedTrait::from_name("Clone"), Some(DerivedTrait::Clone));
        assert_eq!(
            DerivedTrait::from_name("Hashable"),
            Some(DerivedTrait::Hashable)
        );
        assert_eq!(
            DerivedTrait::from_name("Printable"),
            Some(DerivedTrait::Printable)
        );
        assert_eq!(
            DerivedTrait::from_name("Default"),
            Some(DerivedTrait::Default)
        );
        assert_eq!(DerivedTrait::from_name("Unknown"), None);
    }

    #[test]
    fn test_derived_trait_method_name() {
        assert_eq!(DerivedTrait::Eq.method_name(), "eq");
        assert_eq!(DerivedTrait::Clone.method_name(), "clone");
        assert_eq!(DerivedTrait::Hashable.method_name(), "hash");
        assert_eq!(DerivedTrait::Printable.method_name(), "to_string");
        assert_eq!(DerivedTrait::Default.method_name(), "default");
    }
}
