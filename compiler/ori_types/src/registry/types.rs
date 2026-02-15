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

use crate::{Idx, ValueCategory};

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
}

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

    /// Memory semantics for this struct (always `Boxed` for now).
    ///
    /// Reserved for future `inline type` support where structs may be
    /// stack-allocated and copied on assignment rather than ARC-managed.
    pub category: ValueCategory,
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
            kind: TypeKind::Struct(StructDef {
                fields,
                category: ValueCategory::default(),
            }),
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
mod tests;
