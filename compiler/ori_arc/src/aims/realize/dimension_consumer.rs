//! Dimension-to-consumer matrix (test-only).
//!
//! Each AIMS lattice dimension drives a downstream decision predicate
//! that still consumes its dimension correctly when the lattice optimizes over
//! the Phase-5 burden-emitted baseline. The Cardinality + Consumption row
//! (DP-2/DP-3) is pinned in `realize/burden_elim/tests.rs` over burden-emitted
//! IR; this module pins the remaining dimension rows against their consumer
//! decision predicates with one positive and one negative pin each:
//!
//! - Uniqueness → DP-9 `decide_cow` (`realize/decide.rs`).
//! - Shape + Uniqueness → DP-6 `AimsState::is_reuse_candidate` (`lattice/mod.rs`).
//! - Locality → DP-7 `AimsState::is_rc_skip_eligible` + DP-8
//!   `AimsState::is_local` (`lattice/mod.rs`).
//!
//! The Effect row (RL-29/RL-30) consumer lives in `ori_llvm`
//! (`function_compiler/purity_analysis.rs`); its dimension-to-consumer pins are
//! in that crate's `purity_analysis/tests.rs`.

#[cfg(test)]
mod tests;
