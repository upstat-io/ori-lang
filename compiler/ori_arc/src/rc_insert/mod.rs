//! Argument ownership annotation for ARC IR call sites.
//!
//! Populates `arg_ownership` on `Apply`/`Invoke` instructions so that
//! AIMS realization and every physical consumer can read per-argument
//! ownership directly from the shared IR without re-deriving policy.
//!
//! The legacy RC insertion algorithm (backward-walk Perceus) has been
//! replaced by the AIMS unified pipeline (`aims::realize::realize_rc_reuse`).
//! Only the argument annotation logic remains, as it is shared by both
//! the AIMS pipeline and external callers.
//!
//! # Historical design influences
//!
//! The RC-insertion SHAPE drew on prior work as historical influences;
//! Ori's formulation is its own (see Spec: Annex E §AIMS).
//! Influence shapes: Lean 4 LCNF RC insertion; Koka Perceus (Reinking et al.,
//! PLDI 2021) §3.2; Swift ARC optimizer.

mod annotate;
pub(crate) mod closure_resolve;

pub use self::annotate::annotate_arg_ownership;
pub(crate) use self::annotate::annotate_arg_ownership_with_exact_callables;

#[cfg(test)]
mod tests;
