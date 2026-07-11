//! Centralized debug flags for the Ori compiler.
//!
//! All compiler debugging environment variables are defined here as the single
//! source of truth. Flags are checked at runtime via env vars in ALL builds
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
/// ```ignore
/// use crate::debug_flags;
///
/// if dbg_set!(debug_flags::ORI_DEBUG_LLVM) {
///     eprintln!("LLVM IR dump enabled");
/// }
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
/// ```ignore
/// use crate::debug_flags;
///
/// dbg_do!(debug_flags::ORI_DEBUG_LLVM, {
///     eprintln!("=== LLVM IR ===");
///     eprintln!("{}", module.print_to_string());
/// });
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

mod aims_ablation_late_pipeline;
mod aims_ablation_phase5_core;
mod aims_ablation_phase5_lineage;
mod diagnostic_dumps;
mod test_harness_and_runtime;

pub use aims_ablation_late_pipeline::*;
pub use aims_ablation_phase5_core::*;
pub use aims_ablation_phase5_lineage::*;
pub use diagnostic_dumps::*;
pub use test_harness_and_runtime::*;

// Compile-time sync check: verify that audit env var names in `oric::debug_flags`
// match the canonical constants in `ori_llvm::verify`. If either side renames a
// flag, this assertion fails at compile time.
#[cfg(feature = "llvm")]
const _: () = {
    assert!(
        const_str_eq(ORI_AUDIT_CODEGEN, ori_llvm::verify::ENV_AUDIT_CODEGEN),
        "ORI_AUDIT_CODEGEN constant drifted between oric and ori_llvm"
    );
    assert!(
        const_str_eq(ORI_AUDIT_STRICT, ori_llvm::verify::ENV_AUDIT_STRICT),
        "ORI_AUDIT_STRICT constant drifted between oric and ori_llvm"
    );
    assert!(
        const_str_eq(ORI_AUDIT_FUNCTION, ori_llvm::verify::ENV_AUDIT_FUNCTION),
        "ORI_AUDIT_FUNCTION constant drifted between oric and ori_llvm"
    );
    assert!(
        const_str_eq(ORI_NO_REPR_OPT, ori_repr::NarrowingPolicy::ENV_NO_REPR_OPT),
        "ORI_NO_REPR_OPT constant drifted between oric and ori_repr"
    );
};

/// Const-compatible string equality (stable Rust lacks `const PartialEq` for `&str`).
#[cfg(feature = "llvm")]
const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
