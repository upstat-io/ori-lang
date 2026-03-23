//! Drop hints for unique collection drops.
//!
//! Identifies `RcDec` instructions where the target variable is a provably
//! unique collection (RC == 1), allowing the LLVM emitter to skip the
//! atomic RC decrement and call `ori_buffer_drop_unique` directly.
//!
//! # Relation to COW annotations
//!
//! [`CowAnnotations`](super::CowAnnotations) annotate COW _mutation_ sites.
//! `DropHints` annotate _drop_ sites. They use the same `(block_idx, instr_idx)`
//! keying scheme and `PartialEq`/`Hash` no-op pattern (derived data, not
//! structural identity).

use std::hash::{Hash, Hasher};

use rustc_hash::FxHashSet;

/// Per-instruction drop hints for LLVM codegen.
///
/// Maps `(block_index, instruction_index)` to `true` for `RcDec`
/// instructions that can use the unique-drop fast path.
///
/// Instructions not present default to `false` (standard RC dec).
#[derive(Clone, Debug, Default)]
pub struct DropHints {
    /// Set of `(block_idx, instr_idx)` positions where unique drop is valid.
    unique_drops: FxHashSet<(usize, usize)>,
}

impl DropHints {
    /// Create an empty hint set.
    pub fn new() -> Self {
        Self {
            unique_drops: FxHashSet::default(),
        }
    }

    /// Mark an instruction as eligible for unique drop.
    pub(crate) fn mark_unique(&mut self, block_idx: usize, instr_idx: usize) {
        self.unique_drops.insert((block_idx, instr_idx));
    }

    /// Check whether an instruction is eligible for unique drop.
    #[inline]
    pub fn is_unique_drop(&self, block_idx: usize, instr_idx: usize) -> bool {
        self.unique_drops.contains(&(block_idx, instr_idx))
    }

    /// Number of unique-drop annotations.
    pub fn len(&self) -> usize {
        self.unique_drops.len()
    }

    /// Whether there are no annotations.
    pub fn is_empty(&self) -> bool {
        self.unique_drops.is_empty()
    }
}

// Derived data — not part of structural identity (same pattern as CowAnnotations).
impl PartialEq for DropHints {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for DropHints {}

impl Hash for DropHints {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}
