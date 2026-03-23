//! Per-instruction COW mode annotations for LLVM codegen.
//!
//! Maps each COW operation in a function to a [`CowMode`] determined by
//! static uniqueness analysis. Consumed by the LLVM arc emitter to decide
//! whether to emit the runtime uniqueness check, the fast path only, or
//! the slow path only.

use std::hash::{Hash, Hasher};

use rustc_hash::FxHashMap;

use super::CowMode;

/// Per-instruction COW mode annotations for LLVM codegen.
///
/// Maps `(block_index, instruction_index)` to [`CowMode`] for each
/// COW operation in a function. Produced by the AIMS pipeline during
/// Phase 2 realization and consumed by the LLVM arc emitter.
///
/// Instructions not present in the map default to [`CowMode::Dynamic`]
/// (runtime uniqueness check needed).
///
/// # Identity
///
/// `CowAnnotations` is **derived data** (like [`crate::ir::ValueRepr`]),
/// not part of a function's structural identity. `PartialEq`/`Eq`/`Hash`
/// are implemented as no-ops to avoid breaking `ArcFunction`'s derives.
#[derive(Clone, Debug, Default)]
pub struct CowAnnotations {
    annotations: FxHashMap<(usize, usize), CowMode>,
}

impl CowAnnotations {
    /// Create an empty annotation set.
    pub fn new() -> Self {
        Self {
            annotations: FxHashMap::default(),
        }
    }

    /// Set the COW mode for an instruction.
    pub fn set(&mut self, block_idx: usize, instr_idx: usize, mode: CowMode) {
        self.annotations.insert((block_idx, instr_idx), mode);
    }

    /// Get the COW mode for an instruction.
    ///
    /// Returns [`CowMode::Dynamic`] for instructions without explicit annotations.
    #[inline]
    pub fn get(&self, block_idx: usize, instr_idx: usize) -> CowMode {
        self.annotations
            .get(&(block_idx, instr_idx))
            .copied()
            .unwrap_or(CowMode::Dynamic)
    }

    /// Number of annotated instructions.
    pub fn len(&self) -> usize {
        self.annotations.len()
    }

    /// Whether there are no annotations.
    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Count of instructions annotated as [`CowMode::StaticUnique`].
    pub fn static_unique_count(&self) -> usize {
        self.annotations
            .values()
            .filter(|&&m| m == CowMode::StaticUnique)
            .count()
    }

    /// Count of instructions annotated as [`CowMode::StaticShared`].
    pub fn static_shared_count(&self) -> usize {
        self.annotations
            .values()
            .filter(|&&m| m == CowMode::StaticShared)
            .count()
    }

    /// Iterate over all annotations.
    pub fn iter(&self) -> impl Iterator<Item = ((usize, usize), CowMode)> + '_ {
        self.annotations.iter().map(|(&k, &v)| (k, v))
    }

    /// Remap block indices after compaction or renumbering.
    ///
    /// `block_remap[old_idx]` is `Some(new_idx)` for surviving blocks,
    /// `None` for dead blocks. Annotations on dead blocks are dropped;
    /// annotations on surviving blocks get their block index updated.
    pub fn remap_block_indices(&mut self, block_remap: &[Option<usize>]) {
        let old = std::mem::take(&mut self.annotations);
        for ((block_idx, instr_idx), mode) in old {
            if let Some(&Some(new_block)) = block_remap.get(block_idx) {
                self.annotations.insert((new_block, instr_idx), mode);
            }
        }
    }

    /// Remap annotations when merging block `from_block` into `to_block`.
    ///
    /// All annotations keyed at `(from_block, I)` are re-keyed to
    /// `(to_block, instr_offset + I)`, reflecting that B's instructions
    /// are appended to A's body at position `instr_offset`.
    pub fn remap_block_merge(&mut self, from_block: usize, to_block: usize, instr_offset: usize) {
        // Collect entries to move (can't modify map while iterating).
        let to_move: Vec<(usize, CowMode)> = self
            .annotations
            .iter()
            .filter(|&(&(b, _), _)| b == from_block)
            .map(|(&(_, i), &mode)| (i, mode))
            .collect();

        for (instr_idx, mode) in to_move {
            self.annotations.remove(&(from_block, instr_idx));
            self.annotations
                .insert((to_block, instr_offset + instr_idx), mode);
        }
    }
}

// Derived data — not part of structural identity.
impl PartialEq for CowAnnotations {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for CowAnnotations {}

impl Hash for CowAnnotations {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}
