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
//! Historical influence: the `debug_flags` crate SHAPE from Roc:
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

flags! {
    // === Phase Dumps ===

    /// Dump the parsed AST to stderr after parsing.
    ///
    /// Shows the raw AST structure before type checking.
    /// Usage: `ORI_DUMP_AFTER_PARSE=1 ori check file.ori`
    ORI_DUMP_AFTER_PARSE

    /// Dump the typed IR to stderr after type checking.
    ///
    /// Shows type annotations on every node and resolved method dispatch.
    /// Usage: `ORI_DUMP_AFTER_TYPECK=1 ori check file.ori`
    ORI_DUMP_AFTER_TYPECK

    /// Dump the ARC IR to stderr after ARC lowering.
    ///
    /// Shows RC strategy decisions, drop placement, and COW operations.
    /// Usage: `ORI_DUMP_AFTER_ARC=1 ori build file.ori`
    ORI_DUMP_AFTER_ARC

    /// Dump annotated LLVM IR to stderr after LLVM codegen.
    ///
    /// Enhanced version of `ORI_DEBUG_LLVM` with Ori-aware annotations.
    /// Usage: `ORI_DUMP_AFTER_LLVM=1 ori build file.ori`
    ORI_DUMP_AFTER_LLVM

    /// Emit `GraphViz` DOT output of ARC IR control-flow graphs to stderr.
    ///
    /// Each function becomes a digraph with basic blocks as table nodes and
    /// RC operations color-highlighted. Pipe to file and render with `dot`.
    /// Usage: `ORI_EMIT_ARC_DOT=1 ori build file.ori 2> arc.dot`
    ORI_EMIT_ARC_DOT

    // === Repr-Opt Configuration ===
    // Note: Consumed directly in `ori_repr` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Disable all representation optimizations (integer narrowing, enum packing).
    ///
    /// CLI alternative: `--no-repr-opt`
    /// Usage: `ORI_NO_REPR_OPT=1 ori build file.ori`
    ORI_NO_REPR_OPT

    /// Disable the post-realize redundant project-alias dec cleanup pass.
    ///
    /// Consumed in `ori_arc::aims::realize::cleanup_redundant`.
    /// Defined here for documentation and `check-debug-flags.sh` consistency.
    /// Usage: `ORI_DISABLE_REDUNDANT_CLEANUP=1 ori build file.ori`
    ORI_DISABLE_REDUNDANT_CLEANUP

    /// Disable the burden-op emission pass (Step 4b of the AIMS pipeline).
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline` for the
    /// empty-harness predicate-parity check. When set, `emit_burden_ops` is skipped
    /// entirely and the predicate-stack realization path runs as the baseline.
    /// Usage: `ORI_DISABLE_BURDEN_OPS=1 ori build file.ori`
    ORI_DISABLE_BURDEN_OPS

    /// Disable the DP-2/DP-3 burden-op elimination pass.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` to conditionally
    /// bypass burden-op elimination for diagnostic bisection.
    /// Usage: `ORI_DISABLE_BURDEN_ELIM=1 ori build file.ori`
    ORI_DISABLE_BURDEN_ELIM

    /// Dump each function's ARC IR to stderr immediately after Step 4b
    /// `emit_burden_ops`, before any realization.
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Surfaces
    /// the faithful Phase-5 `BurdenInc` / `BurdenDec*` emission for VF-1 residual
    /// localization (post-realize `ORI_DUMP_AFTER_ARC` cannot show pre-realize
    /// burden placement).
    /// Usage: `ORI_DUMP_AFTER_BURDEN=1 ori build file.ori`
    ORI_DUMP_AFTER_BURDEN

    /// Dump each function's ARC IR to stderr immediately after the DP-2/DP-3
    /// burden-op elimination pass.
    ///
    /// Consumed in `ori_arc::pipeline::aims_pipeline::run_aims_pipeline`. Surfaces
    /// the post-elimination `BurdenInc` / `BurdenDec*` set so the eliminator's
    /// effect can be bisected against the pre-elimination `ORI_DUMP_AFTER_BURDEN`
    /// snapshot.
    /// Usage: `ORI_DUMP_AFTER_BURDEN_ELIM=1 ori build file.ori`
    ORI_DUMP_AFTER_BURDEN_ELIM

    /// Disable the predicate-stack `RcInc` / `RcDec` realization phases, leaving
    /// the burden path (elimination + mechanical `BurdenInc → RcInc` /
    /// `BurdenDec → RcDec` lowering) as the sole real-RC emitter.
    ///
    /// Consumed in `ori_arc::aims::realize::emit_unified` to drive the burden-path
    /// self-sufficiency probe. When set, the predicate-stack emission is suppressed
    /// and surviving whole-var burden ops lower to real RC; default (unset) leaves
    /// the default emission path byte-identical.
    /// Usage: `ORI_DISABLE_PREDICATE_STACK_RC=1 ori build file.ori`
    ORI_DISABLE_PREDICATE_STACK_RC

    // === Alive2 IR Capture ===

    /// Dump raw LLVM IR to a `.preopt.ll` file after verification, before optimization.
    ///
    /// Produces machine-readable IR suitable for alive-tv input.
    /// Distinct from `ORI_DUMP_AFTER_LLVM` which dumps annotated IR to stderr
    /// for human debugging (and before verification).
    /// Usage: `ORI_DUMP_PREOPT_LLVM=1 ori build file.ori`
    ORI_DUMP_PREOPT_LLVM

    /// Dump raw LLVM IR to a `.postopt.ll` file after optimization, before emission.
    ///
    /// Produces machine-readable IR suitable for alive-tv input.
    /// Usage: `ORI_DUMP_POSTOPT_LLVM=1 ori build file.ori`
    ORI_DUMP_POSTOPT_LLVM

    /// Enable Alive2 IR capture: dumps both pre-opt and post-opt IR.
    ///
    /// Convenience flag that enables both `ORI_DUMP_PREOPT_LLVM` and
    /// `ORI_DUMP_POSTOPT_LLVM` and places output in `build/alive2-results/`.
    /// Usage: `ORI_ALIVE2_CAPTURE=1 ori build file.ori`
    ORI_ALIVE2_CAPTURE

    // === Verification ===

    /// Enable ARC IR verification after the AIMS pipeline.
    ///
    /// Adds extra correctness checks (RC balance, drop placement).
    /// Usage: `ORI_VERIFY_ARC=1 ori build file.ori`
    ORI_VERIFY_ARC

    /// Enable LLVM IR verification after every optimization pass.
    ///
    /// Catches which optimization pass breaks IR well-formedness.
    /// Significant performance impact (~30-60% slower LLVM tests).
    /// Usage: `ORI_VERIFY_EACH=1 ori build file.ori`
    ORI_VERIFY_EACH

    /// Run in-pipeline RC audit on emitted LLVM IR.
    ///
    /// Detects leaks, double-frees, COW sequencing bugs, and ABI violations.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ori build file.ori`
    ORI_AUDIT_CODEGEN

    /// Enable strict (pessimistic) mode for the codegen audit.
    ///
    /// Treats COW functions as always-freeing (potential double-free becomes
    /// definite error). Also tracks function pointer parameters as RC-managed.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1 ori build file.ori`
    ORI_AUDIT_STRICT

    /// Filter codegen audit to a single function by name.
    ///
    /// Only analyzes the function whose LLVM name contains the given string.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_FUNCTION=main ori build file.ori`
    ORI_AUDIT_FUNCTION

    /// Run LLVM lint pass (`function(lint)`) to detect likely-undefined behavior.
    ///
    /// Detects division by potential zero, suspicious alignment, unreachable
    /// patterns, and UB in instruction operands. Auto-enabled by `ORI_AUDIT_CODEGEN=1`.
    /// Usage: `ORI_LLVM_LINT=1 ori build file.ori`
    ORI_LLVM_LINT

    // === Test Harness Flags ===
    // Note: Consumed directly in `ori_test_harness` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Enable bless mode for the shared test harness.
    ///
    /// When set to `"1"`, `compare_or_bless()` writes actual output as the
    /// new expected baseline instead of comparing. Only `"1"` is accepted —
    /// `"0"`, `"false"`, `"true"` are all treated as disabled.
    /// Usage: `ORI_BLESS=1 cargo test -p ori_arc -- aims_snapshot`
    ORI_BLESS

    // === Existing Flags (migrated) ===

    /// Print LLVM IR to stderr before JIT compilation.
    ///
    /// Legacy flag — `ORI_DUMP_AFTER_LLVM` is the preferred replacement.
    /// Usage: `ORI_DEBUG_LLVM=1 ori check file.ori`
    ORI_DEBUG_LLVM

    // === Sanitizer Flags ===

    /// Enable sanitizer instrumentation on generated AOT binaries.
    ///
    /// Value: comma-separated sanitizer names (`address`, `undefined`).
    /// Example: `ORI_SANITIZE=address,undefined ori build file.ori`
    ///
    /// Requires Clang on PATH (used as compilation driver for sanitizer passes).
    /// For full coverage, also recompiles `ori_rt` with sanitizer flags (nightly Rust).
    /// Significant performance impact (2-10x slower). Not for main test suite.
    ORI_SANITIZE

    // === Runtime Trace Flags ===
    // Note: These are checked directly in `ori_rt` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Enable RC operation tracing in the runtime.
    ///
    /// Modes: `1` (summary on exit), `verbose` (per-operation log), `quiet` (stats only).
    /// Usage: `ORI_TRACE_RC=1 ori run file.ori`
    ORI_TRACE_RC

    /// Enable runtime debug assertions (bounds checks, underflow detection).
    ///
    /// Usage: `ORI_RT_DEBUG=1 ori run file.ori`
    ORI_RT_DEBUG

    /// Enable leak detection (report live RC objects on exit).
    ///
    /// Usage: `ORI_CHECK_LEAKS=1 ori run file.ori`
    ORI_CHECK_LEAKS
}

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
