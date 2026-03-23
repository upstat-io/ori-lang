//! Argument ownership annotation for ARC IR call sites.
//!
//! Populates `arg_ownership` on `Apply`/`Invoke` instructions so that
//! downstream passes (AIMS realization, LLVM emitter) can read per-argument
//! ownership directly from the IR.
//!
//! The legacy RC insertion algorithm (backward-walk Perceus) has been
//! replaced by the AIMS unified pipeline (`aims::realize::realize_rc_reuse`).
//! Only the argument annotation logic remains, as it is shared by both
//! the AIMS pipeline and external callers.
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/RC.lean`
//! - Koka: Perceus paper §3.2
//! - Swift: `lib/SILOptimizer/ARC/`

mod annotate;

pub use self::annotate::annotate_arg_ownership;
