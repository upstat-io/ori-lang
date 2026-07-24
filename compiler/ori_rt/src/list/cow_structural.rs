//! COW structural list mutations: insert and remove.
//!
//! These consuming operations shift elements within the buffer; element-level
//! mutations operate on individual positions without shifting the suffix.

use crate::next_capacity;
use crate::rc::{
    load_elem_dec_fn, ori_rc_alloc, ori_rc_realloc, rt_debug_bounds_warning, store_elem_count,
    store_elem_dec_fn,
};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

use super::cow_context::CowMode;
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
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let old_len = len.max(0) as usize;
    let idx = index.max(0) as usize;

    if idx > old_len {
        rt_debug_bounds_warning("ori_list_insert_cow", index, len);
        // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
        unsafe { write_list_output(out_ptr, len, cap, data) };
        return;
    }

    if data.is_null() && old_len > 0 {
        crate::rc::rt_debug_null_cow_warning("ori_list_insert_cow");
    }

    let new_len = old_len + 1;

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        let old_cap = cap.max(0) as usize;

        let (buf, buf_cap) = if old_cap >= new_len {
            (data, old_cap)
        } else {
            let nc = next_capacity(old_cap, new_len);
            let nd = ori_rc_realloc(data, old_cap * es, nc * es, ea);
            if nd.is_null() {
                // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
                unsafe { write_list_output(out_ptr, len, cap, data) };
                return;
            }
            (nd, nc)
        };

        let tail = old_len - idx;
        if tail > 0 {
            // SAFETY:
            // - `buf` has capacity for `new_len` elements because `ensure_capacity` succeeded.
            // - `idx <= old_len`; `copy` permits the one-element overlap while shifting.
            unsafe {
                std::ptr::copy(buf.add(idx * es), buf.add((idx + 1) * es), tail * es);
            }
        }
        // SAFETY:
        // - `elem_ptr` supplies one readable element and `buf[idx]` is within capacity.
        // - The non-null ABI output slot is writable and aligned for the list triple.
        unsafe {
            std::ptr::copy_nonoverlapping(elem_ptr, buf.add(idx * es), es);
            write_list_output(out_ptr, new_len as i64, buf_cap as i64, buf);
        }
        return;
    }

    // Why: Shared insertion tight-fits to avoid exponential unused capacity.
    let new_cap = if data.is_null() {
        next_capacity(0, new_len)
    } else {
        new_len
    };
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // SAFETY:
    // - `new_data` has capacity for `new_cap >= new_len` elements.
    // - `idx <= old_len`; guarded source copies stay within the initialized input.
    // - The new allocation cannot overlap `data` or `elem_ptr`.
    unsafe {
        if !data.is_null() && idx > 0 {
            std::ptr::copy_nonoverlapping(data, new_data, idx * es);
        }
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(idx * es), es);
        let tail = old_len - idx;
        if !data.is_null() && tail > 0 {
            std::ptr::copy_nonoverlapping(
                data.add(idx * es),
                new_data.add((idx + 1) * es),
                tail * es,
            );
        }
    }

    inc_copied_elements(new_data, idx, es, inc_fn);
    let tail = old_len - idx;
    if tail > 0 {
        // SAFETY: The offset starts the initialized suffix of the `new_len`-element allocation.
        inc_copied_elements(unsafe { new_data.add((idx + 1) * es) }, tail, es, inc_fn);
    }

    if !data.is_null() {
        // SAFETY:
        // - `data` belongs to a live regular list or encoded slice allocation.
        // - `new_data` belongs to the fresh RC allocation returned by `ori_rc_alloc`.
        unsafe {
            let header_data = if is_slice_cap(cap) {
                slice_original_data(data, cap)
            } else {
                data
            };
            store_elem_dec_fn(new_data, load_elem_dec_fn(header_data));
            store_elem_count(new_data, new_len as i64);
        }
    }

    dec_list_buffer(data, len, cap, elem_size);

    // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
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
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let old_len = len.max(0) as usize;
    let idx = index as usize;

    if data.is_null() || index < 0 || idx >= old_len {
        if data.is_null() {
            crate::rc::rt_debug_null_cow_warning("ori_list_remove_cow");
        } else {
            rt_debug_bounds_warning("ori_list_remove_cow", index, len);
        }
        // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
        unsafe { write_list_output(out_ptr, len, cap, data) };
        return;
    }

    let new_len = old_len - 1;
    let tail_count = old_len - idx - 1;

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        if new_len == 0 {
            let old_cap = cap.max(0) as usize;
            crate::rc::ori_rc_free(data, old_cap * es, ea);
            // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
            unsafe {
                write_list_output(out_ptr, 0, 0, std::ptr::null_mut());
            }
            return;
        }

        // SAFETY:
        // - `idx < old_len`; source and destination spans remain within `data`.
        // - `copy` permits overlap while shifting the suffix left.
        // - The non-null ABI output slot is writable and aligned for the list triple.
        unsafe {
            if tail_count > 0 {
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

    if new_len == 0 {
        dec_list_buffer(data, len, cap, elem_size);
        // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
        unsafe {
            write_list_output(out_ptr, 0, 0, std::ptr::null_mut());
        }
        return;
    }

    let new_data = ori_rc_alloc(new_len * es, ea);

    // SAFETY:
    // - `idx < old_len`; both guarded source spans stay within initialized input.
    // - `new_data` holds `new_len` elements and cannot overlap the input allocation.
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

    inc_copied_elements(new_data, idx, es, inc_fn);
    if tail_count > 0 {
        // SAFETY: The offset starts the initialized suffix of the `new_len`-element allocation.
        inc_copied_elements(unsafe { new_data.add(idx * es) }, tail_count, es, inc_fn);
    }

    // SAFETY:
    // - `data` belongs to a live regular list or encoded slice allocation.
    // - `new_data` belongs to the fresh RC allocation returned by `ori_rc_alloc`.
    unsafe {
        let header_data = if is_slice_cap(cap) {
            slice_original_data(data, cap)
        } else {
            data
        };
        store_elem_dec_fn(new_data, load_elem_dec_fn(header_data));
        store_elem_count(new_data, new_len as i64);
    }

    dec_list_buffer(data, len, cap, elem_size);

    // SAFETY: The non-null ABI output slot is writable and aligned for the list triple.
    unsafe {
        write_list_output(out_ptr, new_len as i64, new_len as i64, new_data);
    }
}
