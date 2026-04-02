//! RC allocation lifecycle: `alloc`, `realloc`, `free`, `data_size`.
//!
//! These functions manage the heap memory behind reference-counted objects.
//! All operate on the V5 32-byte header layout (see `mod.rs` for diagram).

#[cfg(debug_assertions)]
use super::check_leaks_enabled;
#[cfg(debug_assertions)]
use super::debug::{alloc_registry_insert, alloc_registry_remove, rt_debug_register_freed};
use super::debug::{rc_trace_alloc, rc_trace_free, rc_trace_realloc};
use super::{rc_trace_enabled, RC_HEADER_SIZE, RC_LIVE_COUNT};

#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

/// Allocate a new reference-counted object.
///
/// Allocates `size + 32` bytes with the given alignment, initializes
/// `data_size` to `size`, `elem_dec_fn` to NULL, `elem_count` to 0,
/// and `strong_count` to 1, then returns a pointer to the data area.
///
/// Layout: `[data_size | elem_dec_fn | elem_count | strong_count | data ...]`
///          ^            ^              ^             ^              ^
///          base(ptr-32) ptr-24         ptr-16        ptr-8          returned `data_ptr`
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

    // Initialize header fields: data_size, elem_dec_fn, elem_count, strong_count.
    // No ordering needed — this allocation is not yet visible to other threads.
    // SAFETY: base is valid and 8-byte aligned, total_size >= 32
    unsafe {
        // data_size at base + 0
        base.cast::<i64>().write(size as i64);

        // elem_dec_fn at base + 8 (NULL — no element destructor yet)
        base.add(8).cast::<*const ()>().write(std::ptr::null());

        // elem_count at base + 16 (0 — no initialized elements yet)
        base.add(16).cast::<i64>().write(0);

        // strong_count at base + 24
        let rc_ptr = base.add(24);
        #[cfg(not(feature = "single-threaded"))]
        rc_ptr.cast::<AtomicI64>().write(AtomicI64::new(1));
        #[cfg(feature = "single-threaded")]
        rc_ptr.cast::<i64>().write(1);
    }

    RC_LIVE_COUNT.fetch_add(1, Ordering::Relaxed);

    // Return data pointer (32 bytes past the base)
    // SAFETY: base is valid for total_size bytes, so base + 32 is valid
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

/// Free a reference-counted allocation unconditionally.
///
/// Deallocates from `data_ptr - 32` with total size `size + 32`.
/// Typically called as the last step of a type-specialized drop function.
///
/// `size` and `align` are the data size and alignment (same values passed
/// to `ori_rc_alloc`). The 32-byte header is accounted for internally.
#[no_mangle]
pub extern "C" fn ori_rc_free(data_ptr: *mut u8, size: usize, align: usize) {
    if data_ptr.is_null() {
        return;
    }

    #[cfg(debug_assertions)]
    rt_debug_register_freed(data_ptr.cast_const());

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 32 is the base
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

/// Reallocate a reference-counted buffer to a new data size.
///
/// Adjusts the underlying allocation (which includes the 32-byte RC header)
/// while preserving the refcount, `elem_dec_fn`, and `elem_count`. Updates the
/// stored `data_size` to `new_data_size`. Returns the new data pointer (32 bytes
/// past the base).
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

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 32 is the
    // base pointer with layout (old_data_size + 32, align). realloc preserves
    // the first min(old_total, new_total) bytes, including the full RC header
    // (data_size + elem_dec_fn + elem_count + strong_count). Since old_total
    // >= 32 always, all header fields are preserved. Only data_size at offset 0
    // is overwritten below.
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

    // Return data pointer (32 bytes past the header)
    // SAFETY: new_base is valid for new_total bytes, so new_base + 32 is valid.
    let new_data = unsafe { new_base.add(RC_HEADER_SIZE) };

    // Update leak tracker metadata.
    if new_data == data_ptr {
        // Same address (in-place realloc) — update size/align metadata,
        // preserving the original alloc_id. Without this, the registry
        // retains stale size/alignment from the original allocation.
        #[cfg(debug_assertions)]
        if super::check_leaks_enabled() {
            super::debug::alloc_registry_update(new_data, new_data_size, align);
        }
    } else {
        // Realloc moved the block — remove old entry, insert new one.
        #[cfg(debug_assertions)]
        if super::check_leaks_enabled() {
            super::debug::alloc_registry_remove(data_ptr);
            super::debug::alloc_registry_insert(new_data, new_data_size, align);
        }
        if rc_trace_enabled() {
            rc_trace_realloc(data_ptr.cast_const(), new_data.cast_const(), new_data_size);
        }
    }

    new_data
}

/// Read the stored data size from an RC allocation's header.
///
/// The data size is stored at `data_ptr - 32` and represents the number of
/// user data bytes (not including the 32-byte header itself). This is the
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

    // SAFETY: data_ptr was returned by ori_rc_alloc, so data_ptr - 32 is valid
    // and 8-byte aligned. This is the data_size field of the RC header.
    unsafe { *data_ptr.sub(RC_HEADER_SIZE).cast::<i64>() }
}
