//! Debug environment-variable registry and access macros.
//!
//! [`dbg_set!`] tests a flag, [`dbg_do!`] gates an expression, and `flags!`
//! declares documented constants. Flags work in every build profile. Crates
//! below `oric` in the dependency graph read their variables directly.

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
pub mod prelude;
mod test_harness_and_runtime;

pub use prelude::*;
