//! Core pool storage queries.

use crate::{Idx, Item, Tag, TypeFlags};

use super::{Pool, VarState};

impl Pool {
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
        match self.tag(idx) {
            Tag::Unit => Some(ori_registry::TypeTag::Unit),
            Tag::Never => Some(ori_registry::TypeTag::Never),
            tag => crate::infer::tag_to_type_tag(tag),
        }
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

    /// True when `idx` is in range for this pool — i.e. `Pool::tag` /
    /// `Pool::data` / `Pool::item` / `Pool::format_type_with_idx` index `items`
    /// without panicking. Canonical bounds check: a consumer guarding a raw or
    /// externally-supplied `Idx` queries this instead of re-deriving
    /// `idx.raw() < len`.
    #[inline]
    #[must_use]
    pub fn is_valid_idx(&self, idx: Idx) -> bool {
        (idx.raw() as usize) < self.items.len()
    }

    /// Iterate every pool `Idx` in interning order (`Idx(0)..Idx(len)`).
    ///
    /// This is the canonical full-pool walk and enforces the `Idx` capacity
    /// before constructing raw indices.
    /// Callers layer their own tag/flag/resolution filter on top.
    pub fn iter_indices(&self) -> impl Iterator<Item = Idx> {
        let Ok(len) = u32::try_from(self.len()) else {
            unreachable!("type pool length exceeds the u32 Idx domain");
        };
        (0..len).map(Idx::from_raw)
    }
}
