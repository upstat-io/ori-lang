//! Alias propagation for struct/tuple layout reordering.
//!
//! When the struct layout pass reorders fields for padding minimization,
//! the new layout must propagate to all Pool entries that represent the
//! same logical type. Monomorphized generics create multiple `Idx` values
//! for structurally identical types — without propagation, call sites may
//! use a different `Idx` than the function body, causing layout mismatch.

use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use crate::plan::ReprPlan;
use crate::{DecisionReason, DecisionSource, MachineRepr, ReprDecision};

/// Pool-wide index of concrete tuple/struct aliases by resolved structure.
///
/// Building this once keeps layout propagation linear in the pool size instead
/// of re-scanning every pool entry for every reordered layout decision.
pub(super) struct LayoutAliasIndex {
    by_signature: FxHashMap<u64, Vec<Idx>>,
    signatures: FxHashMap<Idx, u64>,
}

impl LayoutAliasIndex {
    #[must_use]
    pub(super) fn new(pool: &Pool) -> Self {
        let mut by_signature: FxHashMap<u64, Vec<Idx>> = FxHashMap::default();
        let mut signatures = FxHashMap::default();
        for idx in pool
            .iter_indices()
            .filter(|idx| idx.raw() >= Idx::FIRST_DYNAMIC)
        {
            if !matches!(pool.tag(idx), Tag::Tuple | Tag::Struct) {
                continue;
            }
            let signature = pool.resolved_structural_hash(idx);
            by_signature.entry(signature).or_default().push(idx);
            signatures.insert(idx, signature);
        }

        Self {
            by_signature,
            signatures,
        }
    }

    fn candidates(&self, source: Idx) -> &[Idx] {
        self.signatures
            .get(&source)
            .and_then(|signature| self.by_signature.get(signature))
            .map_or(&[], Vec::as_slice)
    }
}

/// Propagate a reordered layout to all Pool entries that share the same
/// concrete type structure. This handles the Pool aliasing issue where
/// monomorphized generics create multiple Idx values for the same logical type.
pub(super) fn propagate_layout_to_aliases(
    plan: &mut ReprPlan,
    pool: &Pool,
    aliases: &LayoutAliasIndex,
    source_idx: Idx,
    repr: &MachineRepr,
) {
    let source_tag = pool.tag(source_idx);

    // Only propagate tuples and structs.
    if source_tag != Tag::Tuple && source_tag != Tag::Struct {
        return;
    }

    for &idx in aliases.candidates(source_idx) {
        if idx == source_idx {
            continue;
        }
        // The signature is an index key, not the final equality decision: a
        // collision must never propagate a representation across type shapes.
        if !pool.structural_eq(source_idx, idx) {
            continue;
        }

        if source_tag == Tag::Struct {
            // Fixed-layout structs cannot inherit a reordered default layout.
            if let Some(attr) = plan.repr_attr(idx) {
                if !matches!(attr, crate::plan::ReprAttribute::Default) {
                    continue;
                }
            }
        }

        plan.set_repr(
            idx,
            ReprDecision {
                source: DecisionSource::StructLayout,
                type_idx: idx,
                repr: repr.clone(),
                reason: DecisionReason::Custom("field reordering (alias propagation)".into()),
            },
        );
    }
}

#[cfg(test)]
mod tests;
