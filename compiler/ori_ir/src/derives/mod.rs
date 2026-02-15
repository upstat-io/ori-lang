//! Derived trait definitions.
//!
//! This module contains the `DerivedTrait` enum and `DerivedMethodInfo` struct
//! that are used by both the type checker and the evaluator. By placing them
//! in `ori_ir`, we avoid a circular dependency between `ori_types` and `ori_eval`.

use crate::{Name, TypeId};

/// The type of a field for Default trait derivation.
///
/// Captures whether a field has a known primitive default (e.g., `int` → 0)
/// or a named type whose `.default()` must be called recursively.
#[derive(Clone, Debug)]
pub enum DefaultFieldType {
    /// A primitive type with a known default value.
    Primitive(TypeId),
    /// A named type — call `Type.default()` recursively.
    Named(Name),
}

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
    /// Field types for Default derivation (parallel to `field_names`).
    /// Empty for non-Default traits.
    pub field_types: Vec<DefaultFieldType>,
}

impl DerivedMethodInfo {
    /// Create a new derived method info for a struct.
    pub fn new(trait_kind: DerivedTrait, field_names: Vec<Name>) -> Self {
        DerivedMethodInfo {
            trait_kind,
            field_names,
            field_types: Vec::new(),
        }
    }

    /// Create a derived method info with field type information (for Default).
    pub fn with_field_types(
        trait_kind: DerivedTrait,
        field_names: Vec<Name>,
        field_types: Vec<DefaultFieldType>,
    ) -> Self {
        DerivedMethodInfo {
            trait_kind,
            field_names,
            field_types,
        }
    }
}

#[cfg(test)]
mod tests;
