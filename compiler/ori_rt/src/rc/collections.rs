//! Collection-specific RC buffer decrement operations.
//!
//! These functions handle the RC lifecycle for collection data buffers
//! (lists, sets, maps) where per-element cleanup is needed when the
//! buffer's refcount reaches zero.

use super::{
    call_drop_fn, ori_rc_free, rc_trace_enabled, rc_underflow_abort, rt_debug_validate_rc,
};
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
/// - `cap`: allocated capacity (used for `ori_list_free_data`)
/// - `elem_size`: byte size of each element
/// - `elem_dec_fn`: optional function called on each element when the buffer
///   is being freed. Receives a pointer to the element *within* the buffer.
///   Used to decrement RC children of each element (e.g., string data ptrs
///   inside a `[str]` buffer). Pass null for elements with no RC children.
///
/// When the refcount reaches zero:
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

/// Decrement the refcount of a map's combined data buffer.
///
/// Map data layout: `[key0..keyN | val0..valN]` where values start at
/// `data + cap * key_size`. When RC reaches 0, calls `key_dec_fn` on each
/// key and `val_dec_fn` on each value, then frees the buffer.
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
fn map_buffer_cleanup(
    data: *mut u8,
    cap: usize,
    len: usize,
    key_size: usize,
    val_size: usize,
    key_dec_fn: Option<extern "C" fn(*mut u8)>,
    val_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    // Dec children: keys at offset 0, values at offset cap * key_size
    if let Some(f) = key_dec_fn {
        for i in 0..len {
            call_drop_fn(f, unsafe { data.add(i * key_size) });
        }
    }
    if let Some(f) = val_dec_fn {
        let vals_start = unsafe { data.add(cap * key_size) };
        for i in 0..len {
            call_drop_fn(f, unsafe { vals_start.add(i * val_size) });
        }
    }

    // Free the combined buffer
    let total = cap * key_size + cap * val_size;
    ori_rc_free(data, total, 8);
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
