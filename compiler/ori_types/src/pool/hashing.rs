//! Merkle hash computation for pool interning.
//!
//! Content-addressed hashing used by `Pool::intern` + `Pool::intern_complex`
//! for structural deduplication. Hashes child types by their Merkle hashes
//! (from `self.hashes[]`), not by raw `Idx` values, so the hash is stable
//! across independent Pool instances.
//!
//! # Tag categories
//!
//! - `is_merkle_leaf()` — hash `(tag, data)` only
//! - `has_child_in_data()` — hash `(tag, hashes[data])` (one child lookup)
//! - `uses_extra()` — tag-specific layout; dispatched by per-category helpers

use std::hash::{Hash, Hasher};

use crate::Tag;

use super::Pool;

impl Pool {
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
    pub(super) fn merkle_hash(&self, tag: Tag, data: u32, extra: &[u32]) -> u64 {
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
    /// Dispatch by category per.
    fn merkle_hash_extra(&self, tag: Tag, extra: &[u32], h: &mut impl Hasher) {
        match tag {
            Tag::Map | Tag::Result | Tag::Borrowed => {
                self.merkle_hash_two_child(tag, extra, h);
            }
            Tag::Function | Tag::Tuple => {
                self.merkle_hash_complex(tag, extra, h);
            }
            Tag::Struct | Tag::Enum | Tag::Named | Tag::Applied | Tag::Alias | Tag::Projection => {
                self.merkle_hash_named(tag, extra, h);
            }
            Tag::Scheme => {
                self.merkle_hash_scheme(extra, h);
            }
            _ => unreachable!(
                "uses_extra() returned true for {:?} but merkle_hash_extra has no handler",
                tag
            ),
        }
    }

    /// Hash two-child container tags: `Map`, `Result`, `Borrowed`.
    fn merkle_hash_two_child(&self, tag: Tag, extra: &[u32], h: &mut impl Hasher) {
        match tag {
            Tag::Map | Tag::Result => {
                self.hashes[extra[0] as usize].hash(h);
                self.hashes[extra[1] as usize].hash(h);
            }
            Tag::Borrowed => {
                self.hashes[extra[0] as usize].hash(h);
                extra[1].hash(h); // lifetime ID (structural)
            }
            _ => unreachable!("merkle_hash_two_child: unexpected tag {:?}", tag),
        }
    }

    /// Hash complex length-prefixed tags: `Function`, `Tuple`.
    fn merkle_hash_complex(&self, tag: Tag, extra: &[u32], h: &mut impl Hasher) {
        match tag {
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
            _ => unreachable!("merkle_hash_complex: unexpected tag {:?}", tag),
        }
    }

    /// Hash named nominal tags: `Struct`, `Enum`, `Named`, `Applied`, `Alias`,
    /// `Projection`.
    fn merkle_hash_named(&self, tag: Tag, extra: &[u32], h: &mut impl Hasher) {
        match tag {
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
            // Named: [name_lo, name_hi]
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
            // Alias / Projection: all structural, no children
            Tag::Alias | Tag::Projection => {
                for &word in extra {
                    word.hash(h);
                }
            }
            _ => unreachable!("merkle_hash_named: unexpected tag {:?}", tag),
        }
    }

    /// Hash scheme tag: `[var_count, v0_id, v1_id, ..., body_idx]`.
    fn merkle_hash_scheme(&self, extra: &[u32], h: &mut impl Hasher) {
        let var_count = extra[0] as usize;
        var_count.hash(h);
        // Var IDs are positional (de Bruijn-like) — hash as structural
        for i in 0..var_count {
            extra[1 + i].hash(h);
        }
        // Body is a child type
        self.hashes[extra[1 + var_count] as usize].hash(h);
    }
}
