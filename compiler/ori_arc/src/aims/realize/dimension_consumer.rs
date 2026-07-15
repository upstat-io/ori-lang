//! Dimension-to-consumer matrix (test-only).
//!
//! Pins the remaining lattice dimensions against their downstream consumer
//! predicates with one positive and one negative case each:
//!
//! - Uniqueness → DP-9 `decide_cow` (`realize/decide.rs`).
//! - Shape + Uniqueness → DP-6 `AimsState::is_reuse_candidate` (`lattice/mod.rs`).
//! - Locality → DP-7 `AimsState::is_event_pair_elision_eligible` + DP-8
//!   `AimsState::is_local` (`lattice/mod.rs`).
//!
//! The Effect row (RL-29/RL-30) is a backend-neutral AIMS result. The final
//! pipeline freezes it in the validated executable fact carrier so VM, LLVM,
//! native, compiled-WASM, and JIT projections consume one classification.
//! Backend projections do not rescan ARC instructions to derive semantics.

#[cfg(test)]
mod tests;
