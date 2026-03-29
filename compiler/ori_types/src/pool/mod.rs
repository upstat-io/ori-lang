//! Unified type pool - single source of truth for all types.
//!
//! The Pool stores all types using a unified representation:
//! - Types are referenced by [`Idx`] (32-bit indices)
//! - Each type is an [`Item`] with tag and data
//! - Complex types use an extra array for variable-length data
//! - Pre-computed [`TypeFlags`] enable O(1) property queries
//!
//! # Design (from Zig `InternPool`, Roc Subs)
//!
//! - Hash-based deduplication ensures each unique type exists once
//! - Primitives are pre-interned at fixed indices for O(1) lookup
//! - Structure-of-Arrays (`SoA`) layout for cache-friendly bulk operations

mod accessors;
mod collection_surface;
mod construct;
pub mod descriptor;
mod format;
pub mod re_intern;
pub mod substitute;

pub use collection_surface::walk_collection_types;
pub use construct::*;
pub use descriptor::{TypeDescriptor, VariantDescriptor};
pub use re_intern::{re_intern_sig, re_intern_type};
pub use substitute::{extract_var_from_types, substitute_in_pool};

use rustc_hash::FxHashMap;

use crate::{Idx, Item, Rank, Tag, TypeFlags};

/// The unified type pool - stores all types in the compilation.
///
/// All types in the system are stored here and referenced by `Idx`.
/// This provides:
/// - O(1) type equality (index comparison)
/// - Automatic deduplication (each unique type stored once)
/// - Pre-computed metadata (flags, hashes)
/// - Cache-friendly access patterns
// Clone is intentional — used for the merged_pool pattern in cross-module
// re-interning (see `oric::test::runner`). Pool is large (~10 Vecs), so
// clones are O(n); avoid accidental cloning outside the merge path.
#[derive(Clone)]
pub struct Pool {
    // === Core Storage (parallel arrays) ===
    /// All type items (tag + data).
    items: Vec<Item>,
    /// Pre-computed flags for each item (flags[i] corresponds to items[i]).
    flags: Vec<TypeFlags>,
    /// Stable hashes for each item (hashes[i] corresponds to items[i]).
    hashes: Vec<u64>,

    // === Extra Data ===
    /// Variable-length data for complex types.
    /// Layout depends on tag (see documentation on each type).
    extra: Vec<u32>,

    // === Deduplication ===
    /// Hash -> Idx mapping for deduplication.
    intern_map: FxHashMap<u64, Idx>,

    // === Named Type Resolution ===
    /// Maps Named/Applied Idx -> concrete Struct/Enum Idx.
    ///
    /// Populated during type registration to bridge the gap between
    /// named type references (created by the parser) and their concrete
    /// Pool definitions (Struct/Enum with full field data).
    resolutions: FxHashMap<Idx, Idx>,

    // === Type Variables ===
    /// State for each type variable.
    var_states: Vec<VarState>,
    /// Counter for generating fresh variable IDs.
    next_var_id: u32,
}

/// State of a type variable.
#[derive(Clone, Debug)]
pub enum VarState {
    /// Unbound variable - waiting to be unified.
    Unbound {
        /// Unique identifier for this variable.
        id: u32,
        /// Rank (scope depth) for generalization.
        rank: Rank,
        /// Optional name for better error messages.
        name: Option<ori_ir::Name>,
    },

    /// Linked to another type - follow the link.
    Link {
        /// The type this variable is unified with.
        target: Idx,
    },

    /// Rigid variable from annotation - cannot unify with concrete types.
    Rigid {
        /// The name from the annotation.
        name: ori_ir::Name,
    },

    /// Generalized variable - must be instantiated before use.
    Generalized {
        /// Original variable ID.
        id: u32,
        /// Optional name for error messages.
        name: Option<ori_ir::Name>,
    },
}

impl Pool {
    /// Create a new pool with pre-interned primitives.
    pub fn new() -> Self {
        let mut pool = Self {
            items: Vec::with_capacity(256),
            flags: Vec::with_capacity(256),
            hashes: Vec::with_capacity(256),
            extra: Vec::with_capacity(1024),
            intern_map: FxHashMap::default(),
            resolutions: FxHashMap::default(),
            var_states: Vec::new(),
            next_var_id: 0,
        };

        // Pre-intern primitive types at fixed indices
        pool.intern_primitives();

        pool
    }

    /// Pre-intern all primitive types at their fixed indices.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "primitive count is a small constant, always fits u32"
    )]
    fn intern_primitives(&mut self) {
        // Primitives must be interned in exact order to match Idx constants
        self.intern_primitive_at(Tag::Int, Idx::INT);
        self.intern_primitive_at(Tag::Float, Idx::FLOAT);
        self.intern_primitive_at(Tag::Bool, Idx::BOOL);
        self.intern_primitive_at(Tag::Str, Idx::STR);
        self.intern_primitive_at(Tag::Char, Idx::CHAR);
        self.intern_primitive_at(Tag::Byte, Idx::BYTE);
        self.intern_primitive_at(Tag::Unit, Idx::UNIT);
        self.intern_primitive_at(Tag::Never, Idx::NEVER);
        self.intern_primitive_at(Tag::Error, Idx::ERROR);
        self.intern_primitive_at(Tag::Duration, Idx::DURATION);
        self.intern_primitive_at(Tag::Size, Idx::SIZE);
        self.intern_primitive_at(Tag::Ordering, Idx::ORDERING);

        // Pad to FIRST_DYNAMIC with error placeholders
        while (self.items.len() as u32) < Idx::FIRST_DYNAMIC {
            self.items.push(Item::primitive(Tag::Error));
            self.flags.push(TypeFlags::HAS_ERROR);
            self.hashes.push(0);
        }

        debug_assert_eq!(self.items.len() as u32, Idx::FIRST_DYNAMIC);
    }

    /// Intern a primitive type at a specific index.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "items.len() always fits u32 — pool indices are u32"
    )]
    fn intern_primitive_at(&mut self, tag: Tag, expected_idx: Idx) {
        let idx = Idx::from_raw(self.items.len() as u32);
        debug_assert_eq!(idx, expected_idx, "Primitive index mismatch for {tag:?}");

        let item = Item::primitive(tag);
        let flags = Self::compute_primitive_flags(tag);
        let hash = Self::compute_primitive_hash(tag);

        self.items.push(item);
        self.flags.push(flags);
        self.hashes.push(hash);
        self.intern_map.insert(hash, idx);
    }

    /// Compute flags for a primitive type.
    fn compute_primitive_flags(tag: Tag) -> TypeFlags {
        let mut flags = TypeFlags::IS_PRIMITIVE | TypeFlags::IS_RESOLVED | TypeFlags::IS_MONO;

        match tag {
            Tag::Error => {
                flags |= TypeFlags::HAS_ERROR;
            }
            Tag::Never => {
                // Never is special - it's resolved but can unify with anything
            }
            _ => {
                flags |= TypeFlags::IS_COPYABLE;
            }
        }

        flags
    }

    /// Compute hash for a primitive type.
    fn compute_primitive_hash(tag: Tag) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        (tag as u8).hash(&mut hasher);
        hasher.finish()
    }

    // === Query Methods ===

    /// Get the tag for a type index.
    #[inline]
    pub fn tag(&self, idx: Idx) -> Tag {
        self.items[idx.raw() as usize].tag
    }

    /// Get the data field for a type index.
    #[inline]
    pub fn data(&self, idx: Idx) -> u32 {
        self.items[idx.raw() as usize].data
    }

    /// Get the item for a type index.
    #[inline]
    pub fn item(&self, idx: Idx) -> Item {
        self.items[idx.raw() as usize]
    }

    /// Get the flags for a type index.
    #[inline]
    pub fn flags(&self, idx: Idx) -> TypeFlags {
        self.flags[idx.raw() as usize]
    }

    /// Get the hash for a type index.
    #[inline]
    pub fn hash(&self, idx: Idx) -> u64 {
        self.hashes[idx.raw() as usize]
    }

    /// Look up a type by its Merkle hash.
    ///
    /// Returns `Some(idx)` if a type with this hash exists in the pool,
    /// `None` otherwise. O(1) via `intern_map`.
    ///
    /// Used for hash-first import resolution: when importing a function from
    /// another module, we can resolve its parameter/return types by Merkle hash
    /// instead of re-walking the AST — provided those types already exist in
    /// the local pool.
    #[inline]
    pub fn lookup_by_hash(&self, merkle_hash: u64) -> Option<Idx> {
        self.intern_map.get(&merkle_hash).copied()
    }

    /// Get the variable state for a variable ID.
    #[inline]
    pub fn var_state(&self, var_id: u32) -> &VarState {
        &self.var_states[var_id as usize]
    }

    /// Get mutable access to variable state.
    #[inline]
    pub fn var_state_mut(&mut self, var_id: u32) -> &mut VarState {
        &mut self.var_states[var_id as usize]
    }

    /// Get the number of types in the pool.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the pool is empty (only has primitives).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.len() <= Idx::FIRST_DYNAMIC as usize
    }

    // === Interning Methods ===

    /// Intern a simple type (no extra data).
    ///
    /// Returns the canonical index for this type.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "items.len() always fits u32 — pool indices are u32"
    )]
    pub fn intern(&mut self, tag: Tag, data: u32) -> Idx {
        let hash = self.merkle_hash(tag, data, &[]);

        // Check for existing
        if let Some(&idx) = self.intern_map.get(&hash) {
            debug_assert_eq!(
                self.tag(idx),
                tag,
                "Merkle hash collision: hash 0x{hash:016x} maps to {:?} but expected {:?}",
                self.tag(idx),
                tag
            );
            return idx;
        }

        // Create new
        let idx = Idx::from_raw(self.items.len() as u32);
        let item = Item::new(tag, data);
        let flags = self.compute_flags(tag, data, &[]);

        self.items.push(item);
        self.flags.push(flags);
        self.hashes.push(hash);
        self.intern_map.insert(hash, idx);

        idx
    }

    /// Intern a complex type with extra data.
    ///
    /// The `extra_data` slice is copied into the extra array.
    /// Returns the canonical index for this type.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "items.len() and extra.len() always fit u32 — pool storage is u32-indexed"
    )]
    pub fn intern_complex(&mut self, tag: Tag, extra_data: &[u32]) -> Idx {
        let hash = self.merkle_hash(tag, 0, extra_data);

        // Check for existing
        if let Some(&idx) = self.intern_map.get(&hash) {
            debug_assert_eq!(
                self.tag(idx),
                tag,
                "Merkle hash collision: hash 0x{hash:016x} maps to {:?} but expected {:?}",
                self.tag(idx),
                tag
            );
            return idx;
        }

        // Allocate in extra array
        let extra_idx = self.extra.len() as u32;
        self.extra.extend_from_slice(extra_data);

        // Create new item
        let idx = Idx::from_raw(self.items.len() as u32);
        let item = Item::with_extra(tag, extra_idx);
        let flags = self.compute_flags(tag, extra_idx, extra_data);

        self.items.push(item);
        self.flags.push(flags);
        self.hashes.push(hash);
        self.intern_map.insert(hash, idx);

        idx
    }

    /// Compute a content-addressed Merkle hash for interning.
    ///
    /// Unlike the previous `compute_hash`, this hashes child types by their
    /// Merkle hashes (from `self.hashes[]`), not by their raw Idx values.
    /// This makes the hash stable across independent Pool instances:
    /// the same type structure always produces the same hash.
    ///
    /// # Categories
    ///
    /// - `has_child_in_data()`: data = child Idx → hash child's Merkle hash
    /// - `uses_extra()`: tag-specific extra layout → `merkle_hash_extra()`
    /// - Leaf: hash tag + data directly (primitives, variables, specials)
    fn merkle_hash(&self, tag: Tag, data: u32, extra: &[u32]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = rustc_hash::FxHasher::default();

        (tag as u8).hash(&mut h);

        if tag.has_child_in_data() {
            // Simple container: data = child Idx → hash child's Merkle hash
            self.hashes[data as usize].hash(&mut h);
        } else if tag.uses_extra() {
            // Complex type with extra data — tag-specific layout.
            // Note: data is NOT hashed here because for complex types
            // data = extra array offset (pool-local, not structural).
            self.merkle_hash_extra(tag, extra, &mut h);
        } else {
            // Leaf: hash data directly (primitives: data=0, vars: data=var_id, etc.)
            data.hash(&mut h);
        }

        h.finish()
    }

    /// Hash the extra array data for a complex type, using tag-specific layout.
    ///
    /// Child Idx positions are looked up in `self.hashes[]` (Merkle recursion).
    /// Structural data (names, counts, lifetime IDs) is hashed directly.
    fn merkle_hash_extra(&self, tag: Tag, extra: &[u32], h: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        match tag {
            // Two-child: both positions are child Idx
            Tag::Map | Tag::Result => {
                self.hashes[extra[0] as usize].hash(h);
                self.hashes[extra[1] as usize].hash(h);
            }

            // Borrowed: inner is child, lifetime is structural
            Tag::Borrowed => {
                self.hashes[extra[0] as usize].hash(h);
                extra[1].hash(h); // lifetime ID
            }

            // Function: [param_count, p0, p1, ..., return_type]
            Tag::Function => {
                let count = extra[0] as usize;
                count.hash(h);
                for i in 0..count {
                    self.hashes[extra[1 + i] as usize].hash(h);
                }
                self.hashes[extra[1 + count] as usize].hash(h);
            }

            // Tuple: [elem_count, e0, e1, ...]
            Tag::Tuple => {
                let count = extra[0] as usize;
                count.hash(h);
                for i in 0..count {
                    self.hashes[extra[1 + i] as usize].hash(h);
                }
            }

            // Struct: [name_lo, name_hi, field_count, f0_name, f0_type, ...]
            Tag::Struct => {
                extra[0].hash(h); // name_lo (structural)
                extra[1].hash(h); // name_hi (structural)
                let field_count = extra[2] as usize;
                field_count.hash(h);
                for i in 0..field_count {
                    extra[3 + i * 2].hash(h); // field name (structural)
                    self.hashes[extra[3 + i * 2 + 1] as usize].hash(h); // field type (child)
                }
            }

            // Enum: [name_lo, name_hi, variant_count, v0_name, v0_fc, v0_f0, ...]
            Tag::Enum => {
                extra[0].hash(h); // name_lo
                extra[1].hash(h); // name_hi
                let variant_count = extra[2] as usize;
                variant_count.hash(h);
                let mut offset = 3;
                for _ in 0..variant_count {
                    extra[offset].hash(h); // variant name (structural)
                    let fc = extra[offset + 1] as usize;
                    fc.hash(h); // field count (structural)
                    for j in 0..fc {
                        self.hashes[extra[offset + 2 + j] as usize].hash(h); // field type (child)
                    }
                    offset += 2 + fc;
                }
            }

            // Named: [name_lo, name_hi] — all structural, no children
            Tag::Named => {
                extra[0].hash(h); // name_lo
                extra[1].hash(h); // name_hi
            }

            // Applied: [name_lo, name_hi, arg_count, a0, a1, ...]
            Tag::Applied => {
                extra[0].hash(h); // name_lo (structural)
                extra[1].hash(h); // name_hi (structural)
                let arg_count = extra[2] as usize;
                arg_count.hash(h);
                for i in 0..arg_count {
                    self.hashes[extra[3 + i] as usize].hash(h); // arg type (child)
                }
            }

            // Alias: [name_lo, name_hi] — all structural (similar to Named)
            // Projection: [projection data] — transient, all structural
            Tag::Alias | Tag::Projection => {
                for &word in extra {
                    word.hash(h);
                }
            }

            // Scheme: [var_count, v0_id, v1_id, ..., body_idx]
            Tag::Scheme => {
                let var_count = extra[0] as usize;
                var_count.hash(h);
                // Var IDs are positional (de Bruijn-like) — hash as structural
                for i in 0..var_count {
                    extra[1 + i].hash(h);
                }
                // Body is a child type
                self.hashes[extra[1 + var_count] as usize].hash(h);
            }

            _ => unreachable!(
                "uses_extra() returned true for {:?} but merkle_hash_extra has no handler",
                tag
            ),
        }
    }

    /// Compute flags for a type.
    fn compute_flags(&self, tag: Tag, data: u32, extra: &[u32]) -> TypeFlags {
        match tag {
            // Primitives
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Str
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => {
                TypeFlags::IS_PRIMITIVE
                    | TypeFlags::IS_RESOLVED
                    | TypeFlags::IS_MONO
                    | TypeFlags::IS_COPYABLE
            }

            Tag::Never => TypeFlags::IS_PRIMITIVE | TypeFlags::IS_RESOLVED | TypeFlags::IS_MONO,

            Tag::Error => TypeFlags::IS_PRIMITIVE | TypeFlags::HAS_ERROR | TypeFlags::IS_RESOLVED,

            // Simple containers: inherit from child
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => {
                let child_flags = self.flags[data as usize];
                TypeFlags::IS_CONTAINER | TypeFlags::propagate_from(child_flags)
            }

            // Two-child containers
            Tag::Map | Tag::Result => {
                let child1_flags = self.flags[extra[0] as usize];
                let child2_flags = self.flags[extra[1] as usize];
                TypeFlags::IS_CONTAINER
                    | TypeFlags::propagate_from(child1_flags)
                    | TypeFlags::propagate_from(child2_flags)
            }

            // Borrowed reference (reserved, never constructed)
            Tag::Borrowed => {
                let inner_flags = self.flags[extra[0] as usize];
                TypeFlags::IS_CONTAINER | TypeFlags::propagate_from(inner_flags)
            }

            // Variables
            Tag::Var => TypeFlags::HAS_VAR | TypeFlags::NEEDS_SUBST,
            Tag::BoundVar => TypeFlags::HAS_BOUND_VAR | TypeFlags::NEEDS_SUBST,
            Tag::RigidVar => TypeFlags::HAS_RIGID_VAR | TypeFlags::NEEDS_SUBST,

            // Function: propagate from params and return
            Tag::Function => {
                // extra layout: [param_count, param0, param1, ..., return_type]
                let param_count = extra[0] as usize;
                let mut flags = TypeFlags::IS_FUNCTION;

                for i in 0..param_count {
                    let param_idx = extra[1 + i] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[param_idx]);
                }

                let ret_idx = extra[1 + param_count] as usize;
                flags |= TypeFlags::propagate_from(self.flags[ret_idx]);

                flags
            }

            // Tuple: propagate from elements
            Tag::Tuple => {
                // extra layout: [elem_count, elem0, elem1, ...]
                let elem_count = extra[0] as usize;
                let mut flags = TypeFlags::IS_COMPOSITE;

                for i in 0..elem_count {
                    let elem_idx = extra[1 + i] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[elem_idx]);
                }

                flags
            }

            // Struct: propagate from field types
            Tag::Struct => {
                // extra layout: [name_lo, name_hi, field_count, f0_name, f0_type, ...]
                let field_count = extra[2] as usize;
                let mut flags = TypeFlags::IS_COMPOSITE;

                for i in 0..field_count {
                    let field_type_idx = extra[3 + i * 2 + 1] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[field_type_idx]);
                }

                flags
            }

            // Enum: propagate from variant field types
            Tag::Enum => self.compute_enum_flags(extra),

            // Applied: propagate from type arguments
            Tag::Applied => {
                // extra layout: [name_lo, name_hi, arg_count, arg0, arg1, ...]
                let arg_count = extra[2] as usize;
                let mut flags = TypeFlags::IS_NAMED;

                for i in 0..arg_count {
                    let arg_idx = extra[3 + i] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[arg_idx]);
                }

                flags
            }

            // Named/Alias: no children to propagate from
            Tag::Named | Tag::Alias => TypeFlags::IS_NAMED,

            // Scheme
            Tag::Scheme => TypeFlags::IS_SCHEME,

            // Special
            Tag::Projection => TypeFlags::HAS_PROJECTION,
            Tag::Infer => TypeFlags::HAS_INFER,
            Tag::SelfType => TypeFlags::HAS_SELF,
            Tag::ModuleNs => TypeFlags::empty(),
        }
    }

    /// Compute flags for an Enum type, propagating from variant field types.
    ///
    /// Extra layout: `[name_lo, name_hi, variant_count, v0_name, v0_fc, v0_f0, ..., v1_name, ...]`
    fn compute_enum_flags(&self, extra: &[u32]) -> TypeFlags {
        let variant_count = extra[2] as usize;
        let mut flags = TypeFlags::IS_COMPOSITE;
        let mut offset = 3;

        for _ in 0..variant_count {
            let field_count = extra[offset + 1] as usize;
            for j in 0..field_count {
                let field_type_idx = extra[offset + 2 + j] as usize;
                flags |= TypeFlags::propagate_from(self.flags[field_type_idx]);
            }
            offset += 2 + field_count;
        }

        flags
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
