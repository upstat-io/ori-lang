//! Query and functional (non-COW) list operations.
//!
//! These functions do not take ownership of their inputs — they either
//! read from the list (queries) or produce a new list from immutable
//! references (functional operations).

use crate::io::ori_panic_cstr;
use crate::rc::{load_elem_dec_fn_const, ori_rc_alloc, store_elem_count, store_elem_dec_fn};
use crate::slice_encoding::{is_slice_cap, slice_original_data};
use crate::string::{deref_str, OriStr};
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

use super::{inc_copied_elements, write_list_output};

/// Bounds-checked element access: copy `elem_size` bytes at `data[index]` to `out_ptr`.
///
/// Panics if `index < 0 || index >= len` (via `ori_panic_cstr`, which unwinds).
#[no_mangle]
pub extern "C-unwind" fn ori_list_get(
    data: *const u8,
    len: i64,
    index: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if index < 0 || index >= len {
        ori_panic_cstr(c"index out of bounds".as_ptr());
    }
    let es = elem_size.max(1) as usize;
    let offset = index as usize * es;
    unsafe {
        std::ptr::copy_nonoverlapping(data.add(offset), out_ptr, es);
    }
}

/// Return the first element of a list, or write a None tag if empty.
///
/// Writes `{tag, value}` to `out_ptr` where tag=`OPTION_TAG_SOME` with
/// element copied, or tag=`OPTION_TAG_NONE`. The value region is `elem_size` bytes.
#[no_mangle]
pub extern "C" fn ori_list_first(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    if data.is_null() || len <= 0 {
        unsafe {
            out_ptr.cast::<i64>().write(OPTION_TAG_NONE);
        }
        return;
    }
    unsafe {
        out_ptr.cast::<i64>().write(OPTION_TAG_SOME);
        std::ptr::copy_nonoverlapping(data, out_ptr.add(8), es);
    }
}

/// Return the last element of a list, or write a None tag if empty.
///
/// Same layout as `ori_list_first`: `{tag: i64, value: [elem_size]}`.
#[no_mangle]
pub extern "C" fn ori_list_last(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    if data.is_null() || len <= 0 {
        unsafe {
            out_ptr.cast::<i64>().write(OPTION_TAG_NONE);
        }
        return;
    }
    let last_offset = (len as usize - 1) * es;
    unsafe {
        out_ptr.cast::<i64>().write(OPTION_TAG_SOME);
        std::ptr::copy_nonoverlapping(data.add(last_offset), out_ptr.add(8), es);
    }
}

/// Check whether a list of i64 elements contains a given value.
///
/// Returns 1 (true) if found, 0 (false) otherwise.
#[no_mangle]
pub extern "C" fn ori_list_contains_int(data: *const u8, len: i64, needle: i64) -> i64 {
    if data.is_null() || len <= 0 {
        return 0;
    }
    let ptr = data.cast::<i64>();
    for i in 0..len as usize {
        if unsafe { *ptr.add(i) } == needle {
            return 1;
        }
    }
    0
}

/// Check whether a list of strings contains a given string.
///
/// Each string element is an `OriStr` (24 bytes, SSO or heap).
/// Returns 1 (true) if found, 0 (false) otherwise.
#[no_mangle]
pub extern "C" fn ori_list_contains_str(data: *const u8, len: i64, needle: *const OriStr) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() {
        return 0;
    }
    let needle_str = unsafe { deref_str(needle) };
    let elem_size = std::mem::size_of::<OriStr>();
    for i in 0..len as usize {
        let elem_ptr = unsafe { data.add(i * elem_size).cast::<OriStr>() };
        let elem_str = unsafe { (*elem_ptr).as_str() };
        if elem_str == needle_str {
            return 1;
        }
    }
    0
}

/// Create a reversed copy of a list.
///
/// Allocates new data buffer, copies elements in reverse order.
/// Writes `{len, len, new_data}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_list_reverse(
    data: *const u8,
    len: i64,
    elem_size: i64,
    elem_align: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
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
    let new_data = ori_rc_alloc(total, ea);

    for i in 0..n {
        let src_offset = (n - 1 - i) * es;
        let dst_offset = i * es;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_offset), new_data.add(dst_offset), es);
        }
    }

    // Propagate elem_dec_fn and elem_count from source
    unsafe {
        store_elem_dec_fn(new_data, load_elem_dec_fn_const(data));
        store_elem_count(new_data, n as i64);
    }

    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Concatenate two lists, returning a new list.
///
/// Allocates new buffer, copies elements from both lists.
/// Writes `{len1+len2, len1+len2, new_data}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_list_concat(
    data1: *const u8,
    len1: i64,
    data2: *const u8,
    len2: i64,
    elem_size: i64,
    elem_align: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n1 = len1.max(0) as usize;
    let n2 = len2.max(0) as usize;
    let total_len = n1 + n2;

    if total_len == 0 {
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

    let total_bytes = total_len * es;
    let new_data = ori_rc_alloc(total_bytes, ea);

    unsafe {
        if !data1.is_null() && n1 > 0 {
            std::ptr::copy_nonoverlapping(data1, new_data, n1 * es);
        }
        if !data2.is_null() && n2 > 0 {
            std::ptr::copy_nonoverlapping(data2, new_data.add(n1 * es), n2 * es);
        }
    }

    // Propagate elem_dec_fn from either source (both same-typed)
    let src = if data1.is_null() { data2 } else { data1 };
    if !src.is_null() {
        unsafe {
            store_elem_dec_fn(new_data, load_elem_dec_fn_const(src));
            store_elem_count(new_data, total_len as i64);
        }
    }

    unsafe {
        out_ptr.cast::<i64>().write(total_len as i64);
        out_ptr.cast::<i64>().add(1).write(total_len as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Byte size of one `{i64 len, i64 cap, ptr data}` fat-pointer slot.
const FAT_PTR_SLOT_SIZE: usize = 24;
/// Byte offset of the `cap` field within a fat-pointer slot.
const FAT_PTR_CAP_OFFSET: usize = 8;
/// Byte offset of the `data` field within a fat-pointer slot.
const FAT_PTR_DATA_OFFSET: usize = 16;

/// Flatten one level of nested lists: `[[T]] -> [T]`.
///
/// Reads `outer_len` fat-pointer slots (`{i64 len, i64 cap, ptr data}`,
/// `FAT_PTR_SLOT_SIZE` bytes each) starting at `outer_data`, each describing
/// one inner list of `T` elements. Two-pass sum-then-copy: pass 1 sums each
/// inner list's `len` (an empty inner list contributes 0); pass 2 allocates
/// the summed-length output buffer once and byte-copies each inner list's
/// elements into it in order.
///
/// `inc_fn`, when non-null, is invoked once per copied element to increment
/// its RC children — the outer list and every inner list are borrowed (not
/// consumed), so the new output buffer is an additional reference to any
/// heap-backed element data.
///
/// The output buffer's `elem_dec_fn` is sourced from the first non-empty
/// inner list's own header (mirrors `ori_list_concat`), resolving through a
/// seamless-slice interior pointer to the original allocation first. When
/// every inner list is empty, the output is empty and carries no header
/// (never freed through a dec-fn call).
///
/// Writes `{total_len, total_len, new_data}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_list_flatten(
    outer_data: *const u8,
    outer_len: i64,
    inner_elem_size: i64,
    inner_elem_align: i64,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let es = inner_elem_size.max(1) as usize;
    let ea = inner_elem_align.max(1) as usize;
    let n_outer = outer_len.max(0) as usize;

    if outer_data.is_null() || n_outer == 0 {
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        return;
    }

    // Pass 1: sum each inner list's length (an empty inner list contributes 0).
    let mut total_len: usize = 0;
    for i in 0..n_outer {
        let slot = unsafe { outer_data.add(i * FAT_PTR_SLOT_SIZE) };
        let inner_len = unsafe { slot.cast::<i64>().read() }.max(0) as usize;
        total_len += inner_len;
    }

    if total_len == 0 {
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        return;
    }

    let total_bytes = total_len * es;
    let new_data = ori_rc_alloc(total_bytes, ea);

    // Pass 2: byte-copy each inner list's elements into the output buffer,
    // incrementing each copied element's RC children and sourcing the
    // output header's elem_dec_fn from the first non-empty inner list.
    let mut offset: usize = 0;
    let mut dec_fn: Option<extern "C" fn(*mut u8)> = None;
    for i in 0..n_outer {
        let slot = unsafe { outer_data.add(i * FAT_PTR_SLOT_SIZE) };
        let inner_len = unsafe { slot.cast::<i64>().read() }.max(0) as usize;
        if inner_len == 0 {
            continue;
        }
        let inner_cap = unsafe { slot.add(FAT_PTR_CAP_OFFSET).cast::<i64>().read() };
        let inner_data = unsafe { slot.add(FAT_PTR_DATA_OFFSET).cast::<*mut u8>().read() };
        if inner_data.is_null() {
            continue;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(inner_data, new_data.add(offset * es), inner_len * es);
        }
        inc_copied_elements(unsafe { new_data.add(offset * es) }, inner_len, es, inc_fn);
        if dec_fn.is_none() {
            let header_src = if is_slice_cap(inner_cap) {
                slice_original_data(inner_data, inner_cap)
            } else {
                inner_data
            };
            dec_fn = unsafe { load_elem_dec_fn_const(header_src) };
        }
        offset += inner_len;
    }

    unsafe {
        store_elem_dec_fn(new_data, dec_fn);
        store_elem_count(new_data, total_len as i64);
    }

    unsafe { write_list_output(out_ptr, total_len as i64, total_len as i64, new_data) };
}

/// Compare two lists of scalar elements for equality.
///
/// Both lists have layout `{len: i64, cap: i64, data: *mut u8}`.
/// Compares lengths first, then `memcmp` on the raw data buffers.
/// Only correct for scalar element types (int, float, bool, char, byte)
/// where byte-level comparison matches semantic equality.
#[no_mangle]
pub extern "C" fn ori_list_eq_scalar(a: *const u8, b: *const u8, elem_size: i64) -> bool {
    if a.is_null() || b.is_null() {
        return a.is_null() && b.is_null();
    }
    unsafe {
        let a_len = a.cast::<i64>().read();
        let b_len = b.cast::<i64>().read();
        if a_len != b_len {
            return false;
        }
        let a_data = a.add(16).cast::<*const u8>().read();
        let b_data = b.add(16).cast::<*const u8>().read();
        if a_data == b_data {
            return true; // Same buffer (shared via clone)
        }
        let byte_len = a_len as usize * elem_size.max(1) as usize;
        let a_slice = std::slice::from_raw_parts(a_data, byte_len);
        let b_slice = std::slice::from_raw_parts(b_data, byte_len);
        a_slice == b_slice
    }
}

/// Compare two lists element-wise using a caller-supplied equality function.
///
/// Both lists have layout `{len: i64, cap: i64, data: *mut u8}`.
/// Compares lengths first, then calls `elem_eq` on each pair of elements.
/// Required for non-scalar element types (str, nested collections, structs)
/// where byte-level comparison doesn't match semantic equality.
#[no_mangle]
pub extern "C" fn ori_list_eq_deep(
    a: *const u8,
    b: *const u8,
    elem_size: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> bool {
    if a.is_null() || b.is_null() {
        return a.is_null() && b.is_null();
    }
    unsafe {
        let a_len = a.cast::<i64>().read();
        let b_len = b.cast::<i64>().read();
        if a_len != b_len {
            return false;
        }
        let a_data = a.add(16).cast::<*const u8>().read();
        let b_data = b.add(16).cast::<*const u8>().read();
        if a_data == b_data {
            return true; // Same buffer (shared via clone)
        }
        let es = elem_size.max(1) as usize;
        for i in 0..a_len as usize {
            let a_elem = a_data.add(i * es);
            let b_elem = b_data.add(i * es);
            if !elem_eq(a_elem, b_elem) {
                return false;
            }
        }
        true
    }
}
