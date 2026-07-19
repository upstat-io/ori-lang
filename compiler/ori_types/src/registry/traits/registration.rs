//! Mutation (registration) paths for `TraitRegistry`.
//!
//! Trait-definition registration, object-safety-violation propagation, and
//! impl registration with self-type / trait indexing. Read-only lookup lives
//! in the sibling `lookup` module; the parent module owns the struct
//! definition and the `new` constructor.

use ori_ir::Name;

use super::{ImplEntry, ObjectSafetyViolation, RegisteredImplOrigin, TraitEntry, TraitRegistry};

impl TraitRegistry {
    // Trait Registration

    /// Register a trait definition.
    pub fn register_trait(&mut self, entry: TraitEntry) {
        let name = entry.name;
        let idx = entry.idx;
        let trait_idx = self.traits.len();
        self.traits_by_name.insert(name, trait_idx);
        self.traits_by_idx.insert(idx, trait_idx);
        self.traits.push(entry);
    }

    /// Append additional object-safety violations to a registered trait.
    ///
    /// Used by the second-phase `register_object_safety_violations` pass to
    /// propagate `GenericMethod` violations from super-traits to children
    /// (Spec: Clause 8.8).
    pub fn extend_object_safety_violations(
        &mut self,
        name: Name,
        additional: Vec<ObjectSafetyViolation>,
    ) {
        if let Some(&trait_idx) = self.traits_by_name.get(&name) {
            if let Some(entry) = self.traits.get_mut(trait_idx) {
                entry.object_safety_violations.extend(additional);
            }
        }
    }

    /// Register a trait implementation.
    ///
    /// Returns the impl index for reference.
    pub fn register_impl(&mut self, entry: ImplEntry) -> usize {
        self.register_impl_with_origin(entry, None)
    }

    /// Register an implementation with its exact checker-owned body origin.
    pub(crate) fn register_impl_with_origin(
        &mut self,
        entry: ImplEntry,
        origin: Option<RegisteredImplOrigin>,
    ) -> usize {
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
        self.impl_origins.push(origin);
        impl_idx
    }
}
