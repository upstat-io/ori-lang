//! RC tracing, debug validation, leak attribution, and freed-pointer tracking.
//!
//! All diagnostic infrastructure for the RC subsystem lives here:
//! - **RC event tracing**: `ORI_TRACE_RC=1|verbose` logs alloc/inc/dec/free events
//! - **Runtime assertions**: `ORI_RT_DEBUG=1` validates RC headers and detects use-after-free
//! - **Leak attribution**: `ORI_CHECK_LEAKS=1` tracks allocation sites for leak reports
//! - **Freed-pointer tracking**: double-free detection via `HashSet<usize>` (debug builds)

#[cfg(debug_assertions)]
use std::collections::{HashMap, HashSet};
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
#[cfg(debug_assertions)]
use std::sync::Mutex;
use std::sync::OnceLock;

use super::RC_LIVE_COUNT;

// RC Event Tracing

/// RC trace verbosity, cached from `ORI_TRACE_RC` env var.
///
/// - `Disabled`: No trace output (default, zero overhead after first check)
/// - `Basic`: Print `[RC] alloc/inc/dec/free` events to stderr
/// - `Verbose`: Basic + backtrace for each operation
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RcTraceMode {
    Disabled,
    Basic,
    Verbose,
}

/// Check the RC trace mode from `ORI_TRACE_RC` env var.
///
/// Caches the result in a `OnceLock` — after the first call, this is a single
/// atomic load (~1ns). Zero overhead when tracing is disabled.
///
/// - Not set / other: `Disabled`
/// - `ORI_TRACE_RC=1` or `true`: `Basic`
/// - `ORI_TRACE_RC=verbose`: `Verbose`
pub(super) fn rc_trace_mode() -> RcTraceMode {
    static MODE: OnceLock<RcTraceMode> = OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("ORI_TRACE_RC").as_deref() {
        Ok("1" | "true") => RcTraceMode::Basic,
        Ok("verbose") => RcTraceMode::Verbose,
        _ => RcTraceMode::Disabled,
    })
}

/// Quick check whether RC event tracing is enabled.
///
/// After the first call, this compiles down to a single atomic load.
#[inline]
pub(crate) fn rc_trace_enabled() -> bool {
    rc_trace_mode() != RcTraceMode::Disabled
}

/// Test-only: force-enable runtime debug assertions regardless of env var.
///
/// Avoids test dependency on `ORI_RT_DEBUG` env var (which is cached in
/// `OnceLock` and would affect all tests in the same process).
#[cfg(test)]
pub(crate) static RT_DEBUG_FORCE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Check whether runtime assertion mode is enabled via `ORI_RT_DEBUG`.
///
/// Caches the result in a `OnceLock` — after the first call, this is a single
/// atomic load (~1ns). Zero overhead when disabled.
///
/// Enabled by `ORI_RT_DEBUG=1` or `ORI_RT_DEBUG=true`.
fn rt_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    #[cfg(test)]
    if RT_DEBUG_FORCE.load(Ordering::Relaxed) {
        return true;
    }
    *ENABLED.get_or_init(|| {
        std::env::var("ORI_RT_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

/// Trace an `ori_rc_alloc` event.
#[cold]
#[inline(never)]
pub(super) fn rc_trace_alloc(data_ptr: *const u8, size: usize, align: usize) {
    let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
    eprintln!("[RC] alloc   {data_ptr:p} size={size} align={align} → rc=1 (live={live})");
    rc_trace_verbose_backtrace();
}

/// Trace an `ori_rc_inc` event.
#[cold]
#[inline(never)]
pub(super) fn rc_trace_inc(data_ptr: *const u8, new_rc: i64) {
    let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
    eprintln!("[RC] inc     {data_ptr:p} → rc={new_rc} (live={live})");
    rc_trace_verbose_backtrace();
}

/// Trace an `ori_rc_dec` event.
#[cold]
#[inline(never)]
pub(super) fn rc_trace_dec(data_ptr: *const u8, new_rc: i64) {
    let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
    if new_rc == 0 {
        eprintln!("[RC] dec     {data_ptr:p} → rc=0 FREE (live={live})");
    } else {
        eprintln!("[RC] dec     {data_ptr:p} → rc={new_rc} (live={live})");
    }
    rc_trace_verbose_backtrace();
}

/// Trace an `ori_rc_free` event.
#[cold]
#[inline(never)]
pub(super) fn rc_trace_free(data_ptr: *const u8, size: usize, align: usize) {
    let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
    eprintln!("[RC] free    {data_ptr:p} size={size} align={align} (live={live})");
    rc_trace_verbose_backtrace();
}

/// Trace an `ori_rc_realloc` event (address change).
#[cold]
#[inline(never)]
pub(super) fn rc_trace_realloc(old_data_ptr: *const u8, new_data_ptr: *const u8, new_size: usize) {
    let live = RC_LIVE_COUNT.load(Ordering::Relaxed);
    eprintln!("[RC] realloc {old_data_ptr:p} → {new_data_ptr:p} size={new_size} (live={live})");
    rc_trace_verbose_backtrace();
}

/// Print a backtrace if verbose tracing is enabled.
#[cold]
#[inline(never)]
fn rc_trace_verbose_backtrace() {
    if rc_trace_mode() == RcTraceMode::Verbose {
        eprintln!("{}", std::backtrace::Backtrace::force_capture());
    }
}

// Leak Attribution (debug builds only)

/// Monotonic allocation counter for leak attribution.
///
/// Each `ori_rc_alloc` call gets a unique ID, making it easy to correlate
/// unfreed allocations with their creation point in an RC event trace.
/// Only incremented when `ORI_CHECK_LEAKS=1`.
#[cfg(debug_assertions)]
static RC_ALLOC_COUNTER: AtomicI64 = AtomicI64::new(0);

/// Access the allocation registry for leak attribution.
///
/// Maps `data_ptr` (as `usize`) → `(alloc_id, size, align)` for every
/// live RC allocation. Entries are added in `ori_rc_alloc` and removed
/// in `ori_rc_free`. At program exit, any remaining entries are leaked
/// allocations — the report prints their IDs, sizes, and addresses.
///
/// Uses `usize` keys because `*mut u8` is not `Send` (required by `Mutex`).
/// The `OnceLock` + init pattern matches `rc_trace_mode()` and `check_leaks_enabled()`.
///
/// Only exists in debug builds (`#[cfg(debug_assertions)]`). In release builds,
/// `ORI_CHECK_LEAKS` still reports the *count* of leaked allocations via
/// `RC_LIVE_COUNT`, but cannot attribute them to specific allocation sites.
/// `(alloc_id, size, align)` — metadata stored per live allocation.
#[cfg(debug_assertions)]
type AllocEntry = (i64, usize, usize);

#[cfg(debug_assertions)]
fn alloc_registry() -> &'static Mutex<HashMap<usize, AllocEntry>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, AllocEntry>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register an allocation in the leak attribution registry.
///
/// Called from `ori_rc_alloc` when `ORI_CHECK_LEAKS=1` in debug builds.
#[cfg(debug_assertions)]
pub(super) fn alloc_registry_insert(data_ptr: *mut u8, size: usize, align: usize) {
    let id = RC_ALLOC_COUNTER.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut reg) = alloc_registry().lock() {
        reg.insert(data_ptr as usize, (id, size, align));
    }
}

/// Deregister an allocation from the leak attribution registry.
///
/// Called from `ori_rc_free` when `ORI_CHECK_LEAKS=1` in debug builds.
#[cfg(debug_assertions)]
pub(super) fn alloc_registry_remove(data_ptr: *mut u8) {
    if let Ok(mut reg) = alloc_registry().lock() {
        reg.remove(&(data_ptr as usize));
    }
}

/// Print the leak attribution report for all unfreed allocations.
///
/// Called from `ori_run_main` when `ORI_CHECK_LEAKS=1` detects leaks.
/// Prints each unfreed allocation's ID, pointer address, size, and alignment.
#[cfg(debug_assertions)]
pub(crate) fn alloc_registry_report() {
    if let Ok(reg) = alloc_registry().lock() {
        if reg.is_empty() {
            return;
        }
        // Sort by alloc_id for deterministic output
        let mut entries: Vec<_> = reg.iter().collect();
        entries.sort_by_key(|(_, (id, _, _))| *id);
        for (&ptr_addr, &(id, size, align)) in &entries {
            eprintln!("  #{id}: ptr=0x{ptr_addr:x} size={size} align={align} (unfreed)");
        }
    }
}

/// Reset the allocation registry and counter.
///
/// Used for test isolation in JIT test runners where multiple tests
/// execute in the same process.
#[cfg(debug_assertions)]
pub fn reset_alloc_registry() {
    if let Ok(mut reg) = alloc_registry().lock() {
        reg.clear();
    }
    RC_ALLOC_COUNTER.store(0, Ordering::Relaxed);
}

// Runtime Assertion Mode (ORI_RT_DEBUG)

/// Validate that the RC header at `data_ptr - 8` holds a plausible refcount.
///
/// Catches: use-after-free (refcount overwritten), corruption, misaligned pointers.
/// Only runs when `ORI_RT_DEBUG=1`. Aborts on invalid values.
///
/// `#[inline]` so the `rt_debug_enabled()` check is inlined at the call site
/// and the branch is predicted not-taken — zero overhead when disabled.
#[inline]
pub(crate) fn rt_debug_validate_rc(data_ptr: *const u8, op: &str) {
    if !rt_debug_enabled() {
        return;
    }
    rt_debug_validate_rc_impl(data_ptr, op);
}

/// The heavy validation logic — extracted to `#[cold]` so it doesn't pollute
/// the instruction cache in the fast path.
#[cold]
#[inline(never)]
fn rt_debug_validate_rc_impl(data_ptr: *const u8, op: &str) {
    unsafe {
        let rc = data_ptr.sub(8).cast::<i64>().read();
        if rc <= 0 || rc >= 1_000_000 {
            eprintln!(
                "ori: ORI_RT_DEBUG — {op} on {data_ptr:p}: \
                 implausible refcount {rc} (expected 1..999999, \
                 likely use-after-free or corruption)"
            );
            std::process::exit(super::SIGABRT_EXIT_CODE);
        }
    }
}

/// Access the freed-pointer tracking set for double-free detection.
///
/// `HashSet<usize>` stores raw pointer addresses (as `usize` for `Send`
/// compatibility). Only active in debug builds when `ORI_RT_DEBUG=1`.
#[cfg(debug_assertions)]
pub(crate) fn freed_set() -> &'static Mutex<HashSet<usize>> {
    static SET: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Check that a pointer has not already been freed.
///
/// Called from `ori_rc_inc` and `ori_rc_dec` to catch use-after-free.
/// Aborts immediately if the pointer is in the freed set.
#[cfg(debug_assertions)]
pub(crate) fn rt_debug_check_not_freed(data_ptr: *const u8, op: &str) {
    if !rt_debug_enabled() {
        return;
    }
    if let Ok(set) = freed_set().lock() {
        if set.contains(&(data_ptr as usize)) {
            eprintln!(
                "ori: ORI_RT_DEBUG — {op} on {data_ptr:p}: \
                 pointer was already freed (use-after-free)"
            );
            std::process::exit(super::SIGABRT_EXIT_CODE);
        }
    }
}

/// Register a pointer as freed, aborting on double-free.
///
/// Called from `ori_rc_free`. If the pointer was already in the freed set,
/// this is a double-free bug in the compiler's RC codegen.
#[cfg(debug_assertions)]
pub(super) fn rt_debug_register_freed(data_ptr: *const u8) {
    if !rt_debug_enabled() {
        return;
    }
    if let Ok(mut set) = freed_set().lock() {
        if !set.insert(data_ptr as usize) {
            eprintln!("ori: ORI_RT_DEBUG — ori_rc_free on {data_ptr:p}: double-free detected");
            std::process::exit(super::SIGABRT_EXIT_CODE);
        }
    }
}

/// Reset the freed-pointer tracking set.
///
/// Used for test isolation in JIT test runners where multiple tests
/// execute in the same process.
#[cfg(debug_assertions)]
pub fn reset_freed_set() {
    if let Ok(mut set) = freed_set().lock() {
        set.clear();
    }
}

/// Log a warning when a COW function receives a null data pointer.
///
/// This is a diagnostic aid, not an error — empty list mutations are valid
/// but may indicate unexpected state during debugging.
#[cold]
#[inline(never)]
pub(crate) fn rt_debug_null_cow_warning(op: &str) {
    if !rt_debug_enabled() {
        return;
    }
    eprintln!("ori: ORI_RT_DEBUG — {op}: null data_ptr (empty list mutation)");
}

/// Log a warning when a list operation has an out-of-bounds index.
///
/// The COW functions already handle OOB by returning the list unchanged,
/// but in debug mode we want visibility into silent bounds failures.
#[cold]
#[inline(never)]
pub(crate) fn rt_debug_bounds_warning(op: &str, index: i64, len: i64) {
    if !rt_debug_enabled() {
        return;
    }
    eprintln!("ori: ORI_RT_DEBUG — {op}: index {index} out of bounds (len={len})");
}

// Leak Detection

/// Check whether ARC leak detection is enabled via environment variable.
///
/// Caches the result in a `OnceLock` to avoid repeated `getenv` syscalls.
/// Enabled by `ORI_CHECK_LEAKS=1` or `ORI_CHECK_LEAKS=true`.
pub(crate) fn check_leaks_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("ORI_CHECK_LEAKS")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false)
    })
}
