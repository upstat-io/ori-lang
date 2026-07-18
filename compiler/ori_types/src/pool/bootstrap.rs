//! Pool construction and primitive bootstrap.

use rustc_hash::FxHashMap;

use crate::{Idx, Item, Tag, TypeFlags};

use super::Pool;

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

    /// `true` iff `ty` is the registered user-facing `Error` struct, under
    /// EITHER of its two identities: the `Tag::Named` wrapper idx
    /// (`error_struct_idx`) or the concrete `Tag::Struct` idx it resolves to
    /// via [`Self::resolve`]. Chases only `Tag::Var` links on the arbitrary
    /// input `ty` (never [`Self::resolve`]/[`Self::resolve_fully`] on it,
    /// which would re-admit a newtype over `Error` — nominal typing forbids
    /// inheriting `Error`'s methods).
    #[must_use]
    pub fn is_error_struct_receiver(&self, ty: Idx) -> bool {
        let Some(named) = self.error_struct_idx else {
            return false;
        };
        let chased = self.chase_var_links(ty);
        chased == named || self.resolve(named) == Some(chased)
    }

    /// Pre-intern all primitive types at their fixed indices.
    #[expect(
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
    #[expect(
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
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}
