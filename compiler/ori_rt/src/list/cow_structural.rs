//! COW structural list mutations: insert and remove.
//!
//! These operations shift elements within the buffer, unlike the element-level
//! operations in `cow.rs` (push, pop, set) which operate on individual positions.
//! All functions follow consuming semantics (see `cow.rs` module docs).

use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_is_unique, ori_rc_realloc, rt_debug_bounds_warning};
use crate::slice_encoding::is_slice_cap;

use super::{dec_list_buffer, inc_copied_elements, write_list_output};

/// COW-aware list insert with consuming semantics.
///
/// Inserts `elem` at `index`, shifting subsequent elements right.
/// `index` must be in `0..=len`. If out of bounds, returns input unchanged.
#[no_mangle]
pub extern "C" fn ori_list_insert_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    index: i64,
    elem_ptr: *const u8,
    elem_size: i64,
    elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let old_len = len.max(0) as usize;
    let idx = index.max(0) as usize;

    // Bounds: index must be 0..=len. Appending at len is valid (= push).
    if idx > old_len {
        rt_debug_bounds_warning("ori_list_insert_cow", index, len);
        unsafe { write_list_output(out_ptr, len, cap, data) };
        return;
    }

    if data.is_null() && old_len > 0 {
        crate::rc::rt_debug_null_cow_warning("ori_list_insert_cow");
    }

    let new_len = old_len + 1;

    // FAST PATH: unique owner, non-slice — shift in place
    if !data.is_null() && !is_slice_cap(cap) && ori_rc_is_unique(data) {
        let old_cap = cap.max(0) as usize;

        // Ensure capacity (may realloc)
        let (buf, buf_cap) = if old_cap >= new_len {
            (data, old_cap)
        } else {
            let nc = next_capacity(old_cap, new_len);
            let nd = ori_rc_realloc(data, old_cap * es, nc * es, ea);
            if nd.is_null() {
                unsafe { write_list_output(out_ptr, len, cap, data) };
                return;
            }
            (nd, nc)
        };

        // Shift elements right
        let tail = old_len - idx;
        if tail > 0 {
            unsafe {
                std::ptr::copy(buf.add(idx * es), buf.add((idx + 1) * es), tail * es);
            }
        }
        // Write new element
        unsafe {
            std::ptr::copy_nonoverlapping(elem_ptr, buf.add(idx * es), es);
            write_list_output(out_ptr, new_len as i64, buf_cap as i64, buf);
        }
        return;
    }

    // SLOW PATH: shared or empty — build new buffer
    let base_cap = if data.is_null() {
        0
    } else {
        cap.max(0) as usize
    };
    let new_cap = next_capacity(base_cap, new_len);
    let new_data = ori_rc_alloc(new_cap * es, ea);

    unsafe {
        // Copy elements before index
        if !data.is_null() && idx > 0 {
            std::ptr::copy_nonoverlapping(data, new_data, idx * es);
        }
        // Write new element
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(idx * es), es);
        // Copy elements after index
        let tail = old_len - idx;
        if !data.is_null() && tail > 0 {
            std::ptr::copy_nonoverlapping(
                data.add(idx * es),
                new_data.add((idx + 1) * es),
                tail * es,
            );
        }
    }

    // Inc RC for all copied elements (new element excluded — ownership transferred)
    inc_copied_elements(new_data, idx, es, inc_fn);
    let tail = old_len - idx;
    if tail > 0 {
        inc_copied_elements(unsafe { new_data.add((idx + 1) * es) }, tail, es, inc_fn);
    }

    dec_list_buffer(data, cap);

    unsafe {
        write_list_output(out_ptr, new_len as i64, new_cap as i64, new_data);
    }
}

/// COW-aware list remove with consuming semantics.
///
/// Removes element at `index`, shifting subsequent elements left.
/// `index` must be in `0..len`. If out of bounds, returns input unchanged.
#[no_mangle]
pub extern "C" fn ori_list_remove_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    index: i64,
    elem_size: i64,
    elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let old_len = len.max(0) as usize;
    let idx = index as usize;

    // Bounds check
    if data.is_null() || index < 0 || idx >= old_len {
        if data.is_null() {
            crate::rc::rt_debug_null_cow_warning("ori_list_remove_cow");
        } else {
            rt_debug_bounds_warning("ori_list_remove_cow", index, len);
        }
        unsafe { write_list_output(out_ptr, len, cap, data) };
        return;
    }

    let new_len = old_len - 1;
    let tail_count = old_len - idx - 1; // Elements after the removed one

    // FAST PATH: unique owner, non-slice — shift left in place
    if !is_slice_cap(cap) && ori_rc_is_unique(data) {
        if new_len == 0 {
            // Removing last element — free buffer entirely, return empty
            let old_cap = cap.max(0) as usize;
            crate::rc::ori_rc_free(data, old_cap * es, ea);
            unsafe {
                write_list_output(out_ptr, 0, 0, std::ptr::null_mut());
            }
            return;
        }

        unsafe {
            if tail_count > 0 {
                // Shift [index+1..len] left by one elem_size (overlapping)
                std::ptr::copy(
                    data.add((idx + 1) * es),
                    data.add(idx * es),
                    tail_count * es,
                );
            }
            write_list_output(out_ptr, new_len as i64, cap, data);
        }
        return;
    }

    // SLOW PATH: shared or slice — allocate new buffer
    if new_len == 0 {
        dec_list_buffer(data, cap);
        unsafe {
            write_list_output(out_ptr, 0, 0, std::ptr::null_mut());
        }
        return;
    }

    let new_data = ori_rc_alloc(new_len * es, ea);

    unsafe {
        if idx > 0 {
            std::ptr::copy_nonoverlapping(data, new_data, idx * es);
        }
        if tail_count > 0 {
            std::ptr::copy_nonoverlapping(
                data.add((idx + 1) * es),
                new_data.add(idx * es),
                tail_count * es,
            );
        }
    }

    // Inc RC for all copied elements (element at index NOT copied)
    inc_copied_elements(new_data, idx, es, inc_fn);
    if tail_count > 0 {
        inc_copied_elements(unsafe { new_data.add(idx * es) }, tail_count, es, inc_fn);
    }

    dec_list_buffer(data, cap);

    unsafe {
        write_list_output(out_ptr, new_len as i64, new_len as i64, new_data);
    }
}
