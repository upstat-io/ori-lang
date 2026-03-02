//! Collection-specific RC buffer decrement operations.
//!
//! These functions handle the RC lifecycle for collection data buffers
//! (lists, sets, maps) where per-element cleanup is needed when the
//! buffer's refcount reaches zero.

use super::{
    call_drop_fn, ori_rc_data_size, ori_rc_free, rc_trace_enabled, rc_underflow_abort,
    rt_debug_validate_rc,
};
use crate::slice_encoding::{is_slice_cap, slice_original_data};
use debug::rc_trace_dec;

#[cfg(debug_assertions)]
use super::debug::rt_debug_check_not_freed;

#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic;
use std::sync::atomic::{AtomicI64, Ordering};

use super::debug;

/// Decrement the refcount on a collection data buffer.
///
/// Unlike `ori_rc_dec` (which takes a type-level drop function that receives
/// the buffer pointer), this function knows the buffer's element layout:
///
/// - `len`: number of live elements in the buffer
/// - `cap`: allocated capacity (used for `ori_list_free_data`), OR a slice
///   cap (negative, with `SLICE_FLAG` set) for seamless slices
/// - `elem_size`: byte size of each element
/// - `elem_dec_fn`: optional function called on each element when the buffer
///   is being freed. Receives a pointer to the element *within* the buffer.
///   Used to decrement RC children of each element (e.g., string data ptrs
///   inside a `[str]` buffer). Pass null for elements with no RC children.
///
/// **Slice handling**: When `cap` has `SLICE_FLAG` set (i.e., `cap < 0`),
/// this is a seamless slice. The original buffer's RC is decremented instead.
/// If the original's RC reaches zero, the slice's `len` elements get
/// `elem_dec_fn` called (best-effort cleanup for elements in the slice's
/// visible range) and the buffer is freed using the stored `data_size`.
///
/// When the refcount reaches zero (non-slice):
/// 1. Calls `elem_dec_fn` on each of the `len` elements (if non-null)
/// 2. Frees the buffer via `ori_rc_free`
#[no_mangle]
pub extern "C" fn ori_buffer_rc_dec(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }

    // Seamless slice: dec the original buffer's RC, not the slice's data pointer.
    if is_slice_cap(cap) {
        let original_data = slice_original_data(data, cap);
        // Delegate to ori_rc_dec on the original buffer.
        // We build a drop function that cleans up elements + frees.
        // However, we can't pass elem info through ori_rc_dec's function pointer,
        // so we handle the full lifecycle inline here.
        slice_buffer_rc_dec(original_data, data, len, elem_size, elem_dec_fn);
        return;
    }

    rt_debug_validate_rc(data.cast_const(), "ori_buffer_rc_dec");
    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data.cast_const(), "ori_buffer_rc_dec");

    let es = elem_size.max(1) as usize;
    let n = len.max(0) as usize;

    #[cfg(not(feature = "single-threaded"))]
    {
        let prev = unsafe {
            let rc_ptr = data.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_sub(1, Ordering::Release)
        };

        if prev <= 0 {
            rc_underflow_abort(data);
        }

        if rc_trace_enabled() {
            rc_trace_dec(data.cast_const(), prev - 1);
        }

        if prev <= 1 {
            atomic::fence(Ordering::Acquire);

            if let Some(f) = elem_dec_fn {
                for i in 0..n {
                    call_drop_fn(f, unsafe { data.add(i * es) });
                }
            }

            // Free the list data buffer (cap * elem_size bytes, RC-managed)
            let total = cap.max(0) as usize * es;
            ori_rc_free(data, total, 8);
        }
    }

    #[cfg(feature = "single-threaded")]
    {
        let (should_drop, new_rc) = unsafe {
            let rc_ptr = data.sub(8).cast::<i64>();
            if *rc_ptr <= 0 {
                rc_underflow_abort(data);
            }
            *rc_ptr -= 1;
            (*rc_ptr <= 0, *rc_ptr)
        };

        if rc_trace_enabled() {
            rc_trace_dec(data.cast_const(), new_rc);
        }

        if should_drop {
            if let Some(f) = elem_dec_fn {
                for i in 0..n {
                    call_drop_fn(f, unsafe { data.add(i * es) });
                }
            }

            // Free the list data buffer (cap * elem_size bytes, RC-managed)
            let total = cap.max(0) as usize * es;
            ori_rc_free(data, total, 8);
        }
    }
}

/// Decrement the original buffer's RC on behalf of a slice.
///
/// When a seamless slice is dropped, it decs the *original* buffer's RC.
/// If the original's RC reaches zero:
/// 1. The slice's visible elements get `elem_dec_fn` called (best-effort
///    cleanup — elements outside the slice's range are NOT cleaned up)
/// 2. The buffer is freed using the stored `data_size` from the RC header
///
/// This is a known limitation: if a slice is the last reference and its range
/// doesn't cover all elements of the original buffer, elements outside the
/// slice's range will have their child RCs leaked. This is acceptable because:
/// - Most common case is scalar elements (int, float) with no child RCs
/// - In practice, original lists usually outlive their slices
/// - Full cleanup would require storing the original `len` in the header
fn slice_buffer_rc_dec(
    original_data: *mut u8,
    slice_data: *mut u8,
    slice_len: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    rt_debug_validate_rc(original_data.cast_const(), "slice_buffer_rc_dec");
    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(original_data.cast_const(), "slice_buffer_rc_dec");

    let es = elem_size.max(1) as usize;
    let n = slice_len.max(0) as usize;

    #[cfg(not(feature = "single-threaded"))]
    {
        let prev = unsafe {
            let rc_ptr = original_data.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_sub(1, Ordering::Release)
        };

        if prev <= 0 {
            rc_underflow_abort(original_data);
        }

        if rc_trace_enabled() {
            rc_trace_dec(original_data.cast_const(), prev - 1);
        }

        if prev <= 1 {
            atomic::fence(Ordering::Acquire);

            // Best-effort element cleanup: dec elements in the slice's range
            if let Some(f) = elem_dec_fn {
                for i in 0..n {
                    call_drop_fn(f, unsafe { slice_data.add(i * es) });
                }
            }

            // Free the buffer using the stored data_size from the RC header
            let data_size = ori_rc_data_size(original_data.cast_const()) as usize;
            ori_rc_free(original_data, data_size, 8);
        }
    }

    #[cfg(feature = "single-threaded")]
    {
        let (should_drop, new_rc) = unsafe {
            let rc_ptr = original_data.sub(8).cast::<i64>();
            if *rc_ptr <= 0 {
                rc_underflow_abort(original_data);
            }
            *rc_ptr -= 1;
            (*rc_ptr <= 0, *rc_ptr)
        };

        if rc_trace_enabled() {
            rc_trace_dec(original_data.cast_const(), new_rc);
        }

        if should_drop {
            // Best-effort element cleanup: dec elements in the slice's range
            if let Some(f) = elem_dec_fn {
                for i in 0..n {
                    call_drop_fn(f, unsafe { slice_data.add(i * es) });
                }
            }

            // Free the buffer using the stored data_size from the RC header
            let data_size = ori_rc_data_size(original_data.cast_const()) as usize;
            ori_rc_free(original_data, data_size, 8);
        }
    }
}

/// Decrement the refcount of a map's hash table data buffer.
///
/// Map data layout: `[metadata | keys | values]` (hash table with open
/// addressing). When RC reaches 0, scans metadata for OCCUPIED buckets,
/// calls `key_dec_fn`/`val_dec_fn` on each, then frees the buffer.
#[no_mangle]
pub extern "C" fn ori_map_buffer_rc_dec(
    data: *mut u8,
    cap: i64,
    len: i64,
    key_size: i64,
    val_size: i64,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }

    rt_debug_validate_rc(data.cast_const(), "ori_map_buffer_rc_dec");
    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data.cast_const(), "ori_map_buffer_rc_dec");

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    #[cfg(not(feature = "single-threaded"))]
    {
        let prev = unsafe {
            let rc_ptr = data.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_sub(1, Ordering::Release)
        };

        if prev <= 0 {
            rc_underflow_abort(data);
        }

        if rc_trace_enabled() {
            rc_trace_dec(data.cast_const(), prev - 1);
        }

        if prev <= 1 {
            atomic::fence(Ordering::Acquire);
            map_buffer_cleanup(data, c, n, ks, vs, key_dec_fn, val_dec_fn);
        }
    }

    #[cfg(feature = "single-threaded")]
    {
        let (should_drop, new_rc) = unsafe {
            let rc_ptr = data.sub(8).cast::<i64>();
            if *rc_ptr <= 0 {
                rc_underflow_abort(data);
            }
            *rc_ptr -= 1;
            (*rc_ptr <= 0, *rc_ptr)
        };

        if rc_trace_enabled() {
            rc_trace_dec(data.cast_const(), new_rc);
        }

        if should_drop {
            map_buffer_cleanup(data, c, n, ks, vs, key_dec_fn, val_dec_fn);
        }
    }
}

/// Clean up and free a map data buffer. Called when RC reaches 0.
///
/// Scans metadata for OCCUPIED buckets and calls `key_dec_fn`/`val_dec_fn`
/// on each occupied key/value. Frees the buffer using hash table layout size.
fn map_buffer_cleanup(
    data: *mut u8,
    cap: usize,
    _len: usize,
    key_size: usize,
    val_size: usize,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    use crate::map::hash_table::{get_meta, HashTableLayout, META_OCCUPIED};

    let layout = HashTableLayout::for_map(cap, key_size, val_size);

    // Dec children: scan metadata for OCCUPIED buckets
    if key_dec_fn.is_some() || val_dec_fn.is_some() {
        for bucket in 0..cap {
            if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
                if let Some(f) = key_dec_fn {
                    call_drop_fn(f, unsafe {
                        data.add(layout.keys_offset + bucket * key_size)
                    });
                }
                if let Some(f) = val_dec_fn {
                    call_drop_fn(f, unsafe {
                        data.add(layout.vals_offset + bucket * val_size)
                    });
                }
            }
        }
    }

    ori_rc_free(data, layout.total_size, 8);
}

/// Drop a collection buffer that is known to be uniquely owned (RC == 1).
///
/// Skips the atomic RC decrement entirely. Directly calls `elem_dec_fn` on
/// each element (if non-null), then frees the buffer via `ori_rc_free`.
///
/// # Safety
///
/// The caller guarantees RC == 1 (from static uniqueness analysis). Calling
/// this on a shared buffer (RC > 1) is undefined behavior: other references
/// will become dangling.
///
/// Seamless slices cannot use this function — their cap encodes a byte offset
/// to the original buffer, not an allocation size. The caller must fall back
/// to `ori_buffer_rc_dec` for slices.
#[no_mangle]
pub extern "C" fn ori_buffer_drop_unique(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }

    // Defense-in-depth: slices must not reach this path.
    debug_assert!(
        !is_slice_cap(cap),
        "ori_buffer_drop_unique called on a seamless slice (cap={cap})"
    );

    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data.cast_const(), "ori_buffer_drop_unique");

    if rc_trace_enabled() {
        rc_trace_dec(data.cast_const(), 0);
    }

    let es = elem_size.max(1) as usize;
    let n = len.max(0) as usize;

    // Clean up element children (e.g., dec RC on strings inside [str]).
    if let Some(f) = elem_dec_fn {
        for i in 0..n {
            call_drop_fn(f, unsafe { data.add(i * es) });
        }
    }

    // Free the buffer — no atomic dec needed, we know RC == 1.
    let total = cap.max(0) as usize * es;
    ori_rc_free(data, total, 8);
}

/// Drop a map buffer that is known to be uniquely owned (RC == 1).
///
/// Skips the atomic RC decrement. Directly cleans up keys and values,
/// then frees the combined buffer.
///
/// # Safety
///
/// Same as [`ori_buffer_drop_unique`]: caller guarantees RC == 1.
#[no_mangle]
pub extern "C" fn ori_map_buffer_drop_unique(
    data: *mut u8,
    cap: i64,
    len: i64,
    key_size: i64,
    val_size: i64,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }

    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data.cast_const(), "ori_map_buffer_drop_unique");

    if rc_trace_enabled() {
        rc_trace_dec(data.cast_const(), 0);
    }

    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Clean up key and value children.
    map_buffer_cleanup(data, c, n, ks, vs, key_dec_fn, val_dec_fn);
}

/// Slice-aware RC increment for collection buffers.
///
/// If `cap` indicates a seamless slice (`is_slice_cap(cap) == true`),
/// increments the RC on the *original* buffer (computed from the byte
/// offset encoded in `cap`). Otherwise, increments RC on `data` directly.
///
/// This is the correct RC increment for any list/set value, whether it's
/// an owned buffer or a seamless slice view into another buffer.
#[no_mangle]
pub extern "C" fn ori_list_rc_inc(data: *mut u8, cap: i64) {
    if data.is_null() {
        return;
    }

    let rc_target = if is_slice_cap(cap) {
        slice_original_data(data, cap)
    } else {
        data
    };

    crate::rc::ori_rc_inc(rc_target);
}

/// Copy `count * elem_size` bytes from `src` to `dst` (non-overlapping).
///
/// Thin wrapper over `ptr::copy_nonoverlapping` for use from LLVM-generated
/// code. Does NOT perform reference count operations on elements — the caller
/// is responsible for incrementing RC on copied RC'd elements.
#[no_mangle]
pub extern "C" fn ori_memcpy_elements(
    dst: *mut u8,
    src: *const u8,
    count: usize,
    elem_size: usize,
) {
    if count == 0 || elem_size == 0 || dst.is_null() || src.is_null() {
        return;
    }

    // SAFETY: caller guarantees dst and src are valid for count * elem_size
    // bytes and do not overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(src, dst, count * elem_size);
    }
}

/// Decrement the refcount of a set's hash table data buffer.
///
/// Set data layout: `[metadata | elements]` (hash table with open addressing).
/// When RC reaches 0, scans metadata for OCCUPIED buckets, calls `elem_dec_fn`
/// on each, then frees the buffer.
#[no_mangle]
pub extern "C" fn ori_set_buffer_rc_dec(
    data: *mut u8,
    cap: i64,
    _len: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }

    rt_debug_validate_rc(data.cast_const(), "ori_set_buffer_rc_dec");
    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data.cast_const(), "ori_set_buffer_rc_dec");

    let es = elem_size.max(1) as usize;
    let c = cap.max(0) as usize;

    #[cfg(not(feature = "single-threaded"))]
    {
        let prev = unsafe {
            let rc_ptr = data.sub(8).cast::<AtomicI64>();
            (*rc_ptr).fetch_sub(1, Ordering::Release)
        };

        if prev <= 0 {
            rc_underflow_abort(data);
        }

        if rc_trace_enabled() {
            rc_trace_dec(data.cast_const(), prev - 1);
        }

        if prev <= 1 {
            atomic::fence(Ordering::Acquire);
            set_buffer_cleanup(data, c, es, elem_dec_fn);
        }
    }

    #[cfg(feature = "single-threaded")]
    {
        let (should_drop, new_rc) = unsafe {
            let rc_ptr = data.sub(8).cast::<i64>();
            if *rc_ptr <= 0 {
                rc_underflow_abort(data);
            }
            *rc_ptr -= 1;
            (*rc_ptr <= 0, *rc_ptr)
        };

        if rc_trace_enabled() {
            rc_trace_dec(data.cast_const(), new_rc);
        }

        if should_drop {
            set_buffer_cleanup(data, c, es, elem_dec_fn);
        }
    }
}

/// Drop a set buffer that is known to be uniquely owned (RC == 1).
///
/// Scans metadata for OCCUPIED buckets, calls `elem_dec_fn` on each,
/// then frees the buffer. Skips atomic RC decrement.
#[no_mangle]
pub extern "C" fn ori_set_buffer_drop_unique(
    data: *mut u8,
    cap: i64,
    _len: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }

    #[cfg(debug_assertions)]
    rt_debug_check_not_freed(data.cast_const(), "ori_set_buffer_drop_unique");

    if rc_trace_enabled() {
        rc_trace_dec(data.cast_const(), 0);
    }

    let es = elem_size.max(1) as usize;
    let c = cap.max(0) as usize;
    set_buffer_cleanup(data, c, es, elem_dec_fn);
}

/// Clean up and free a set data buffer. Called when RC reaches 0.
fn set_buffer_cleanup(
    data: *mut u8,
    cap: usize,
    elem_size: usize,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    use crate::map::hash_table::{get_meta, HashTableLayout, META_OCCUPIED};

    let layout = HashTableLayout::for_set(cap, elem_size);

    if let Some(f) = elem_dec_fn {
        for bucket in 0..cap {
            if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
                call_drop_fn(f, unsafe {
                    data.add(layout.keys_offset + bucket * elem_size)
                });
            }
        }
    }

    ori_rc_free(data, layout.total_size, 8);
}

/// Move `count * elem_size` bytes from `src` to `dst` (may overlap).
///
/// Thin wrapper over `ptr::copy` for use from LLVM-generated code when
/// shifting elements during insert/remove operations. Does NOT perform
/// reference count operations on elements.
#[no_mangle]
pub extern "C" fn ori_memmove_elements(
    dst: *mut u8,
    src: *const u8,
    count: usize,
    elem_size: usize,
) {
    if count == 0 || elem_size == 0 || dst.is_null() || src.is_null() {
        return;
    }

    // SAFETY: caller guarantees dst and src are valid for count * elem_size
    // bytes. Regions may overlap.
    unsafe {
        std::ptr::copy(src, dst, count * elem_size);
    }
}
