//! List operations for AOT-compiled Ori programs.
//!
//! Ori lists use the layout `{ i64 len, i64 cap, *mut u8 data }` where the
//! data buffer is RC-managed via `ori_rc_alloc`. This module provides:
//! - **Allocation**: `ori_list_new`, `ori_list_alloc_data`, `ori_list_box_new`
//! - **Lifecycle**: `ori_list_free`, `ori_list_free_data`
//! - **COW mutations**: `ori_list_push_cow`, `ori_list_pop_cow`, `ori_list_set_cow`,
//!   `ori_list_insert_cow`, `ori_list_remove_cow`, `ori_list_concat_cow`,
//!   `ori_list_reverse_cow`, `ori_list_sort_cow`, `ori_list_sort_stable_cow`
//! - **Queries**: `ori_list_first`, `ori_list_last`, `ori_list_contains_*`
//! - **Functional**: `ori_list_reverse`, `ori_list_concat`

mod cow;
mod cow_sort;
mod cow_structural;
mod query;
mod reset;
pub mod slice;

pub use cow::*;
pub use cow_sort::*;
pub use cow_structural::*;
pub use query::*;
pub use reset::*;
pub use slice::*;

use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_dec, ori_rc_free, ori_rc_realloc};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

/// Ori list representation: { i64 len, i64 cap, *mut u8 data }
///
/// Also used for sets, which share the same memory layout.
#[repr(C)]
pub struct OriList {
    pub len: i64,
    pub cap: i64,
    pub data: *mut u8,
}

/// Return an empty list sentinel (no allocation).
///
/// Returns null — the boxed list model uses null as the empty sentinel.
/// `ori_rc_inc(null)` and `ori_rc_dec(null)` are no-ops.
#[no_mangle]
pub extern "C" fn ori_list_empty() -> *mut u8 {
    std::ptr::null_mut()
}

/// Ensure a list has capacity for at least `required` elements.
///
/// If `list.cap >= required`, this is a no-op. Otherwise, reallocates the
/// buffer using `next_capacity()` for amortized O(1) growth.
///
/// # Preconditions
/// - `list` is a valid pointer to an `OriList`
/// - The list's data buffer is uniquely owned (`ori_rc_is_unique(list.data)`)
///   OR the data pointer is null (empty sentinel)
/// - `elem_size` and `elem_align` describe the element layout
#[no_mangle]
pub extern "C" fn ori_list_ensure_capacity(
    list: *mut OriList,
    required: i64,
    elem_size: usize,
    elem_align: usize,
) {
    if list.is_null() || required <= 0 {
        return;
    }

    let list = unsafe { &mut *list };
    let required = required as usize;

    if (list.cap as usize) >= required {
        return;
    }

    let new_cap = next_capacity(list.cap as usize, required);
    let new_byte_size = new_cap * elem_size;

    if list.data.is_null() {
        // Sentinel (empty list) → first allocation.
        // Data buffers are RC-managed (32-byte V5 header) so COW
        // functions can call ori_rc_is_unique/ori_rc_dec on them.
        list.data = ori_rc_alloc(new_byte_size, elem_align);
    } else {
        let old_byte_size = list.cap as usize * elem_size;
        list.data = ori_rc_realloc(list.data, old_byte_size, new_byte_size, elem_align);
    }

    if !list.data.is_null() {
        list.cap = new_cap as i64;
    }
}

// List allocation/management

/// Allocate a new RC-boxed list struct with the given fields.
///
/// The `OriList` metadata `{len, cap, data}` is RC-allocated via
/// `ori_rc_alloc`. The data buffer (`data`) is plain-allocated separately
/// and owned by the `OriList`. Returns a pointer to the `OriList` data area
/// (RC header at `ptr - 32`; `strong_count` at `ptr - 8`).
///
/// Returns null on allocation failure.
#[no_mangle]
pub extern "C" fn ori_list_box_new(len: i64, cap: i64, data: *mut u8) -> *mut u8 {
    let box_ptr = ori_rc_alloc(
        std::mem::size_of::<OriList>(),
        std::mem::align_of::<OriList>(),
    );
    if box_ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: box_ptr is a valid RC allocation of OriList size
    unsafe {
        let list = &mut *box_ptr.cast::<OriList>();
        list.len = len;
        list.cap = cap;
        list.data = data;
    }
    box_ptr
}

/// Allocate a raw data buffer for a list with given capacity.
///
/// Returns a pointer to a contiguous buffer of `capacity * elem_size` bytes,
/// suitable for storing list elements directly. Used by codegen to allocate
/// the data buffer before boxing it with `ori_list_box_new`.
///
/// The buffer is **RC-managed** (32-byte V5 header, initial count = 1),
/// so COW functions can call `ori_rc_is_unique(data)` and `ori_rc_dec(data)`
/// without UB. This is critical: all list data buffers must be allocated
/// through `ori_rc_alloc` so the RC header is present and initialized.
///
/// For a complete RC-boxed list (metadata + data), use `ori_list_new`.
#[no_mangle]
pub extern "C" fn ori_list_alloc_data(capacity: i64, elem_size: i64) -> *mut u8 {
    let cap = capacity.max(0) as usize;
    let size = elem_size.max(1) as usize;
    if cap > 0 {
        let total = cap * size;
        ori_rc_alloc(total, 8)
    } else {
        std::ptr::null_mut()
    }
}

/// Allocate a new list with given capacity (full `OriList` struct on heap).
///
/// Used by JIT/test code. Not called from `arc_emitter/` codegen.
#[no_mangle]
pub extern "C" fn ori_list_new(capacity: i64, elem_size: i64) -> *mut OriList {
    let cap = capacity.max(0) as usize;
    let size = elem_size.max(1) as usize;

    let list = Box::new(OriList {
        len: 0,
        cap: cap as i64,
        data: if cap > 0 {
            let total = cap * size;
            let data = ori_rc_alloc(total, 8);
            if data.is_null() {
                return std::ptr::null_mut();
            }
            data
        } else {
            std::ptr::null_mut()
        },
    });

    Box::into_raw(list)
}

/// Free a heap-allocated `OriList` (from `ori_list_new`).
#[no_mangle]
pub extern "C" fn ori_list_free(list: *mut OriList, elem_size: i64) {
    if list.is_null() {
        return;
    }

    // SAFETY: Caller ensures list is valid
    unsafe {
        let list = Box::from_raw(list);
        if !list.data.is_null() && list.cap > 0 {
            let size = elem_size.max(1) as usize;
            let total = list.cap as usize * size;
            ori_rc_free(list.data, total, 8);
        }
    }
}

/// Free a raw data buffer allocated by `ori_list_alloc_data`.
///
/// For stack-struct lists (`{len, cap, data}`) where only the data buffer
/// is heap-allocated. The list header lives on the stack and doesn't need
/// freeing. Used by ARC cleanup when decrementing list refcounts.
///
/// The buffer was allocated via `ori_rc_alloc` (32-byte V5 RC header),
/// so we use `ori_rc_free` (alignment 8) to deallocate correctly.
#[no_mangle]
pub extern "C" fn ori_list_free_data(data: *mut u8, capacity: i64, elem_size: i64) {
    if data.is_null() || capacity <= 0 {
        return;
    }
    let cap = capacity as usize;
    let size = elem_size.max(1) as usize;
    let total = cap * size;
    ori_rc_free(data, total, 8);
}

/// Get the length of a list.
#[no_mangle]
pub extern "C" fn ori_list_len(list: *const OriList) -> i64 {
    if list.is_null() {
        return 0;
    }
    // SAFETY: Caller ensures list is valid
    unsafe { (*list).len }
}

/// Push an element onto a heap-allocated list, growing capacity if needed.
///
/// `list` is a pointer to a heap-allocated `OriList` (from `ori_list_new`).
/// `elem_ptr` points to the raw element bytes to copy.
/// `elem_size` is the byte size of each element.
#[no_mangle]
pub extern "C" fn ori_list_push(list: *mut u8, elem_ptr: *const u8, elem_size: i64) {
    if list.is_null() || elem_ptr.is_null() {
        return;
    }
    let list = unsafe { &mut *list.cast::<OriList>() };
    let es = elem_size.max(1) as usize;

    // Grow if needed
    if list.len >= list.cap {
        let new_cap = if list.cap <= 0 {
            8
        } else {
            list.cap as usize * 2
        };
        let old_total = list.cap.max(0) as usize * es;
        let new_total = new_cap * es;
        let new_data = if list.data.is_null() {
            ori_rc_alloc(new_total, 8)
        } else {
            ori_rc_realloc(list.data, old_total, new_total, 8)
        };
        list.data = new_data;
        list.cap = new_cap as i64;
    }

    // Copy element bytes into the data buffer
    unsafe {
        std::ptr::copy_nonoverlapping(elem_ptr, list.data.add(list.len as usize * es), es);
    }
    list.len += 1;
}

/// Extract the `OriList` contents and free the heap wrapper.
///
/// Writes `{len, cap, data}` to `out_ptr` (sret pattern — avoids ABI
/// mismatch for >16 byte struct returns). The data buffer ownership
/// transfers to the caller; only the `OriList` wrapper is freed.
#[no_mangle]
pub extern "C" fn ori_list_take(list: *mut u8, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    if list.is_null() {
        unsafe {
            out_ptr.cast::<i64>().write(0); // len
            out_ptr.cast::<i64>().add(1).write(0); // cap
            out_ptr
                .add(16)
                .cast::<*mut u8>()
                .write(std::ptr::null_mut()); // data
        }
        return;
    }
    let boxed = unsafe { Box::from_raw(list.cast::<OriList>()) };
    let len = boxed.len;
    let cap = boxed.cap;
    let data = boxed.data;
    // Box::drop frees the OriList struct; data buffer is NOT freed
    drop(boxed);
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

// Functional list operations

/// Create a new list with an element appended (functional push).
///
/// Allocates a new data buffer, copies all elements from the original,
/// then copies the new element at the end. Writes the resulting
/// `{len+1, len+1, new_data}` to `out_ptr` (sret pattern).
///
/// The original list data is NOT freed — the caller retains ownership.
#[no_mangle]
pub extern "C" fn ori_list_push_new(
    data: *const u8,
    len: i64,
    elem_ptr: *const u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let old_len = len.max(0) as usize;
    let new_len = old_len + 1;
    let new_total = new_len * es;

    let new_data = ori_rc_alloc(new_total, 8);

    // Copy old elements
    if !data.is_null() && old_len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(data, new_data, old_len * es);
        }
    }

    // Copy new element at the end
    unsafe {
        std::ptr::copy_nonoverlapping(elem_ptr, new_data.add(old_len * es), es);
    }

    // Write result: {len, cap, data}
    unsafe {
        out_ptr.cast::<i64>().write(new_len as i64);
        out_ptr.cast::<i64>().add(1).write(new_len as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Write `{ len, cap, data }` triple to the output pointer.
///
/// This is the common output pattern for all COW list functions.
/// The layout matches `[T]`'s ABI: `i64 len`, `i64 cap`, `*mut u8 data`.
#[inline]
pub(crate) unsafe fn write_list_output(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    out_ptr.cast::<i64>().write(len);
    out_ptr.cast::<i64>().add(1).write(cap);
    out_ptr.add(16).cast::<*mut u8>().write(data);
}

/// Decrement a list buffer's refcount, handling seamless slices.
///
/// For regular buffers (`cap >= 0`), decs `data`'s RC directly.
/// For slices (`cap < 0`), computes the original buffer's data pointer and
/// decs that instead. Does NOT perform element cleanup — this is a
/// buffer-level RC dec only, used by COW functions that have already
/// handled element RC via `inc_copied_elements`.
#[inline]
pub(crate) fn dec_list_buffer(data: *mut u8, cap: i64) {
    if data.is_null() {
        return;
    }
    if is_slice_cap(cap) {
        let original = slice_original_data(data, cap);
        ori_rc_dec(original, None);
    } else {
        ori_rc_dec(data, None);
    }
}

/// Increment RC for each copied element in a data buffer.
///
/// Called on COW slow paths after `copy_nonoverlapping` to ensure each
/// byte-copied RC-managed element gets its reference count incremented
/// (the new buffer is a new reference to each element's sub-objects).
///
/// No-op if `inc_fn` is None (scalar element types have no RC children).
#[inline]
pub(crate) fn inc_copied_elements(
    data: *mut u8,
    count: usize,
    elem_size: usize,
    inc_fn: Option<extern "C" fn(*mut u8)>,
) {
    if let Some(f) = inc_fn {
        for i in 0..count {
            f(unsafe { data.add(i * elem_size) });
        }
    }
}

/// Shared helper: copy a contiguous array into a new list struct via sret.
///
/// Stores `elem_dec_fn` and `elem_count` in the new buffer's RC header
/// for element cleanup when the buffer is freed.
pub(crate) fn write_array_to_list(
    data: *const u8,
    len: i64,
    elem_size: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let n = len.max(0) as usize;

    if data.is_null() || n == 0 {
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

    let total = n * es;
    let new_data = ori_rc_alloc(total, 8);
    unsafe {
        std::ptr::copy_nonoverlapping(data, new_data, total);
    }
    // SAFETY: new_data was just returned by ori_rc_alloc — header offsets are valid.
    unsafe {
        crate::rc::store_elem_dec_fn(new_data, elem_dec_fn);
        crate::rc::store_elem_count(new_data, n as i64);
    }
    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}
