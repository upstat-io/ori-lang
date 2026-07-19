//! COW list concatenation.

use crate::list::{dec_list_buffer, inc_copied_elements, write_list_output};
use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_free, ori_rc_realloc};

use super::header::propagate_header;
use crate::list::cow_context::CowMode;

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

/// Dispose of list2's buffer after concat has consumed it.
///
/// A unique source moves its element ownership into the destination, so only
/// its allocation is freed. A shared source copies and increments its elements;
/// the canonical buffer decrement then balances those increments if this was
/// the final buffer owner. That path also handles seamless slices.
#[inline]
fn dispose_consumed_list2(
    data2: *mut u8,
    len2: i64,
    cap2: i64,
    es: usize,
    ea: usize,
    elements_moved: bool,
) {
    if data2.is_null() {
        return;
    }
    if elements_moved {
        ori_rc_free(data2, cap2.max(0) as usize * es, ea);
    } else {
        dec_list_buffer(data2, len2, cap2, es as i64);
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
    cow_mode: i32,
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
    let mode = CowMode::from_abi(cow_mode);
    let data2_unique = mode.allows_in_place(data2, cap2);

    // Empty concatenation
    if new_len == 0 {
        unsafe { write_list_output(out_ptr, 0, 0, std::ptr::null_mut()) };
        dec_list_buffer(data1, len1, cap1, elem_size);
        dec_list_buffer(data2, len2, cap2, elem_size);
        return;
    }

    // list2 is empty — return list1 unchanged, consume list2
    if n2 == 0 {
        unsafe { write_list_output(out_ptr, len1, cap1, data1) };
        dec_list_buffer(data2, len2, cap2, elem_size);
        return;
    }

    // list1 is empty
    if n1 == 0 || data1.is_null() {
        if data2_unique {
            // TAKEOVER: list2 is unique — transfer its buffer directly (O(1))
            unsafe { write_list_output(out_ptr, len2, cap2, data2) };
            dec_list_buffer(data1, len1, cap1, elem_size);
            return;
        }
        // list2 is shared or slice — copy into fresh buffer
        let new_cap = next_capacity(0, new_len);
        let new_data = ori_rc_alloc(new_cap * es, ea);
        copy_list2_elements(new_data, data2, n2, es, false, inc_fn);
        // Propagate header from list2 (source of elements)
        unsafe { propagate_header(data2, cap2, new_data, n2 as i64) };
        dec_list_buffer(data1, len1, cap1, elem_size);
        dec_list_buffer(data2, len2, cap2, elem_size);
        unsafe { write_list_output(out_ptr, n2 as i64, new_cap as i64, new_data) };
        return;
    }

    // FAST PATH: list1 unique, non-slice — reuse its buffer
    let data1_unique = mode.allows_in_place(data1, cap1);
    if data1_unique {
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
            dispose_consumed_list2(data2, len2, cap2, es, ea, data2_unique);
            unsafe { write_list_output(out_ptr, new_len as i64, cap1, data1) };
            return;
        }
        // Needs growth — realloc, then memcpy list2
        let new_cap = next_capacity(old_cap, new_len);
        let new_data = ori_rc_realloc(data1, old_cap * es, new_cap * es, ea);
        if new_data.is_null() {
            dispose_consumed_list2(data2, len2, cap2, es, ea, false);
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
        dispose_consumed_list2(data2, len2, cap2, es, ea, data2_unique);
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
    // Propagate header from either source (both same-typed)
    unsafe { propagate_header(data1, cap1, new_data, new_len as i64) };
    dec_list_buffer(data1, len1, cap1, elem_size);
    dispose_consumed_list2(data2, len2, cap2, es, ea, data2_unique);
    unsafe { write_list_output(out_ptr, new_len as i64, new_cap as i64, new_data) };
}
