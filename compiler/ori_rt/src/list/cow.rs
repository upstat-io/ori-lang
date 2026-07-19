//! COW (Copy-on-Write) list mutation functions.
//!
//! All functions in this module follow consuming semantics: they take ownership
//! of the caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.

use crate::next_capacity;
use crate::rc::{
    load_elem_dec_fn, ori_rc_alloc, ori_rc_realloc, rt_debug_bounds_warning,
    rt_debug_null_cow_warning, store_elem_count, store_elem_dec_fn,
};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

use super::cow_context::{CowMode, ElementOps, ListBuffer};
use super::{dec_list_buffer, inc_copied_elements};

// SAFETY:
// - `new_data` must have been returned by `ori_rc_alloc`.
// - `old_data` must be an `ori_rc_alloc` pointer or an encoded slice into one.
unsafe fn propagate_elem_header(
    old_data: *mut u8,
    old_cap: i64,
    new_data: *mut u8,
    new_elem_count: i64,
) {
    let header_data = if is_slice_cap(old_cap) {
        slice_original_data(old_data, old_cap)
    } else {
        old_data
    };
    let dec_fn = load_elem_dec_fn(header_data);
    store_elem_dec_fn(new_data, dec_fn);
    store_elem_count(new_data, new_elem_count);
}

/// COW-aware list push with consuming semantics.
///
/// Appends `elem` to a list. The data buffer must be RC-allocated (via
/// `ori_rc_alloc`) or null (empty sentinel). This function **takes ownership**
/// of the caller's reference to `data` (consuming semantics):
///
/// - **Fast path** (unique, has capacity): Writes in place. Same `data` pointer
///   returned in `out_ptr`. No RC changes — the sole reference transfers to
///   the output.
/// - **Fast path** (unique, needs growth): `ori_rc_realloc` grows the buffer.
///   A possibly different pointer is returned. Old pointer invalidated by
///   realloc. RC preserved by realloc.
/// - **Slow path** (shared or empty): New buffer allocated (RC=1), old elements
///   byte-copied, new element written. Old buffer's RC decremented (without
///   element cleanup — `elem_dec_fn` in the V5 RC header handles cleanup).
///
/// # Element RC
///
/// On the slow path, byte-copied elements get their RC incremented via the
/// `inc_fn` callback (if non-null). Pass null for scalar element types.
///
/// # Returns
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_list_push_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
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

    if data.is_null() {
        rt_debug_null_cow_warning("ori_list_push_cow");
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let old_len = len.max(0) as usize;
    let new_len = old_len + 1;

    // Why: Slice data is interior, so a direct uniqueness check would read outside its header.
    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        let old_cap = cap.max(0) as usize;

        if old_cap >= new_len {
            // SAFETY: Uniqueness verified; old_len * es < old_cap * es (has capacity); out_ptr non-null.
            unsafe {
                std::ptr::copy_nonoverlapping(elem_ptr, data.add(old_len * es), es);
                out_ptr.cast::<i64>().write(new_len as i64);
                out_ptr.cast::<i64>().add(1).write(cap);
                out_ptr.add(16).cast::<*mut u8>().write(data);
            }
            return;
        }

        let new_cap = next_capacity(old_cap, new_len);
        let new_data = ori_rc_realloc(data, old_cap * es, new_cap * es, ea);
        if new_data.is_null() {
            // SAFETY: out_ptr validated non-null at function entry.
            unsafe {
                out_ptr.cast::<i64>().write(len);
                out_ptr.cast::<i64>().add(1).write(cap);
                out_ptr.add(16).cast::<*mut u8>().write(data);
            }
            return;
        }
        // SAFETY: new_data has new_cap * es bytes; old_len * es < new_cap * es; out_ptr non-null.
        unsafe {
            std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(old_len * es), es);
            out_ptr.cast::<i64>().write(new_len as i64);
            out_ptr.cast::<i64>().add(1).write(new_cap as i64);
            out_ptr.add(16).cast::<*mut u8>().write(new_data);
        }
        return;
    }

    // Why: Tight fitting prevents repeated shared pushes from compounding unused capacity.
    let new_cap = if data.is_null() {
        next_capacity(0, new_len)
    } else {
        new_len
    };
    let new_data = ori_rc_alloc(new_cap * es, ea);

    if !data.is_null() && old_len > 0 {
        // SAFETY: data has old_len * es valid bytes; new_data has new_cap * es >= old_len * es bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(data, new_data, old_len * es);
        }
        inc_copied_elements(new_data, old_len, es, inc_fn);
    }

    // SAFETY: new_data has new_cap * es bytes; old_len * es < new_cap * es.
    unsafe {
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(old_len * es), es);
    }

    if !data.is_null() {
        // SAFETY: data is a valid RC-allocated (or slice) buffer; new_data from ori_rc_alloc.
        unsafe { propagate_elem_header(data, cap, new_data, new_len as i64) };
    }

    // Why: Copied elements acquire destination credits before the consumed buffer is released.
    dec_list_buffer(data, len, cap, elem_size);

    // SAFETY: out_ptr validated non-null at function entry; writing {len, cap, data} triple.
    unsafe {
        out_ptr.cast::<i64>().write(new_len as i64);
        out_ptr.cast::<i64>().add(1).write(new_cap as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// COW-aware list pop with consuming semantics.
///
/// Removes the last element from the list. If the data buffer is uniquely
/// owned (RC==1), simply decrements `len` (O(1) — the element remains in
/// the buffer but is logically inaccessible). If shared, allocates a new
/// buffer and copies `len - 1` elements.
///
/// The popped element must be extracted BEFORE calling pop (via `last()` or
/// index access). This function only shortens the list — it does not return
/// the removed element.
///
/// Returns the input unchanged if `len <= 0` (empty list).
///
/// # Consuming semantics
///
/// Same ownership transfer as `ori_list_push_cow`:
/// - **Fast path** (unique): Same buffer reused with decremented len.
/// - **Slow path** (shared): New buffer allocated, old buffer's RC decremented.
///
/// # Capacity reclamation
///
/// Does NOT auto-shrink. Capacity grows but never shrinks automatically.
/// Matches Rust's `Vec` behavior.
///
/// # Returns
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_list_pop_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    if len <= 0 || data.is_null() {
        if data.is_null() {
            rt_debug_null_cow_warning("ori_list_pop_cow");
        }
        // SAFETY: The entry guard proves `out_ptr` is non-null.
        unsafe {
            out_ptr.cast::<i64>().write(0);
            out_ptr.cast::<i64>().add(1).write(0);
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut());
        }
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let new_len = (len - 1) as usize;

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        // SAFETY: Uniqueness verified; buffer remains valid; just decrements logical len.
        unsafe {
            out_ptr.cast::<i64>().write(new_len as i64);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    if new_len == 0 {
        dec_list_buffer(data, len, cap, elem_size);
        // SAFETY: out_ptr validated non-null; writing empty list triple.
        unsafe {
            out_ptr.cast::<i64>().write(0);
            out_ptr.cast::<i64>().add(1).write(0);
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut());
        }
        return;
    }

    let new_cap = new_len;
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // SAFETY: data has len * es valid bytes; new_data has new_len * es bytes; new_len < len.
    unsafe {
        std::ptr::copy_nonoverlapping(data, new_data, new_len * es);
    }
    inc_copied_elements(new_data, new_len, es, inc_fn);

    // SAFETY: data is a valid RC-allocated (or slice) buffer; new_data from ori_rc_alloc.
    unsafe { propagate_elem_header(data, cap, new_data, new_len as i64) };

    // Why: Copied elements acquire destination credits before the consumed buffer is released.
    dec_list_buffer(data, len, cap, elem_size);

    // SAFETY: out_ptr validated non-null; writing {len, cap, data} triple.
    unsafe {
        out_ptr.cast::<i64>().write(new_len as i64);
        out_ptr.cast::<i64>().add(1).write(new_cap as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// COW-aware list index assignment with consuming semantics.
///
/// Replaces the element at `index` with `elem`. If the data buffer is
/// uniquely owned (RC==1), overwrites in place (O(1)). If shared,
/// copies the entire list and overwrites in the copy (O(n)).
///
/// Returns the input unchanged if `index` is out of bounds.
///
/// # Element RC
///
/// The OLD element at `index` must be decremented by the caller (codegen
/// responsibility). The new element is moved in (no inc needed — ownership
/// transfers from the caller's temporary to the list).
///
/// # Returns
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_list_set_cow(
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

    if data.is_null() || index < 0 || index >= len {
        if data.is_null() {
            rt_debug_null_cow_warning("ori_list_set_cow");
        } else {
            rt_debug_bounds_warning("ori_list_set_cow", index, len);
        }
        // SAFETY: out_ptr validated non-null; returning input unchanged on bounds error.
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let idx = index as usize;

    let is_unique = CowMode::from_abi(cow_mode).allows_in_place(data, cap);
    if is_unique {
        // SAFETY: Uniqueness verified; idx < len so idx * es is within the buffer.
        unsafe {
            std::ptr::copy_nonoverlapping(elem_ptr, data.add(idx * es), es);
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    slow_copy_replace_element(
        ListBuffer::new(data, len, cap),
        idx,
        elem_ptr,
        ElementOps::new(es, ea, inc_fn),
        out_ptr,
    );
}

/// Shared-buffer replacement: copy the list, overwrite the element at `idx`.
///
/// The replacement element keeps the caller's reference (no inc); all other copied
/// elements get their RC incremented for the new buffer. The old buffer's
/// reference is released slice-aware; the old element at `idx` stays owned by
/// the old buffer (or is released by its teardown).
pub(super) fn slow_copy_replace_element(
    list: ListBuffer,
    idx: usize,
    elem_ptr: *const u8,
    elements: ElementOps,
    out_ptr: *mut u8,
) {
    let old_len = list.len as usize;
    let new_data = ori_rc_alloc(old_len * elements.size, elements.align);

    // SAFETY: data has old_len * es bytes; new_data has old_len * es bytes; idx < old_len.
    unsafe {
        std::ptr::copy_nonoverlapping(list.data, new_data, old_len * elements.size);
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(idx * elements.size), elements.size);
    }

    // Why: The replacement keeps the caller's credit; every retained copy needs a new one.
    inc_copied_elements(new_data, idx, elements.size, elements.inc);
    if idx + 1 < old_len {
        inc_copied_elements(
            // SAFETY: (idx + 1) * es is within new_data's allocation of old_len * es bytes.
            unsafe { new_data.add((idx + 1) * elements.size) },
            old_len - idx - 1,
            elements.size,
            elements.inc,
        );
    }

    // SAFETY: data is a valid RC-allocated (or slice) buffer; new_data from ori_rc_alloc.
    unsafe { propagate_elem_header(list.data, list.cap, new_data, old_len as i64) };

    // Why: Retained copies acquire destination credits before the consumed buffer is released.
    dec_list_buffer(list.data, list.len, list.cap, elements.size as i64);

    // SAFETY: out_ptr validated non-null; writing {len, cap, data} triple.
    unsafe {
        out_ptr.cast::<i64>().write(list.len);
        out_ptr.cast::<i64>().add(1).write(old_len as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}
