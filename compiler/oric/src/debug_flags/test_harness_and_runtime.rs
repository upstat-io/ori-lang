//! Test-harness, legacy-migrated, sanitizer, and AOT-runtime debug flags.
//!
//! Test-harness flags are consumed directly in `ori_test_harness` (which
//! can't depend on `oric`); runtime-trace flags are consumed directly in
//! `ori_rt` (same constraint). Defined here for documentation and
//! `check-debug-flags.sh` consistency. See the crate-level `debug_flags`
//! module doc for the `dbg_set!`/`dbg_do!` macro pattern and usage.

flags! {
    // Test Harness Flags
    // Note: Consumed directly in `ori_test_harness` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Enable bless mode for the shared test harness.
    ///
    /// When set to `"1"`, `compare_or_bless()` writes actual output as the
    /// new expected baseline instead of comparing. Only `"1"` is accepted —
    /// `"0"`, `"false"`, `"true"` are all treated as disabled.
    /// Usage: `ORI_BLESS=1 cargo test -p ori_arc -- aims_snapshot`
    ORI_BLESS

    /// Per-file wall-clock budget (in seconds) for `ori test --backend=llvm`
    /// worker subprocesses.
    ///
    /// A worker still alive at the budget is killed by the runner's watchdog
    /// and its in-flight test counted FAILED (timeout). Default: 120.
    /// Usage: `ORI_TEST_WORKER_TIMEOUT_SECS=30 ori test --backend=llvm tests/`
    ORI_TEST_WORKER_TIMEOUT_SECS

    /// Path of the on-disk cache file for `ori test --incremental`.
    ///
    /// When set, the parent test runner loads the per-function body-hash
    /// snapshots from this file at startup and saves them after each run, so
    /// unchanged-test skipping works across CLI invocations (without it, the
    /// cache is in-memory only and lives for the runner's lifetime). The
    /// parent owns the file exclusively; LLVM worker subprocesses never read
    /// or write it. A missing or unreadable file is an empty cache.
    /// Usage: `ORI_TEST_INCREMENTAL_CACHE=.ori-test-cache ori test --incremental tests/`
    ORI_TEST_INCREMENTAL_CACHE

    /// Per-spawn worker-protocol nonce for `ori test --backend=llvm`
    /// subprocess isolation. Internal — set by the parent runner, not users.
    ///
    /// The parent generates a fresh unguessable token for each worker spawn;
    /// the worker stamps every protocol line with it and refuses `--__worker`
    /// mode without it. Protocol-shaped stdout lines whose token is absent or
    /// mismatched pass through as plain output, so test programs cannot forge
    /// protocol records (test `print()` shares the worker's stdout). The
    /// worker scrubs the variable from its own environment before any test
    /// code runs, so JIT'd code (and anything it spawns) never sees it.
    ORI_TEST_PROTOCOL_TOKEN

    // Migrated Flags

    /// Print LLVM IR to stderr before JIT compilation.
    ///
    /// Legacy flag — `ORI_DUMP_AFTER_LLVM` is the preferred replacement.
    /// Usage: `ORI_DEBUG_LLVM=1 ori check file.ori`
    ORI_DEBUG_LLVM

    // Sanitizer Flags

    /// Enable sanitizer instrumentation on generated AOT binaries.
    ///
    /// Value: comma-separated sanitizer names (`address`, `undefined`).
    /// Example: `ORI_SANITIZE=address,undefined ori build file.ori`
    ///
    /// Requires Clang on PATH (used as compilation driver for sanitizer passes).
    /// For full coverage, also recompiles `ori_rt` with sanitizer flags (nightly Rust).
    /// Significant performance impact (2-10x slower). Not for main test suite.
    ORI_SANITIZE

    // Runtime Trace Flags
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
