//! Seamless list slice operations.
//!
//! Creates zero-copy views into existing list buffers. A slice shares the
//! original buffer's data — no elements are copied. The original buffer's
//! RC is incremented (the slice is an additional reference).
//!
//! Slice-of-slice is supported: offsets accumulate to always reference
//! the true original allocation.

use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_dec, ori_rc_inc};
use crate::slice_encoding::{is_slice_cap, make_slice_cap, slice_byte_offset, slice_original_data};

use super::{inc_copied_elements, write_list_output};

/// Create a seamless slice of a list from index `start` to `end` (exclusive).
///
/// The slice shares the original list's data buffer. No elements are copied.
/// The original buffer's RC is incremented (the slice references it).
///
/// Returns `{len: end-start, cap: SLICE_FLAG|byte_offset, data: original+offset}`
/// via the `out_ptr` sret pattern.
///
/// If `start == end`, returns an empty list (null data).
/// If `data` is null (empty list), returns an empty list.
///
/// # Preconditions
/// - `0 <= start <= end <= len`
/// - `elem_size > 0`
#[no_mangle]
pub extern "C" fn ori_list_slice(
    data: *mut u8,
    len: i64,
    cap: i64,
    start: i64,
    end: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;

    // Empty input or empty range → empty list
    if data.is_null() || start >= end || start >= len {
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        return;
    }

    // Clamp end to len (defensive)
    let end = end.min(len);
    let start = start.max(0);
    let slice_len = end - start;

    if slice_len <= 0 {
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        return;
    }

    // Compute the byte offset from the true original allocation's data start.
    // For regular lists: the offset is just `start * elem_size`.
    // For slices: accumulate with the existing offset.
    let new_elem_offset = (start as usize) * es;
    let total_byte_offset = if is_slice_cap(cap) {
        slice_byte_offset(cap) + new_elem_offset
    } else {
        new_elem_offset
    };

    // Find the true original allocation's data pointer (for RC inc).
    let original_data = if is_slice_cap(cap) {
        slice_original_data(data, cap)
    } else {
        data
    };

    // The slice's data pointer is into the original buffer.
    let slice_data = unsafe { original_data.add(total_byte_offset) };

    // Increment RC on the original buffer (new reference for this slice).
    ori_rc_inc(original_data);

    unsafe {
        write_list_output(
            out_ptr,
            slice_len,
            make_slice_cap(total_byte_offset),
            slice_data,
        );
    }
}

/// Take the first `n` elements as a seamless slice.
///
/// Equivalent to `ori_list_slice(data, len, cap, 0, min(n, len), ...)`.
/// Returns a zero-copy view of the first `n` elements.
#[no_mangle]
pub extern "C" fn ori_list_slice_take(
    data: *mut u8,
    len: i64,
    cap: i64,
    n: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    ori_list_slice(data, len, cap, 0, n.min(len), elem_size, out_ptr);
}

/// Skip the first `n` elements, returning the rest as a seamless slice.
///
/// Equivalent to `ori_list_slice(data, len, cap, min(n, len), len, ...)`.
/// Returns a zero-copy view of elements after index `n`.
#[no_mangle]
pub extern "C" fn ori_list_slice_drop(
    data: *mut u8,
    len: i64,
    cap: i64,
    n: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    ori_list_slice(data, len, cap, n.min(len), len, elem_size, out_ptr);
}

/// Materialize a seamless slice into a standalone owned list.
///
/// Allocates a new RC-managed buffer and copies the slice's elements into it.
/// The result is a regular list (`cap >= 0`) that owns its data independently.
///
/// - Element RCs are incremented via `inc_fn` (the new buffer is a new
///   reference to each element's sub-objects).
/// - The original buffer's RC is decremented (the slice no longer references it).
///
/// If `cap` is not a slice cap, the input is returned unchanged (no-op).
/// If `data` is null or `len` is zero, returns an empty list and decs the
/// original buffer.
///
/// # Arguments
///
/// * `data` - Slice data pointer (interior to original buffer)
/// * `len` - Number of elements in the slice
/// * `cap` - Slice cap (`SLICE_FLAG | byte_offset`)
/// * `elem_size` - Byte size of each element
/// * `elem_align` - Alignment of each element
/// * `inc_fn` - Optional function to increment RC of each copied element
/// * `out_ptr` - Output sret pointer for `{len, cap, data}`
#[no_mangle]
pub extern "C" fn ori_list_materialize_slice(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    // Not a slice — return as-is (no materialization needed)
    if !is_slice_cap(cap) {
        unsafe { write_list_output(out_ptr, len, cap, data) };
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;

    let original = slice_original_data(data, cap);

    if n == 0 || data.is_null() {
        // Empty slice → empty list, dec original
        ori_rc_dec(original, None);
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        return;
    }

    let new_cap = next_capacity(0, n);
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // Copy elements from slice view
    unsafe {
        std::ptr::copy_nonoverlapping(data, new_data, n * es);
    }

    // Inc RC for all copied elements (new buffer is a new reference)
    inc_copied_elements(new_data, n, es, inc_fn);

    // Dec original buffer RC (slice no longer references it)
    ori_rc_dec(original, None);

    unsafe { write_list_output(out_ptr, n as i64, new_cap as i64, new_data) };
}

#[cfg(test)]
mod tests;
