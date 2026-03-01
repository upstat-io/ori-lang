//! Set operations for AOT-compiled Ori programs.
//!
//! Sets use the same boxed layout as lists (contiguous array of elements).
//! Elements are compared with memcmp (byte-by-byte equality).
//!
//! # Submodules
//!
//! - `cow` — COW mutation functions (`ori_set_insert_cow`, etc.) with consuming
//!   semantics: fast path mutates in place when RC==1, slow path copies.

pub mod cow;

use crate::list::write_array_to_list;

/// Return an empty set sentinel (no allocation).
///
/// Sets use the same boxed layout as lists — null is the empty sentinel.
#[no_mangle]
pub extern "C" fn ori_set_empty() -> *mut u8 {
    std::ptr::null_mut()
}

/// Check if a set contains an element (memcmp-based).
///
/// Elements are stored as a contiguous array. Scans linearly, comparing
/// `elem_size` bytes at each position. Works for all fixed-representation
/// types (int, float, bool, byte, char, Duration, Size).
/// Returns 1 if found, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_set_contains(
    data: *const u8,
    len: i64,
    needle: *const u8,
    elem_size: i64,
) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() || elem_size <= 0 {
        return 0;
    }
    let es = elem_size as usize;
    let n = len as usize;
    for i in 0..n {
        let elem = unsafe { data.add(i * es) };
        if unsafe { raw_bytes_eq(elem, needle, es) } {
            return 1;
        }
    }
    0
}

/// Insert an element into a set, returning a new set via sret.
///
/// If the element already exists (memcmp), returns a copy of the original set.
/// Otherwise appends the element. Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_insert(
    data: *const u8,
    len: i64,
    elem: *const u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem.is_null() || elem_size <= 0 {
        return;
    }
    let es = elem_size as usize;
    let n = len.max(0) as usize;

    // Check if element already exists
    if !data.is_null() && n > 0 {
        for i in 0..n {
            let existing = unsafe { data.add(i * es) };
            if unsafe { raw_bytes_eq(existing, elem, es) } {
                // Already present — return copy of original
                write_set_copy(data, n, es, out_ptr);
                return;
            }
        }
    }

    // Not found — append
    let new_len = n + 1;
    let new_data = crate::rc::ori_rc_alloc(new_len * es, 8);
    if !data.is_null() && n > 0 {
        unsafe { std::ptr::copy_nonoverlapping(data, new_data, n * es) };
    }
    unsafe { std::ptr::copy_nonoverlapping(elem, new_data.add(n * es), es) };
    write_set_struct(out_ptr, new_len as i64, new_data);
}

/// Remove an element from a set, returning a new set via sret.
///
/// If the element is not found, returns a copy of the original set.
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_remove(
    data: *const u8,
    len: i64,
    needle: *const u8,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_size <= 0 {
        return;
    }
    let es = elem_size as usize;
    let n = len.max(0) as usize;

    if data.is_null() || n == 0 {
        write_set_struct(out_ptr, 0, std::ptr::null_mut());
        return;
    }

    // Find element to remove
    let mut remove_idx: Option<usize> = None;
    if !needle.is_null() {
        for i in 0..n {
            let existing = unsafe { data.add(i * es) };
            if unsafe { raw_bytes_eq(existing, needle, es) } {
                remove_idx = Some(i);
                break;
            }
        }
    }

    let Some(idx) = remove_idx else {
        // Not found — return copy
        write_set_copy(data, n, es, out_ptr);
        return;
    };

    let new_len = n - 1;
    if new_len == 0 {
        write_set_struct(out_ptr, 0, std::ptr::null_mut());
        return;
    }

    let new_data = crate::rc::ori_rc_alloc(new_len * es, 8);
    // Copy elements before idx
    if idx > 0 {
        unsafe { std::ptr::copy_nonoverlapping(data, new_data, idx * es) };
    }
    // Copy elements after idx
    let after = n - idx - 1;
    if after > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.add((idx + 1) * es),
                new_data.add(idx * es),
                after * es,
            );
        }
    }
    write_set_struct(out_ptr, new_len as i64, new_data);
}

/// Compute the union of two sets, returning a new set via sret.
///
/// Starts with a copy of set1, then appends elements from set2 not already present.
#[no_mangle]
pub extern "C" fn ori_set_union(
    d1: *const u8,
    l1: i64,
    d2: *const u8,
    l2: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_size <= 0 {
        return;
    }
    let es = elem_size as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;

    if n1 == 0 && n2 == 0 {
        write_set_struct(out_ptr, 0, std::ptr::null_mut());
        return;
    }
    if n2 == 0 || d2.is_null() {
        write_set_copy(d1, n1, es, out_ptr);
        return;
    }
    if n1 == 0 || d1.is_null() {
        write_set_copy(d2, n2, es, out_ptr);
        return;
    }

    // Collect: start with all of set1, add unique elements from set2
    let max_len = n1 + n2;
    let buf = crate::rc::ori_rc_alloc(max_len * es, 8);
    unsafe { std::ptr::copy_nonoverlapping(d1, buf, n1 * es) };
    let mut result_len = n1;

    for i in 0..n2 {
        let elem = unsafe { d2.add(i * es) };
        if !set_raw_contains(buf, result_len, elem, es) {
            unsafe { std::ptr::copy_nonoverlapping(elem, buf.add(result_len * es), es) };
            result_len += 1;
        }
    }

    write_set_struct(out_ptr, result_len as i64, buf);
}

/// Compute the intersection of two sets, returning a new set via sret.
///
/// Returns elements present in both sets.
#[no_mangle]
pub extern "C" fn ori_set_intersection(
    d1: *const u8,
    l1: i64,
    d2: *const u8,
    l2: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_size <= 0 {
        return;
    }
    let es = elem_size as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;

    if n1 == 0 || n2 == 0 || d1.is_null() || d2.is_null() {
        write_set_struct(out_ptr, 0, std::ptr::null_mut());
        return;
    }

    let buf = crate::rc::ori_rc_alloc(n1.min(n2) * es, 8);
    let mut result_len = 0;

    for i in 0..n1 {
        let elem = unsafe { d1.add(i * es) };
        if set_raw_contains(d2, n2, elem, es) {
            unsafe { std::ptr::copy_nonoverlapping(elem, buf.add(result_len * es), es) };
            result_len += 1;
        }
    }

    write_set_struct(out_ptr, result_len as i64, buf);
}

/// Compute the difference of two sets (set1 - set2), returning a new set via sret.
///
/// Returns elements in set1 that are NOT in set2.
#[no_mangle]
pub extern "C" fn ori_set_difference(
    d1: *const u8,
    l1: i64,
    d2: *const u8,
    l2: i64,
    elem_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem_size <= 0 {
        return;
    }
    let es = elem_size as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;

    if n1 == 0 || d1.is_null() {
        write_set_struct(out_ptr, 0, std::ptr::null_mut());
        return;
    }
    if n2 == 0 || d2.is_null() {
        write_set_copy(d1, n1, es, out_ptr);
        return;
    }

    let buf = crate::rc::ori_rc_alloc(n1 * es, 8);
    let mut result_len = 0;

    for i in 0..n1 {
        let elem = unsafe { d1.add(i * es) };
        if !set_raw_contains(d2, n2, elem, es) {
            unsafe { std::ptr::copy_nonoverlapping(elem, buf.add(result_len * es), es) };
            result_len += 1;
        }
    }

    write_set_struct(out_ptr, result_len as i64, buf);
}

/// Convert a set to a list (same layout — just copies the data).
#[no_mangle]
pub extern "C" fn ori_set_to_list(data: *const u8, len: i64, elem_size: i64, out_ptr: *mut u8) {
    write_array_to_list(data, len, elem_size, out_ptr);
}

/// Internal helper: check if a raw element exists in a data array.
fn set_raw_contains(data: *const u8, len: usize, needle: *const u8, elem_size: usize) -> bool {
    for i in 0..len {
        let existing = unsafe { data.add(i * elem_size) };
        if unsafe { raw_bytes_eq(existing, needle, elem_size) } {
            return true;
        }
    }
    false
}

/// Byte-by-byte equality comparison (no libc dependency).
///
/// # Safety
/// Both `a` and `b` must be valid for `len` bytes.
unsafe fn raw_bytes_eq(a: *const u8, b: *const u8, len: usize) -> bool {
    let a_slice = std::slice::from_raw_parts(a, len);
    let b_slice = std::slice::from_raw_parts(b, len);
    a_slice == b_slice
}

/// Write a set struct `{i64 len, i64 cap, ptr data}` to `out_ptr`.
pub(crate) fn write_set_struct(out_ptr: *mut u8, len: i64, data: *mut u8) {
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(len); // cap = len
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

/// Copy a set's data buffer and write the result struct.
fn write_set_copy(data: *const u8, len: usize, elem_size: usize, out_ptr: *mut u8) {
    if len == 0 || data.is_null() {
        write_set_struct(out_ptr, 0, std::ptr::null_mut());
        return;
    }
    let total = len * elem_size;
    let new_data = crate::rc::ori_rc_alloc(total, 8);
    unsafe { std::ptr::copy_nonoverlapping(data, new_data, total) };
    write_set_struct(out_ptr, len as i64, new_data);
}
