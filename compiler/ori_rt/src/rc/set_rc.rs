//! Set buffer RC operations: decrement and unique-drop.

use super::debug::rc_trace_dec;
use super::{
    call_drop_fn, load_elem_dec_fn, ori_rc_free, rc_trace_enabled, rc_underflow_abort,
    rt_debug_validate_rc, store_elem_dec_fn_once,
};

#[cfg(debug_assertions)]
use super::debug::rt_debug_check_not_freed;

#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic;
use std::sync::atomic::{AtomicI64, Ordering};

/// Decrement the refcount of a set's hash table data buffer.
///
/// Set data layout: `[metadata | elements]` (hash table with open addressing).
/// When RC reaches 0, reads `elem_dec_fn` from the RC header and scans
/// metadata for OCCUPIED buckets, calling the stored function on each,
/// then frees the buffer.
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

    // Store elem_dec_fn in header (write-once, defense-in-depth).
    unsafe { store_elem_dec_fn_once(data, elem_dec_fn) };

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
            set_buffer_cleanup(data, c, es);
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
            set_buffer_cleanup(data, c, es);
        }
    }
}

/// Drop a set buffer that is known to be uniquely owned (RC == 1).
///
/// Reads `elem_dec_fn` from the RC header, scans metadata for OCCUPIED
/// buckets, calls the stored function on each, then frees the buffer.
/// Skips atomic RC decrement.
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

    // Store elem_dec_fn in header (write-once, defense-in-depth).
    unsafe { store_elem_dec_fn_once(data, elem_dec_fn) };

    if rc_trace_enabled() {
        rc_trace_dec(data.cast_const(), 0);
    }

    let es = elem_size.max(1) as usize;
    let c = cap.max(0) as usize;
    set_buffer_cleanup(data, c, es);
}

/// Clean up and free a set data buffer. Called when RC reaches 0.
///
/// Reads `elem_dec_fn` from the RC header instead of taking it as a parameter.
fn set_buffer_cleanup(data: *mut u8, cap: usize, elem_size: usize) {
    use crate::map::hash_table::{get_meta, HashTableLayout, META_OCCUPIED};

    let layout = HashTableLayout::for_set(cap, elem_size);

    // Read elem_dec_fn from header
    let elem_dec_fn = unsafe { load_elem_dec_fn(data) };
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
