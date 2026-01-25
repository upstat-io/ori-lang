//! Registry for user-defined types.
//!
//! The TypeRegistry maintains a centralized collection of all user-defined types
//! (structs, enums, type aliases) encountered during compilation. It provides:
//!
//! - Registration of new user types
//! - TypeId generation for compound types
//! - Lookup of type definitions by name or TypeId
//!
//! # Salsa Compatibility
//! All types implement Clone, Eq, Hash for use in query results.

use crate::ir::{Name, Span, TypeId};
use crate::types::Type;
use std::collections::HashMap;

/// Kind of user-defined type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeKind {
    /// Struct type with named fields.
    Struct {
        fields: Vec<(Name, Type)>,
    },
    /// Sum type (enum) with variants.
    Enum {
        variants: Vec<VariantDef>,
    },
    /// Type alias (newtype).
    Alias {
        target: Type,
    },
}

/// Variant definition for enum types.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct VariantDef {
    /// Variant name.
    pub name: Name,
    /// Variant fields (empty for unit variants).
    pub fields: Vec<(Name, Type)>,
}

/// Entry for a user-defined type.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeEntry {
    /// Type name.
    pub name: Name,
    /// The assigned TypeId for this type.
    pub type_id: TypeId,
    /// Kind of type (struct, enum, alias).
    pub kind: TypeKind,
    /// Source location of the type definition.
    pub span: Span,
    /// Generic type parameters (if any).
    pub type_params: Vec<Name>,
}

/// Registry for user-defined types.
///
/// Maintains a mapping from type names to their definitions, and generates
/// unique TypeIds for compound types.
///
/// # Salsa Compatibility
/// Has Clone, Eq, PartialEq, Debug for use in query results.
/// Note: HashMap doesn't implement Hash, so TypeRegistry can't either.
/// Salsa queries that return TypeRegistry should use interior mutability
/// or return individual TypeEntry values instead.
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct TypeRegistry {
    /// Types indexed by name.
    types_by_name: HashMap<Name, TypeEntry>,
    /// Types indexed by TypeId.
    types_by_id: HashMap<TypeId, TypeEntry>,
    /// Next available TypeId for compound types.
    next_type_id: u32,
}

impl TypeRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        TypeRegistry {
            types_by_name: HashMap::new(),
            types_by_id: HashMap::new(),
            next_type_id: TypeId::FIRST_COMPOUND,
        }
    }

    /// Generate the next available TypeId for a compound type.
    fn next_id(&mut self) -> TypeId {
        let id = TypeId::new(self.next_type_id);
        self.next_type_id += 1;
        id
    }

    /// Register a struct type.
    ///
    /// Returns the assigned TypeId.
    pub fn register_struct(
        &mut self,
        name: Name,
        fields: Vec<(Name, Type)>,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let type_id = self.next_id();
        let entry = TypeEntry {
            name,
            type_id,
            kind: TypeKind::Struct { fields },
            span,
            type_params,
        };
        self.types_by_name.insert(name, entry.clone());
        self.types_by_id.insert(type_id, entry);
        type_id
    }

    /// Register an enum type.
    ///
    /// Returns the assigned TypeId.
    pub fn register_enum(
        &mut self,
        name: Name,
        variants: Vec<VariantDef>,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let type_id = self.next_id();
        let entry = TypeEntry {
            name,
            type_id,
            kind: TypeKind::Enum { variants },
            span,
            type_params,
        };
        self.types_by_name.insert(name, entry.clone());
        self.types_by_id.insert(type_id, entry);
        type_id
    }

    /// Register a type alias.
    ///
    /// Returns the assigned TypeId.
    pub fn register_alias(
        &mut self,
        name: Name,
        target: Type,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let type_id = self.next_id();
        let entry = TypeEntry {
            name,
            type_id,
            kind: TypeKind::Alias { target },
            span,
            type_params,
        };
        self.types_by_name.insert(name, entry.clone());
        self.types_by_id.insert(type_id, entry);
        type_id
    }

    /// Look up a type entry by name.
    pub fn get_by_name(&self, name: Name) -> Option<&TypeEntry> {
        self.types_by_name.get(&name)
    }

    /// Look up a type entry by TypeId.
    pub fn get_by_id(&self, type_id: TypeId) -> Option<&TypeEntry> {
        self.types_by_id.get(&type_id)
    }

    /// Check if a type name is already registered.
    pub fn contains(&self, name: Name) -> bool {
        self.types_by_name.contains_key(&name)
    }

    /// Get the number of registered types.
    pub fn len(&self) -> usize {
        self.types_by_name.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.types_by_name.is_empty()
    }

    /// Iterate over all registered types.
    pub fn iter(&self) -> impl Iterator<Item = &TypeEntry> {
        self.types_by_name.values()
    }

    /// Convert a registered type to the type checker's Type representation.
    ///
    /// For struct and enum types, returns Type::Named(name).
    /// For aliases, returns the target type directly.
    pub fn to_type(&self, type_id: TypeId) -> Option<Type> {
        self.get_by_id(type_id).map(|entry| {
            match &entry.kind {
                TypeKind::Struct { .. } | TypeKind::Enum { .. } => {
                    Type::Named(entry.name)
                }
                TypeKind::Alias { target } => target.clone(),
            }
        })
    }
}

// =============================================================================
// Trait Registry
// =============================================================================

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
    /// Trait implementations: (trait_name, self_type) -> ImplEntry.
    /// For inherent impls, trait_name is stored as the self type's name.
    trait_impls: HashMap<(Name, String), ImplEntry>,
    /// Inherent implementations by type name.
    inherent_impls: HashMap<String, ImplEntry>,
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
        let type_key = format!("{:?}", entry.self_ty);

        if let Some(trait_name) = entry.trait_name {
            // Trait implementation - check for duplicate
            let key = (trait_name, type_key);
            if let Some(existing) = self.trait_impls.get(&key) {
                return Err(CoherenceError {
                    message: format!(
                        "conflicting implementation: trait already implemented for this type"
                    ),
                    span: entry.span,
                    existing_span: existing.span,
                });
            }
            self.trait_impls.insert(key, entry);
        } else {
            // Inherent implementation - check for duplicate methods
            if let Some(existing) = self.inherent_impls.get(&type_key) {
                // Check if any methods conflict
                for new_method in &entry.methods {
                    if existing.methods.iter().any(|m| m.name == new_method.name) {
                        return Err(CoherenceError {
                            message: format!(
                                "conflicting implementation: method already defined for this type"
                            ),
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
        let type_key = format!("{:?}", self_ty);
        self.trait_impls.get(&(trait_name, type_key))
    }

    /// Find inherent implementation for a type.
    pub fn get_inherent_impl(&self, self_ty: &Type) -> Option<&ImplEntry> {
        let type_key = format!("{:?}", self_ty);
        self.inherent_impls.get(&type_key)
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
        let type_key = format!("{:?}", self_ty);
        for ((trait_name, impl_type_key), impl_entry) in &self.trait_impls {
            if impl_type_key == &type_key {
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
mod tests {
    use super::*;
    use crate::ir::SharedInterner;

    fn make_span() -> Span {
        Span::new(0, 0)
    }

    #[test]
    fn test_registry_creation() {
        let registry = TypeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_struct() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let point_name = interner.intern("Point");
        let x_name = interner.intern("x");
        let y_name = interner.intern("y");

        let fields = vec![
            (x_name, Type::Int),
            (y_name, Type::Int),
        ];

        let type_id = registry.register_struct(point_name, fields.clone(), make_span(), vec![]);

        assert!(!type_id.is_primitive());
        assert_eq!(registry.len(), 1);

        let entry = registry.get_by_name(point_name).unwrap();
        assert_eq!(entry.name, point_name);
        assert_eq!(entry.type_id, type_id);

        if let TypeKind::Struct { fields: entry_fields } = &entry.kind {
            assert_eq!(entry_fields.len(), 2);
            assert_eq!(entry_fields[0].0, x_name);
            assert_eq!(entry_fields[0].1, Type::Int);
        } else {
            panic!("Expected struct type");
        }
    }

    #[test]
    fn test_register_enum() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let result_name = interner.intern("MyResult");
        let ok_name = interner.intern("Ok");
        let err_name = interner.intern("Err");
        let value_name = interner.intern("value");

        let variants = vec![
            VariantDef {
                name: ok_name,
                fields: vec![(value_name, Type::Int)],
            },
            VariantDef {
                name: err_name,
                fields: vec![(value_name, Type::Str)],
            },
        ];

        let type_id = registry.register_enum(result_name, variants, make_span(), vec![]);

        assert!(!type_id.is_primitive());
        assert!(registry.contains(result_name));

        let entry = registry.get_by_id(type_id).unwrap();
        if let TypeKind::Enum { variants: entry_variants } = &entry.kind {
            assert_eq!(entry_variants.len(), 2);
            assert_eq!(entry_variants[0].name, ok_name);
        } else {
            panic!("Expected enum type");
        }
    }

    #[test]
    fn test_register_alias() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let id_name = interner.intern("UserId");
        let type_id = registry.register_alias(id_name, Type::Int, make_span(), vec![]);

        assert!(!type_id.is_primitive());

        let entry = registry.get_by_name(id_name).unwrap();
        if let TypeKind::Alias { target } = &entry.kind {
            assert_eq!(*target, Type::Int);
        } else {
            panic!("Expected alias type");
        }
    }

    #[test]
    fn test_unique_type_ids() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let name1 = interner.intern("Type1");
        let name2 = interner.intern("Type2");
        let name3 = interner.intern("Type3");

        let id1 = registry.register_struct(name1, vec![], make_span(), vec![]);
        let id2 = registry.register_enum(name2, vec![], make_span(), vec![]);
        let id3 = registry.register_alias(name3, Type::Int, make_span(), vec![]);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_to_type_struct() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let point_name = interner.intern("Point");
        let type_id = registry.register_struct(point_name, vec![], make_span(), vec![]);

        let typ = registry.to_type(type_id).unwrap();
        assert_eq!(typ, Type::Named(point_name));
    }

    #[test]
    fn test_to_type_alias() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let id_name = interner.intern("UserId");
        let type_id = registry.register_alias(id_name, Type::Int, make_span(), vec![]);

        let typ = registry.to_type(type_id).unwrap();
        assert_eq!(typ, Type::Int);
    }

    #[test]
    fn test_generic_type_params() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let container_name = interner.intern("Container");
        let t_name = interner.intern("T");

        let type_id = registry.register_struct(
            container_name,
            vec![],
            make_span(),
            vec![t_name],
        );

        let entry = registry.get_by_id(type_id).unwrap();
        assert_eq!(entry.type_params.len(), 1);
        assert_eq!(entry.type_params[0], t_name);
    }

    #[test]
    fn test_iterate_types() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let name1 = interner.intern("A");
        let name2 = interner.intern("B");

        registry.register_struct(name1, vec![], make_span(), vec![]);
        registry.register_struct(name2, vec![], make_span(), vec![]);

        let names: Vec<_> = registry.iter().map(|e| e.name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&name1));
        assert!(names.contains(&name2));
    }

    #[test]
    fn test_salsa_traits() {
        let interner = SharedInterner::default();
        let mut registry1 = TypeRegistry::new();
        let mut registry2 = TypeRegistry::new();

        let name = interner.intern("Point");
        registry1.register_struct(name, vec![], make_span(), vec![]);
        registry2.register_struct(name, vec![], make_span(), vec![]);

        // Clone
        let cloned = registry1.clone();
        assert_eq!(cloned.len(), registry1.len());

        // Eq
        assert_eq!(registry1, registry2);
    }

    #[test]
    fn test_type_entry_hash() {
        use std::collections::HashSet;

        let interner = SharedInterner::default();
        let name = interner.intern("Point");

        let entry1 = TypeEntry {
            name,
            type_id: TypeId::new(TypeId::FIRST_COMPOUND),
            kind: TypeKind::Struct { fields: vec![] },
            span: make_span(),
            type_params: vec![],
        };
        let entry2 = entry1.clone();

        let mut set = HashSet::new();
        set.insert(entry1);
        set.insert(entry2); // duplicate
        assert_eq!(set.len(), 1);
    }

    // =========================================================================
    // Trait Registry Tests
    // =========================================================================

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
        };
        assert!(registry.register_impl(impl2).is_ok());

        // Both methods should be accessible
        assert!(registry.lookup_method(&Type::Named(point), method1).is_some());
        assert!(registry.lookup_method(&Type::Named(point), method2).is_some());
    }
}
