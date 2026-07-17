//! Centralized debug flags for the Ori compiler.
//!
//! This module is the single source of truth for compiler debugging environment
//! variables. Flags are checked at runtime via env vars in ALL builds
//! (debug and release). The overhead is a single `std::env::var()` call per
//! flag — negligible for a CLI compiler.
//!
//! # Usage
//!
//! ```bash
//! ORI_DUMP_AFTER_ARC=1 ori build program.ori
//! ORI_DEBUG_LLVM=1 ori check program.ori
//! ```
//!
//! # Pattern
//!
//! Three macros:
//! - `dbg_set!` — returns `true` if the flag is set
//! - `dbg_do!` — executes an expression if the flag is set
//! - `flags!` — defines flag constants with doc comments
//!
//! Note: `ori_llvm` cannot depend on `oric` (the dep direction is reversed),
//! so flags consumed inside `ori_llvm` (e.g., evaluator JIT path) use raw
//! `std::env::var` checks. The `oric` call sites use `dbg_do!`/`dbg_set!`
//! macros for consistent flag checking.

/// Check if a debug flag is set. Works in both debug and release builds.
///
/// The flag is considered "set" if the env var exists and is not `"0"`.
///
/// # Examples
///
/// ```rust
/// use oric::{dbg_set, debug_flags};
///
/// let expected = std::env::var(debug_flags::ORI_DEBUG_LLVM)
///     .is_ok_and(|value| value != "0");
/// assert_eq!(dbg_set!(debug_flags::ORI_DEBUG_LLVM), expected);
/// ```
#[macro_export]
macro_rules! dbg_set {
    ($flag:expr) => {{
        let flag = std::env::var($flag);
        flag.is_ok() && flag.as_deref() != Ok("0")
    }};
}

/// Execute an expression only if a debug flag is set.
///
/// Works in both debug and release builds.
///
/// # Examples
///
/// ```rust
/// use oric::{dbg_do, dbg_set, debug_flags};
///
/// let expected = dbg_set!(debug_flags::ORI_DEBUG_LLVM);
/// let mut ran = false;
/// dbg_do!(debug_flags::ORI_DEBUG_LLVM, {
///     ran = true;
/// });
/// assert_eq!(ran, expected);
/// ```
#[macro_export]
macro_rules! dbg_do {
    ($flag:expr, $expr:expr) => {
        if $crate::dbg_set!($flag) {
            $expr
        }
    };
}

/// Define debug flag constants with doc comments.
///
/// Generates `pub const FLAG: &str = "FLAG"` for each flag, preserving
/// the doc comments for IDE support and `check-debug-flags.sh` parsing.
macro_rules! flags {
    ($($(#[doc = $doc:expr])+ $flag:ident)*) => {$(
        $(#[doc = $doc])+
        pub const $flag: &str = stringify!($flag);
    )*};
}

mod aims_ablation_phase5_core;
mod aims_ablation_phase5_lineage;
mod diagnostic_dumps;
mod test_harness_and_runtime;

pub use aims_ablation_phase5_core::*;
pub use aims_ablation_phase5_lineage::*;
pub use diagnostic_dumps::*;
pub use test_harness_and_runtime::*;
