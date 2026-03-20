//! Reference counting and memory lifecycle management.
//!
//! Provides the RC infrastructure for ARC-managed allocations:
//! - Allocation (`allocate`): `ori_rc_alloc`, `ori_rc_realloc`, `ori_rc_free`, `ori_rc_data_size`
//! - Refcounting: `ori_rc_inc`, `ori_rc_dec`
//! - COW support: `ori_rc_is_unique`
//! - Collection buffers: `ori_buffer_rc_dec`, `ori_map_buffer_rc_dec`
//! - Diagnostics (`debug`): RC tracing, leak attribution, debug assertions

mod allocate;
mod debug;
mod list_rc;
mod map_rc;
mod set_rc;

pub use allocate::*;
#[cfg(all(test, debug_assertions))]
pub(crate) use debug::{
    alloc_registry_insert, alloc_registry_query, alloc_registry_remove, alloc_registry_update,
    freed_set, RT_DEBUG_FORCE,
};
#[cfg(debug_assertions)]
pub(crate) use debug::{alloc_registry_report, rt_debug_check_not_freed};
pub(crate) use debug::{
    check_leaks_enabled, rc_trace_enabled, rt_debug_bounds_warning, rt_debug_null_cow_warning,
    rt_debug_validate_rc,
};
#[cfg(debug_assertions)]
pub use debug::{reset_alloc_registry, reset_freed_set};
pub use list_rc::*;
pub use map_rc::*;
pub use set_rc::*;

use debug::{rc_trace_dec, rc_trace_inc};

/// Exit code for fatal RC errors (underflow, double-free, drop panic).
/// 128 + 6 mirrors the POSIX convention for SIGABRT (signal 6).
/// We use `exit()` instead of `abort()` because `abort()` raises SIGABRT
/// which can hang when signal handlers interfere with process termination.
const SIGABRT_EXIT_CODE: i32 = 128 + 6;

#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic;
use std::sync::atomic::{AtomicI64, Ordering};

// Reference Counting
//
// V5 header layout (32 bytes):
//
//   base+0          base+8            base+16          base+24           base+32
//   [data_size: i64 | elem_dec_fn: ptr | elem_count: i64 | strong_count: i64 | data ...]
//                                                                               ^
//                                                                               data_ptr
//
// From data_ptr: strong_count = data-8, elem_count = data-16,
//                elem_dec_fn = data-24, data_size = data-32
//
// strong_count stays at data_ptr - 8 — all RC operations are unchanged from V2/V3/V4.
// elem_dec_fn stores the element destructor for collection buffers (V4).
// elem_count stores the number of initialized elements for full-range cleanup (V5).
//   When a slice is the last owner, elem_count tells slice_buffer_rc_dec how
//   many elements to clean up (not just the slice's visible range).

/// Live RC allocation counter for debugging and testing.
///
/// Incremented by `ori_rc_alloc`, decremented by `ori_rc_free`.
/// Read by `ori_rc_live_count` to verify all allocations were freed.
pub(crate) static RC_LIVE_COUNT: AtomicI64 = AtomicI64::new(0);

/// Maximum allowed reference count.
///
/// Matches Rust's `Arc` overflow protection: if a single allocation reaches
/// this many live references, something is catastrophically wrong (likely an
/// infinite increment loop). We abort rather than allowing silent wrap-around
/// to negative counts, which would cause use-after-free.
///
/// Value: `isize::MAX` (same as Rust's `Arc`). On 64-bit systems this is
/// `i64::MAX` (9.2 quintillion) — unreachable through normal increments.
///
/// Also serves as the **immortal sentinel**: objects with refcount equal to
/// `MAX_REFCOUNT` are immortal. `ori_rc_inc` and `ori_rc_dec` skip (no-op)
/// when they encounter this value. This is safe because reaching `MAX_REFCOUNT`
/// through increments is physically impossible (~9.2 quintillion increments),
/// so the only way to have this value is intentional pre-allocation.
pub(crate) const MAX_REFCOUNT: i64 = isize::MAX as i64;

// Core RC Functions

/// Size of the RC header in bytes.
///
/// V5 layout: `[data_size | elem_dec_fn | elem_count | strong_count | data ...]`
/// The header is 32 bytes: 8 each for `data_size`, `elem_dec_fn`, `elem_count`,
/// and `strong_count`.
pub const RC_HEADER_SIZE: usize = 32;

/// Byte offset from data pointer to the `elem_dec_fn` field in the RC header.
///
/// `data_ptr.sub(ELEM_DEC_FN_OFFSET)` reaches `elem_dec_fn` at base + 8.
pub const ELEM_DEC_FN_OFFSET: usize = 24;

/// Byte offset from data pointer to the `elem_count` field in the RC header.
///
/// `data_ptr.sub(ELEM_COUNT_OFFSET)` reaches `elem_count` at base + 16.
/// Stores the number of initialized elements for full-range cleanup when
/// a slice is the last owner.
pub const ELEM_COUNT_OFFSET: usize = 16;

/// Store an element destructor function pointer in the RC header.
///
/// Writes `f` at `data_ptr - 24` (the `elem_dec_fn` slot). If `f` is `None`,
/// writes null. Used by `ori_buffer_rc_dec` to persist the element cleanup
/// function so that any dec call reaching zero can perform element cleanup.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 24` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn store_elem_dec_fn(data: *mut u8, f: Option<extern "C" fn(*mut u8)>) {
    let slot = data
        .sub(ELEM_DEC_FN_OFFSET)
        .cast::<Option<extern "C" fn(*mut u8)>>();
    slot.write(f);
}

/// Load the element destructor function pointer from the RC header.
///
/// Reads from `data_ptr - 24` (the `elem_dec_fn` slot). Returns `None` if
/// the slot is null (zero-initialized by `ori_rc_alloc`).
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 24` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn load_elem_dec_fn(data: *mut u8) -> Option<extern "C" fn(*mut u8)> {
    let slot = data
        .sub(ELEM_DEC_FN_OFFSET)
        .cast::<Option<extern "C" fn(*mut u8)>>();
    slot.read()
}

/// Atomically store `elem_dec_fn` in the header using a write-once pattern.
///
/// If the current header value is NULL and `f` is non-NULL, stores `f`.
/// If the header already has a non-NULL value, this is a no-op (the existing
/// value is kept). Thread-safe: uses compare-exchange on the non-single-threaded
/// path, plain write on the single-threaded path.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc`.
pub unsafe fn store_elem_dec_fn_once(data: *mut u8, f: Option<extern "C" fn(*mut u8)>) {
    let Some(func) = f else { return };
    let slot = data.sub(ELEM_DEC_FN_OFFSET);

    #[cfg(not(feature = "single-threaded"))]
    {
        use std::sync::atomic::AtomicPtr;
        let atomic_slot = slot.cast::<AtomicPtr<()>>();
        // CAS: null → func. If already non-null, the existing value wins.
        // Relaxed ordering: the function pointer is always the same value for
        // a given buffer type, so no synchronization is needed beyond atomicity.
        let _ = (*atomic_slot).compare_exchange(
            std::ptr::null_mut(),
            func as *mut (),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    #[cfg(feature = "single-threaded")]
    {
        let current = slot.cast::<*const ()>().read();
        if current.is_null() {
            slot.cast::<*const ()>().write(func as *const ());
        }
    }
}

/// Store the initialized element count in the RC header.
///
/// Writes `count` at `data_ptr - 16` (the `elem_count` slot). Non-slice callers
/// of `ori_buffer_rc_dec` store their `len` here so that if a slice is the last
/// owner, `slice_buffer_rc_dec` can read the full element count for cleanup.
///
/// Uses a "last write wins" strategy: each non-slice dec overwrites with the
/// current `len`. Slices never write to this field. At RC → 0, the stored value
/// reflects the element count from the most recent non-slice owner's dec call.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 16` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn store_elem_count(data: *mut u8, count: i64) {
    let slot = data.sub(ELEM_COUNT_OFFSET).cast::<i64>();
    slot.write(count);
}

/// Load the initialized element count from the RC header.
///
/// Reads from `data_ptr - 16` (the `elem_count` slot). Returns 0 if never
/// stored (zero-initialized by `ori_rc_alloc`).
///
/// Used by `slice_buffer_rc_dec` to determine how many elements to clean up
/// when a slice is the last owner of a buffer.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 16` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn load_elem_count(data: *mut u8) -> i64 {
    let slot = data.sub(ELEM_COUNT_OFFSET).cast::<i64>();
    slot.read()
}

/// Increment the reference count of an RC'd object.
///
/// `data_ptr` points to the data area (V5 header: `data_ptr - 8` = `strong_count`,
/// `data_ptr - 16` = `elem_count`, `data_ptr - 24` = `elem_dec_fn`,
/// `data_ptr - 32` = `data_size`).
///
/// Uses `Relaxed` ordering: the increment only needs to be atomic, not
/// ordered with respect to other memory operations. The incrementing
/// thread already holds a valid reference, so no synchronization is
/// needed. Matches Swift's `swift_retain` and Rust's `Arc::clone`.
#[no_mangle]
pub extern "C" fn ori_rc_inc(data_ptr: *mut u8) {
    if data_ptr.is_null() {
        return;
    }

    rt_debug_validate_rc(data_ptr.cast_const(), "ori_rc_inc");
    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data_ptr.cast_const(), "ori_rc_inc");

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    unsafe {
        #[cfg(not(feature = "single-threaded"))]
        {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            let prev = (*rc_ptr).fetch_add(1, Ordering::Relaxed);

            // Immortal sentinel: skip if refcount is MAX_REFCOUNT. Immortal
            // objects (e.g., pre-allocated empty string) have their RC set to
            // MAX_REFCOUNT at creation and never participate in RC operations.
            // Undo the increment we just did.
            if prev == MAX_REFCOUNT {
                (*rc_ptr).fetch_sub(1, Ordering::Relaxed);
                return;
            }

            if rc_trace_enabled() {
                rc_trace_inc(data_ptr.cast_const(), prev + 1);
            }
        }
        #[cfg(feature = "single-threaded")]
        {
            let rc_ptr = data_ptr.sub(8).cast::<i64>();
            // Immortal sentinel: skip for immortal objects.
            if *rc_ptr == MAX_REFCOUNT {
                return;
            }
            *rc_ptr += 1;

            if rc_trace_enabled() {
                rc_trace_inc(data_ptr.cast_const(), *rc_ptr);
            }
        }
    }
}

/// Abort on refcount underflow (decrement of already-zero refcount).
///
/// Separate `#[cold]` function keeps the fast path in `ori_rc_dec` small.
/// NOT gated behind any flag — this is a safety net for all builds.
/// One branch per decrement (~0.5ns overhead, always predicted not-taken).
#[cold]
#[inline(never)]
pub(super) fn rc_underflow_abort(data_ptr: *mut u8) -> ! {
    // Use raw write to stderr fd to avoid deadlocking on the stderr lock
    // held by the Rust test harness (eprintln! acquires that lock).
    use std::io::Write;
    let msg = format!(
        "ori: FATAL — ori_rc_dec called on already-freed allocation at {data_ptr:p}\n\
         ori: this is a double-free bug in the compiler's RC codegen\n"
    );
    let _ = std::io::stderr().write_all(msg.as_bytes());
    // Use _exit equivalent instead of abort() — abort() raises SIGABRT which
    // can hang when signal handlers interfere. Exit code mirrors SIGABRT convention.
    std::process::exit(SIGABRT_EXIT_CODE);
}

/// Decrement the reference count. If it reaches zero, call the drop function.
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
///
/// `drop_fn` is a type-specialized function generated at compile time that:
/// 1. Decrements reference counts of any RC'd child fields
/// 2. Calls `ori_rc_free(data_ptr, size, align)` to release the memory
///
/// If `drop_fn` is null, the memory is leaked when refcount reaches zero.
/// This should not happen in well-formed programs — every RC type must have
/// a drop function.
///
/// Uses `Release` ordering on the decrement: ensures all writes to the
/// object through this reference are visible before any thread deallocates.
/// An `Acquire` fence before calling the drop function ensures the
/// deallocating thread sees all prior writes from all threads.
///
/// This is the standard ARC pattern from Rust's `Arc::drop` and Swift's
/// `swift_release`.
#[no_mangle]
pub extern "C" fn ori_rc_dec(data_ptr: *mut u8, drop_fn: Option<extern "C" fn(*mut u8)>) {
    if data_ptr.is_null() {
        return;
    }

    rt_debug_validate_rc(data_ptr.cast_const(), "ori_rc_dec");
    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data_ptr.cast_const(), "ori_rc_dec");

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    #[cfg(not(feature = "single-threaded"))]
    {
        // Immortal sentinel check: read refcount first. If MAX_REFCOUNT,
        // this is an immortal object — skip the decrement entirely.
        let current_rc = unsafe {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).load(Ordering::Relaxed)
        };
        if current_rc == MAX_REFCOUNT {
            return;
        }

        let prev = unsafe {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_sub(1, Ordering::Release)
        };

        // Release-mode underflow detection: abort if refcount was already zero.
        // NOT gated behind a flag — one branch per dec (~0.5ns, always not-taken).
        if prev <= 0 {
            rc_underflow_abort(data_ptr);
        }

        if rc_trace_enabled() {
            rc_trace_dec(data_ptr.cast_const(), prev - 1);
        }

        if prev <= 1 {
            // Acquire fence: synchronize with all Release decrements from other
            // threads. This ensures the drop function sees all writes that any
            // thread made through their reference before decrementing.
            atomic::fence(Ordering::Acquire);

            if let Some(f) = drop_fn {
                call_drop_fn(f, data_ptr);
            }
        }
    }

    #[cfg(feature = "single-threaded")]
    {
        let (should_drop, new_rc) = unsafe {
            let rc_ptr = data_ptr.sub(8).cast::<i64>();
            // Immortal sentinel: skip for immortal objects.
            if *rc_ptr == MAX_REFCOUNT {
                return;
            }
            // Release-mode underflow detection (single-threaded path)
            if *rc_ptr <= 0 {
                rc_underflow_abort(data_ptr);
            }
            *rc_ptr -= 1;
            (*rc_ptr <= 0, *rc_ptr)
        };

        if rc_trace_enabled() {
            rc_trace_dec(data_ptr.cast_const(), new_rc);
        }

        if should_drop {
            if let Some(f) = drop_fn {
                call_drop_fn(f, data_ptr);
            }
        }
    }
}

/// Call a drop function with abort-on-panic guard.
///
/// `ori_rc_dec` is declared `nounwind` in LLVM IR, meaning unwinding through
/// it is UB. This wrapper ensures that if a drop function panics, we abort
/// immediately rather than unwinding through the `nounwind` boundary.
/// Matches Rust's `Drop` + `nounwind` contract.
pub(super) fn call_drop_fn(f: extern "C" fn(*mut u8), data_ptr: *mut u8) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f(data_ptr);
    }));
    if result.is_err() {
        eprintln!("ori: drop function panicked — terminating (drop must not unwind)");
        std::process::exit(SIGABRT_EXIT_CODE);
    }
}

/// Get the number of live RC allocations (for testing and debugging).
///
/// Returns the number of `ori_rc_alloc` calls minus `ori_rc_free` calls.
/// Should be 0 at program exit if all memory was properly freed.
#[no_mangle]
pub extern "C" fn ori_rc_live_count() -> i64 {
    RC_LIVE_COUNT.load(Ordering::Relaxed)
}

/// Reset the live RC allocation counter to zero.
///
/// Used by JIT test runners where multiple tests execute in the same process.
/// Each test can start with a fresh counter by calling this before execution.
#[no_mangle]
pub extern "C" fn ori_rc_reset_live_count() {
    RC_LIVE_COUNT.store(0, Ordering::Relaxed);
}

/// Get the current reference count (for testing and debugging).
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
/// Uses `Relaxed` ordering — this is a diagnostic read, not a synchronization point.
#[no_mangle]
pub extern "C" fn ori_rc_count(data_ptr: *const u8) -> i64 {
    if data_ptr.is_null() {
        return 0;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    unsafe {
        #[cfg(not(feature = "single-threaded"))]
        {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).load(Ordering::Relaxed)
        }
        #[cfg(feature = "single-threaded")]
        {
            *data_ptr.sub(8).cast::<i64>()
        }
    }
}

/// Check whether an RC'd object is uniquely owned (refcount == 1).
///
/// This is the foundational COW (copy-on-write) primitive. When the refcount
/// is 1, the caller is the sole owner and may mutate the allocation in place
/// without copying. When the refcount is > 1, the caller must copy before
/// mutating.
///
/// Returns `false` for null pointers (empty collection sentinels are never
/// "unique" — they have no buffer to mutate in place).
///
/// Uses `Relaxed` ordering, which is sufficient because:
/// - If truly unique (RC=1), no other thread holds a reference, so there are
///   no concurrent writers to synchronize with.
/// - If another thread just dropped its reference (Release decrement), the
///   Acquire fence in `ori_rc_dec` ensures visibility before deallocation.
///   We're only reading here, not deallocating.
/// - A stale read of RC=2 when the true value is 1 is safe (we take the slow
///   copy path unnecessarily). A stale read of RC=1 when the true value is 2
///   is impossible: the incrementing thread must have cloned from an existing
///   reference, so the count was already >= 2 before the clone.
///
/// Matches Swift's `isKnownUniquelyReferenced` and Lean 4's `isShared` (inverted).
#[no_mangle]
pub extern "C" fn ori_rc_is_unique(data_ptr: *const u8) -> bool {
    if data_ptr.is_null() {
        return false;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 8 is valid
    // and 8-byte aligned. AtomicI64 has the same layout as i64.
    unsafe {
        #[cfg(not(feature = "single-threaded"))]
        {
            let rc_ptr = data_ptr.sub(8).cast::<AtomicI64>();
            (*rc_ptr).load(Ordering::Relaxed) == 1
        }
        #[cfg(feature = "single-threaded")]
        {
            *data_ptr.sub(8).cast::<i64>() == 1
        }
    }
}

/// Check whether an RC'd object is uniquely owned OR is a null sentinel.
///
/// Returns `true` if `data_ptr` is null (sentinel — no buffer exists) or if
/// the refcount is exactly 1. Used by COW operations that handle sentinels
/// separately: if null, allocate a new buffer; if unique, mutate in place.
#[no_mangle]
pub extern "C" fn ori_rc_is_unique_or_null(data_ptr: *const u8) -> bool {
    data_ptr.is_null() || ori_rc_is_unique(data_ptr)
}
