//! Registry for user-defined types (structs, enums, newtypes, aliases).
//!
//! The `TypeRegistry` stores semantic information about type definitions,
//! enabling efficient lookup by name or by pool index.
//!
//! # Design
//!
//! - Dual indexing: `BTreeMap` for sorted iteration, `FxHashMap` for O(1) lookup
//! - Variant index: O(1) constructor lookup by name
//! - Field storage uses Idx for compact representation

use std::collections::BTreeMap;

use ori_ir::{Name, Span};
use rustc_hash::FxHashMap;

use crate::Idx;

/// Registry for user-defined types.
///
/// Provides efficient lookup of struct, enum, newtype, and alias definitions.
/// All types are stored once and can be looked up by name or pool index.
#[derive(Clone, Debug, Default)]
pub struct TypeRegistry {
    /// Types indexed by name (`BTreeMap` for deterministic iteration).
    types_by_name: BTreeMap<Name, TypeEntry>,

    /// Types indexed by pool Idx (`FxHashMap` for fast lookup).
    types_by_idx: FxHashMap<Idx, TypeEntry>,

    /// Variant name -> (containing type Idx, variant index).
    /// Enables O(1) lookup of enum variant constructors.
    variants_by_name: FxHashMap<Name, (Idx, usize)>,

    /// Counter for generating unique internal type IDs.
    /// Starts at `FIRST_USER_TYPE` to avoid collision with primitives.
    next_internal_id: u32,
}

/// First internal ID for user-defined types.
/// Primitives occupy indices 0-63 in the Pool.
const FIRST_USER_TYPE: u32 = 1000;

/// A registered type definition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeEntry {
    /// The type name.
    pub name: Name,

    /// Pool index for this type.
    pub idx: Idx,

    /// The kind of type (struct, enum, etc.).
    pub kind: TypeKind,

    /// Source location of the definition.
    pub span: Span,

    /// Generic type parameters (e.g., `T` in `struct Foo<T>`).
    pub type_params: Vec<Name>,

    /// Visibility of this type.
    pub visibility: Visibility,
}

/// The kind of a user-defined type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// A struct with named fields.
    Struct(StructDef),

    /// An enum with variants.
    Enum {
        /// The enum variants.
        variants: Vec<VariantDef>,
    },

    /// A newtype wrapper (nominally distinct).
    Newtype {
        /// The underlying type being wrapped.
        underlying: Idx,
    },

    /// A type alias (structurally equivalent).
    Alias {
        /// The aliased type.
        target: Idx,
    },
}

/// Definition of a struct.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StructDef {
    /// The struct fields.
    pub fields: Vec<FieldDef>,
}

/// Definition of a struct or record variant field.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FieldDef {
    /// Field name.
    pub name: Name,

    /// Field type (as pool Idx).
    pub ty: Idx,

    /// Source location.
    pub span: Span,

    /// Field visibility.
    pub visibility: Visibility,
}

/// Definition of an enum variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VariantDef {
    /// Variant name.
    pub name: Name,

    /// Variant fields (unit, tuple, or record).
    pub fields: VariantFields,

    /// Source location.
    pub span: Span,
}

/// Fields of an enum variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VariantFields {
    /// Unit variant: `None`.
    Unit,

    /// Tuple variant: `Some(int)`, `Pair(int, str)`.
    Tuple(Vec<Idx>),

    /// Record variant: `Point { x: int, y: int }`.
    Record(Vec<FieldDef>),
}

/// Visibility of a type, field, or method.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Visible everywhere.
    #[default]
    Public,

    /// Visible only within the defining module.
    Private,
}

impl TypeRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            types_by_name: BTreeMap::new(),
            types_by_idx: FxHashMap::default(),
            variants_by_name: FxHashMap::default(),
            next_internal_id: FIRST_USER_TYPE,
        }
    }

    /// Register a struct type.
    ///
    /// Returns the pool index for this type.
    pub fn register_struct(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        fields: Vec<FieldDef>,
        span: Span,
        visibility: Visibility,
    ) {
        let entry = TypeEntry {
            name,
            idx,
            kind: TypeKind::Struct(StructDef { fields }),
            span,
            type_params,
            visibility,
        };

        self.insert_entry(entry);
    }

    /// Register an enum type.
    ///
    /// Also populates the variant index for O(1) constructor lookup.
    pub fn register_enum(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        variants: Vec<VariantDef>,
        span: Span,
        visibility: Visibility,
    ) {
        // Index variants for O(1) lookup
        for (variant_idx, variant) in variants.iter().enumerate() {
            self.variants_by_name
                .insert(variant.name, (idx, variant_idx));
        }

        let entry = TypeEntry {
            name,
            idx,
            kind: TypeKind::Enum { variants },
            span,
            type_params,
            visibility,
        };

        self.insert_entry(entry);
    }

    /// Register a newtype (nominally distinct wrapper).
    pub fn register_newtype(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        underlying: Idx,
        span: Span,
        visibility: Visibility,
    ) {
        let entry = TypeEntry {
            name,
            idx,
            kind: TypeKind::Newtype { underlying },
            span,
            type_params,
            visibility,
        };

        self.insert_entry(entry);
    }

    /// Register a type alias (structural equivalent).
    pub fn register_alias(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        target: Idx,
        span: Span,
        visibility: Visibility,
    ) {
        let entry = TypeEntry {
            name,
            idx,
            kind: TypeKind::Alias { target },
            span,
            type_params,
            visibility,
        };

        self.insert_entry(entry);
    }

    /// Insert a type entry into both indices.
    fn insert_entry(&mut self, entry: TypeEntry) {
        let name = entry.name;
        let idx = entry.idx;
        self.types_by_name.insert(name, entry.clone());
        self.types_by_idx.insert(idx, entry);
        self.next_internal_id += 1;
    }

    // === Lookup Methods ===

    /// Look up a type by name.
    #[inline]
    pub fn get_by_name(&self, name: Name) -> Option<&TypeEntry> {
        self.types_by_name.get(&name)
    }

    /// Look up a type by pool index.
    #[inline]
    pub fn get_by_idx(&self, idx: Idx) -> Option<&TypeEntry> {
        self.types_by_idx.get(&idx)
    }

    /// Check if a type with the given name exists.
    #[inline]
    pub fn contains_name(&self, name: Name) -> bool {
        self.types_by_name.contains_key(&name)
    }

    /// Check if a type with the given index exists.
    #[inline]
    pub fn contains_idx(&self, idx: Idx) -> bool {
        self.types_by_idx.contains_key(&idx)
    }

    /// Look up an enum variant constructor by name.
    ///
    /// Returns `Some((type_idx, variant_index))` if found.
    #[inline]
    pub fn lookup_variant(&self, name: Name) -> Option<(Idx, usize)> {
        self.variants_by_name.get(&name).copied()
    }

    /// Get the variant definition for a variant name.
    ///
    /// Returns `Some((type_entry, variant_def))` if found.
    pub fn lookup_variant_def(&self, name: Name) -> Option<(&TypeEntry, &VariantDef)> {
        let (type_idx, variant_idx) = self.lookup_variant(name)?;
        let entry = self.get_by_idx(type_idx)?;

        match &entry.kind {
            TypeKind::Enum { variants } => {
                let variant = variants.get(variant_idx)?;
                Some((entry, variant))
            }
            _ => None,
        }
    }

    /// Get struct fields by type index.
    ///
    /// Returns `None` if the type is not a struct.
    pub fn struct_fields(&self, idx: Idx) -> Option<&[FieldDef]> {
        let entry = self.get_by_idx(idx)?;
        match &entry.kind {
            TypeKind::Struct(def) => Some(&def.fields),
            _ => None,
        }
    }

    /// Get a struct field by name.
    ///
    /// Returns the field index and definition if found.
    pub fn struct_field(&self, idx: Idx, field_name: Name) -> Option<(usize, &FieldDef)> {
        let fields = self.struct_fields(idx)?;
        fields
            .iter()
            .enumerate()
            .find(|(_, f)| f.name == field_name)
    }

    /// Get enum variants by type index.
    ///
    /// Returns `None` if the type is not an enum.
    pub fn enum_variants(&self, idx: Idx) -> Option<&[VariantDef]> {
        let entry = self.get_by_idx(idx)?;
        match &entry.kind {
            TypeKind::Enum { variants } => Some(variants),
            _ => None,
        }
    }

    /// Get a variant by index within an enum.
    pub fn variant_at(&self, idx: Idx, variant_idx: usize) -> Option<&VariantDef> {
        self.enum_variants(idx)?.get(variant_idx)
    }

    // === Iteration ===

    /// Iterate over all registered types in name order.
    pub fn iter(&self) -> impl Iterator<Item = &TypeEntry> {
        self.types_by_name.values()
    }

    /// Consume the registry and return all type entries in name order.
    pub fn into_entries(self) -> Vec<TypeEntry> {
        self.types_by_name.into_values().collect()
    }

    /// Get the number of registered types.
    #[inline]
    pub fn len(&self) -> usize {
        self.types_by_name.len()
    }

    /// Check if the registry is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.types_by_name.is_empty()
    }

    /// Get all registered type names.
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        self.types_by_name.keys().copied()
    }
}

impl VariantFields {
    /// Check if this is a unit variant.
    #[inline]
    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    /// Check if this is a tuple variant.
    #[inline]
    pub fn is_tuple(&self) -> bool {
        matches!(self, Self::Tuple(_))
    }

    /// Check if this is a record variant.
    #[inline]
    pub fn is_record(&self) -> bool {
        matches!(self, Self::Record(_))
    }

    /// Get the arity (number of fields) of this variant.
    pub fn arity(&self) -> usize {
        match self {
            Self::Unit => 0,
            Self::Tuple(fields) => fields.len(),
            Self::Record(fields) => fields.len(),
        }
    }

    /// Get tuple field types if this is a tuple variant.
    pub fn tuple_types(&self) -> Option<&[Idx]> {
        match self {
            Self::Tuple(types) => Some(types),
            _ => None,
        }
    }

    /// Get record fields if this is a record variant.
    pub fn record_fields(&self) -> Option<&[FieldDef]> {
        match self {
            Self::Record(fields) => Some(fields),
            _ => None,
        }
    }
}

impl TypeKind {
    /// Check if this is a struct.
    #[inline]
    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Struct(_))
    }

    /// Check if this is an enum.
    #[inline]
    pub fn is_enum(&self) -> bool {
        matches!(self, Self::Enum { .. })
    }

    /// Check if this is a newtype.
    #[inline]
    pub fn is_newtype(&self) -> bool {
        matches!(self, Self::Newtype { .. })
    }

    /// Check if this is an alias.
    #[inline]
    pub fn is_alias(&self) -> bool {
        matches!(self, Self::Alias { .. })
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
mod tests {
    use super::*;
    use ori_ir::{Name, Span};

    fn test_name(s: &str) -> Name {
        Name::from_raw(
            s.as_bytes()
                .iter()
                .fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b))),
        )
    }

    fn test_span() -> Span {
        Span::DUMMY
    }

    #[test]
    fn register_and_lookup_struct() {
        let mut registry = TypeRegistry::new();

        let name = test_name("Point");
        let idx = Idx::from_raw(100);
        let fields = vec![
            FieldDef {
                name: test_name("x"),
                ty: Idx::INT,
                span: test_span(),
                visibility: Visibility::Public,
            },
            FieldDef {
                name: test_name("y"),
                ty: Idx::INT,
                span: test_span(),
                visibility: Visibility::Public,
            },
        ];

        registry.register_struct(name, idx, vec![], fields, test_span(), Visibility::Public);

        // Lookup by name
        let entry = registry.get_by_name(name).expect("should find by name");
        assert_eq!(entry.name, name);
        assert_eq!(entry.idx, idx);
        assert!(entry.kind.is_struct());

        // Lookup by idx
        let entry = registry.get_by_idx(idx).expect("should find by idx");
        assert_eq!(entry.name, name);

        // Get fields
        let fields = registry.struct_fields(idx).expect("should get fields");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].ty, Idx::INT);
    }

    #[test]
    fn register_and_lookup_enum() {
        let mut registry = TypeRegistry::new();

        let name = test_name("Option");
        let idx = Idx::from_raw(101);
        let some_name = test_name("Some");
        let none_name = test_name("None");

        let variants = vec![
            VariantDef {
                name: some_name,
                fields: VariantFields::Tuple(vec![Idx::INT]),
                span: test_span(),
            },
            VariantDef {
                name: none_name,
                fields: VariantFields::Unit,
                span: test_span(),
            },
        ];

        registry.register_enum(name, idx, vec![], variants, test_span(), Visibility::Public);

        // Lookup by name
        let entry = registry.get_by_name(name).expect("should find by name");
        assert!(entry.kind.is_enum());

        // Lookup variant
        let (type_idx, variant_idx) = registry
            .lookup_variant(some_name)
            .expect("should find variant");
        assert_eq!(type_idx, idx);
        assert_eq!(variant_idx, 0);

        let (type_idx, variant_idx) = registry
            .lookup_variant(none_name)
            .expect("should find variant");
        assert_eq!(type_idx, idx);
        assert_eq!(variant_idx, 1);

        // Get variants
        let variants = registry.enum_variants(idx).expect("should get variants");
        assert_eq!(variants.len(), 2);
        assert!(variants[0].fields.is_tuple());
        assert!(variants[1].fields.is_unit());
    }

    #[test]
    fn register_newtype() {
        let mut registry = TypeRegistry::new();

        let name = test_name("UserId");
        let idx = Idx::from_raw(102);

        registry.register_newtype(name, idx, vec![], Idx::INT, test_span(), Visibility::Public);

        let entry = registry.get_by_name(name).expect("should find");
        assert!(entry.kind.is_newtype());

        match &entry.kind {
            TypeKind::Newtype { underlying } => {
                assert_eq!(*underlying, Idx::INT);
            }
            _ => panic!("expected newtype"),
        }
    }

    #[test]
    fn register_alias() {
        let mut registry = TypeRegistry::new();

        let name = test_name("IntList");
        let idx = Idx::from_raw(103);
        let target = Idx::from_raw(200); // Some list type

        registry.register_alias(name, idx, vec![], target, test_span(), Visibility::Public);

        let entry = registry.get_by_name(name).expect("should find");
        assert!(entry.kind.is_alias());

        match &entry.kind {
            TypeKind::Alias { target: t } => {
                assert_eq!(*t, target);
            }
            _ => panic!("expected alias"),
        }
    }

    #[test]
    fn variant_fields_helpers() {
        let unit = VariantFields::Unit;
        assert!(unit.is_unit());
        assert_eq!(unit.arity(), 0);

        let tuple = VariantFields::Tuple(vec![Idx::INT, Idx::STR]);
        assert!(tuple.is_tuple());
        assert_eq!(tuple.arity(), 2);
        assert_eq!(tuple.tuple_types(), Some(&[Idx::INT, Idx::STR][..]));

        let record = VariantFields::Record(vec![FieldDef {
            name: test_name("x"),
            ty: Idx::INT,
            span: test_span(),
            visibility: Visibility::Public,
        }]);
        assert!(record.is_record());
        assert_eq!(record.arity(), 1);
        assert!(record.record_fields().is_some());
    }

    #[test]
    fn iteration_is_sorted() {
        let mut registry = TypeRegistry::new();

        // Register in non-alphabetical order
        let name_z = test_name("Zebra");
        let name_a = test_name("Apple");
        let name_m = test_name("Mango");

        registry.register_struct(
            name_z,
            Idx::from_raw(100),
            vec![],
            vec![],
            test_span(),
            Visibility::Public,
        );
        registry.register_struct(
            name_a,
            Idx::from_raw(101),
            vec![],
            vec![],
            test_span(),
            Visibility::Public,
        );
        registry.register_struct(
            name_m,
            Idx::from_raw(102),
            vec![],
            vec![],
            test_span(),
            Visibility::Public,
        );

        // Iteration should be in sorted order (by Name's Ord impl)
        let names: Vec<_> = registry.names().collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn generic_type_params() {
        let mut registry = TypeRegistry::new();

        let name = test_name("Box");
        let idx = Idx::from_raw(104);
        let t_param = test_name("T");

        registry.register_struct(
            name,
            idx,
            vec![t_param],
            vec![FieldDef {
                name: test_name("value"),
                ty: Idx::from_raw(500), // Would be a type variable in real code
                span: test_span(),
                visibility: Visibility::Public,
            }],
            test_span(),
            Visibility::Public,
        );

        let entry = registry.get_by_name(name).expect("should find");
        assert_eq!(entry.type_params.len(), 1);
        assert_eq!(entry.type_params[0], t_param);
    }

    #[test]
    fn struct_field_lookup() {
        let mut registry = TypeRegistry::new();

        let name = test_name("Point");
        let idx = Idx::from_raw(105);
        let x_name = test_name("x");
        let y_name = test_name("y");

        registry.register_struct(
            name,
            idx,
            vec![],
            vec![
                FieldDef {
                    name: x_name,
                    ty: Idx::INT,
                    span: test_span(),
                    visibility: Visibility::Public,
                },
                FieldDef {
                    name: y_name,
                    ty: Idx::FLOAT,
                    span: test_span(),
                    visibility: Visibility::Public,
                },
            ],
            test_span(),
            Visibility::Public,
        );

        // Find field x
        let (field_idx, field) = registry
            .struct_field(idx, x_name)
            .expect("should find field x");
        assert_eq!(field_idx, 0);
        assert_eq!(field.ty, Idx::INT);

        // Find field y
        let (field_idx, field) = registry
            .struct_field(idx, y_name)
            .expect("should find field y");
        assert_eq!(field_idx, 1);
        assert_eq!(field.ty, Idx::FLOAT);

        // Unknown field
        assert!(registry.struct_field(idx, test_name("z")).is_none());
    }
}
