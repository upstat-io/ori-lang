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
use rustc_hash::{FxHashMap, FxHashSet};

use crate::Idx;

/// Registry for traits and their implementations.
///
/// Provides efficient lookup of trait definitions and implementations
/// for method resolution.
///
/// Traits are stored in a single `Vec<TraitEntry>`, with lookup maps
/// holding `usize` indices (same pattern as `impls`). This avoids
/// cloning `TraitEntry` on registration.
#[derive(Clone, Debug, Default)]
pub struct TraitRegistry {
    /// All registered trait definitions.
    traits: Vec<TraitEntry>,

    /// Name → trait index (`BTreeMap` for deterministic iteration).
    traits_by_name: BTreeMap<Name, usize>,

    /// Pool `Idx` → trait index.
    traits_by_idx: FxHashMap<Idx, usize>,

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

    /// Super-trait pool indices (direct parents in the inheritance DAG).
    pub super_traits: Vec<Idx>,

    /// Method signatures defined by this trait.
    pub methods: FxHashMap<Name, TraitMethodDef>,

    /// Associated types defined by this trait.
    pub assoc_types: FxHashMap<Name, TraitAssocTypeDef>,

    /// Object safety violations found in this trait's methods.
    ///
    /// Empty means the trait is object-safe (can be used as a trait object).
    /// Computed during registration by analyzing method signatures for:
    /// - `Self` in return position (can't know size at runtime)
    /// - `Self` in parameter position except receiver (can't verify type match)
    /// - Generic methods (require monomorphization, incompatible with vtable)
    pub object_safety_violations: Vec<ObjectSafetyViolation>,

    /// Source location of the definition.
    pub span: Span,
}

impl TraitEntry {
    /// Check if this trait can be used as a trait object.
    ///
    /// A trait is object-safe if none of its methods violate the three rules:
    /// 1. No `Self` in return position
    /// 2. No `Self` in parameter position (except receiver)
    /// 3. No generic methods
    #[inline]
    pub fn is_object_safe(&self) -> bool {
        self.object_safety_violations.is_empty()
    }
}

/// A reason why a trait is not object-safe.
///
/// Each variant corresponds to a rule that the trait violates.
/// A trait with any violations cannot be used as a trait object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectSafetyViolation {
    /// Method returns `Self` — unknown size at runtime (Rule 1).
    SelfReturn {
        /// The method that returns `Self`.
        method: Name,
        /// Source location of the method.
        span: Span,
    },

    /// Method takes `Self` as a non-receiver parameter — can't verify type
    /// match at runtime (Rule 2).
    SelfParam {
        /// The method with `Self` parameter.
        method: Name,
        /// The parameter name that has `Self` type.
        param: Name,
        /// Source location of the method.
        span: Span,
    },

    /// Method has its own generic type parameters — requires monomorphization,
    /// which is incompatible with vtable dispatch (Rule 3).
    GenericMethod {
        /// The method with generic parameters.
        method: Name,
        /// Source location of the method.
        span: Span,
    },
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

    /// Concrete type arguments for the trait (e.g., `[INT, STR]` for
    /// `impl Index<int, str> for T`). Empty for non-generic traits or
    /// inherent impls. Used by coherence checking to distinguish different
    /// instantiations of the same generic trait.
    pub trait_type_args: Vec<Idx>,

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

    /// How specific this implementation is (Concrete > Constrained > Generic).
    pub specificity: ImplSpecificity,

    /// Source location.
    pub span: Span,
}

/// How specific a trait implementation is.
///
/// Used for overlap detection: when multiple impls could apply, the most
/// specific one wins. Equal-specificity impls for the same trait are an
/// overlap error (E2021).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImplSpecificity {
    /// `impl<T> Trait for T` — applies to all types.
    Generic = 0,
    /// `impl<T: Bound> Trait for T` — applies to types satisfying bounds.
    Constrained = 1,
    /// `impl Trait for ConcreteType` — applies to exactly one type.
    Concrete = 2,
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
        let trait_idx = self.traits.len();
        self.traits_by_name.insert(name, trait_idx);
        self.traits_by_idx.insert(idx, trait_idx);
        self.traits.push(entry);
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
        self.traits_by_name
            .get(&name)
            .and_then(|&i| self.traits.get(i))
    }

    /// Look up a trait by pool index.
    #[inline]
    pub fn get_trait_by_idx(&self, idx: Idx) -> Option<&TraitEntry> {
        self.traits_by_idx
            .get(&idx)
            .and_then(|&i| self.traits.get(i))
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

    // === Super-trait Queries ===

    /// Collect all super-traits transitively (DAG walk with cycle protection).
    ///
    /// Returns a de-duplicated list of all ancestor trait indices, in
    /// breadth-first order. Gracefully handles missing traits (returns empty
    /// for unknown indices, allowing registration-order independence).
    pub fn all_super_traits(&self, trait_idx: Idx) -> Vec<Idx> {
        let mut visited = FxHashSet::default();
        let mut result = Vec::new();
        let mut queue = std::collections::VecDeque::new();

        // Seed with direct super-traits
        if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            for &st in &entry.super_traits {
                if visited.insert(st) {
                    queue.push_back(st);
                }
            }
        }

        while let Some(idx) = queue.pop_front() {
            result.push(idx);
            if let Some(entry) = self.get_trait_by_idx(idx) {
                for &st in &entry.super_traits {
                    if visited.insert(st) {
                        queue.push_back(st);
                    }
                }
            }
        }

        result
    }

    /// Gather methods from a trait and all its ancestors, deduplicating by name.
    ///
    /// Closest override wins: methods defined on the trait itself take priority
    /// over those inherited from super-traits. Returns `(method_name,
    /// owning_trait_idx, &TraitMethodDef)` tuples.
    pub fn collected_methods(&self, trait_idx: Idx) -> Vec<(Name, Idx, &TraitMethodDef)> {
        let mut seen = FxHashSet::default();
        let mut result = Vec::new();

        // Direct methods first (highest priority)
        if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            for (name, method) in &entry.methods {
                if seen.insert(*name) {
                    result.push((*name, trait_idx, method));
                }
            }
        }

        // Walk ancestors in BFS order; skip already-seen names
        for ancestor_idx in self.all_super_traits(trait_idx) {
            if let Some(entry) = self.get_trait_by_idx(ancestor_idx) {
                for (name, method) in &entry.methods {
                    if seen.insert(*name) {
                        result.push((*name, ancestor_idx, method));
                    }
                }
            }
        }

        result
    }

    /// Find methods with conflicting defaults from different super-trait paths.
    ///
    /// Returns `(method_name, [conflicting_trait_indices])` for each method
    /// that has default implementations from multiple unrelated super-trait
    /// paths. Only includes methods where the conflict isn't resolved by the
    /// trait itself (i.e., the trait's own methods are excluded from conflict
    /// detection since they are the resolution).
    pub fn find_conflicting_defaults(&self, trait_idx: Idx) -> Vec<(Name, Vec<Idx>)> {
        // Track which methods we've seen defaults for, and from which traits
        let mut defaults_by_method: FxHashMap<Name, Vec<Idx>> = FxHashMap::default();

        // Collect defaults from all direct super-traits and their ancestors.
        // Only look at direct super-traits' collected_methods — each super-trait
        // provides its resolved set (closest override wins within its branch).
        let direct_supers = self
            .get_trait_by_idx(trait_idx)
            .map(|e| e.super_traits.clone())
            .unwrap_or_default();

        for &super_idx in &direct_supers {
            for (name, _owner, method) in self.collected_methods(super_idx) {
                if method.has_default {
                    defaults_by_method.entry(name).or_default().push(super_idx);
                }
            }
        }

        // Methods defined directly on this trait are NOT conflicting — they
        // serve as the resolution. Also, if this trait declares a method
        // (default or not), it overrides any inherited version.
        if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            for name in entry.methods.keys() {
                defaults_by_method.remove(name);
            }
        }

        // Conflicting: >1 distinct super-trait provides a default for the same method
        defaults_by_method
            .into_iter()
            .filter(|(_, providers)| providers.len() > 1)
            .collect()
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
        self.find_impl_with_args(trait_idx, self_type, &[])
    }

    /// Find an implementation of a generic trait for a specific type, matching
    /// concrete type arguments.
    ///
    /// For non-generic traits, pass an empty `trait_type_args` slice.
    /// For generic traits like `Index<Key, Value>`, pass the resolved type
    /// arguments to distinguish between different instantiations.
    pub fn find_impl_with_args(
        &self,
        trait_idx: Idx,
        self_type: Idx,
        trait_type_args: &[Idx],
    ) -> Option<(usize, &ImplEntry)> {
        self.impls_by_type.get(&self_type).and_then(|indices| {
            indices.iter().find_map(|&i| {
                let entry = &self.impls[i];
                if entry.trait_idx == Some(trait_idx)
                    && (trait_type_args.is_empty()
                        || entry.trait_type_args.is_empty()
                        || entry.trait_type_args == trait_type_args)
                {
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

    /// Look up a method with ambiguity detection.
    ///
    /// Like `lookup_method()`, but instead of returning the first trait match,
    /// collects ALL trait impls that provide the method. Returns `Ambiguous`
    /// when multiple trait impls match.
    pub fn lookup_method_checked(
        &self,
        self_type: Idx,
        method_name: Name,
    ) -> MethodLookupResult<'_> {
        // 1. Inherent impl always wins (no ambiguity possible)
        if let Some((impl_idx, impl_entry)) = self.inherent_impl(self_type) {
            if let Some(method) = impl_entry.methods.get(&method_name) {
                return MethodLookupResult::Found(MethodLookup::Inherent { impl_idx, method });
            }
        }

        // 2. Collect ALL trait impls with the method + their specificity
        let mut candidates: Vec<(usize, Idx, &ImplMethodDef, ImplSpecificity)> = Vec::new();
        for (impl_idx, impl_entry) in self
            .impls_by_type
            .get(&self_type)
            .into_iter()
            .flat_map(|indices| indices.iter())
            .filter_map(|&i| Some((i, self.impls.get(i)?)))
        {
            if let Some(method) = impl_entry.methods.get(&method_name) {
                if let Some(trait_idx) = impl_entry.trait_idx {
                    candidates.push((impl_idx, trait_idx, method, impl_entry.specificity));
                }
            }
        }

        match candidates.len() {
            0 => MethodLookupResult::NotFound,
            1 => {
                let (impl_idx, trait_idx, method, _) = candidates[0];
                MethodLookupResult::Found(MethodLookup::Trait {
                    trait_idx,
                    impl_idx,
                    method,
                })
            }
            _ => {
                // Multiple candidates: first filter by super-trait relationships.
                // If trait A is a super-trait of trait B and both provide the method,
                // keep only B (the sub-trait inherits or overrides A's method).
                let trait_idxs: Vec<Idx> = candidates.iter().map(|c| c.1).collect();
                let mut superseded: FxHashSet<Idx> = FxHashSet::default();
                for &t in &trait_idxs {
                    let supers = self.all_super_traits(t);
                    for &s in &supers {
                        if trait_idxs.contains(&s) {
                            superseded.insert(s);
                        }
                    }
                }
                let candidates: Vec<_> = candidates
                    .into_iter()
                    .filter(|c| !superseded.contains(&c.1))
                    .collect();

                if candidates.len() == 1 {
                    let (impl_idx, trait_idx, method, _) = candidates[0];
                    return MethodLookupResult::Found(MethodLookup::Trait {
                        trait_idx,
                        impl_idx,
                        method,
                    });
                }

                // Then try to disambiguate by specificity.
                // Keep only the most-specific candidates.
                let max_spec = candidates
                    .iter()
                    .map(|c| c.3)
                    .max()
                    .unwrap_or(ImplSpecificity::Generic);
                let best: Vec<_> = candidates.iter().filter(|c| c.3 == max_spec).collect();

                if best.len() == 1 {
                    let (impl_idx, trait_idx, method, _) = *best[0];
                    MethodLookupResult::Found(MethodLookup::Trait {
                        trait_idx,
                        impl_idx,
                        method,
                    })
                } else {
                    let trait_candidates: Vec<(Idx, Name)> = best
                        .iter()
                        .filter_map(|&&(_, trait_idx, _, _)| {
                            let name = self.get_trait_by_idx(trait_idx)?.name;
                            Some((trait_idx, name))
                        })
                        .collect();
                    MethodLookupResult::Ambiguous {
                        candidates: trait_candidates,
                    }
                }
            }
        }
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
        self.traits_by_name
            .values()
            .filter_map(|&i| self.traits.get(i))
    }

    /// Iterate over all implementations.
    pub fn iter_impls(&self) -> impl Iterator<Item = &ImplEntry> {
        self.impls.iter()
    }

    /// Get the number of registered traits.
    #[inline]
    pub fn trait_count(&self) -> usize {
        self.traits.len()
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

/// Result of a checked method lookup (with ambiguity detection).
#[derive(Clone, Debug)]
pub enum MethodLookupResult<'a> {
    /// Exactly one method found (inherent or trait).
    Found(MethodLookup<'a>),

    /// Multiple trait impls provide the same method for this type.
    Ambiguous {
        /// The trait indices and names that provide the method.
        candidates: Vec<(Idx, Name)>,
    },

    /// No method found in any impl.
    NotFound,
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
mod tests;
