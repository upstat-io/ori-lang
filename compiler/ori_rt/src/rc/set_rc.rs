//! Set buffer RC operations: decrement and unique-drop.

use super::debug::rc_trace_dec;
use super::{
    call_drop_fn, load_elem_dec_fn, ori_rc_free, rc_trace_enabled, rt_debug_validate_rc,
    store_elem_dec_fn_once,
};

#[cfg(debug_assertions)]
use super::debug::rt_debug_check_not_freed;

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
    // SAFETY: data is non-null (checked above) and was returned by ori_rc_alloc.
    unsafe { store_elem_dec_fn_once(data, elem_dec_fn) };

    let es = elem_size.max(1) as usize;
    let c = cap.max(0) as usize;

    // SAFETY: data is non-null (checked above) and was returned by ori_rc_alloc.
    // Note: before this refactor, both single-threaded and multi-threaded paths
    // were missing the immortal sentinel check — rc_dec_to_zero includes it.
    if unsafe { super::rc_dec_to_zero(data) } {
        set_buffer_cleanup(data, c, es);
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
    // SAFETY: data is non-null (checked above) and was returned by ori_rc_alloc.
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
    // SAFETY: data was returned by ori_rc_alloc, so data - 24 is valid.
    let elem_dec_fn = unsafe { load_elem_dec_fn(data) };
    if let Some(f) = elem_dec_fn {
        for bucket in 0..cap {
            // SAFETY: data + bucket is within the metadata region (allocated by ori_rc_alloc).
            if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
                // SAFETY: keys_offset + bucket * elem_size is within the element region.
                call_drop_fn(f, unsafe {
                    data.add(layout.keys_offset + bucket * elem_size)
                });
            }
        }
    }

    ori_rc_free(data, layout.total_size, 8);
}
