//! Map buffer RC operations: decrement and unique-drop.

use super::debug::rc_trace_dec;
use super::{call_drop_fn, ori_rc_free, rc_trace_enabled, rt_debug_validate_rc};

#[cfg(debug_assertions)]
use super::debug::rt_debug_check_not_freed;

/// Decrement the refcount of a map's hash table data buffer.
///
/// Map data layout: `[metadata | keys | values]` (hash table with open
/// addressing). When RC reaches 0, scans metadata for OCCUPIED buckets,
/// calls `key_dec_fn`/`val_dec_fn` on each, then frees the buffer.
#[no_mangle]
pub extern "C" fn ori_map_buffer_rc_dec(
    data: *mut u8,
    cap: i64,
    _len: i64,
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
    let c = cap.max(0) as usize;

    // SAFETY: data is non-null (checked above) and was returned by ori_rc_alloc.
    if unsafe { super::rc_dec_to_zero(data) } {
        map_buffer_cleanup(data, c, ks, vs, key_dec_fn, val_dec_fn);
    }
}

/// Clean up and free a map data buffer. Called when RC reaches 0.
///
/// Scans metadata for OCCUPIED buckets and calls `key_dec_fn`/`val_dec_fn`
/// on each occupied key/value. Frees the buffer using hash table layout size.
///
/// Maps use parameter-based dec functions (not header-based) because they
/// require TWO cleanup functions (key + value) but the RC header has only
/// one `elem_dec_fn` slot.
fn map_buffer_cleanup(
    data: *mut u8,
    cap: usize,
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
            // SAFETY: data + bucket is within the metadata region (allocated by ori_rc_alloc).
            if unsafe { get_meta(data, bucket) } == META_OCCUPIED {
                if let Some(f) = key_dec_fn {
                    // SAFETY: keys_offset + bucket * key_size is within the key region.
                    call_drop_fn(f, unsafe {
                        data.add(layout.keys_offset + bucket * key_size)
                    });
                }
                if let Some(f) = val_dec_fn {
                    // SAFETY: vals_offset + bucket * val_size is within the value region.
                    call_drop_fn(f, unsafe {
                        data.add(layout.vals_offset + bucket * val_size)
                    });
                }
            }
        }
    }

    ori_rc_free(data, layout.total_size, 8);
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
    _len: i64,
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
    let c = cap.max(0) as usize;

    // Clean up key and value children.
    map_buffer_cleanup(data, c, ks, vs, key_dec_fn, val_dec_fn);
}
