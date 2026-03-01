//! Query and functional (non-COW) list operations.
//!
//! These functions do not take ownership of their inputs — they either
//! read from the list (queries) or produce a new list from immutable
//! references (functional operations).

use crate::rc::ori_rc_alloc;
use crate::string::{deref_str, OriStr};

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
