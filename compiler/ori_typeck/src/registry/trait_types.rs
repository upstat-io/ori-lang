//! Trait definition types.
//!
//! Contains types for representing trait definitions in the registry.

use ori_ir::{Name, Span, TypeId, Visibility};

/// Method signature in a trait definition.
///
/// Parameter and return types are stored as `TypeId` for efficient equality comparisons.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitMethodDef {
    /// Method name.
    pub name: Name,
    /// Parameter types (first is self type if present).
    pub params: Vec<TypeId>,
    /// Return type.
    pub return_ty: TypeId,
    /// Whether this method has a default implementation.
    pub has_default: bool,
}

/// Associated type in a trait definition.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitAssocTypeDef {
    /// Associated type name.
    pub name: Name,
}

/// Entry for a trait definition.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitEntry {
    /// Trait name.
    pub name: Name,
    /// Source location.
    pub span: Span,
    /// Generic type parameters.
    pub type_params: Vec<Name>,
    /// Super-trait names (bounds this trait inherits from).
    pub super_traits: Vec<Name>,
    /// Required and default methods.
    pub methods: Vec<TraitMethodDef>,
    /// Associated types.
    pub assoc_types: Vec<TraitAssocTypeDef>,
    /// Visibility of this trait.
    pub visibility: Visibility,
}

impl TraitEntry {
    /// Look up a method by name.
    pub fn get_method(&self, name: Name) -> Option<&TraitMethodDef> {
        self.methods.iter().find(|m| m.name == name)
    }
}
