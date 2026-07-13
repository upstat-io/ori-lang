//! Dimension-to-consumer matrix (test-only).
//!
//! Pins the remaining lattice dimensions against their downstream consumer
//! predicates with one positive and one negative case each:
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
