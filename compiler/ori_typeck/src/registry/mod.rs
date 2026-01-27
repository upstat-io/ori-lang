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

mod trait_registry;

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
    /// Type alias (newtype).
    Alias { target: TypeId },
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

    /// Register a type alias.
    ///
    /// Target type is converted to `TypeId` using the registry's interner.
    /// Returns the assigned `TypeId`.
    pub fn register_alias(
        &mut self,
        name: Name,
        target: &Type,
        span: Span,
        type_params: Vec<Name>,
    ) -> TypeId {
        let target_id = target.to_type_id(&self.interner);
        self.register_entry(
            name,
            TypeKind::Alias { target: target_id },
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
    /// For struct and enum types, returns `Type::Named(name)`.
    /// For aliases, returns the target type directly (converted from `TypeId`).
    pub fn to_type(&self, type_id: TypeId) -> Option<Type> {
        self.get_by_id(type_id).map(|entry| match &entry.kind {
            TypeKind::Struct { .. } | TypeKind::Enum { .. } => Type::Named(entry.name),
            TypeKind::Alias { target } => self.interner.to_type(*target),
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
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

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

        let fields = vec![(x_name, Type::Int), (y_name, Type::Int)];

        let type_id = registry.register_struct(point_name, fields.clone(), make_span(), vec![]);

        assert!(!type_id.is_primitive());
        assert_eq!(registry.len(), 1);

        let entry = registry.get_by_name(point_name).unwrap();
        assert_eq!(entry.name, point_name);
        assert_eq!(entry.type_id, type_id);

        if let TypeKind::Struct {
            fields: entry_fields,
        } = &entry.kind
        {
            assert_eq!(entry_fields.len(), 2);
            assert_eq!(entry_fields[0].0, x_name);
            // Fields are now stored as TypeId
            assert_eq!(entry_fields[0].1, TypeId::INT);
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

        // Use the new API: (variant_name, fields) tuples
        let variants = vec![
            (ok_name, vec![(value_name, Type::Int)]),
            (err_name, vec![(value_name, Type::Str)]),
        ];

        let type_id = registry.register_enum(result_name, variants, make_span(), vec![]);

        assert!(!type_id.is_primitive());
        assert!(registry.contains(result_name));

        let entry = registry.get_by_id(type_id).unwrap();
        if let TypeKind::Enum {
            variants: entry_variants,
        } = &entry.kind
        {
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
        let type_id = registry.register_alias(id_name, &Type::Int, make_span(), vec![]);

        assert!(!type_id.is_primitive());

        let entry = registry.get_by_name(id_name).unwrap();
        if let TypeKind::Alias { target } = &entry.kind {
            // Target is now stored as TypeId
            assert_eq!(*target, TypeId::INT);
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
        let id3 = registry.register_alias(name3, &Type::Int, make_span(), vec![]);

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
        let type_id = registry.register_alias(id_name, &Type::Int, make_span(), vec![]);

        let typ = registry.to_type(type_id).unwrap();
        assert_eq!(typ, Type::Int);
    }

    #[test]
    fn test_generic_type_params() {
        let interner = SharedInterner::default();
        let mut registry = TypeRegistry::new();

        let container_name = interner.intern("Container");
        let t_name = interner.intern("T");

        let type_id = registry.register_struct(container_name, vec![], make_span(), vec![t_name]);

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
}
