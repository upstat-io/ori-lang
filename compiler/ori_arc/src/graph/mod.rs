//! Shared graph analysis utilities for ARC optimization passes.
//!
//! Functions in this module are generic graph operations on [`ArcFunction`]
//! that multiple independent passes need. They live here rather than in a
//! specific pass module so that passes do not import from each other —
//! keeping the dependency graph flat (all passes depend on `graph`, none
//! depend on each other).
//!
//! ## Submodules
//!
//! - [`call_graph`] — inter-function call graph for SCC-based borrow inference
//! - [`dominator`] — dominator tree (Cooper-Harvey-Kennedy algorithm)
//! - [`post_dominator`] — post-dominator tree (CHK on reverse CFG)

pub mod call_graph;
mod cycle_regions;
mod dominator;
mod post_dominator;
pub mod scc;
mod traversal;

pub(crate) use cycle_regions::CycleRegions;
pub use dominator::DominatorTree;
pub use post_dominator::PostDominatorTree;
pub(crate) use traversal::{chk_intersect, collect_invoke_defs, compute_pred_counts};
pub use traversal::{compute_postorder, compute_predecessors, successor_block_ids};

#[cfg(test)]
mod tests;
