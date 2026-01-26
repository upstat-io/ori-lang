//! Trait and Implementation Registry
//!
//! Maintains mappings for:
//! - Trait definitions by name
//! - Implementations indexed by (trait, type) pair
//! - Inherent implementations indexed by type
//!
//! # Salsa Compatibility
//! All types implement Clone, Eq, Hash for use in query results.

use sigil_ir::{Name, Span};
use sigil_types::Type;
use std::collections::{HashMap, HashSet};

/// Method signature in a trait definition.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TraitMethodDef {
    /// Method name.
    pub name: Name,
    /// Parameter types (first is self type if present).
    pub params: Vec<Type>,
    /// Return type.
    pub return_ty: Type,
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
    /// Whether this trait is public.
    pub is_public: bool,
}

impl TraitEntry {
    /// Look up a method by name.
    pub fn get_method(&self, name: Name) -> Option<&TraitMethodDef> {
        self.methods.iter().find(|m| m.name == name)
    }
}

/// Implementation method.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplMethodDef {
    /// Method name.
    pub name: Name,
    /// Parameter types.
    pub params: Vec<Type>,
    /// Return type.
    pub return_ty: Type,
}

/// Associated type definition in an impl block (e.g., `type Item = T`).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ImplAssocTypeDef {
    /// Associated type name (e.g., `Item`).
    pub name: Name,
    /// Concrete type assigned (e.g., `T` or `int`).
    pub ty: Type,
}

/// Entry for an implementation block.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
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

/// Registry for traits and implementations.
///
/// Maintains mappings for:
/// - Trait definitions by name
/// - Implementations indexed by (trait, type) pair
/// - Inherent implementations indexed by type
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct TraitRegistry {
    /// Trait definitions by name.
    traits: HashMap<Name, TraitEntry>,
    /// Trait implementations: (`trait_name`, `self_type`) -> `ImplEntry`.
    trait_impls: HashMap<(Name, Type), ImplEntry>,
    /// Inherent implementations by type.
    inherent_impls: HashMap<Type, ImplEntry>,
}

impl TraitRegistry {
    /// Create a new empty trait registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a trait definition.
    pub fn register_trait(&mut self, entry: TraitEntry) {
        self.traits.insert(entry.name, entry);
    }

    /// Get a trait definition by name.
    pub fn get_trait(&self, name: Name) -> Option<&TraitEntry> {
        self.traits.get(&name)
    }

    /// Check if a trait exists.
    pub fn has_trait(&self, name: Name) -> bool {
        self.traits.contains_key(&name)
    }

    /// Register a trait implementation.
    ///
    /// Returns an error if there's already an impl for the same trait/type combination.
    pub fn register_impl(&mut self, entry: ImplEntry) -> Result<(), CoherenceError> {
        let type_key = entry.self_ty.clone();

        if let Some(trait_name) = entry.trait_name {
            // Trait implementation - check for duplicate
            let key = (trait_name, type_key);
            if let Some(existing) = self.trait_impls.get(&key) {
                return Err(CoherenceError {
                    message: "conflicting implementation: trait already implemented for this type".to_string(),
                    span: entry.span,
                    existing_span: existing.span,
                });
            }
            self.trait_impls.insert(key, entry);
        } else {
            // Inherent implementation - check for duplicate methods
            if let Some(existing) = self.inherent_impls.get(&type_key) {
                // Build set of existing method names for O(1) lookup
                let existing_names: HashSet<Name> =
                    existing.methods.iter().map(|m| m.name).collect();
                // Check if any methods conflict
                for new_method in &entry.methods {
                    if existing_names.contains(&new_method.name) {
                        return Err(CoherenceError {
                            message: "conflicting implementation: method already defined for this type".to_string(),
                            span: entry.span,
                            existing_span: existing.span,
                        });
                    }
                }
                // No conflicts - merge methods into existing impl
                // (This allows multiple inherent impl blocks for the same type)
                let mut merged = existing.clone();
                merged.methods.extend(entry.methods);
                self.inherent_impls.insert(type_key, merged);
            } else {
                self.inherent_impls.insert(type_key, entry);
            }
        }
        Ok(())
    }

    /// Find implementation of a trait for a type.
    pub fn get_trait_impl(&self, trait_name: Name, self_ty: &Type) -> Option<&ImplEntry> {
        self.trait_impls.get(&(trait_name, self_ty.clone()))
    }

    /// Find inherent implementation for a type.
    pub fn get_inherent_impl(&self, self_ty: &Type) -> Option<&ImplEntry> {
        self.inherent_impls.get(self_ty)
    }

    /// Check if a type implements a trait.
    pub fn implements(&self, self_ty: &Type, trait_name: Name) -> bool {
        self.get_trait_impl(trait_name, self_ty).is_some()
    }

    /// Look up a method on a type (checks inherent impls first, then trait impls).
    pub fn lookup_method(&self, self_ty: &Type, method_name: Name) -> Option<MethodLookup> {
        // First check inherent impls
        if let Some(impl_entry) = self.get_inherent_impl(self_ty) {
            if let Some(method) = impl_entry.methods.iter().find(|m| m.name == method_name) {
                return Some(MethodLookup {
                    trait_name: None,
                    method_name,
                    params: method.params.clone(),
                    return_ty: method.return_ty.clone(),
                });
            }
        }

        // Then check all trait impls for this type
        for ((trait_name, impl_type), impl_entry) in &self.trait_impls {
            if impl_type == self_ty {
                if let Some(method) = impl_entry.methods.iter().find(|m| m.name == method_name) {
                    return Some(MethodLookup {
                        trait_name: Some(*trait_name),
                        method_name,
                        params: method.params.clone(),
                        return_ty: method.return_ty.clone(),
                    });
                }
            }
        }

        // Finally check if any trait has this as a default method
        for (trait_name, trait_entry) in &self.traits {
            if let Some(method) = trait_entry.get_method(method_name) {
                if method.has_default && self.implements(self_ty, *trait_name) {
                    return Some(MethodLookup {
                        trait_name: Some(*trait_name),
                        method_name,
                        params: method.params.clone(),
                        return_ty: method.return_ty.clone(),
                    });
                }
            }
        }

        None
    }

    /// Iterate over all registered traits.
    pub fn iter_traits(&self) -> impl Iterator<Item = &TraitEntry> {
        self.traits.values()
    }

    /// Get the number of registered traits.
    pub fn trait_count(&self) -> usize {
        self.traits.len()
    }

    /// Get the number of registered implementations.
    pub fn impl_count(&self) -> usize {
        self.trait_impls.len() + self.inherent_impls.len()
    }

    /// Look up an associated type definition for a type implementing a trait.
    ///
    /// Returns `Some(concrete_type)` if the type has an impl for the trait
    /// with an associated type definition for `assoc_name`.
    pub fn lookup_assoc_type(
        &self,
        self_ty: &Type,
        trait_name: Name,
        assoc_name: Name,
    ) -> Option<Type> {
        // Get the trait impl for this type
        let impl_entry = self.get_trait_impl(trait_name, self_ty)?;

        // Find the associated type definition
        impl_entry.assoc_types
            .iter()
            .find(|at| at.name == assoc_name)
            .map(|at| at.ty.clone())
    }

    /// Look up an associated type definition for a type by name only.
    ///
    /// Searches all trait implementations for the given type to find one
    /// that defines an associated type with the given name.
    ///
    /// This is used when we don't know which trait defines the associated type,
    /// such as when resolving `T.Item` from a where clause.
    pub fn lookup_assoc_type_by_name(
        &self,
        type_name: Name,
        assoc_name: Name,
    ) -> Option<Type> {
        let target_type = Type::Named(type_name);

        // Search all trait impls for this type
        for ((_, impl_type), impl_entry) in &self.trait_impls {
            if impl_type == &target_type {
                // Check if this impl has the associated type we're looking for
                if let Some(assoc_def) = impl_entry.assoc_types.iter().find(|at| at.name == assoc_name) {
                    return Some(assoc_def.ty.clone());
                }
            }
        }

        None
    }
}

/// Result of a method lookup.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct MethodLookup {
    /// Trait providing the method (None for inherent methods).
    pub trait_name: Option<Name>,
    /// Method name.
    pub method_name: Name,
    /// Parameter types.
    pub params: Vec<Type>,
    /// Return type.
    pub return_ty: Type,
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use sigil_ir::SharedInterner;

    fn make_span() -> Span {
        Span::new(0, 0)
    }

    #[test]
    fn test_trait_registry_creation() {
        let registry = TraitRegistry::new();
        assert_eq!(registry.trait_count(), 0);
        assert_eq!(registry.impl_count(), 0);
    }

    #[test]
    fn test_register_trait() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let printable = interner.intern("Printable");
        let to_string = interner.intern("to_string");

        let entry = TraitEntry {
            name: printable,
            span: make_span(),
            type_params: vec![],
            super_traits: vec![],
            methods: vec![TraitMethodDef {
                name: to_string,
                params: vec![],
                return_ty: Type::Str,
                has_default: false,
            }],
            assoc_types: vec![],
            is_public: true,
        };

        registry.register_trait(entry);

        assert!(registry.has_trait(printable));
        assert_eq!(registry.trait_count(), 1);

        let retrieved = registry.get_trait(printable).unwrap();
        assert_eq!(retrieved.methods.len(), 1);
        assert_eq!(retrieved.methods[0].name, to_string);
    }

    #[test]
    fn test_register_inherent_impl() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let point = interner.intern("Point");
        let new_name = interner.intern("new");

        let entry = ImplEntry {
            trait_name: None,
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: new_name,
                params: vec![Type::Int, Type::Int],
                return_ty: Type::Named(point),
            }],
            assoc_types: vec![],
        };

        registry.register_impl(entry).unwrap();
        assert_eq!(registry.impl_count(), 1);

        let lookup = registry.lookup_method(&Type::Named(point), new_name);
        assert!(lookup.is_some());
        assert!(lookup.unwrap().trait_name.is_none());
    }

    #[test]
    fn test_register_trait_impl() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let printable = interner.intern("Printable");
        let to_string = interner.intern("to_string");
        let point = interner.intern("Point");

        // First register the trait
        let trait_entry = TraitEntry {
            name: printable,
            span: make_span(),
            type_params: vec![],
            super_traits: vec![],
            methods: vec![TraitMethodDef {
                name: to_string,
                params: vec![],
                return_ty: Type::Str,
                has_default: false,
            }],
            assoc_types: vec![],
            is_public: true,
        };
        registry.register_trait(trait_entry);

        // Then register the impl
        let impl_entry = ImplEntry {
            trait_name: Some(printable),
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: to_string,
                params: vec![],
                return_ty: Type::Str,
            }],
            assoc_types: vec![],
        };
        registry.register_impl(impl_entry).unwrap();

        assert!(registry.implements(&Type::Named(point), printable));

        let lookup = registry.lookup_method(&Type::Named(point), to_string);
        assert!(lookup.is_some());
        let lookup = lookup.unwrap();
        assert_eq!(lookup.trait_name, Some(printable));
        assert_eq!(lookup.return_ty, Type::Str);
    }

    #[test]
    fn test_method_lookup_priority() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let point = interner.intern("Point");
        let describe = interner.intern("describe");

        // Register inherent impl
        let inherent_entry = ImplEntry {
            trait_name: None,
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: describe,
                params: vec![],
                return_ty: Type::Str,
            }],
            assoc_types: vec![],
        };
        registry.register_impl(inherent_entry).unwrap();

        // Lookup should find inherent method (no trait)
        let lookup = registry.lookup_method(&Type::Named(point), describe).unwrap();
        assert!(lookup.trait_name.is_none());
    }

    #[test]
    fn test_coherence_duplicate_trait_impl() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let printable = interner.intern("Printable");
        let to_string = interner.intern("to_string");
        let point = interner.intern("Point");

        // Register the trait
        let trait_entry = TraitEntry {
            name: printable,
            span: make_span(),
            type_params: vec![],
            super_traits: vec![],
            methods: vec![TraitMethodDef {
                name: to_string,
                params: vec![],
                return_ty: Type::Str,
                has_default: false,
            }],
            assoc_types: vec![],
            is_public: true,
        };
        registry.register_trait(trait_entry);

        // First impl should succeed
        let impl1 = ImplEntry {
            trait_name: Some(printable),
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: to_string,
                params: vec![],
                return_ty: Type::Str,
            }],
            assoc_types: vec![],
        };
        assert!(registry.register_impl(impl1).is_ok());

        // Second impl for same trait/type should fail
        let impl2 = ImplEntry {
            trait_name: Some(printable),
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: to_string,
                params: vec![],
                return_ty: Type::Str,
            }],
            assoc_types: vec![],
        };
        assert!(registry.register_impl(impl2).is_err());
    }

    #[test]
    fn test_coherence_duplicate_inherent_method() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let point = interner.intern("Point");
        let describe = interner.intern("describe");

        // First inherent impl should succeed
        let impl1 = ImplEntry {
            trait_name: None,
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: describe,
                params: vec![],
                return_ty: Type::Str,
            }],
            assoc_types: vec![],
        };
        assert!(registry.register_impl(impl1).is_ok());

        // Second inherent impl with same method name should fail
        let impl2 = ImplEntry {
            trait_name: None,
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: describe,
                params: vec![],
                return_ty: Type::Int,
            }],
            assoc_types: vec![],
        };
        assert!(registry.register_impl(impl2).is_err());
    }

    #[test]
    fn test_coherence_multiple_inherent_impls_different_methods() {
        let interner = SharedInterner::default();
        let mut registry = TraitRegistry::new();

        let point = interner.intern("Point");
        let method1 = interner.intern("method1");
        let method2 = interner.intern("method2");

        // First inherent impl
        let impl1 = ImplEntry {
            trait_name: None,
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: method1,
                params: vec![],
                return_ty: Type::Int,
            }],
            assoc_types: vec![],
        };
        assert!(registry.register_impl(impl1).is_ok());

        // Second inherent impl with different method should succeed (methods get merged)
        let impl2 = ImplEntry {
            trait_name: None,
            self_ty: Type::Named(point),
            span: make_span(),
            type_params: vec![],
            methods: vec![ImplMethodDef {
                name: method2,
                params: vec![],
                return_ty: Type::Str,
            }],
            assoc_types: vec![],
        };
        assert!(registry.register_impl(impl2).is_ok());

        // Both methods should be accessible
        assert!(registry.lookup_method(&Type::Named(point), method1).is_some());
        assert!(registry.lookup_method(&Type::Named(point), method2).is_some());
    }
}
