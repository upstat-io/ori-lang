//! Drop hints for unique collection drops.
//!
//! Identifies `RcDec` instructions where the target variable is a provably
//! unique collection (RC == 1), allowing the LLVM emitter to skip the
//! atomic RC decrement and call `ori_buffer_drop_unique` directly.
//!
//! # Analysis
//!
//! A variable `v` at `RcDec { var: v }` is **unique-droppable** if:
//!
//! 1. `v` is defined by `Construct` in the same function (fresh allocation,
//!    RC starts at 1)
//! 2. No `RcInc { var: v }` exists anywhere in the function (never shared)
//! 3. `v`'s type is a collection (List, Set, or Map)
//!
//! This is a conservative O(n) analysis: one pass to collect definitions
//! and increments, then annotate matching `RcDec` instructions.
//!
//! # Relation to COW annotations
//!
//! [`CowAnnotations`](super::CowAnnotations) annotate COW _mutation_ sites.
//! `DropHints` annotate _drop_ sites. They use the same `(block_idx, instr_idx)`
//! keying scheme and `PartialEq`/`Hash` no-op pattern (derived data, not
//! structural identity).

use std::hash::{Hash, Hasher};

use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcVarId, CtorKind};

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

/// Returns true if the type tag represents a collection with a buffer.
fn is_collection_tag(tag: Tag) -> bool {
    matches!(tag, Tag::List | Tag::Set | Tag::Map)
}

/// Returns true if this constructor kind produces a fresh collection buffer.
fn is_collection_construct(ctor: &CtorKind) -> bool {
    matches!(
        ctor,
        CtorKind::ListLiteral | CtorKind::SetLiteral | CtorKind::MapLiteral
    )
}

/// Compute drop hints for a function after RC insertion.
///
/// Must be called AFTER the full pipeline (cross-block RC elimination)
/// so that the IR is final and `(block_idx, instr_idx)` positions are
/// stable.
///
/// # Algorithm
///
/// 1. Scan all blocks to find variables defined by collection `Construct`
///    (these have RC == 1 at definition).
/// 2. Scan all blocks to find variables with `RcInc` (shared at some point).
/// 3. For each `RcDec` of a collection variable that is in the "constructed"
///    set but NOT in the "incremented" set, mark as unique-droppable.
pub fn compute_drop_hints(func: &ArcFunction, pool: &Pool) -> DropHints {
    let mut hints = DropHints::new();

    // Phase 1: Collect variables defined by collection Construct.
    let mut constructed_collections: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Construct { dst, ctor, .. } = instr {
                if is_collection_construct(ctor) {
                    constructed_collections.insert(*dst);
                }
            }
        }
    }

    if constructed_collections.is_empty() {
        return hints;
    }

    // Phase 2: Collect variables with RcInc (have been shared).
    let mut incremented: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::RcInc { var, .. } = instr {
                incremented.insert(*var);
            }
        }
    }

    // Phase 3: Annotate eligible RcDec instructions.
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                if !constructed_collections.contains(var) {
                    continue;
                }
                if incremented.contains(var) {
                    continue;
                }
                // Verify the type is actually a collection at the Pool level.
                // The Construct ctor kind should match, but defense-in-depth.
                let ty = var_type_tag(func, *var, pool);
                if ty.is_some_and(is_collection_tag) {
                    hints.mark_unique(block_idx, instr_idx);
                }
            }
        }
    }

    if !hints.is_empty() {
        tracing::debug!(
            function = func.name.raw(),
            unique_drops = hints.len(),
            "drop hint analysis complete"
        );
    }

    hints
}

/// Resolve a variable's type tag from the Pool, if available.
fn var_type_tag(func: &ArcFunction, var: ArcVarId, pool: &Pool) -> Option<Tag> {
    let idx: Idx = *func.var_types.get(var.index())?;
    let resolved = pool.resolve_fully(idx);
    Some(pool.tag(resolved))
}

#[cfg(test)]
mod tests;
