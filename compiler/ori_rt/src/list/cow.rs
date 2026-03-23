//! COW (Copy-on-Write) list mutation functions.
//!
//! All functions in this module follow consuming semantics: they take ownership
//! of the caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.

use crate::next_capacity;
use crate::rc::{
    load_elem_dec_fn, ori_rc_alloc, ori_rc_is_unique, ori_rc_realloc, rt_debug_bounds_warning,
    rt_debug_null_cow_warning, store_elem_count, store_elem_dec_fn,
};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

use super::{dec_list_buffer, inc_copied_elements};

// Propagate elem_dec_fn and elem_count from old buffer header to new buffer.
// When `old_data` is a slice interior pointer, resolves to the original
// allocation's data pointer before reading the header.
// SAFETY: `new_data` must have been returned by `ori_rc_alloc`.
//         `old_data` must be either an `ori_rc_alloc` pointer or a slice into one.
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
/// # Output
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

    // FAST PATH: unique owner, non-slice — can mutate in place
    // Slices MUST NOT enter this path: `data` is interior to another
    // allocation, so `ori_rc_is_unique(data)` would read garbage.
    // cow_mode: 0=dynamic (normal RC check), 1=static unique (skip check),
    //           2=static shared (force slow path)
    let is_unique = !data.is_null()
        && !is_slice_cap(cap)
        && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        let old_cap = cap.max(0) as usize;

        if old_cap >= new_len {
            // Has capacity — write element in place, same buffer
            unsafe {
                std::ptr::copy_nonoverlapping(elem_ptr, data.add(old_len * es), es);
                out_ptr.cast::<i64>().write(new_len as i64);
                out_ptr.cast::<i64>().add(1).write(cap);
                out_ptr.add(16).cast::<*mut u8>().write(data);
            }
            return;
        }

        // Needs growth — realloc (may extend in place or move)
        let new_cap = next_capacity(old_cap, new_len);
        let new_data = ori_rc_realloc(data, old_cap * es, new_cap * es, ea);
        if new_data.is_null() {
            // Realloc failed — return original unchanged (data still valid)
            unsafe {
                out_ptr.cast::<i64>().write(len);
                out_ptr.cast::<i64>().add(1).write(cap);
                out_ptr.add(16).cast::<*mut u8>().write(data);
            }
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(old_len * es), es);
            out_ptr.cast::<i64>().write(new_len as i64);
            out_ptr.cast::<i64>().add(1).write(new_cap as i64);
            out_ptr.add(16).cast::<*mut u8>().write(new_data);
        }
        return;
    }

    // SLOW PATH: shared or empty — allocate new buffer.
    // Use tight-fit capacity (= new_len) instead of doubling the old capacity.
    // Doubling only benefits the fast path (unique owner, same buffer reused).
    // On the slow path we're creating a fresh buffer that may itself be shared
    // on the next iteration, so over-allocating just wastes memory and causes
    // exponential capacity growth in shared-push loops.
    let new_cap = if data.is_null() {
        next_capacity(0, new_len)
    } else {
        new_len
    };
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // Copy old elements and increment their RC (they're now in a new buffer)
    if !data.is_null() && old_len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(data, new_data, old_len * es);
        }
        inc_copied_elements(new_data, old_len, es, inc_fn);
    }

    // Write new element
    unsafe {
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(old_len * es), es);
    }

    // Propagate elem_dec_fn and elem_count from old header to new buffer
    if !data.is_null() {
        unsafe { propagate_elem_header(data, cap, new_data, new_len as i64) };
    }

    // Release our reference to the old buffer. For shared buffers (RC > 1),
    // this decrements without triggering deallocation. For empty (null), this
    // is a no-op. For slices, this decs the original buffer's RC.
    // Element RC was already handled by inc_copied_elements above.
    dec_list_buffer(data, cap);

    // Write result
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
/// Users can explicitly call `list.compact()` (future) to reclaim.
/// Matches Rust's `Vec` behavior.
///
/// # Output
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

    // Empty list — return unchanged
    if len <= 0 || data.is_null() {
        if data.is_null() {
            rt_debug_null_cow_warning("ori_list_pop_cow");
        }
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

    // FAST PATH: unique owner, non-slice — just shrink len
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique =
        !is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        unsafe {
            out_ptr.cast::<i64>().write(new_len as i64);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // SLOW PATH: shared or slice — allocate new buffer with len-1 elements
    if new_len == 0 {
        // Popping last element from shared/slice list → empty sentinel
        dec_list_buffer(data, cap);
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

    let new_cap = new_len; // No excess capacity needed for copied lists
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // Copy all-but-last elements and increment their RC
    unsafe {
        std::ptr::copy_nonoverlapping(data, new_data, new_len * es);
    }
    inc_copied_elements(new_data, new_len, es, inc_fn);

    // Propagate elem_dec_fn and elem_count from old header
    unsafe { propagate_elem_header(data, cap, new_data, new_len as i64) };

    // Release our reference to the old buffer (slice-aware)
    dec_list_buffer(data, cap);

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
/// # Output
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

    // Bounds check — return unchanged if out of range
    if data.is_null() || index < 0 || index >= len {
        if data.is_null() {
            rt_debug_null_cow_warning("ori_list_set_cow");
        } else {
            rt_debug_bounds_warning("ori_list_set_cow", index, len);
        }
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

    // FAST PATH: unique owner, non-slice — overwrite in place
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique =
        !is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        unsafe {
            std::ptr::copy_nonoverlapping(elem_ptr, data.add(idx * es), es);
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // SLOW PATH: shared — copy entire list, overwrite in copy
    let old_len = len as usize;
    let new_data = ori_rc_alloc(old_len * es, ea);

    unsafe {
        // Copy all elements
        std::ptr::copy_nonoverlapping(data, new_data, old_len * es);
        // Overwrite element at index
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(idx * es), es);
    }

    // Inc RC for all copied elements EXCEPT the one at index (it was overwritten)
    inc_copied_elements(new_data, idx, es, inc_fn);
    if idx + 1 < old_len {
        inc_copied_elements(
            unsafe { new_data.add((idx + 1) * es) },
            old_len - idx - 1,
            es,
            inc_fn,
        );
    }

    // Propagate elem_dec_fn and elem_count from old header
    unsafe { propagate_elem_header(data, cap, new_data, old_len as i64) };

    // Release our reference to the old buffer (slice-aware)
    dec_list_buffer(data, cap);

    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(old_len as i64); // cap = len (tight fit)
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}
