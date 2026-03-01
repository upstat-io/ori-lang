//! COW list concatenation, reverse, and sort operations.
//!
//! These are the "advanced" COW operations that either operate on two lists
//! (concat) or require auxiliary data structures (sort indices, swap buffer).

use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_dec, ori_rc_free, ori_rc_is_unique, ori_rc_realloc};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

use super::{dec_list_buffer, inc_copied_elements, write_list_output};

/// Helper: copy list2's RC'd elements, incrementing child RC.
fn copy_list2_elements(
    dst: *mut u8,
    data2: *const u8,
    n2: usize,
    es: usize,
    list2_unique: bool,
    inc_fn: Option<extern "C" fn(*mut u8)>,
) {
    if !data2.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(data2, dst, n2 * es) };
        if !list2_unique {
            inc_copied_elements(dst, n2, es, inc_fn);
        }
    }
}

/// Dispose of list2's buffer after its elements have been copied out.
///
/// Elements have already been moved (unique) or copied+inc'd (shared), so
/// no child cleanup is needed. We just need to either free the buffer
/// (last reference) or decrement the RC (other references remain).
///
/// For slices, decs the original buffer's RC instead.
///
/// Uses `ori_rc_is_unique` at disposal time (not the initial snapshot) to
/// handle self-concat (`x + x`) where a prior dec on data1 (same buffer)
/// may have reduced the RC.
#[inline]
fn dec_consumed_list2(data2: *mut u8, cap2: i64, es: usize, ea: usize) {
    if data2.is_null() {
        return;
    }
    if is_slice_cap(cap2) {
        let original = slice_original_data(data2, cap2);
        ori_rc_dec(original, None);
        return;
    }
    let alloc_size = cap2.max(0) as usize * es;
    if ori_rc_is_unique(data2) {
        // Last reference — free the buffer directly.
        // No drop_fn needed: elements already moved/inc'd by the caller.
        ori_rc_free(data2, alloc_size, ea);
    } else {
        // Other references remain — just decrement.
        ori_rc_dec(data2, None);
    }
}

/// COW-aware list concatenation with dual-consuming semantics.
///
/// Both `list1` and `list2` are **consumed** (ownership transferred). The
/// runtime checks uniqueness of each buffer at runtime to select the optimal
/// strategy:
///
/// | list1   | list2   | Strategy                                        |
/// |---------|---------|-------------------------------------------------|
/// | unique  | unique  | Reuse list1 buffer, **move** list2 (no inc)     |
/// | unique  | shared  | Reuse list1 buffer, **copy** list2 (inc each)   |
/// | shared  | unique  | New buffer, copy list1 (inc), **move** list2    |
/// | shared  | shared  | New buffer, copy both (inc all)                 |
///
/// **Bonus**: list1 empty + list2 unique → takeover list2's buffer (O(1)).
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_list_concat_cow(
    data1: *mut u8,
    len1: i64,
    cap1: i64,
    data2: *mut u8,
    len2: i64,
    cap2: i64,
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
    let n1 = len1.max(0) as usize;
    let n2 = len2.max(0) as usize;
    let new_len = n1 + n2;
    // Slices are never "unique" — their data is interior to another buffer.
    let data2_unique = !is_slice_cap(cap2) && ori_rc_is_unique(data2);

    // Empty concatenation
    if new_len == 0 {
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        dec_list_buffer(data1, cap1);
        dec_list_buffer(data2, cap2);
        return;
    }

    // list2 is empty — return list1 unchanged, consume list2
    if n2 == 0 {
        unsafe { write_list_output(out_ptr, len1, cap1, data1) };
        dec_list_buffer(data2, cap2);
        return;
    }

    // list1 is empty
    if n1 == 0 || data1.is_null() {
        if data2_unique {
            // TAKEOVER: list2 is unique — transfer its buffer directly (O(1))
            unsafe { write_list_output(out_ptr, len2, cap2, data2) };
            dec_list_buffer(data1, cap1);
            return;
        }
        // list2 is shared or slice — copy into fresh buffer
        let new_cap = next_capacity(0, new_len);
        let new_data = ori_rc_alloc(new_cap * es, ea);
        copy_list2_elements(new_data, data2, n2, es, false, inc_fn);
        dec_list_buffer(data1, cap1);
        dec_list_buffer(data2, cap2);
        unsafe { write_list_output(out_ptr, n2 as i64, new_cap as i64, new_data) };
        return;
    }

    // FAST PATH: list1 unique, non-slice — reuse its buffer
    if !is_slice_cap(cap1) && ori_rc_is_unique(data1) {
        let old_cap = cap1.max(0) as usize;
        if old_cap >= new_len {
            // Has capacity — memcpy list2 elements after list1
            copy_list2_elements(
                unsafe { data1.add(n1 * es) },
                data2,
                n2,
                es,
                data2_unique,
                inc_fn,
            );
            dec_consumed_list2(data2, cap2, es, ea);
            unsafe { write_list_output(out_ptr, new_len as i64, cap1, data1) };
            return;
        }
        // Needs growth — realloc, then memcpy list2
        let new_cap = next_capacity(old_cap, new_len);
        let new_data = ori_rc_realloc(data1, old_cap * es, new_cap * es, ea);
        if new_data.is_null() {
            dec_consumed_list2(data2, cap2, es, ea);
            unsafe { write_list_output(out_ptr, len1, cap1, data1) };
            return;
        }
        copy_list2_elements(
            unsafe { new_data.add(n1 * es) },
            data2,
            n2,
            es,
            data2_unique,
            inc_fn,
        );
        dec_consumed_list2(data2, cap2, es, ea);
        unsafe { write_list_output(out_ptr, new_len as i64, new_cap as i64, new_data) };
        return;
    }

    // SLOW PATH: list1 shared or slice — allocate new buffer, copy both
    let new_cap = next_capacity(0, new_len);
    let new_data = ori_rc_alloc(new_cap * es, ea);
    unsafe { std::ptr::copy_nonoverlapping(data1, new_data, n1 * es) };
    copy_list2_elements(
        new_data.wrapping_add(n1 * es),
        data2,
        n2,
        es,
        data2_unique,
        inc_fn,
    );
    inc_copied_elements(new_data, n1, es, inc_fn);
    dec_list_buffer(data1, cap1);
    dec_consumed_list2(data2, cap2, es, ea);
    unsafe { write_list_output(out_ptr, new_len as i64, new_cap as i64, new_data) };
}

/// COW-aware list reverse with consuming semantics.
///
/// Reverses the list in place if uniquely owned (O(n), no allocation).
/// If shared, allocates a new buffer and copies in reverse order (O(n)).
///
/// # Element RC
///
/// No element RC changes needed — elements are just rearranged (unique)
/// or byte-copied in reverse order (shared, codegen handles RC inc).
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_list_reverse_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
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
    let n = len.max(0) as usize;

    // Empty or single-element — return unchanged
    if data.is_null() || n <= 1 {
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // FAST PATH: unique owner, non-slice — swap in place
    if !is_slice_cap(cap) && ori_rc_is_unique(data) {
        // Swap pairs from both ends working inward
        let mut tmp = vec![0u8; es];
        let mut lo = 0usize;
        let mut hi = n - 1;
        while lo < hi {
            unsafe {
                let lo_ptr = data.add(lo * es);
                let hi_ptr = data.add(hi * es);
                // tmp = data[lo]
                std::ptr::copy_nonoverlapping(lo_ptr, tmp.as_mut_ptr(), es);
                // data[lo] = data[hi]
                std::ptr::copy_nonoverlapping(hi_ptr, lo_ptr, es);
                // data[hi] = tmp
                std::ptr::copy_nonoverlapping(tmp.as_ptr(), hi_ptr, es);
            }
            lo += 1;
            hi -= 1;
        }
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // SLOW PATH: shared — allocate new, copy in reverse order
    let new_data = ori_rc_alloc(n * es, ea);

    for i in 0..n {
        let src_offset = (n - 1 - i) * es;
        let dst_offset = i * es;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_offset), new_data.add(dst_offset), es);
        }
    }

    // Inc RC for all copied elements
    inc_copied_elements(new_data, n, es, inc_fn);

    // Release old buffer (slice-aware)
    dec_list_buffer(data, cap);

    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64); // cap = len (tight)
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
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
    out_ptr: *mut u8,
) {
    list_sort_cow_impl(
        data, len, cap, elem_size, elem_align, compare_fn, inc_fn, out_ptr, false,
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
    out_ptr: *mut u8,
) {
    list_sort_cow_impl(
        data, len, cap, elem_size, elem_align, compare_fn, inc_fn, out_ptr, true,
    );
}

/// Shared implementation for COW list sort (unstable and stable variants).
#[expect(
    clippy::too_many_arguments,
    reason = "C FFI parameters from two sort entry points"
)]
fn list_sort_cow_impl(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem_size: i64,
    elem_align: i64,
    compare_fn: extern "C" fn(*const u8, *const u8) -> i32,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
    stable: bool,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;

    // Empty or single-element — already sorted
    if data.is_null() || n <= 1 {
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // Build sorted index array — works for both paths
    let mut indices: Vec<usize> = (0..n).collect();
    let cmp = |&a: &usize, &b: &usize| {
        let c = compare_fn(unsafe { data.add(a * es) }, unsafe { data.add(b * es) });
        c.cmp(&0)
    };
    if stable {
        indices.sort_by(cmp);
    } else {
        indices.sort_unstable_by(cmp);
    }

    // FAST PATH: unique owner, non-slice — permute in place
    if !is_slice_cap(cap) && ori_rc_is_unique(data) {
        apply_permutation_in_place(data, &indices, es);
        unsafe {
            out_ptr.cast::<i64>().write(len);
            out_ptr.cast::<i64>().add(1).write(cap);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // SLOW PATH: shared — copy in sorted order to new buffer
    let new_data = ori_rc_alloc(n * es, ea);

    for (dst_idx, &src_idx) in indices.iter().enumerate() {
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(src_idx * es), new_data.add(dst_idx * es), es);
        }
    }

    // Inc RC for all copied elements
    inc_copied_elements(new_data, n, es, inc_fn);

    // Release old buffer (slice-aware)
    dec_list_buffer(data, cap);

    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64); // cap = len (tight)
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Apply a permutation in place using cycle-following.
///
/// Given `indices[i] = j`, moves the element originally at position `j`
/// to position `i`. Uses `O(elem_size)` temporary space (one element buffer).
fn apply_permutation_in_place(data: *mut u8, indices: &[usize], elem_size: usize) {
    let n = indices.len();
    let mut placed = vec![false; n];
    let mut tmp = vec![0u8; elem_size];

    for start in 0..n {
        if placed[start] || indices[start] == start {
            placed[start] = true;
            continue;
        }

        // Follow the cycle starting at `start`
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(start * elem_size), tmp.as_mut_ptr(), elem_size);
        }

        let mut current = start;
        loop {
            let next = indices[current];
            placed[current] = true;

            if next == start {
                // Close the cycle — write tmp to current position
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        tmp.as_ptr(),
                        data.add(current * elem_size),
                        elem_size,
                    );
                }
                break;
            }

            // Move element from `next` to `current`
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
