//! Registry for user-defined types.
//!
//! The `TypeRegistry` maintains a centralized collection of all user-defined types
//! (structs, enums, type aliases) encountered during compilation. It provides:
//!
//! - Registration of new user types
//! - `TypeId` generation for compound types
//! - Lookup of type definitions by name or `TypeId`
//!
//! # Salsa Compatibility
//! All types implement Clone, Eq, Hash for use in query results.

mod impl_types;
mod method_lookup;
mod trait_registry;
mod trait_types;

#[cfg(test)]
mod tests;

use ori_ir::{Name, Span, TypeId};
use ori_types::{SharedTypeInterner, Type, TypeInterner};
use std::collections::HashMap;

pub use trait_registry::{
    CoherenceError, ImplAssocTypeDef, ImplEntry, ImplMethodDef, MethodLookup, TraitAssocTypeDef,
    TraitEntry, TraitMethodDef, TraitRegistry,
};

/// Kind of user-defined type.
///
/// Field types are stored as `TypeId` for efficient equality comparisons.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeKind {
    /// Struct type with named fields.
    Struct { fields: Vec<(Name, TypeId)> },
    /// Sum type (enum) with variants.
    Enum { variants: Vec<VariantDef> },
    /// Newtype: nominally distinct wrapper around an existing type.
    /// Unlike transparent aliases, newtypes have their own type identity.
    Newtype { underlying: TypeId },
}

/// Variant definition for enum types.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct VariantDef {
    /// Variant name.
    pub name: Name,
    /// Variant fields (empty for unit variants).
    pub fields: Vec<(Name, TypeId)>,
}

/// Entry for a user-defined type.
///
/// # Salsa Compatibility
/// Has Clone, Eq, Hash for use in query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeEntry {
    /// Type name.
    pub name: Name,
    /// The assigned `TypeId` for this type.
    pub type_id: TypeId,
    /// Kind of type (struct, enum, newtype).
    pub kind: TypeKind,
    /// Source location of the type definition.
    pub span: Span,
    /// Generic type parameters (if any).
    pub type_params: Vec<Name>,
}

/// Registry for user-defined types.
///
/// Maintains a mapping from type names to their definitions, and generates
/// unique `TypeIds` for compound types.
///
/// # Type Interning
/// The registry stores field types as `TypeId` for efficient equality comparisons.
/// The type interner is used to convert between `Type` and `TypeId`.
///
/// # Salsa Compatibility
/// Has Clone, Eq, `PartialEq`, Debug for use in query results.
/// Note: `HashMap` doesn't implement Hash, so `TypeRegistry` can't either.
/// Salsa queries that return `TypeRegistry` should use interior mutability
/// or return individual `TypeEntry` values instead.
#[derive(Clone, Debug)]
pub struct TypeRegistry {
    /// Types indexed by name.
    types_by_name: HashMap<Name, TypeEntry>,
    /// Types indexed by `TypeId`.
    types_by_id: HashMap<TypeId, TypeEntry>,
    /// Next available `TypeId` for compound types.
    next_type_id: u32,
    /// Type interner for Type↔TypeId conversions.
    interner: SharedTypeInterner,
}

impl PartialEq for TypeRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.types_by_name == other.types_by_name
            && self.types_by_id == other.types_by_id
            && self.next_type_id == other.next_type_id
        // Interner is not compared - two registries with the same data are equal
        // regardless of which interner they use
    }
}

impl Eq for TypeRegistry {}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeRegistry {
    /// Create a new empty registry with a new type interner.
    pub fn new() -> Self {
        TypeRegistry {
            types_by_name: HashMap::new(),
            types_by_id: HashMap::new(),
            next_type_id: TypeId::FIRST_COMPOUND,
            interner: SharedTypeInterner::new(),
        }
    }

    /// Create a new empty registry with a shared type interner.
    ///
    /// Use this when you want to share the interner with other compiler phases.
    pub fn with_interner(interner: SharedTypeInterner) -> Self {
        TypeRegistry {
            types_by_name: HashMap::new(),
            types_by_id: HashMap::new(),
            next_type_id: TypeId::FIRST_COMPOUND,
            interner,
        }
    }

    /// Get a reference to the type interner.
    pub fn interner(&self) -> &TypeInterner {
        &self.interner
    }

    /// Generate the next available `TypeId` for a compound type.
    fn next_id(&mut self) -> TypeId {
        let id = TypeId::new(self.next_type_id);
        self.next_type_id += 1;
        id
    }

    /// Internal helper to register a type entry.
    fn register_entry(
        &mut self,
        name: Name,
        kind: TypeKind,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let type_id = self.next_id();
        let entry = TypeEntry {
            name,
            type_id,
            kind,
            span,
            type_params,
        };
        self.types_by_name.insert(name, entry.clone());
        self.types_by_id.insert(type_id, entry);
        type_id
    }

    /// Register a struct type.
    ///
    /// Field types are converted to `TypeId` using the registry's interner.
    /// Returns the assigned `TypeId`.
    pub fn register_struct(
        &mut self,
        name: Name,
        fields: Vec<(Name, Type)>,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let field_ids: Vec<(Name, TypeId)> = fields
            .into_iter()
            .map(|(name, ty)| (name, ty.to_type_id(&self.interner)))
            .collect();
        self.register_entry(
            name,
            TypeKind::Struct { fields: field_ids },
            span,
            type_params,
        )
    }

    /// Register an enum type.
    ///
    /// Variant field types are converted to `TypeId` using the registry's interner.
    /// Returns the assigned `TypeId`.
    pub fn register_enum(
        &mut self,
        name: Name,
        variants: Vec<(Name, Vec<(Name, Type)>)>,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let variant_defs: Vec<VariantDef> = variants
            .into_iter()
            .map(|(vname, vfields)| {
                let field_ids: Vec<(Name, TypeId)> = vfields
                    .into_iter()
                    .map(|(fname, ty)| (fname, ty.to_type_id(&self.interner)))
                    .collect();
                VariantDef {
                    name: vname,
                    fields: field_ids,
                }
            })
            .collect();
        self.register_entry(
            name,
            TypeKind::Enum {
                variants: variant_defs,
            },
            span,
            type_params,
        )
    }

    /// Register a newtype (nominally distinct type wrapper).
    ///
    /// Underlying type is converted to `TypeId` using the registry's interner.
    /// Returns the assigned `TypeId`.
    pub fn register_newtype(
        &mut self,
        name: Name,
        underlying: &Type,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let underlying_id = underlying.to_type_id(&self.interner);
        self.register_entry(
            name,
            TypeKind::Newtype {
                underlying: underlying_id,
            },
            span,
            type_params,
        )
    }

    /// Look up a type entry by name.
    pub fn get_by_name(&self, name: Name) -> Option<&TypeEntry> {
        self.types_by_name.get(&name)
    }

    /// Look up a type entry by `TypeId`.
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
    /// For all user-defined types (struct, enum, newtype), returns `Type::Named(name)`.
    /// Newtypes are nominally distinct from their underlying type.
    pub fn to_type(&self, type_id: TypeId) -> Option<Type> {
        self.get_by_id(type_id).map(|entry| match &entry.kind {
            TypeKind::Struct { .. } | TypeKind::Enum { .. } | TypeKind::Newtype { .. } => {
                Type::Named(entry.name)
            }
        })
    }

    /// Get the underlying type for a newtype.
    ///
    /// Returns `None` if the type is not a newtype.
    pub fn get_newtype_underlying(&self, type_id: TypeId) -> Option<Type> {
        self.get_by_id(type_id).and_then(|entry| match &entry.kind {
            TypeKind::Newtype { underlying } => Some(self.interner.to_type(*underlying)),
            _ => None,
        })
    }

    /// Get field types for a struct type.
    ///
    /// Returns the fields as (Name, Type) pairs by converting from `TypeId`.
    pub fn get_struct_fields(&self, type_id: TypeId) -> Option<Vec<(Name, Type)>> {
        self.get_by_id(type_id).and_then(|entry| match &entry.kind {
            TypeKind::Struct { fields } => Some(
                fields
                    .iter()
                    .map(|(name, ty_id)| (*name, self.interner.to_type(*ty_id)))
                    .collect(),
            ),
            _ => None,
        })
    }

    /// Get field types for an enum variant.
    ///
    /// Returns the fields as (Name, Type) pairs by converting from `TypeId`.
    pub fn get_variant_fields(
        &self,
        type_id: TypeId,
        variant_name: Name,
    ) -> Option<Vec<(Name, Type)>> {
        self.get_by_id(type_id).and_then(|entry| match &entry.kind {
            TypeKind::Enum { variants } => {
                variants.iter().find(|v| v.name == variant_name).map(|v| {
                    v.fields
                        .iter()
                        .map(|(name, ty_id)| (*name, self.interner.to_type(*ty_id)))
                        .collect()
                })
            }
            _ => None,
        })
    }

    /// Look up a variant constructor by name.
    ///
    /// Searches all registered enum types for a variant with the given name.
    /// Returns the enum type name and variant definition if found.
    ///
    /// This is used for resolving variant constructors like `Running` or `Done`
    /// when they appear as identifiers or function calls.
    pub fn lookup_variant_constructor(&self, variant_name: Name) -> Option<VariantConstructorInfo> {
        for entry in self.types_by_name.values() {
            if let TypeKind::Enum { variants } = &entry.kind {
                for variant in variants {
                    if variant.name == variant_name {
                        let field_types: Vec<Type> = variant
                            .fields
                            .iter()
                            .map(|(_, ty_id)| self.interner.to_type(*ty_id))
                            .collect();
                        return Some(VariantConstructorInfo {
                            enum_name: entry.name,
                            variant_name,
                            field_types,
                            type_params: entry.type_params.clone(),
                        });
                    }
                }
            }
        }
        None
    }

    /// Look up a newtype constructor by name.
    ///
    /// Searches all registered newtype types for one with the given name.
    /// Returns the newtype info if found.
    ///
    /// This is used for resolving newtype constructors like `UserId("abc")`
    /// when they appear as function calls.
    pub fn lookup_newtype_constructor(&self, name: Name) -> Option<NewtypeConstructorInfo> {
        self.types_by_name.get(&name).and_then(|entry| {
            if let TypeKind::Newtype { underlying } = &entry.kind {
                Some(NewtypeConstructorInfo {
                    newtype_name: entry.name,
                    underlying_type: self.interner.to_type(*underlying),
                    type_params: entry.type_params.clone(),
                })
            } else {
                None
            }
        })
    }
}

/// Information about a variant constructor.
#[derive(Clone, Debug)]
pub struct VariantConstructorInfo {
    /// The enum type name (e.g., "Status").
    pub enum_name: Name,
    /// The variant name (e.g., "Running").
    pub variant_name: Name,
    /// Types of the variant fields (empty for unit variants).
    pub field_types: Vec<Type>,
    /// Generic type parameters of the enum (if any).
    pub type_params: Vec<Name>,
}

/// Information about a newtype constructor.
#[derive(Clone, Debug)]
pub struct NewtypeConstructorInfo {
    /// The newtype name (e.g., `UserId`).
    pub newtype_name: Name,
    /// The underlying type (e.g., `str` for `type UserId = str`).
    pub underlying_type: Type,
    /// Generic type parameters of the newtype (if any).
    pub type_params: Vec<Name>,
}
