//! Reference counting and memory lifecycle management.
//!
//! Provides the RC infrastructure for ARC-managed allocations:
//! - Allocation: `ori_rc_alloc` (data pointer with 16-byte RC header)
//! - Lifecycle: `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free`
//! - COW support: `ori_rc_is_unique`, `ori_rc_realloc`
//! - Slice support: `ori_rc_data_size` (read stored allocation size)
//! - Collection buffers: `ori_buffer_rc_dec`, `ori_map_buffer_rc_dec`
//! - Diagnostics: RC tracing, leak attribution, debug assertions

mod collections;
mod debug;

pub use collections::*;
#[cfg(all(test, debug_assertions))]
pub(crate) use debug::freed_set;
#[cfg(test)]
pub(crate) use debug::RT_DEBUG_FORCE;
#[cfg(debug_assertions)]
pub(crate) use debug::{alloc_registry_report, rt_debug_check_not_freed};
pub(crate) use debug::{
    check_leaks_enabled, rc_trace_enabled, rt_debug_bounds_warning, rt_debug_null_cow_warning,
    rt_debug_validate_rc,
};
#[cfg(debug_assertions)]
pub use debug::{reset_alloc_registry, reset_freed_set};

#[cfg(debug_assertions)]
use debug::{alloc_registry_insert, alloc_registry_remove, rt_debug_register_freed};
use debug::{rc_trace_alloc, rc_trace_dec, rc_trace_free, rc_trace_inc};

#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic;
use std::sync::atomic::{AtomicI64, Ordering};

// ── Reference Counting (V3: 16-byte header, data-pointer style) ──────────
//
// Heap layout for RC'd objects:
//
//   +──────────────────+──────────────────+───────────────────────+
//   | data_size: i64    | strong_count: i64 | data bytes ...      |
//   +──────────────────+──────────────────+───────────────────────+
//   ^                   ^                   ^
//   base (ptr - 16)     ptr - 8             data_ptr (returned by ori_rc_alloc)
//
// The data pointer points directly to user data, NOT to the header.
// strong_count lives at `data_ptr - 8` (unchanged from V2).
// data_size lives at `data_ptr - 16` (new in V3).
//
// Advantages:
// - Data pointer can be passed to C FFI without adjustment
// - Single pointer on stack (no separate header pointer)
// - data_size enables seamless slice deallocation: when a slice is the
//   last reference, it can compute the original data pointer and read
//   the allocation size from the header without external bookkeeping
// - strong_count stays at `data_ptr - 8`, so ALL refcount operations
//   (inc, dec, count, is_unique) are unchanged from V2
//
// When refcount reaches zero, a type-specialized drop function handles:
// 1. Decrementing reference counts of RC'd child fields
// 2. Calling ori_rc_free(data_ptr, size, align) to release memory

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
/// `i64::MAX` (9.2 quintillion) — unreachable in practice, but the check
/// costs essentially nothing (one compare per `ori_rc_inc`).
pub(crate) const MAX_REFCOUNT: i64 = isize::MAX as i64;

// ── Core RC Functions ────────────────────────────────────────────────

/// Size of the RC header in bytes.
///
/// V3 layout: `[data_size: i64 | strong_count: i64 | data ...]`
/// The header is 16 bytes: 8 for `data_size` + 8 for `strong_count`.
pub const RC_HEADER_SIZE: usize = 16;

/// Allocate a new reference-counted object.
///
/// Allocates `size + 16` bytes with the given alignment, initializes
/// `data_size` to `size` and `strong_count` to 1, and returns a pointer
/// to the data area.
///
/// Layout: `[data_size: i64 | strong_count: i64 | data bytes ...]`
///          ^                 ^                    ^
///          base (ptr - 16)   ptr - 8              returned `data_ptr`
///
/// Returns null on allocation failure.
#[no_mangle]
pub extern "C" fn ori_rc_alloc(size: usize, align: usize) -> *mut u8 {
    let align = align.max(8); // Minimum 8-byte alignment for header fields
    let total_size = size + RC_HEADER_SIZE;

    let base = crate::ori_alloc(total_size, align);
    if base.is_null() {
        return std::ptr::null_mut();
    }

    // Initialize data_size and strong_count.
    // No ordering needed — this allocation is not yet visible to other threads.
    // SAFETY: base is valid and 8-byte aligned
    unsafe {
        // data_size at base + 0
        base.cast::<i64>().write(size as i64);

        // strong_count at base + 8
        let rc_ptr = base.add(8);
        #[cfg(not(feature = "single-threaded"))]
        rc_ptr.cast::<AtomicI64>().write(AtomicI64::new(1));
        #[cfg(feature = "single-threaded")]
        rc_ptr.cast::<i64>().write(1);
    }

    RC_LIVE_COUNT.fetch_add(1, Ordering::Relaxed);

    // Return data pointer (16 bytes past the base)
    // SAFETY: base is valid for total_size bytes, so base + 16 is valid
    let data_ptr = unsafe { base.add(RC_HEADER_SIZE) };

    #[cfg(debug_assertions)]
    if check_leaks_enabled() {
        alloc_registry_insert(data_ptr, size, align);
    }

    if rc_trace_enabled() {
        rc_trace_alloc(data_ptr.cast_const(), size, align);
    }

    data_ptr
}

/// Increment the reference count of an RC'd object.
///
/// `data_ptr` points to the data area. `strong_count` is at `data_ptr - 8`.
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

            // Overflow protection: abort if refcount was already at the maximum.
            // fetch_add returns the *previous* value, so prev == MAX_REFCOUNT
            // means the new value overflowed. Matches Rust's Arc::clone check.
            if prev == MAX_REFCOUNT {
                rc_overflow_abort();
            }

            if rc_trace_enabled() {
                rc_trace_inc(data_ptr.cast_const(), prev + 1);
            }
        }
        #[cfg(feature = "single-threaded")]
        {
            let rc_ptr = data_ptr.sub(8).cast::<i64>();
            if *rc_ptr == MAX_REFCOUNT {
                rc_overflow_abort();
            }
            *rc_ptr += 1;

            if rc_trace_enabled() {
                rc_trace_inc(data_ptr.cast_const(), *rc_ptr);
            }
        }
    }
}

/// Abort on refcount overflow.
///
/// Separate `#[cold]` function keeps the fast path in `ori_rc_inc` small
/// and avoids polluting the instruction cache with error handling code.
#[cold]
#[inline(never)]
fn rc_overflow_abort() -> ! {
    eprintln!("ori: refcount overflow — aborting (possible reference cycle or infinite clone)");
    std::process::abort();
}

/// Abort on refcount underflow (decrement of already-zero refcount).
///
/// Separate `#[cold]` function keeps the fast path in `ori_rc_dec` small.
/// NOT gated behind any flag — this is a safety net for all builds.
/// One branch per decrement (~0.5ns overhead, always predicted not-taken).
#[cold]
#[inline(never)]
pub(super) fn rc_underflow_abort(data_ptr: *mut u8) -> ! {
    eprintln!("ori: FATAL — ori_rc_dec called on already-freed allocation at {data_ptr:p}");
    eprintln!("ori: this is a double-free bug in the compiler's RC codegen");
    std::process::abort();
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
        eprintln!("ori: drop function panicked — aborting (drop must not unwind)");
        std::process::abort();
    }
}

/// Free a reference-counted allocation unconditionally.
///
/// Deallocates from `data_ptr - 16` with total size `size + 16`.
/// Typically called as the last step of a type-specialized drop function.
///
/// `size` and `align` are the data size and alignment (same values passed
/// to `ori_rc_alloc`). The 16-byte header is accounted for internally.
#[no_mangle]
pub extern "C" fn ori_rc_free(data_ptr: *mut u8, size: usize, align: usize) {
    if data_ptr.is_null() {
        return;
    }

    #[cfg(debug_assertions)]
    rt_debug_register_freed(data_ptr.cast_const());

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 16 is the base
    let base = unsafe { data_ptr.sub(RC_HEADER_SIZE) };
    let total_size = size + RC_HEADER_SIZE;
    let align = align.max(8);

    crate::ori_free(base, total_size, align);

    RC_LIVE_COUNT.fetch_sub(1, Ordering::Relaxed);

    #[cfg(debug_assertions)]
    if check_leaks_enabled() {
        alloc_registry_remove(data_ptr);
    }

    if rc_trace_enabled() {
        rc_trace_free(data_ptr.cast_const(), size, align);
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

/// Reallocate a reference-counted buffer to a new data size.
///
/// Adjusts the underlying allocation (which includes the 16-byte RC header)
/// while preserving the refcount. Updates the stored `data_size` to
/// `new_data_size`. Returns the new data pointer (16 bytes past the base).
///
/// # Preconditions
/// - `data_ptr` was returned by `ori_rc_alloc`
/// - `ori_rc_is_unique(data_ptr)` is true (caller is the sole owner)
/// - `new_data_size > 0` (use `ori_rc_free` to deallocate)
///
/// Returns null on allocation failure (original allocation is NOT freed in
/// that case — caller retains ownership).
#[no_mangle]
pub extern "C" fn ori_rc_realloc(
    data_ptr: *mut u8,
    old_data_size: usize,
    new_data_size: usize,
    align: usize,
) -> *mut u8 {
    if data_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let align = align.max(8);
    let old_total = old_data_size + RC_HEADER_SIZE;
    let new_total = new_data_size + RC_HEADER_SIZE;

    let old_layout = match std::alloc::Layout::from_size_align(old_total, align) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 16 is the
    // base pointer with layout (old_data_size + 16, align). realloc preserves
    // the first min(old_total, new_total) bytes, including the RC header.
    let base = unsafe { data_ptr.sub(RC_HEADER_SIZE) };
    let new_base = unsafe { std::alloc::realloc(base, old_layout, new_total) };

    if new_base.is_null() {
        return std::ptr::null_mut();
    }

    // Update stored data_size to reflect the new allocation
    // SAFETY: new_base is valid for new_total bytes
    unsafe {
        new_base.cast::<i64>().write(new_data_size as i64);
    }

    // Return data pointer (16 bytes past the header)
    unsafe { new_base.add(RC_HEADER_SIZE) }
}

/// Read the stored data size from an RC allocation's header.
///
/// The data size is stored at `data_ptr - 16` and represents the number of
/// user data bytes (not including the 16-byte header itself). This is the
/// same value that was passed to `ori_rc_alloc`.
///
/// Used by seamless slice deallocation: when a slice is the last reference,
/// it computes the original data pointer and reads the allocation size from
/// the header to pass to `ori_rc_free`.
///
/// Returns 0 for null pointers.
#[no_mangle]
pub extern "C" fn ori_rc_data_size(data_ptr: *const u8) -> i64 {
    if data_ptr.is_null() {
        return 0;
    }

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 16 is valid
    // and 8-byte aligned. This is the data_size field of the RC header.
    unsafe { *data_ptr.sub(RC_HEADER_SIZE).cast::<i64>() }
}
