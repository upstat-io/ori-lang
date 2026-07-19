//! COW list sort operations (unstable and stable variants).
//!
//! Uses index-permutation for unique buffers, copy-in-sorted-order for shared.

use crate::list::cow_context::{CowMode, ElementOps, ListBuffer};
use crate::list::{dec_list_buffer, inc_copied_elements, write_list_output};
use crate::rc::ori_rc_alloc;

use super::header::propagate_header;

/// Maximum element size for stack-allocated swap buffers (covers int, float, str).
const STACK_MAX: usize = 24;

#[derive(Clone, Copy)]
enum SortStability {
    Stable,
    Unstable,
}

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
        ListBuffer::new(data, len, cap),
        ElementOps::new(
            elem_size.max(1) as usize,
            elem_align.max(1) as usize,
            inc_fn,
        ),
        compare_fn,
        cow_mode,
        out_ptr,
        SortStability::Unstable,
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
        ListBuffer::new(data, len, cap),
        ElementOps::new(
            elem_size.max(1) as usize,
            elem_align.max(1) as usize,
            inc_fn,
        ),
        compare_fn,
        cow_mode,
        out_ptr,
        SortStability::Stable,
    );
}

/// Shared implementation for COW list sort (unstable and stable variants).
fn list_sort_cow_impl(
    list: ListBuffer,
    elements: ElementOps,
    compare_fn: extern "C" fn(*const u8, *const u8) -> i32,
    cow_mode: i32,
    out_ptr: *mut u8,
    stability: SortStability,
) {
    if out_ptr.is_null() {
        return;
    }

    let n = list.len.max(0) as usize;

    if list.data.is_null() || n <= 1 {
        // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
        unsafe { write_list_output(out_ptr, list.len, list.cap, list.data) };
        return;
    }

    let mut indices: Vec<usize> = (0..n).collect();
    let cmp = |&a: &usize, &b: &usize| {
        // SAFETY: Both indices come from `0..n`, whose elements are initialized in `list.data`.
        let a_ptr = unsafe { list.data.add(a * elements.size) };
        // SAFETY: Both indices come from `0..n`, whose elements are initialized in `list.data`.
        let b_ptr = unsafe { list.data.add(b * elements.size) };
        let c = compare_fn(a_ptr, b_ptr);
        c.cmp(&0)
    };
    match stability {
        SortStability::Stable => indices.sort_by(cmp),
        SortStability::Unstable => indices.sort_unstable_by(cmp),
    }

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(list.data, list.cap);
    if is_unique {
        apply_permutation_in_place(list.data, &indices, elements.size);
        // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
        unsafe { write_list_output(out_ptr, list.len, list.cap, list.data) };
        return;
    }

    let new_data = ori_rc_alloc(n * elements.size, elements.align);

    for (dst_idx, &src_idx) in indices.iter().enumerate() {
        // SAFETY:
        // - Both indices are below `n`, and both allocations hold `n` elements.
        // - The source and newly allocated destination do not overlap.
        unsafe {
            std::ptr::copy_nonoverlapping(
                list.data.add(src_idx * elements.size),
                new_data.add(dst_idx * elements.size),
                elements.size,
            );
        }
    }

    inc_copied_elements(new_data, n, elements.size, elements.inc);

    // SAFETY: Both pointers address RC allocations with accessible list headers.
    unsafe { propagate_header(list.data, list.cap, new_data, n as i64) };

    dec_list_buffer(list.data, list.len, list.cap, elements.size as i64);

    // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
    unsafe { write_list_output(out_ptr, n as i64, n as i64, new_data) };
}

/// Apply a permutation in place using cycle-following.
///
/// Given `indices[i] = j`, moves the element originally at position `j`
/// to position `i`. Uses `O(elem_size)` temporary space (one element buffer).
fn apply_permutation_in_place(data: *mut u8, indices: &[usize], elem_size: usize) {
    let n = indices.len();
    let mut placed = vec![false; n];
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

        // SAFETY: `start < n`, and `data` contains `n` elements of `elem_size` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(start * elem_size), tmp.as_mut_ptr(), elem_size);
        }

        let mut current = start;
        loop {
            let next = indices[current];
            placed[current] = true;

            if next == start {
                // SAFETY: `current < n`, and `tmp` contains exactly one element.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        tmp.as_ptr(),
                        data.add(current * elem_size),
                        elem_size,
                    );
                }
                break;
            }

            // SAFETY:
            // - `indices` is a permutation of `0..n`, so both positions are in bounds.
            // - This branch excludes the cycle-closing position, so the elements do not overlap.
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
