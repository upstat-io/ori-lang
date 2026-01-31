//! Trait and Implementation Registry
//!
//! Maintains mappings for:
//! - Trait definitions by name
//! - Implementations indexed by (trait, type) pair
//! - Inherent implementations indexed by type
//!
//! # Type Interning
//! Method parameter types and return types are stored as `TypeId` for efficient
//! equality comparisons. The type interner is used to convert between `Type` and `TypeId`.
//!
//! # Salsa Compatibility
//! All types implement Clone, Eq, Hash for use in query results.

pub use super::impl_types::{CoherenceError, ImplAssocTypeDef, ImplEntry, ImplMethodDef};
pub use super::method_lookup::MethodLookup;
pub use super::trait_types::{TraitAssocTypeDef, TraitEntry, TraitMethodDef};

use ori_ir::Name;
use ori_types::{SharedTypeInterner, Type, TypeInterner};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::collections::HashSet;

/// Registry for traits and implementations.
///
/// Maintains mappings for:
/// - Trait definitions by name
/// - Implementations indexed by (trait, type) pair
/// - Inherent implementations indexed by type
///
/// # Type Interning
/// The registry stores method types as `TypeId` and uses the interner
/// for Type↔TypeId conversions at API boundaries.
#[derive(Clone, Debug)]
pub struct TraitRegistry {
    /// Trait definitions by name.
    traits: FxHashMap<Name, TraitEntry>,
    /// Trait implementations: (`trait_name`, `self_type`) -> `ImplEntry`.
    trait_impls: FxHashMap<(Name, Type), ImplEntry>,
    /// Inherent implementations by type.
    inherent_impls: FxHashMap<Type, ImplEntry>,
    /// Type interner for Type↔TypeId conversions.
    interner: SharedTypeInterner,
    /// Secondary index: type -> traits it implements.
    ///
    /// Enables O(1) lookup of which traits a type implements, avoiding O(n) scan
    /// of all `trait_impls` entries. Updated when implementations are registered.
    traits_by_type: FxHashMap<Type, Vec<Name>>,
    /// Secondary index: `method_name` -> traits with that default method.
    ///
    /// Enables O(k) lookup of traits with a specific default method, where k is the
    /// number of traits with that default method name. Avoids O(n) scan of all traits.
    /// Updated when traits are registered.
    default_methods_by_name: FxHashMap<Name, Vec<Name>>,
    /// Lazily-populated method lookup cache: `(self_type, method_name)` -> cached result.
    ///
    /// Uses `RefCell` for interior mutability so `lookup_method` can remain `&self`.
    /// Cache entries store `Option<MethodLookup>`: `None` means the method was looked up
    /// but not found. The cache is invalidated (cleared) whenever traits or impls are registered.
    method_cache: RefCell<FxHashMap<(Type, Name), Option<MethodLookup>>>,
    /// Secondary index: `(type_name, assoc_type_name)` -> `TypeId`.
    ///
    /// Enables O(1) lookup of associated types by type and name, avoiding O(n*m) scan
    /// of all trait impls. Updated when implementations with associated types are registered.
    assoc_types_index: FxHashMap<(Name, Name), ori_ir::TypeId>,
    /// Default implementations for traits: `trait_name` -> `ImplEntry`.
    ///
    /// Stores `def impl TraitName { ... }` blocks. These provide stateless default
    /// implementations for capability traits. Methods are called as `TraitName.method()`.
    def_impls: FxHashMap<Name, ImplEntry>,
}

impl PartialEq for TraitRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.traits == other.traits
            && self.trait_impls == other.trait_impls
            && self.inherent_impls == other.inherent_impls
        // Interner, traits_by_type, and method_cache are not compared - they are derived state
    }
}

impl Eq for TraitRegistry {}

impl Default for TraitRegistry {
    fn default() -> Self {
        TraitRegistry {
            traits: FxHashMap::default(),
            trait_impls: FxHashMap::default(),
            inherent_impls: FxHashMap::default(),
            interner: SharedTypeInterner::new(),
            traits_by_type: FxHashMap::default(),
            default_methods_by_name: FxHashMap::default(),
            method_cache: RefCell::new(FxHashMap::default()),
            assoc_types_index: FxHashMap::default(),
            def_impls: FxHashMap::default(),
        }
    }
}

impl TraitRegistry {
    /// Create a new empty trait registry with a new type interner.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new empty registry with a shared type interner.
    ///
    /// Use this when you want to share the interner with other compiler phases.
    pub fn with_interner(interner: SharedTypeInterner) -> Self {
        TraitRegistry {
            traits: FxHashMap::default(),
            trait_impls: FxHashMap::default(),
            inherent_impls: FxHashMap::default(),
            interner,
            traits_by_type: FxHashMap::default(),
            default_methods_by_name: FxHashMap::default(),
            method_cache: RefCell::new(FxHashMap::default()),
            assoc_types_index: FxHashMap::default(),
            def_impls: FxHashMap::default(),
        }
    }

    /// Get a reference to the type interner.
    pub fn interner(&self) -> &TypeInterner {
        &self.interner
    }

    /// Register a trait definition.
    ///
    /// Updates the default methods index and invalidates the method cache
    /// since new default methods may affect lookups.
    pub fn register_trait(&mut self, entry: TraitEntry) {
        // Update secondary index for default methods
        for method in &entry.methods {
            if method.has_default {
                self.default_methods_by_name
                    .entry(method.name)
                    .or_default()
                    .push(entry.name);
            }
        }
        self.traits.insert(entry.name, entry);
        self.method_cache.borrow_mut().clear();
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
    /// Invalidates the method cache on success since new methods affect lookups.
    pub fn register_impl(&mut self, entry: ImplEntry) -> Result<(), CoherenceError> {
        let type_key = entry.self_ty.clone();

        if let Some(trait_name) = entry.trait_name {
            // Trait implementation - check for duplicate
            let key = (trait_name, type_key.clone());
            if let Some(existing) = self.trait_impls.get(&key) {
                return Err(CoherenceError {
                    message: "conflicting implementation: trait already implemented for this type"
                        .to_string(),
                    span: entry.span,
                    existing_span: existing.span,
                });
            }
            // Update secondary index: type -> traits it implements
            self.traits_by_type
                .entry(type_key.clone())
                .or_default()
                .push(trait_name);

            // Update associated types index for O(1) lookup
            if let Type::Named(type_name)
            | Type::Applied {
                name: type_name, ..
            } = &type_key
            {
                for assoc_def in &entry.assoc_types {
                    self.assoc_types_index
                        .insert((*type_name, assoc_def.name), assoc_def.ty);
                }
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
                            message:
                                "conflicting implementation: method already defined for this type"
                                    .to_string(),
                            span: entry.span,
                            existing_span: existing.span,
                        });
                    }
                }
                // No conflicts - merge methods into existing impl
                // (This allows multiple inherent impl blocks for the same type)
                let mut merged = existing.clone();
                merged.methods.extend(entry.methods);
                merged.rebuild_indices();
                self.inherent_impls.insert(type_key, merged);
            } else {
                self.inherent_impls.insert(type_key, entry);
            }
        }
        self.method_cache.borrow_mut().clear();
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
    ///
    /// Returns the method signature with types converted from `TypeId` to Type.
    ///
    /// Results are cached: the first lookup for a `(type, method_name)` pair performs the
    /// full scan through inherent impls, trait impls, and default methods, then caches the
    /// result. Subsequent lookups for the same pair return the cached value in O(1).
    /// The cache is invalidated whenever traits or implementations are registered.
    pub fn lookup_method(&self, self_ty: &Type, method_name: Name) -> Option<MethodLookup> {
        let cache_key = (self_ty.clone(), method_name);

        // Check the cache first
        if let Some(cached) = self.method_cache.borrow().get(&cache_key) {
            return cached.clone();
        }

        // Cache miss — perform the full lookup
        let result = self.lookup_method_uncached(self_ty, method_name);

        // Cache the result (including None, to avoid repeated misses)
        self.method_cache
            .borrow_mut()
            .insert(cache_key, result.clone());

        result
    }

    /// Perform method lookup without consulting the cache.
    ///
    /// Checks inherent impls first, then trait impls, then default methods.
    fn lookup_method_uncached(&self, self_ty: &Type, method_name: Name) -> Option<MethodLookup> {
        // First check inherent impls
        if let Some(impl_entry) = self.get_inherent_impl(self_ty) {
            if let Some(method) = impl_entry.get_method(method_name) {
                return Some(MethodLookup {
                    trait_name: None,
                    method_name,
                    params: method
                        .params
                        .iter()
                        .map(|id| self.interner.to_type(*id))
                        .collect(),
                    return_ty: self.interner.to_type(method.return_ty),
                });
            }
        }

        // Then check trait impls for this type using secondary index (O(k) where k = traits for type)
        if let Some(trait_names) = self.traits_by_type.get(self_ty) {
            for trait_name in trait_names {
                if let Some(impl_entry) = self.trait_impls.get(&(*trait_name, self_ty.clone())) {
                    if let Some(method) = impl_entry.get_method(method_name) {
                        return Some(MethodLookup {
                            trait_name: Some(*trait_name),
                            method_name,
                            params: method
                                .params
                                .iter()
                                .map(|id| self.interner.to_type(*id))
                                .collect(),
                            return_ty: self.interner.to_type(method.return_ty),
                        });
                    }
                }
            }
        }

        // Finally check if any trait has this as a default method using the index
        // This is O(k) where k is the number of traits with this default method name
        if let Some(trait_names) = self.default_methods_by_name.get(&method_name) {
            for trait_name in trait_names {
                if self.implements(self_ty, *trait_name) {
                    // Get the trait entry to access the method definition
                    if let Some(trait_entry) = self.traits.get(trait_name) {
                        if let Some(method) = trait_entry.get_method(method_name) {
                            return Some(MethodLookup {
                                trait_name: Some(*trait_name),
                                method_name,
                                params: method
                                    .params
                                    .iter()
                                    .map(|id| self.interner.to_type(*id))
                                    .collect(),
                                return_ty: self.interner.to_type(method.return_ty),
                            });
                        }
                    }
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
    /// with an associated type definition for `assoc_name`. The `TypeId` is
    /// converted to Type using the registry's interner.
    ///
    /// If the impl does not explicitly define the associated type but the trait
    /// has a default, the default is resolved with `Self` substituted for the
    /// implementing type.
    pub fn lookup_assoc_type(
        &self,
        self_ty: &Type,
        trait_name: Name,
        assoc_name: Name,
    ) -> Option<Type> {
        // Get the trait impl for this type
        let impl_entry = self.get_trait_impl(trait_name, self_ty)?;

        // First, check if the impl explicitly defines this associated type
        if let Some(at) = impl_entry
            .assoc_types
            .iter()
            .find(|at| at.name == assoc_name)
        {
            return Some(self.interner.to_type(at.ty));
        }

        // Not explicitly defined - check if the trait has a default
        let trait_entry = self.traits.get(&trait_name)?;
        let trait_assoc = trait_entry
            .assoc_types
            .iter()
            .find(|at| at.name == assoc_name)?;

        // If there's a default, resolve it with Self substituted
        trait_assoc
            .default_type
            .as_ref()
            .map(|default_parsed_type| {
                self.resolve_parsed_type_with_self_substitution(default_parsed_type, self_ty)
            })
    }

    /// Resolve a `ParsedType` to a `Type`, substituting `Self` with a concrete type.
    ///
    /// This is used when resolving default associated types that may contain `Self`.
    fn resolve_parsed_type_with_self_substitution(
        &self,
        parsed: &ori_ir::ParsedType,
        self_ty: &Type,
    ) -> Type {
        match parsed {
            ori_ir::ParsedType::SelfType => self_ty.clone(),
            ori_ir::ParsedType::Primitive(type_id) => self.interner.to_type(*type_id),
            ori_ir::ParsedType::Infer => Type::Var(ori_types::TypeVar(0)), // Fresh var - should rarely happen in defaults
            ori_ir::ParsedType::Named { name, type_args } => {
                if type_args.is_empty() {
                    Type::Named(*name)
                } else {
                    // For now, return Named without resolving type args
                    // Full resolution would require arena access
                    Type::Named(*name)
                }
            }
            ori_ir::ParsedType::List(_)
            | ori_ir::ParsedType::Tuple(_)
            | ori_ir::ParsedType::Function { .. }
            | ori_ir::ParsedType::Map { .. }
            | ori_ir::ParsedType::AssociatedType { .. } => {
                // Complex types in defaults - return Self as fallback
                // Full resolution would require arena access
                self_ty.clone()
            }
        }
    }

    /// Look up an associated type definition for a type by name only.
    ///
    /// Uses a secondary index for O(1) lookup, avoiding the O(n*m) scan
    /// of all trait implementations.
    ///
    /// This is used when we don't know which trait defines the associated type,
    /// such as when resolving `T.Item` from a where clause.
    pub fn lookup_assoc_type_by_name(&self, type_name: Name, assoc_name: Name) -> Option<Type> {
        // Use O(1) index lookup instead of scanning all trait impls
        self.assoc_types_index
            .get(&(type_name, assoc_name))
            .map(|&type_id| self.interner.to_type(type_id))
    }

    /// Look up an associated function on a type by name.
    ///
    /// Associated functions are methods without a `self` parameter, called on the type itself:
    /// `Point.origin()`, `Duration.from_seconds(s: 10)`.
    ///
    /// Returns `Some(MethodLookup)` if the type has an inherent impl with an associated function
    /// of the given name. Returns `None` if the function doesn't exist or if it's an instance
    /// method (has `self` parameter).
    pub fn lookup_associated_function(
        &self,
        type_name: Name,
        method_name: Name,
    ) -> Option<MethodLookup> {
        let self_ty = Type::Named(type_name);
        let impl_entry = self.get_inherent_impl(&self_ty)?;
        let method = impl_entry.get_associated_function(method_name)?;

        Some(MethodLookup {
            trait_name: None,
            method_name,
            params: method
                .params
                .iter()
                .map(|id| self.interner.to_type(*id))
                .collect(),
            return_ty: self.interner.to_type(method.return_ty),
        })
    }

    /// Check if a type has any associated functions defined.
    ///
    /// Returns `true` if the type has an inherent impl block with at least one
    /// method that doesn't have a `self` parameter.
    pub fn has_associated_functions(&self, type_name: Name) -> bool {
        let self_ty = Type::Named(type_name);
        self.get_inherent_impl(&self_ty)
            .is_some_and(|entry| entry.methods.iter().any(|m| m.is_associated))
    }

    /// Register a default implementation for a trait.
    ///
    /// This registers a `def impl TraitName { ... }` block, which provides
    /// stateless default methods that can be called as `TraitName.method()`.
    ///
    /// Returns an error if there's already a def impl for this trait.
    pub fn register_def_impl(
        &mut self,
        trait_name: Name,
        entry: ImplEntry,
    ) -> Result<(), CoherenceError> {
        if let Some(existing) = self.def_impls.get(&trait_name) {
            return Err(CoherenceError {
                message: "conflicting default implementation: trait already has a def impl"
                    .to_string(),
                span: entry.span,
                existing_span: existing.span,
            });
        }
        self.def_impls.insert(trait_name, entry);
        self.method_cache.borrow_mut().clear();
        Ok(())
    }

    /// Check if a trait has a default implementation (def impl).
    pub fn has_def_impl(&self, trait_name: Name) -> bool {
        self.def_impls.contains_key(&trait_name)
    }

    /// Look up a method in a trait's default implementation.
    ///
    /// Returns the method signature if the trait has a def impl with the method.
    pub fn lookup_def_impl_method(
        &self,
        trait_name: Name,
        method_name: Name,
    ) -> Option<MethodLookup> {
        let entry = self.def_impls.get(&trait_name)?;
        let method = entry.methods.iter().find(|m| m.name == method_name)?;
        Some(MethodLookup {
            trait_name: Some(trait_name),
            method_name,
            params: method
                .params
                .iter()
                .map(|id| self.interner.to_type(*id))
                .collect(),
            return_ty: self.interner.to_type(method.return_ty),
        })
    }
}
