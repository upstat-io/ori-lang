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
mod flags_compute;
mod format;
mod hashing;
mod interning;
pub mod re_intern;
mod structural_eq;
pub mod substitute;

pub use collection_surface::walk_collection_types;
pub use construct::*;
pub use descriptor::{TypeDescriptor, VariantDescriptor};
pub use re_intern::{
    re_intern_sig, re_intern_sig_with_var_remap, re_intern_type, re_intern_type_with_var_remap,
};
pub use substitute::{
    build_mono_body_type_map, extend_var_subst_with_roots, extract_var_from_types,
    substitute_in_pool, BodyTypeMapSink,
};

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
    // Core Storage (parallel arrays)
    /// All type items (tag + data).
    items: Vec<Item>,
    /// Pre-computed flags for each item (flags[i] corresponds to items[i]).
    flags: Vec<TypeFlags>,
    /// Stable hashes for each item (hashes[i] corresponds to items[i]).
    hashes: Vec<u64>,

    // Extra Data
    /// Variable-length data for complex types.
    /// Layout depends on tag (see documentation on each type).
    extra: Vec<u32>,

    // Deduplication
    /// Hash -> Idx mapping for deduplication.
    intern_map: FxHashMap<u64, Idx>,

    // Named Type Resolution
    /// Maps Named/Applied Idx -> concrete Struct/Enum Idx.
    ///
    /// Populated during type registration to bridge the gap between
    /// named type references (created by the parser) and their concrete
    /// Pool definitions (Struct/Enum with full field data).
    resolutions: FxHashMap<Idx, Idx>,

    // FFI C-ABI kind carrier
    /// Maps an FFI `c_*` type's distinct `Tag::Named` `Idx` -> its symbolic
    /// `CAbiKind`. Set at FFI type resolution alongside `resolutions`; the
    /// Named `Idx` keeps its `set_resolution(-> Idx::INT/FLOAT)` for value
    /// ergonomics (`f(x: 42)` still type-checks), while this side table carries
    /// the C-ABI width identity the `set_resolution` discards. `ori_repr` reads
    /// it at `AbiBoundary::Ffi` and maps the kind to the target-concrete width.
    /// Keyed by the distinct per-c_* Named `Idx` — no collision (each `c_*`
    /// Name interns a distinct Named `Idx`).
    cabi_kinds: FxHashMap<Idx, ori_ir::CAbiKind>,

    // Newtype Registry
    /// Maps newtype `Name` -> underlying `Idx` for newtype declarations
    /// (`type N = Existing`).
    ///
    /// Newtypes are layout-transparent (Spec: Clause 8.6.3 — same `abi_size`,
    /// `abi_alignment`, `layout`, `niche` as the inner type) but nominally
    /// distinct at the type level. Constructor calls `N(value)` and accessors
    /// `n.unwrap()` / `n.inner` should lower to no-op transparent wraps in
    /// codegen. This map is the SSOT for "which names are newtype constructors"
    /// — `ori_arc::lower` consults it to dispatch newtype calls to transparent
    /// `Let { Var(arg) }` instead of unresolvable `PartialApply` (the prior
    /// behavior surfaced as `emit_partial_apply: callee not found name="UserId"`).
    ///
    /// Unlike `resolutions`, this map is keyed by `Name`, not `Idx`, because
    /// the lowering pass sees the constructor's source-level name (`Ident`,
    /// `FunctionRef`, `TypeRef`) before any pool lookup.
    newtype_ctors: FxHashMap<ori_ir::Name, Idx>,

    // User-facing `Error` struct
    /// `Idx` of the registered user-facing `Error` struct (`{ message: str }`),
    /// distinct from the `Idx::ERROR` poison sentinel. Set once during builtin
    /// registration. SSOT for "is this Idx the user-`Error` type?" — queried by
    /// the registry-bridge Tag↔TypeTag maps + the codegen `TypeInfo` builder so
    /// engine-less sites can route the registered Error to the `TypeTag::Error`
    /// behavior table without re-mapping it to poison. `None` until registered.
    error_struct_idx: Option<Idx>,

    // Type Variables
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
            cabi_kinds: FxHashMap::default(),
            newtype_ctors: FxHashMap::default(),
            error_struct_idx: None,
            var_states: Vec::new(),
            next_var_id: 0,
        };

        // Pre-intern primitive types at fixed indices
        pool.intern_primitives();

        pool
    }

    /// Record the `Idx` of the registered user-facing `Error` struct.
    /// Set once during builtin registration; the SSOT for distinguishing the
    /// user-`Error` type from the `Idx::ERROR` poison sentinel.
    pub fn set_error_struct_idx(&mut self, idx: Idx) {
        self.error_struct_idx = Some(idx);
    }

    /// The registered user-facing `Error` struct `Idx`, or `None` before
    /// registration. Queried by engine-less Tag↔TypeTag bridges + the codegen
    /// `TypeInfo` builder to route the registered Error to its behavior table.
    pub fn error_struct_idx(&self) -> Option<Idx> {
        self.error_struct_idx
    }

    /// `true` iff `idx` is the registered user-facing `Error` struct.
    pub fn is_error_struct(&self, idx: Idx) -> bool {
        self.error_struct_idx == Some(idx)
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

    // Query Methods

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

    /// Map a type index to its `ori_registry::TypeTag` when the type is a
    /// builtin the registry knows. `None` for user-defined / generic /
    /// non-builtin types. Lets backend consumers ask the builtin registry
    /// "does this receiver type carry a builtin method named X?" without
    /// re-deriving the `Tag` → `TypeTag` mapping (the SSOT is `registry_bridge`).
    #[inline]
    #[must_use]
    pub fn builtin_type_tag(&self, idx: Idx) -> Option<ori_registry::TypeTag> {
        crate::infer::tag_to_type_tag(self.tag(idx))
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
    ///
    /// # Panics
    /// Panics if `var_id` is out of bounds. Use `var_state_checked()` when
    /// the `var_id` might be from a Generalized type variable that leaked
    /// past type checking.
    #[inline]
    pub fn var_state(&self, var_id: u32) -> &VarState {
        debug_assert!(
            (var_id as usize) < self.var_states.len(),
            "var_state: var_id {} out of bounds (pool has {} vars)",
            var_id,
            self.var_states.len()
        );
        &self.var_states[var_id as usize]
    }

    /// Get mutable access to variable state.
    ///
    /// # Panics
    /// Panics if `var_id` is out of bounds.
    #[inline]
    pub fn var_state_mut(&mut self, var_id: u32) -> &mut VarState {
        debug_assert!(
            (var_id as usize) < self.var_states.len(),
            "var_state_mut: var_id {} out of bounds (pool has {} vars)",
            var_id,
            self.var_states.len()
        );
        &mut self.var_states[var_id as usize]
    }

    /// Safely get variable state, returning `None` if `var_id` is out of bounds.
    ///
    /// Use this when the `var_id` might be from a Generalized type variable
    /// that leaked past type checking into codegen/repr phases.
    #[inline]
    pub fn var_state_checked(&self, var_id: u32) -> Option<&VarState> {
        self.var_states.get(var_id as usize)
    }

    /// Return the number of `var_id`s the pool has allocated.
    ///
    /// Equivalent to `var_states.len() as u32`. Any `var_id < next_var_id()`
    /// is guaranteed to have a backing `VarState` slot; callers preparing
    /// to re-intern or substitute foreign `var_id`s can pass this directly
    /// to [`Self::ensure_var_capacity`] instead of walking cache maps to
    /// re-derive the bound.
    #[inline]
    pub fn next_var_id(&self) -> u32 {
        self.next_var_id
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
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
