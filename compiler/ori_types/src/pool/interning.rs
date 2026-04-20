//! Pool interning: structural deduplication of types via Merkle hashing.
//!
//! `intern` handles simple tags (`data` carries all the payload); `intern_complex`
//! handles tags whose payload lives in the extra array. Both consult
//! `self.intern_map` (`hash → Idx`) to deduplicate.

use crate::{Idx, Item, Tag};

use super::Pool;

impl Pool {
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
}
