//! COW list sort operations (unstable and stable variants).
//!
//! Extracted from `cow_sort/mod.rs` to keep files under the 500-line limit.
//! Uses index-permutation for unique buffers, copy-in-sorted-order for shared.

use crate::rc::{ori_rc_alloc, ori_rc_is_unique};
use crate::slice_encoding::is_slice_cap;

use super::propagate_header;
use crate::list::{dec_list_buffer, inc_copied_elements};

/// Maximum element size for stack-allocated swap buffers (covers int, float, str).
const STACK_MAX: usize = 24;

/// COW-aware list sort with consuming semantics.
///
/// Sorts the list using the provided comparison function. If uniquely
/// owned (RC==1), sorts in place via index permutation (O(n log n),
/// no allocation beyond the index array). If shared, allocates a new
/// buffer and writes elements in sorted order (O(n) copy + O(n log n)
/// sort).
///
/// The comparison function has C ABI: `fn(a: *const u8, b: *const u8) -> i32`
/// returning negative (a < b), zero (a == b), positive (a > b).
///
/// Uses unstable sort (not guaranteed to preserve order of equal elements).
///
/// # Element RC
///
/// No element RC changes needed — elements are just rearranged (unique)
/// or byte-copied in sorted order (shared, codegen handles RC inc).
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_list_sort_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    compare_fn: extern "C" fn(*const u8, *const u8) -> i32,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    list_sort_cow_impl(
        data, len, cap, elem_size, elem_align, compare_fn, inc_fn, cow_mode, out_ptr, false,
    );
}

/// Stable sort (`TimSort`) — preserves relative order of equal elements.
/// Same COW semantics as `ori_list_sort_cow`.
#[unsafe(no_mangle)]
pub extern "C" fn ori_list_sort_stable_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    compare_fn: extern "C" fn(*const u8, *const u8) -> i32,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    list_sort_cow_impl(
        data, len, cap, elem_size, elem_align, compare_fn, inc_fn, cow_mode, out_ptr, true,
    );
}

/// Shared implementation for COW list sort (unstable and stable variants).
#[expect(
    clippy::too_many_arguments,
    reason = "C FFI parameters from two sort entry points"
)]
fn list_sort_cow_impl(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    compare_fn: extern "C" fn(*const u8, *const u8) -> i32,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
    stable: bool,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;

    // Empty or single-element — already sorted
    if data.is_null() || n <= 1 {
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // Build sorted index array — works for both paths
    let mut indices: Vec<usize> = (0..n).collect();
    let cmp = |&a: &usize, &b: &usize| {
        let c = compare_fn(unsafe { data.add(a * es) }, unsafe { data.add(b * es) });
        c.cmp(&0)
    };
    if stable {
        indices.sort_by(cmp);
    } else {
        indices.sort_unstable_by(cmp);
    }

    // FAST PATH: unique owner, non-slice — permute in place
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique =
        !is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        apply_permutation_in_place(data, &indices, es);
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // SLOW PATH: shared — copy in sorted order to new buffer
    let new_data = ori_rc_alloc(n * es, ea);

    for (dst_idx, &src_idx) in indices.iter().enumerate() {
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_idx * es), new_data.add(dst_idx * es), es);
        }
    }

    // Inc RC for all copied elements
    inc_copied_elements(new_data, n, es, inc_fn);

    // Propagate header from source
    unsafe { propagate_header(data, cap, new_data, n as i64) };

    // Release old buffer (slice-aware)
    dec_list_buffer(data, cap);

    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64); // cap = len (tight)
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Apply a permutation in place using cycle-following.
///
/// Given `indices[i] = j`, moves the element originally at position `j`
/// to position `i`. Uses `O(elem_size)` temporary space (one element buffer).
fn apply_permutation_in_place(data: *mut u8, indices: &[usize], elem_size: usize) {
    let n = indices.len();
    let mut placed = vec![false; n];
    // Stack buffer for common element sizes (8, 16, 24 bytes),
    // heap fallback for larger elements.
    let mut stack_buf = [0u8; STACK_MAX];
    let mut heap_buf = Vec::new();
    let tmp: &mut [u8] = if elem_size <= STACK_MAX {
        &mut stack_buf[..elem_size]
    } else {
        heap_buf.resize(elem_size, 0);
        &mut heap_buf
    };

    for start in 0..n {
        if placed[start] || indices[start] == start {
            placed[start] = true;
            continue;
        }

        // Follow the cycle starting at `start`
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(start * elem_size), tmp.as_mut_ptr(), elem_size);
        }

        let mut current = start;
        loop {
            let next = indices[current];
            placed[current] = true;

            if next == start {
                // Close the cycle — write tmp to current position
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        tmp.as_ptr(),
                        data.add(current * elem_size),
                        elem_size,
                    );
                }
                break;
            }

            // Move element from `next` to `current`
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.add(next * elem_size),
                    data.add(current * elem_size),
                    elem_size,
                );
            }
            current = next;
        }
    }
}
