//! Trait definition types.
//!
//! Contains types for representing trait definitions in the registry.

use ori_ir::{Name, Span, TypeId, Visibility};
use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};

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
///
/// Contains both the method definitions and an index for O(1) method lookup.
/// The index is excluded from Hash/Eq as it's derived from `methods`.
#[derive(Clone, Debug)]
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
    /// Index for O(1) method lookup by name.
    /// Maps method name to index in `methods` vector.
    method_index: FxHashMap<Name, usize>,
}

impl PartialEq for TraitEntry {
    fn eq(&self, other: &Self) -> bool {
        // Exclude method_index from comparison (it's derived from methods)
        self.name == other.name
            && self.span == other.span
            && self.type_params == other.type_params
            && self.super_traits == other.super_traits
            && self.methods == other.methods
            && self.assoc_types == other.assoc_types
            && self.visibility == other.visibility
    }
}

impl Eq for TraitEntry {}

impl Hash for TraitEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Exclude method_index from hash (it's derived from methods)
        self.name.hash(state);
        self.span.hash(state);
        self.type_params.hash(state);
        self.super_traits.hash(state);
        self.methods.hash(state);
        self.assoc_types.hash(state);
        self.visibility.hash(state);
    }
}

impl TraitEntry {
    /// Create a new trait entry with the method index built automatically.
    pub fn new(
        name: Name,
        span: Span,
        type_params: Vec<Name>,
        super_traits: Vec<Name>,
        methods: Vec<TraitMethodDef>,
        assoc_types: Vec<TraitAssocTypeDef>,
        visibility: Visibility,
    ) -> Self {
        let method_index = methods
            .iter()
            .enumerate()
            .map(|(i, m)| (m.name, i))
            .collect();

        Self {
            name,
            span,
            type_params,
            super_traits,
            methods,
            assoc_types,
            visibility,
            method_index,
        }
    }

    /// Look up a method by name in O(1) time.
    ///
    /// Uses the internal method index for fast lookup.
    pub fn get_method(&self, name: Name) -> Option<&TraitMethodDef> {
        self.method_index.get(&name).map(|&idx| &self.methods[idx])
    }
}
