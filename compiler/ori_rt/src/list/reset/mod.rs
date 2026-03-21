//! Collection buffer reset for reuse.
//!
//! When an `RcDec(old_list)` is immediately followed by `Construct(ListLiteral)`
//! of the same element type, the old buffer can be reused instead of freed and
//! reallocated. This function handles the runtime side of that optimization.

use crate::rc::{ori_buffer_rc_dec, ori_rc_alloc, ori_rc_realloc};
use crate::slice_encoding::is_slice_cap;

/// Reset a list/set buffer for reuse with new elements.
///
/// Checks whether the old buffer is uniquely owned. If so, cleans up old
/// elements and reuses (or reallocs) the buffer. If shared (RC > 1), decs
/// the old buffer's RC and allocates fresh.
///
/// # Arguments
///
/// * `old_data` — data pointer of the old collection
/// * `old_len` — number of live elements in the old buffer
/// * `old_cap` — capacity of the old buffer (negative = seamless slice)
/// * `new_len` — number of elements to store in the new collection
/// * `elem_size` — byte size of each element
/// * `elem_dec_fn` — optional function to dec RC children of each old element
/// * `out_cap` — output pointer; receives the capacity of the returned buffer
///
/// # Returns
///
/// Pointer to the (reused or freshly allocated) data buffer.
///
/// # Safety
///
/// `out_cap` must be a valid, aligned pointer to an `i64`.
#[no_mangle]
pub extern "C" fn ori_list_reset_buffer(
    old_data: *mut u8,
    old_len: i64,
    old_cap: i64,
    new_len: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_cap: *mut i64,
) -> *mut u8 {
    let es = elem_size.max(1) as usize;
    let nl = new_len.max(0) as usize;

    // Null old data → fresh allocation (no buffer to reuse).
    if old_data.is_null() {
        return alloc_fresh(nl, es, out_cap);
    }

    // Seamless slices cannot be reused — their cap encodes a byte offset to
    // the original buffer, not an allocation size. Fall back to dec + alloc.
    if is_slice_cap(old_cap) {
        ori_buffer_rc_dec(old_data, old_len, old_cap, elem_size, elem_dec_fn);
        return alloc_fresh(nl, es, out_cap);
    }

    // Check uniqueness: RC == 1 means we can reuse in-place.
    if is_unique(old_data) {
        reuse_buffer(old_data, old_len, old_cap, nl, es, elem_dec_fn, out_cap)
    } else {
        // Shared: dec RC (won't free since RC > 1), alloc fresh.
        ori_buffer_rc_dec(old_data, old_len, old_cap, elem_size, elem_dec_fn);
        alloc_fresh(nl, es, out_cap)
    }
}

/// Check if a buffer is uniquely owned (RC == 1).
///
/// Delegates to `ori_rc_is_unique` which uses `AtomicI64` on the
/// non-single-threaded path (V5: `strong_count` at `data_ptr - 8`).
fn is_unique(data: *const u8) -> bool {
    crate::rc::ori_rc_is_unique(data)
}

/// Reuse a uniquely-owned buffer for new elements.
///
/// Cleans up old elements, then either reuses the buffer as-is (if capacity
/// suffices) or reallocs it.
fn reuse_buffer(
    data: *mut u8,
    old_len: i64,
    old_cap: i64,
    new_len: usize,
    elem_size: usize,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_cap: *mut i64,
) -> *mut u8 {
    let ol = old_len.max(0) as usize;
    let oc = old_cap.max(0) as usize;

    // Clean up old elements.
    if let Some(f) = elem_dec_fn {
        for i in 0..ol {
            f(unsafe { data.add(i * elem_size) });
        }
    }

    if oc >= new_len {
        // Existing capacity suffices — reuse buffer as-is.
        unsafe { *out_cap = old_cap };
        data
    } else {
        // Need more space — realloc.
        let old_bytes = oc * elem_size;
        let new_bytes = new_len * elem_size;
        let new_data = ori_rc_realloc(data, old_bytes, new_bytes, 8);
        unsafe { *out_cap = new_len as i64 };
        new_data
    }
}

/// Allocate a fresh buffer for `count` elements of `elem_size` bytes each.
fn alloc_fresh(count: usize, elem_size: usize, out_cap: *mut i64) -> *mut u8 {
    if count == 0 {
        unsafe { *out_cap = 0 };
        return std::ptr::null_mut();
    }
    let total = count * elem_size;
    let data = ori_rc_alloc(total, 8);
    unsafe { *out_cap = count as i64 };
    data
}

#[cfg(test)]
mod tests;
