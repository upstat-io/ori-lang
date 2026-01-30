//! Implementation definition types.
//!
//! Contains types for representing impl blocks in the registry.

use ori_ir::{Name, Span, TypeId};
use ori_types::Type;
use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};

/// Implementation method.
///
/// Parameter and return types are stored as `TypeId` for efficient equality comparisons.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplMethodDef {
    /// Method name.
    pub name: Name,
    /// Parameter types.
    pub params: Vec<TypeId>,
    /// Return type.
    pub return_ty: TypeId,
}

/// Associated type definition in an impl block (e.g., `type Item = T`).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplAssocTypeDef {
    /// Associated type name (e.g., `Item`).
    pub name: Name,
    /// Concrete type assigned (e.g., `T` or `int`), stored as `TypeId`.
    pub ty: TypeId,
}

/// Entry for an implementation block.
///
/// Contains both the method definitions and indices for O(1) lookups.
/// The indices are excluded from Hash/Eq as they're derived from the vectors.
#[derive(Clone, Debug)]
pub struct ImplEntry {
    /// The trait being implemented (None for inherent impl).
    pub trait_name: Option<Name>,
    /// The type implementing the trait.
    pub self_ty: Type,
    /// Source location.
    pub span: Span,
    /// Generic parameters.
    pub type_params: Vec<Name>,
    /// Methods in this impl block.
    pub methods: Vec<ImplMethodDef>,
    /// Associated type definitions (e.g., `type Item = T`).
    pub assoc_types: Vec<ImplAssocTypeDef>,
    /// Index for O(1) method lookup by name.
    /// Maps method name to index in `methods` vector.
    method_index: FxHashMap<Name, usize>,
    /// Index for O(1) associated type lookup by name.
    /// Maps assoc type name to index in `assoc_types` vector.
    assoc_type_index: FxHashMap<Name, usize>,
}

impl PartialEq for ImplEntry {
    fn eq(&self, other: &Self) -> bool {
        // Exclude method_index from comparison (it's derived from methods)
        self.trait_name == other.trait_name
            && self.self_ty == other.self_ty
            && self.span == other.span
            && self.type_params == other.type_params
            && self.methods == other.methods
            && self.assoc_types == other.assoc_types
    }
}

impl Eq for ImplEntry {}

impl Hash for ImplEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Exclude method_index from hash (it's derived from methods)
        self.trait_name.hash(state);
        self.self_ty.hash(state);
        self.span.hash(state);
        self.type_params.hash(state);
        self.methods.hash(state);
        self.assoc_types.hash(state);
    }
}

impl ImplEntry {
    /// Create a new impl entry with lookup indices built automatically.
    pub fn new(
        trait_name: Option<Name>,
        self_ty: Type,
        span: Span,
        type_params: Vec<Name>,
        methods: Vec<ImplMethodDef>,
        assoc_types: Vec<ImplAssocTypeDef>,
    ) -> Self {
        let method_index = methods
            .iter()
            .enumerate()
            .map(|(i, m)| (m.name, i))
            .collect();

        let assoc_type_index = assoc_types
            .iter()
            .enumerate()
            .map(|(i, a)| (a.name, i))
            .collect();

        Self {
            trait_name,
            self_ty,
            span,
            type_params,
            methods,
            assoc_types,
            method_index,
            assoc_type_index,
        }
    }

    /// Get a method by name in O(1) time.
    ///
    /// Uses the internal method index for fast lookup.
    pub fn get_method(&self, name: Name) -> Option<&ImplMethodDef> {
        self.method_index.get(&name).map(|&idx| &self.methods[idx])
    }

    /// Get an associated type definition by name in O(1) time.
    ///
    /// Uses the internal associated type index for fast lookup.
    pub fn get_assoc_type(&self, name: Name) -> Option<&ImplAssocTypeDef> {
        self.assoc_type_index
            .get(&name)
            .map(|&idx| &self.assoc_types[idx])
    }

    /// Rebuild indices after modifying methods or associated types.
    ///
    /// Call this after directly modifying the `methods` or `assoc_types` fields.
    pub fn rebuild_indices(&mut self) {
        self.method_index = self
            .methods
            .iter()
            .enumerate()
            .map(|(i, m)| (m.name, i))
            .collect();
        self.assoc_type_index = self
            .assoc_types
            .iter()
            .enumerate()
            .map(|(i, a)| (a.name, i))
            .collect();
    }
}

/// Error when coherence rules are violated.
#[derive(Clone, Debug)]
pub struct CoherenceError {
    /// Description of the conflict.
    pub message: String,
    /// Span of the conflicting impl.
    pub span: Span,
    /// Span of the existing impl.
    pub existing_span: Span,
}
