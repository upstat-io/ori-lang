//! Read-only lookup paths for `TraitRegistry`.
//!
//! Trait/impl lookup, super-trait DAG walks, method resolution, and
//! coherence checks. Mutation (registration) lives in the parent
//! module's `impl TraitRegistry` block.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::Idx;

use super::{
    BoundChainLookup, ImplEntry, ImplMethodDef, ImplSpecificity, MethodLookup, MethodLookupResult,
    TraitAssocTypeDef, TraitEntry, TraitMethodDef, TraitRegistry,
};

impl TraitRegistry {
    // Trait Lookup

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

    /// Iterate `(impl_idx, &ImplEntry)` over every registered impl.
    ///
    /// Used by the base-name dispatch fallback in `infer/expr/calls/impl_lookup.rs`
    /// when exact-`Idx` `lookup_method_checked` misses on a generic impl whose
    /// `entry.self_type = Applied(Name, [Named(U)])` no longer matches a
    /// concrete receiver `Applied(Name, [int])`. The engine iterates, filters
    /// by base name + method name, then unifies the receiver against
    /// `entry.self_type` to produce the impl-level substitution map.
    #[inline]
    pub fn impls_iter(&self) -> impl Iterator<Item = (usize, &ImplEntry)> {
        self.impls.iter().enumerate()
    }

    /// Get an associated type definition from a trait.
    pub fn trait_assoc_type(&self, trait_idx: Idx, assoc_name: Name) -> Option<&TraitAssocTypeDef> {
        self.get_trait_by_idx(trait_idx)
            .and_then(|t| t.assoc_types.get(&assoc_name))
    }

    // Super-trait Queries

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
        //
        // Registration discipline: trait_idx MUST already be registered before
        // conflict-default analysis runs (Registration group pass 0c precedes
        // any consumer per CK-1). Missing entry = missing-registration bug,
        // NOT "no super-traits" — silently swallowing as empty Vec hid the
        // bug class. debug_assert in debug builds; production fallback returns
        // empty conflict set (consumer sees zero conflicts vs panicking).
        let direct_supers = if let Some(entry) = self.get_trait_by_idx(trait_idx) {
            entry.super_traits.clone()
        } else {
            debug_assert!(
                false,
                "find_conflicting_defaults called with unregistered trait_idx={trait_idx:?} — \
                 Registration group pass 0c must precede this query (CK-1)"
            );
            return Vec::new();
        };

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

    // Impl Lookup

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

    /// Project an associated-type binding for a concrete `self_type` carrying a
    /// `type <assoc_name> = …` binding. Used to resolve a cross-impl
    /// associated-type projection (`T.Assoc`) once the receiver type is
    /// concretely known.
    ///
    /// `trait_idx` disambiguates by `(trait_idx, self_type, assoc_name)` when a
    /// type implements two traits each declaring a same-named associated type:
    /// `Some(t)` prefers the impl whose `trait_idx == Some(t)`, falling back to
    /// the first binding only when no trait-matched impl carries it; `None`
    /// keeps the trait-blind first-match behavior (caller has no trait in scope).
    ///
    /// Returns `None` when no registered impl of `self_type` defines the
    /// associated type — the caller treats that as clean poison (`Idx::ERROR`).
    pub fn find_impl_assoc_binding(
        &self,
        trait_idx: Option<Idx>,
        self_type: Idx,
        assoc_name: Name,
    ) -> Option<Idx> {
        let indices = self.impls_by_type.get(&self_type)?;
        // Prefer the binding from the impl of the requested trait when known.
        if let Some(want) = trait_idx {
            if let Some(binding) = indices.iter().find_map(|&i| {
                let entry = &self.impls[i];
                if entry.trait_idx == Some(want) {
                    entry.assoc_types.get(&assoc_name).copied()
                } else {
                    None
                }
            }) {
                return Some(binding);
            }
        }
        // No trait given, or no trait-matched impl carried the binding: keep the
        // first-match fallback so existing single-trait cases are unaffected.
        indices
            .iter()
            .find_map(|&i| self.impls[i].assoc_types.get(&assoc_name).copied())
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

    // Bound-Chain Dispatch

    /// Find a trait method via the bound chain of a generic type parameter.
    ///
    /// When the receiver type is `Tag::RigidVar` (e.g., `T` in `@f<T: Clone>`),
    /// `lookup_method_checked` keyed on `Idx` always misses because `RigidVar`
    /// indices are never registered as `impls_by_type` keys. This helper walks
    /// the declared bounds (`type_param_bounds_for_var`) on the rigid var,
    /// resolves each bound trait name, and looks for a method matching
    /// `method_name` in that trait's `collected_methods` (covering supertrait
    /// inheritance per `BI-1`). Returns the first match — ambiguous bounds
    /// are detected by counting candidates across all bounds.
    ///
    /// Cycle protection: trait names are deduplicated via `FxHashSet`. Depth
    /// guard is implicit in the bound list (which is finite per
    /// `FunctionSig.type_param_bounds`).
    ///
    /// `type_param_bounds_for_var` is the slice from
    /// `FunctionSig.type_param_bounds[bound_idx]` where `bound_idx` is the
    /// index of the rigid var in `FunctionSig.scheme_var_ids` / `type_params`.
    /// Callers extract this from the current function signature.
    pub fn find_trait_method_via_bound_chain(
        &self,
        method_name: Name,
        type_param_bounds_for_var: &[Name],
    ) -> BoundChainLookup<'_> {
        let mut seen_traits: FxHashSet<Idx> = FxHashSet::default();
        let mut candidates: Vec<(Idx, &TraitMethodDef)> = Vec::new();

        for &trait_name in type_param_bounds_for_var {
            let Some(trait_entry) = self.get_trait_by_name(trait_name) else {
                continue;
            };
            let trait_idx = trait_entry.idx;
            if !seen_traits.insert(trait_idx) {
                continue;
            }
            for (mname, _owner_idx, method_def) in self.collected_methods(trait_idx) {
                if mname == method_name {
                    candidates.push((trait_idx, method_def));
                    break;
                }
            }
        }

        match candidates.len() {
            0 => BoundChainLookup::NotFound,
            1 => {
                let (trait_idx, method) = candidates[0];
                BoundChainLookup::Found { trait_idx, method }
            }
            _ => {
                let trait_candidates: Vec<(Idx, Name)> = candidates
                    .iter()
                    .filter_map(|&(trait_idx, _)| {
                        let name = self.get_trait_by_idx(trait_idx)?.name;
                        Some((trait_idx, name))
                    })
                    .collect();
                BoundChainLookup::Ambiguous {
                    candidates: trait_candidates,
                }
            }
        }
    }

    // Coherence

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

    // Iteration

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
