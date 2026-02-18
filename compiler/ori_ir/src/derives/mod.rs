//! Derived trait definitions.
//!
//! This module contains the `DerivedTrait` enum and `DerivedMethodInfo` struct
//! that are used by both the type checker and the evaluator. By placing them
//! in `ori_ir`, we avoid a circular dependency between `ori_types` and `ori_eval`.

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
    /// Debug trait - developer-facing structural representation
    Debug,
    /// Default trait - default value construction
    Default,
    /// Comparable trait - lexicographic field comparison
    Comparable,
}

impl DerivedTrait {
    /// Parse a trait name string into a `DerivedTrait`.
    pub fn from_name(s: &str) -> Option<DerivedTrait> {
        match s {
            "Eq" => Some(DerivedTrait::Eq),
            "Clone" => Some(DerivedTrait::Clone),
            "Hashable" => Some(DerivedTrait::Hashable),
            "Printable" => Some(DerivedTrait::Printable),
            "Debug" => Some(DerivedTrait::Debug),
            "Default" => Some(DerivedTrait::Default),
            "Comparable" => Some(DerivedTrait::Comparable),
            _ => None,
        }
    }

    /// Get the method name for this derived trait.
    pub fn method_name(&self) -> &'static str {
        match self {
            DerivedTrait::Eq => "eq",
            DerivedTrait::Clone => "clone",
            DerivedTrait::Hashable => "hash",
            DerivedTrait::Printable => "to_str",
            DerivedTrait::Debug => "debug",
            DerivedTrait::Default => "default",
            DerivedTrait::Comparable => "compare",
        }
    }
}

/// Information about a derived method.
///
/// Unlike user-defined methods, derived methods don't have expression bodies.
/// Instead, they operate on struct/enum field information.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DerivedMethodInfo {
    /// The trait being derived.
    pub trait_kind: DerivedTrait,
    /// Field names for struct types (in declaration order).
    pub field_names: Vec<Name>,
    /// Variant names for sum types (in declaration order).
    ///
    /// Empty for struct types. Used by the evaluator to determine variant
    /// ordering for `Comparable` derivation and variant-aware `Hash` dispatch.
    pub variant_names: Vec<Name>,
}

impl DerivedMethodInfo {
    /// Create a new derived method info for a struct.
    pub fn new(trait_kind: DerivedTrait, field_names: Vec<Name>) -> Self {
        DerivedMethodInfo {
            trait_kind,
            field_names,
            variant_names: Vec::new(),
        }
    }

    /// Create a new derived method info for a sum type.
    pub fn new_sum(trait_kind: DerivedTrait, variant_names: Vec<Name>) -> Self {
        DerivedMethodInfo {
            trait_kind,
            field_names: Vec::new(),
            variant_names,
        }
    }
}

#[cfg(test)]
mod tests;
