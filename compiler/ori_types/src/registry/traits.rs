//! Registry for traits and their implementations.
//!
//! The `TraitRegistry` stores trait definitions and their implementations,
//! enabling efficient lookup for method resolution and coherence checking.
//!
//! # Design
//!
//! - Traits indexed by name for definition lookup
//! - Implementations indexed by self type for method resolution
//! - Secondary index by trait for coherence checking
//! - All types derive Salsa-required traits

use std::collections::BTreeMap;

use ori_ir::{ExprId, Name, Span};
use rustc_hash::FxHashMap;

use crate::Idx;

/// Registry for traits and their implementations.
///
/// Provides efficient lookup of trait definitions and implementations
/// for method resolution.
#[derive(Clone, Debug, Default)]
pub struct TraitRegistry {
    /// Traits indexed by name (`BTreeMap` for deterministic iteration).
    traits_by_name: BTreeMap<Name, TraitEntry>,

    /// Traits indexed by pool Idx.
    traits_by_idx: FxHashMap<Idx, TraitEntry>,

    /// All implementations.
    impls: Vec<ImplEntry>,

    /// Quick lookup: `self_type` -> impl indices.
    /// Enables O(1) lookup of implementations for a given type.
    impls_by_type: FxHashMap<Idx, Vec<usize>>,

    /// Quick lookup: `trait_idx` -> impl indices.
    /// Enables coherence checking and trait method resolution.
    impls_by_trait: FxHashMap<Idx, Vec<usize>>,
}

/// A registered trait definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitEntry {
    /// The trait name.
    pub name: Name,

    /// Pool index for this trait type.
    pub idx: Idx,

    /// Generic type parameters (e.g., `T` in `trait Foo<T>`).
    pub type_params: Vec<Name>,

    /// Method signatures defined by this trait.
    pub methods: FxHashMap<Name, TraitMethodDef>,

    /// Associated types defined by this trait.
    pub assoc_types: FxHashMap<Name, TraitAssocTypeDef>,

    /// Source location of the definition.
    pub span: Span,
}

/// A trait method signature.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitMethodDef {
    /// Method name.
    pub name: Name,

    /// Method signature as a function type index.
    pub signature: Idx,

    /// Whether this method has a default implementation.
    pub has_default: bool,

    /// Default implementation body (if `has_default` is true).
    pub default_body: Option<ExprId>,

    /// Source location.
    pub span: Span,
}

/// An associated type in a trait.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitAssocTypeDef {
    /// Associated type name.
    pub name: Name,

    /// Bounds on the associated type (trait constraints).
    pub bounds: Vec<Idx>,

    /// Default type (if any).
    pub default: Option<Idx>,

    /// Source location.
    pub span: Span,
}

/// A trait implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplEntry {
    /// The trait being implemented (`None` for inherent impls).
    pub trait_idx: Option<Idx>,

    /// The self type for this implementation.
    pub self_type: Idx,

    /// Generic type parameters on this impl.
    pub type_params: Vec<Name>,

    /// Method implementations.
    pub methods: FxHashMap<Name, ImplMethodDef>,

    /// Associated type implementations.
    pub assoc_types: FxHashMap<Name, Idx>,

    /// Where clause constraints.
    pub where_clause: Vec<WhereConstraint>,

    /// Source location.
    pub span: Span,
}

/// A method implementation in an impl block.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImplMethodDef {
    /// Method name.
    pub name: Name,

    /// Method signature (function type).
    pub signature: Idx,

    /// Whether the first parameter is `self` (instance method vs associated function).
    pub has_self: bool,

    /// Method body expression.
    pub body: ExprId,

    /// Source location.
    pub span: Span,
}

/// A where clause constraint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WhereConstraint {
    /// The constrained type.
    pub ty: Idx,

    /// The trait bounds on this type.
    pub bounds: Vec<Idx>,
}

impl TraitRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // === Trait Registration ===

    /// Register a trait definition.
    pub fn register_trait(&mut self, entry: TraitEntry) {
        let name = entry.name;
        let idx = entry.idx;
        self.traits_by_name.insert(name, entry.clone());
        self.traits_by_idx.insert(idx, entry);
    }

    /// Register a trait implementation.
    ///
    /// Returns the impl index for reference.
    pub fn register_impl(&mut self, entry: ImplEntry) -> usize {
        let impl_idx = self.impls.len();

        // Index by self type
        self.impls_by_type
            .entry(entry.self_type)
            .or_default()
            .push(impl_idx);

        // Index by trait (if not an inherent impl)
        if let Some(trait_idx) = entry.trait_idx {
            self.impls_by_trait
                .entry(trait_idx)
                .or_default()
                .push(impl_idx);
        }

        self.impls.push(entry);
        impl_idx
    }

    // === Trait Lookup ===

    /// Look up a trait by name.
    #[inline]
    pub fn get_trait_by_name(&self, name: Name) -> Option<&TraitEntry> {
        self.traits_by_name.get(&name)
    }

    /// Look up a trait by pool index.
    #[inline]
    pub fn get_trait_by_idx(&self, idx: Idx) -> Option<&TraitEntry> {
        self.traits_by_idx.get(&idx)
    }

    /// Check if a trait with the given name exists.
    #[inline]
    pub fn contains_trait(&self, name: Name) -> bool {
        self.traits_by_name.contains_key(&name)
    }

    /// Get a trait method signature.
    pub fn trait_method(&self, trait_idx: Idx, method_name: Name) -> Option<&TraitMethodDef> {
        self.get_trait_by_idx(trait_idx)
            .and_then(|t| t.methods.get(&method_name))
    }

    /// Get an associated type definition from a trait.
    pub fn trait_assoc_type(&self, trait_idx: Idx, assoc_name: Name) -> Option<&TraitAssocTypeDef> {
        self.get_trait_by_idx(trait_idx)
            .and_then(|t| t.assoc_types.get(&assoc_name))
    }

    // === Impl Lookup ===

    /// Get all implementations for a given self type.
    pub fn impls_for_type(&self, self_type: Idx) -> impl Iterator<Item = &ImplEntry> {
        self.impls_by_type
            .get(&self_type)
            .into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|&i| self.impls.get(i))
    }

    /// Get all implementations of a specific trait.
    pub fn impls_of_trait(&self, trait_idx: Idx) -> impl Iterator<Item = &ImplEntry> {
        self.impls_by_trait
            .get(&trait_idx)
            .into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|&i| self.impls.get(i))
    }

    /// Get an impl entry by index.
    #[inline]
    pub fn get_impl(&self, impl_idx: usize) -> Option<&ImplEntry> {
        self.impls.get(impl_idx)
    }

    /// Get a mutable impl entry by index.
    #[inline]
    pub fn get_impl_mut(&mut self, impl_idx: usize) -> Option<&mut ImplEntry> {
        self.impls.get_mut(impl_idx)
    }

    /// Find an impl of a specific trait for a specific type.
    pub fn find_impl(&self, trait_idx: Idx, self_type: Idx) -> Option<(usize, &ImplEntry)> {
        self.impls_by_type.get(&self_type).and_then(|indices| {
            indices.iter().find_map(|&i| {
                let entry = &self.impls[i];
                if entry.trait_idx == Some(trait_idx) {
                    Some((i, entry))
                } else {
                    None
                }
            })
        })
    }

    /// Find the inherent impl for a type (impl without a trait).
    pub fn inherent_impl(&self, self_type: Idx) -> Option<(usize, &ImplEntry)> {
        self.impls_by_type.get(&self_type).and_then(|indices| {
            indices.iter().find_map(|&i| {
                let entry = &self.impls[i];
                if entry.trait_idx.is_none() {
                    Some((i, entry))
                } else {
                    None
                }
            })
        })
    }

    /// Look up a method implementation for a given type.
    ///
    /// Searches inherent impls first, then trait impls.
    pub fn lookup_method(&self, self_type: Idx, method_name: Name) -> Option<MethodLookup<'_>> {
        // 1. Check inherent impl first
        if let Some((impl_idx, impl_entry)) = self.inherent_impl(self_type) {
            if let Some(method) = impl_entry.methods.get(&method_name) {
                return Some(MethodLookup::Inherent { impl_idx, method });
            }
        }

        // 2. Check trait impls
        for (impl_idx, impl_entry) in self
            .impls_by_type
            .get(&self_type)
            .into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|&i| Some((i, self.impls.get(i)?)))
        {
            if let Some(method) = impl_entry.methods.get(&method_name) {
                // Non-inherent impls should always have a trait_idx
                let Some(trait_idx) = impl_entry.trait_idx else {
                    continue;
                };
                return Some(MethodLookup::Trait {
                    trait_idx,
                    impl_idx,
                    method,
                });
            }
        }

        None
    }

    // === Coherence ===

    /// Check if implementing a trait for a type would be a duplicate.
    ///
    /// Returns `true` if an implementation already exists.
    pub fn has_impl(&self, trait_idx: Idx, self_type: Idx) -> bool {
        self.find_impl(trait_idx, self_type).is_some()
    }

    /// Check if an inherent impl exists for a type.
    pub fn has_inherent_impl(&self, self_type: Idx) -> bool {
        self.inherent_impl(self_type).is_some()
    }

    // === Iteration ===

    /// Iterate over all registered traits in name order.
    pub fn iter_traits(&self) -> impl Iterator<Item = &TraitEntry> {
        self.traits_by_name.values()
    }

    /// Iterate over all implementations.
    pub fn iter_impls(&self) -> impl Iterator<Item = &ImplEntry> {
        self.impls.iter()
    }

    /// Get the number of registered traits.
    #[inline]
    pub fn trait_count(&self) -> usize {
        self.traits_by_name.len()
    }

    /// Get the number of registered implementations.
    #[inline]
    pub fn impl_count(&self) -> usize {
        self.impls.len()
    }
}

/// Result of a method lookup.
#[derive(Clone, Debug)]
pub enum MethodLookup<'a> {
    /// Method from an inherent impl.
    Inherent {
        /// Index of the impl block.
        impl_idx: usize,
        /// The method definition.
        method: &'a ImplMethodDef,
    },

    /// Method from a trait impl.
    Trait {
        /// The trait being implemented.
        trait_idx: Idx,
        /// Index of the impl block.
        impl_idx: usize,
        /// The method definition.
        method: &'a ImplMethodDef,
    },
}

impl<'a> MethodLookup<'a> {
    /// Get the method definition.
    #[inline]
    pub fn method(&self) -> &'a ImplMethodDef {
        match self {
            Self::Inherent { method, .. } | Self::Trait { method, .. } => method,
        }
    }

    /// Get the impl index.
    #[inline]
    pub fn impl_idx(&self) -> usize {
        match self {
            Self::Inherent { impl_idx, .. } | Self::Trait { impl_idx, .. } => *impl_idx,
        }
    }

    /// Check if this is an inherent method.
    #[inline]
    pub fn is_inherent(&self) -> bool {
        matches!(self, Self::Inherent { .. })
    }

    /// Get the trait index if this is a trait method.
    #[inline]
    pub fn trait_idx(&self) -> Option<Idx> {
        match self {
            Self::Inherent { .. } => None,
            Self::Trait { trait_idx, .. } => Some(*trait_idx),
        }
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
mod tests;
