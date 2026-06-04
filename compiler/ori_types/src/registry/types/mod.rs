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

mod type_impls;

use std::collections::BTreeMap;

use ori_ir::{Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::registry::burden::UserBurdenSpec;
use crate::registry::burden_dedup::BurdenSignature;
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

    /// Reverse-index from [`BurdenSignature`] to the canonical `Idx`
    /// carrying that signature's `UserBurdenSpec`. Populated by
    /// [`Self::register_user_burden`]; consulted on every monomorphized
    /// burden registration to short-circuit storage of structurally
    /// equivalent specs. Collisions (matching signature, distinct spec)
    /// fall through to a fresh entry (collision tolerance is canonical in
    /// both debug and release builds; structural equality check on collision
    /// is the correctness floor).
    burden_by_signature: FxHashMap<BurdenSignature, Idx>,

    /// Types carrying the `Value` marker (per Annex E §AIMS +
    /// `ori-syntax.md §Prelude`). Populated at Surface 1
    /// (`register_user_types`) when a type declaration carries `Value` in
    /// its trait set; queried at Surface 2 (`register_impl`) to enforce
    /// E2049's mutual exclusion against `impl T: Drop`. Set instead of
    /// `bool` field on `TypeEntry` to avoid widening every `TypeEntry`
    /// constructor — Value-carrying types are a minority of registered
    /// types in practice.
    value_marker_types: FxHashSet<Idx>,

    /// Burden side-table for monomorphized generic-builtin instances
    /// (`[T]`, `{K: V}`, `Set<T>`, `Option<T>`, `Result<T, E>`, `Range<T>`)
    /// that have NO `TypeEntry` — they are structural pool types, not
    /// user-declared nominals (per `types.md §RG-1`). `register_user_burden`
    /// writes here when `typeid` is absent from `types_by_idx`; `burden(idx)`
    /// reads this as a fallback. Keyed by bare `Idx` — same shape as
    /// `value_marker_types` for non-nominal pool indices. This is the
    /// documented post-Signatures-freeze burden-only write path: it touches
    /// no `TypeEntry`, so the registry's nominal-type freeze (RG-1) is
    /// preserved.
    collection_burdens: FxHashMap<Idx, UserBurdenSpec>,
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

    /// Merkle hash of the type's Pool representation.
    ///
    /// Content-addressed hash stable across Pool instances. Enables
    /// cross-module type identity: two modules can verify they refer
    /// to the same type by comparing `u64` hashes instead of
    /// re-walking the AST.
    ///
    /// Zero when constructed without pool access (e.g., test helpers).
    pub merkle_hash: u64,

    /// Representation attribute from `#repr("c")`, `#repr("packed")`, etc.
    ///
    /// `None` means default layout (all optimizations permitted).
    /// Populated from `TypeDecl.repr` during type registration.
    /// Consumed by `ori_repr` during representation planning.
    pub repr: Option<ori_ir::ReprAttrKind>,

    /// Per-type burden specification for AIMS drop-walk + reuse analysis.
    ///
    /// `None` for types whose burden is empty (all-scalar fields, all-scalar
    /// primitives newtyped) AND for aliases (burden inherits from the aliased
    /// target via the lookup helper). Populated during user-type registration;
    /// consumed at Phase 5 emission via the `Burden` trait.
    pub burden: Option<UserBurdenSpec>,
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
            burden_by_signature: FxHashMap::default(),
            value_marker_types: FxHashSet::default(),
            collection_burdens: FxHashMap::default(),
        }
    }

    /// Register a struct type.
    #[expect(
        clippy::too_many_arguments,
        reason = "type registration requires all fields"
    )]
    pub fn register_struct(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        fields: Vec<FieldDef>,
        span: Span,
        visibility: Visibility,
        merkle_hash: u64,
        repr: Option<ori_ir::ReprAttrKind>,
        burden: Option<UserBurdenSpec>,
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
            merkle_hash,
            repr,
            burden,
        };

        self.insert_entry(entry);
    }

    /// Register an enum type.
    ///
    /// Also populates the variant index for O(1) constructor lookup.
    #[expect(
        clippy::too_many_arguments,
        reason = "type registration requires all fields"
    )]
    pub fn register_enum(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        variants: Vec<VariantDef>,
        span: Span,
        visibility: Visibility,
        merkle_hash: u64,
        repr: Option<ori_ir::ReprAttrKind>,
        burden: Option<UserBurdenSpec>,
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
            merkle_hash,
            repr,
            burden,
        };

        self.insert_entry(entry);
    }

    /// Register a newtype (nominally distinct wrapper).
    #[expect(
        clippy::too_many_arguments,
        reason = "type registration requires all fields"
    )]
    pub fn register_newtype(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        underlying: Idx,
        span: Span,
        visibility: Visibility,
        merkle_hash: u64,
        repr: Option<ori_ir::ReprAttrKind>,
        burden: Option<UserBurdenSpec>,
    ) {
        let entry = TypeEntry {
            name,
            idx,
            kind: TypeKind::Newtype { underlying },
            span,
            type_params,
            visibility,
            merkle_hash,
            repr,
            burden,
        };

        self.insert_entry(entry);
    }

    /// Register a type alias (structural equivalent).
    #[expect(
        clippy::too_many_arguments,
        reason = "type registration requires all fields"
    )]
    pub fn register_alias(
        &mut self,
        name: Name,
        idx: Idx,
        type_params: Vec<Name>,
        target: Idx,
        span: Span,
        visibility: Visibility,
        merkle_hash: u64,
        burden: Option<UserBurdenSpec>,
    ) {
        let entry = TypeEntry {
            name,
            idx,
            kind: TypeKind::Alias { target },
            span,
            type_params,
            visibility,
            merkle_hash,
            repr: None, // aliases don't have repr attributes
            burden,     // typically None; aliases inherit burden from target via lookup_burden
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

    /// Look up the per-type burden spec for AIMS drop-walk and reuse analysis.
    ///
    /// Resolution order: the nominal `TypeEntry` burden (struct / enum /
    /// newtype / alias) first, then the `collection_burdens` side-table for
    /// monomorphized generic-builtin instances (`[T]`, `{K: V}`, `Set<T>`,
    /// `Option<T>`, …) that have no `TypeEntry`. Returns `None` when the type
    /// has no burden (all-scalar fields, empty newtype payload) or has not
    /// been registered. Lifetime is tied to the `TypeRegistry` borrow.
    #[inline]
    #[must_use]
    pub fn burden(&self, idx: Idx) -> Option<&UserBurdenSpec> {
        self.types_by_idx
            .get(&idx)
            .and_then(|entry| entry.burden.as_ref())
            .or_else(|| self.collection_burdens.get(&idx))
    }

    /// Record `idx` as carrying the `Value` marker.
    ///
    /// Populated at Surface 1 (`register_user_types`) when the
    /// declaration's trait set contains `Value`. Idempotent — repeat
    /// inserts collapse via the underlying set.
    pub fn record_value_marker(&mut self, idx: Idx) {
        self.value_marker_types.insert(idx);
    }

    /// Query whether `idx` carries the `Value` marker.
    ///
    /// Consumed at Surface 2 (`register_impl`) when a `Drop` impl is
    /// being registered: if `idx` is in the set, the impl conflicts with
    /// the type's `Value` declaration and E2049 fires.
    #[inline]
    #[must_use]
    pub fn carries_value_marker(&self, idx: Idx) -> bool {
        self.value_marker_types.contains(&idx)
    }

    /// Register a monomorphized / user `UserBurdenSpec` against `typeid`,
    /// deduplicating against the structural-signature reverse-index.
    ///
    /// Computes `BurdenSignature::compute(&spec)`. When an existing entry
    /// shares that signature AND is structurally equal to `spec`, returns
    /// the existing canonical `Idx` and DOES NOT mutate either index.
    /// Otherwise (no prior entry, OR a signature collision with a
    /// structurally different spec), stores `spec` against `typeid` and
    /// records `typeid` as that signature's canonical owner.
    ///
    /// Returns the canonical `Idx` for the registered spec. Callers map
    /// their original `typeid` to the returned `Idx` to participate in
    /// dedup (when distinct, `typeid` is the new canonical; when equal to
    /// an existing canonical, the registry shared the prior allocation).
    ///
    /// Soundness gate: identical to debug + release builds. A signature
    /// collision (same hash, different spec) MUST yield a fresh entry,
    /// not a `debug_assert!` panic — collisions are valid (rare) hash
    /// events, not invariant failures. The pre-existing slot keeps its
    /// occupant; the incoming spec lands at `typeid` without updating the
    /// reverse-index (the prior signature owner remains the canonical
    /// owner for the colliding key).
    ///
    /// When `typeid` is present in the registry as a [`TypeKind::Struct`] /
    /// `TypeKind::Enum` / `TypeKind::Newtype` / `TypeKind::Alias` entry, the
    /// burden field on that entry is overwritten with the canonical spec.
    /// When the registry has no `TypeEntry` for `typeid` — the monomorphized
    /// generic-builtin case (`[T]`, `{K: V}`, `Set<T>`, `Option<T>`, …) — the
    /// spec lands in the `collection_burdens` side-table, read by
    /// [`Self::burden`] as a fallback. Either way `burden(typeid)` returns
    /// `Some(spec)` after this call.
    pub fn register_user_burden(&mut self, typeid: Idx, spec: UserBurdenSpec) -> Idx {
        let sig = BurdenSignature::compute_singlelevel(&spec);

        // Probe the reverse-index. On hit, verify structural equality
        // before treating the existing slot as canonical.
        if let Some(&existing_idx) = self.burden_by_signature.get(&sig) {
            let is_structural_match = self
                .burden(existing_idx)
                .is_some_and(|existing_spec| existing_spec == &spec);
            if is_structural_match {
                // Structural match — share the existing signature slot at
                // existing_idx AND also write spec at caller's typeid so that
                // burden(typeid) returns Some(spec) for the typeid the caller
                // observed. Without this dual-write, callers passing typeid
                // != existing_idx (structurally-equivalent specs from genuinely
                // different typeids) silently lose burden lookups, producing
                // AIMS-realization-time miscompilation (AIMS Invariant 3 — no
                // stale summaries).
                self.write_spec_to_idx(typeid, spec);
                return existing_idx;
            }
            // Signature collision with a structurally different spec.
            // Fall through to fresh insertion at `typeid`; DO NOT
            // touch the reverse-index entry (the prior owner keeps
            // the signature slot).
        }

        // Claim the signature slot when it is currently vacant (collisions
        // leave the prior owner intact per the Debug/Release Parity gate).
        // The slot claim is unconditional bookkeeping — `write_spec_to_idx`
        // below always lands the spec (nominal entry OR side-table), so the
        // claim never needs backing out.
        if let std::collections::hash_map::Entry::Vacant(slot) = self.burden_by_signature.entry(sig)
        {
            slot.insert(typeid);
        }

        // Write the spec at `typeid`. A nominal `TypeEntry` (struct / enum /
        // newtype / alias) takes the burden on its entry; a monomorphized
        // generic-builtin instance (`[T]`, `{K: V}`, `Set<T>`, …) has no
        // `TypeEntry` and lands in the `collection_burdens` side-table. The
        // signature-slot bookkeeping above stands either way — the slot is
        // claimed for both nominal and side-table specs so dedup spans both
        // storage homes.
        self.write_spec_to_idx(typeid, spec);

        typeid
    }

    /// Write `spec` at `typeid`, dispatching to the nominal `TypeEntry` burden
    /// field when one exists or to the `collection_burdens` side-table when it
    /// does not. The side-table home is the post-Signatures-freeze burden-only
    /// write path for monomorphized generic-builtin instances that carry no
    /// `TypeEntry` (per `types.md §RG-1`). SSOT for the
    /// `register_user_burden` storage-dispatch so the structural-match and
    /// fresh-insert paths cannot diverge.
    fn write_spec_to_idx(&mut self, typeid: Idx, spec: UserBurdenSpec) {
        if let Some(entry) = self.types_by_idx.get_mut(&typeid) {
            entry.burden = Some(spec.clone());
            // Also update the by-name entry (TypeEntry stored in both maps).
            for name_entry in self.types_by_name.values_mut() {
                if name_entry.idx == typeid {
                    name_entry.burden = Some(spec);
                    break;
                }
            }
        } else {
            // No nominal entry — monomorphized collection / generic-builtin
            // instance. Land in the side-table read by `burden(idx)` as a
            // fallback after `types_by_idx`.
            self.collection_burdens.insert(typeid, spec);
        }
    }

    /// Number of distinct burden signatures currently registered.
    ///
    /// Equals the count of unique signatures that have claimed the
    /// reverse-index — i.e., the count of `register_user_burden` calls
    /// that produced a fresh canonical entry. Useful for matrix tests
    /// pinning sub-linear collapse in stdlib monomorphization stress
    /// scenarios.
    #[inline]
    #[must_use]
    pub fn burden_signature_count(&self) -> usize {
        self.burden_by_signature.len()
    }

    /// Number of `TypeEntry` slots whose `burden` field is populated.
    ///
    /// Distinct from [`Self::burden_signature_count`] when collisions
    /// have produced fresh entries that share signatures with prior
    /// slots. Matrix tests pin the relationship between the two counts.
    #[must_use]
    pub fn burden_entry_count(&self) -> usize {
        self.types_by_idx
            .values()
            .filter(|entry| entry.burden.is_some())
            .count()
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

    /// Set the resolved repr attribute for a registered type.
    ///
    /// Called after `validate_and_merge_repr_attrs` merges stacked `#repr`
    /// attributes into a single resolved value.
    pub fn set_repr(&mut self, idx: Idx, repr: Option<ori_ir::ReprAttrKind>) {
        if let Some(entry) = self.types_by_idx.get_mut(&idx) {
            entry.repr = repr;
        }
        // Also update the by-name entry (same allocation via insert_entry).
        // TypeEntry is stored in both maps, so we need to update both.
        for entry in self.types_by_name.values_mut() {
            if entry.idx == idx {
                entry.repr = repr;
                break;
            }
        }
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

    /// Drain the monomorphized-collection burden side-table into a
    /// deterministically-ordered `Vec<(Idx, UserBurdenSpec)>`.
    ///
    /// Side-table entries (`[T]`, `{K: V}`, `Set<T>`, `Option<T>`,
    /// `Result<T, E>`, `Range<T>` instances with no nominal `TypeEntry`) are
    /// keyed by bare `Idx` and never surface through `into_entries`. Export
    /// time uses this to carry the side-table out of the in-memory
    /// `TypeRegistry` onto `TypedModule`, where the ARC pipeline reconstructs
    /// it. Sorted ascending by `Idx` for Salsa-deterministic output.
    /// Spec: Annex E §AIMS.
    #[must_use]
    pub fn drain_collection_burdens(&mut self) -> Vec<(Idx, UserBurdenSpec)> {
        let mut out: Vec<(Idx, UserBurdenSpec)> = self.collection_burdens.drain().collect();
        out.sort_by_key(|(idx, _)| idx.raw());
        out
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

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
mod tests;
