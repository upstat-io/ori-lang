//! Cross-block reuse planner.
//!
//! Consumes logical donor/recipient facts from AIMS analysis and validates
//! them against CFG geometry (dominator/post-dominator trees).
//!
//! The current transitional pipeline runs this after logical release placement
//! and before COW annotations. Production freezes only the eligible pair and
//! owner-credit transfer here; a VM or compiled layout plan selects and
//! validates any storage-recycling mechanism.
//!
//! # v1 scope
//!
//! Static-unique cross-block eligibility only. `MaybeShared` requires a
//! sharing-observation obligation; the physical planner chooses how to
//! discharge it. The current carrier's `IsShared` CFG expansion is one
//! projection, not an AIMS rule.
//!
//! # Reference
//!
//! - Dominator-guided reset/reuse expansion (counting-immutable-beans technique)
//! - FP2 reuse credits (Lorenzen et al., ICFP 2023)

use rustc_hash::FxHashSet;

use crate::aims::lattice::Uniqueness;
use crate::graph::DominatorTree;
use crate::graph::PostDominatorTree;
use crate::ir::{ArcBlockId, ArcFunction};

use super::{AllocEvent, DeathEvent, ReuseOpportunity};

/// Cross-block logical reuse-eligibility planner.
///
/// Consumes semantic facts (death/alloc events) from AIMS analysis
/// and validates them against CFG geometry. Built lazily — dominator
/// and post-dominator trees are only constructed when cross-block
/// candidate pairs exist.
pub(crate) struct ReusePlanner<'a> {
    func: &'a ArcFunction,
    /// Built lazily — only constructed if there are candidate pairs.
    dom_tree: Option<DominatorTree>,
    /// Built lazily — only constructed if there are cross-block candidates.
    post_dom_tree: Option<PostDominatorTree>,
}

impl<'a> ReusePlanner<'a> {
    pub(super) fn new(func: &'a ArcFunction) -> Self {
        Self {
            func,
            dom_tree: None,
            post_dom_tree: None,
        }
    }

    /// Find cross-block reuse opportunities from unmatched donor/recipient
    /// events.
    ///
    /// # Matching rule
    ///
    /// For `DeathEvent d` and `AllocEvent a`:
    /// 1. `d.uniqueness` is `Unique` (v1: static-unique only)
    /// 2. `d.ty == a.ty` (same type — v1 requirement)
    /// 3. `d.block != a.block` (cross-block — same-block already handled)
    /// 4. `d.block` dominates `a.block` (from `DominatorTree`)
    /// 5. `a.block` post-dominates `d.block` (from `PostDominatorTree`)
    /// 6. No earlier match has consumed the death or alloc
    ///
    /// # Selection strategy
    ///
    /// Prefer nearest dominated/post-dominating target by block index
    /// (proxy for dominator depth). Same-shape preference is moot in v1
    /// (same-type implies same-shape).
    pub(super) fn find_opportunities(
        &mut self,
        remaining_deaths: &[&DeathEvent],
        remaining_allocs: &[&AllocEvent],
    ) -> Vec<ReuseOpportunity> {
        // v1: only Unique deaths for cross-block reuse.
        // MaybeShared cross-block: the logical facts require a sharing
        // observation. This transitional planner handles only statically
        // unique pairs and leaves mechanism selection downstream.
        let unique_deaths: Vec<_> = remaining_deaths
            .iter()
            .filter(|d| d.uniqueness == Uniqueness::Unique)
            .collect();

        if unique_deaths.is_empty() || remaining_allocs.is_empty() {
            return Vec::new();
        }

        // Check if any cross-block type match exists before building trees.
        let has_candidate = unique_deaths
            .iter()
            .any(|d| remaining_allocs.iter().any(|a| a.ty == d.ty));

        if !has_candidate {
            return Vec::new();
        }

        // Build dominator trees (lazy — only when needed).
        // Cost control: only build when candidates exist.
        let dom_tree = self
            .dom_tree
            .get_or_insert_with(|| DominatorTree::build(self.func));
        let post_dom_tree = self
            .post_dom_tree
            .get_or_insert_with(|| PostDominatorTree::build(self.func));

        let mut opportunities = Vec::new();
        let mut consumed_allocs: FxHashSet<(ArcBlockId, usize)> = FxHashSet::default();
        let mut consumed_deaths: FxHashSet<(ArcBlockId, usize)> = FxHashSet::default();

        for death in &unique_deaths {
            if consumed_deaths.contains(&(death.block, death.instr_idx)) {
                continue;
            }

            // Find compatible cross-block allocs. Criteria:
            // - Same type (v1: same-type only)
            // - Different block (cross-block)
            // - Death block dominates alloc block
            // - Alloc block post-dominates death block
            // - Not already consumed
            let best = remaining_allocs
                .iter()
                .filter(|a| {
                    a.ty == death.ty
                        && a.block != death.block
                        && !consumed_allocs.contains(&(a.block, a.instr_idx))
                        && dom_tree.dominates(death.block, a.block)
                        && post_dom_tree.post_dominates(a.block, death.block)
                })
                // Prefer nearest target by block index (proxy for dominator depth).
                .min_by_key(|a| a.block.raw());

            let Some(alloc) = best else { continue };

            consumed_allocs.insert((alloc.block, alloc.instr_idx));
            consumed_deaths.insert((death.block, death.instr_idx));

            opportunities.push(ReuseOpportunity {
                source_var: death.var,
                source_block: death.block,
                source_instr: death.instr_idx,
                target_instr: (alloc.block, alloc.instr_idx),
                is_static_unique: true, // v1: only Unique cross-block
            });
        }

        if !opportunities.is_empty() {
            tracing::debug!(
                function = self.func.name.raw(),
                count = opportunities.len(),
                "cross-block reuse opportunities detected"
            );
        }

        opportunities
    }
}
