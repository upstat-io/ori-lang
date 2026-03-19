//! Query and functional (non-COW) list operations.
//!
//! These functions do not take ownership of their inputs — they either
//! read from the list (queries) or produce a new list from immutable
//! references (functional operations).

use crate::io::ori_panic_cstr;
use crate::rc::ori_rc_alloc;
use crate::string::{deref_str, OriStr};

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
/// Writes `{tag, value}` to `out_ptr` where tag=1 (Some) with element
/// copied, or tag=0 (None). The value region is `elem_size` bytes.
#[no_mangle]
pub extern "C" fn ori_list_first(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    if data.is_null() || len <= 0 {
        // None: tag = 1 (Ori convention: 0=Some, 1=None)
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
        return;
    }
    // Some: tag = 0, copy first element
    unsafe {
        out_ptr.cast::<i64>().write(0);
        std::ptr::copy_nonoverlapping(data, out_ptr.add(8), es);
    }
}

/// Return the last element of a list, or write a None tag if empty.
///
/// Same layout as `ori_list_first`: `{tag: i64, value: [elem_size]}`.
/// Ori convention: tag 0=Some, 1=None.
#[no_mangle]
pub extern "C" fn ori_list_last(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
    if data.is_null() || len <= 0 {
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
        return;
    }
    let last_offset = (len as usize - 1) * es;
    unsafe {
        out_ptr.cast::<i64>().write(0);
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
pub extern "C" fn ori_list_reverse(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
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

    for i in 0..n {
        let src_offset = (n - 1 - i) * es;
        let dst_offset = i * es;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_offset), new_data.add(dst_offset), es);
        }
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
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let es = elem_size.max(1) as usize;
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
    let new_data = ori_rc_alloc(total_bytes, 8);

    unsafe {
        if !data1.is_null() && n1 > 0 {
            std::ptr::copy_nonoverlapping(data1, new_data, n1 * es);
        }
        if !data2.is_null() && n2 > 0 {
            std::ptr::copy_nonoverlapping(data2, new_data.add(n1 * es), n2 * es);
        }
        out_ptr.cast::<i64>().write(total_len as i64);
        out_ptr.cast::<i64>().add(1).write(total_len as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
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
