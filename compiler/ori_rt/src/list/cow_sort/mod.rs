//! COW list concatenation, reverse, and sort operations.
//! Advanced COW ops: two-list (concat) or auxiliary data structures (sort, swap).

mod sort;

pub use sort::*;

use crate::next_capacity;
use crate::rc::{
    load_elem_dec_fn, ori_rc_alloc, ori_rc_dec, ori_rc_free, ori_rc_is_unique, ori_rc_realloc,
    store_elem_count, store_elem_dec_fn,
};
use crate::slice_encoding::{is_slice_cap, slice_original_data};

use super::{dec_list_buffer, inc_copied_elements, write_list_output};

/// Maximum element size for stack-allocated swap buffers (covers int, float, str).
const STACK_MAX: usize = 24;

// Propagate elem_dec_fn and elem_count from old to new buffer header.
// When `src` is a slice interior pointer, resolves to the original
// allocation's data pointer before reading the header.
unsafe fn propagate_header(src: *mut u8, src_cap: i64, dst: *mut u8, count: i64) {
    let header_src = if is_slice_cap(src_cap) {
        slice_original_data(src, src_cap)
    } else {
        src
    };
    store_elem_dec_fn(dst, load_elem_dec_fn(header_src));
    store_elem_count(dst, count);
}

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
    // Slices are never "unique" — their data is interior to another buffer.
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let data2_unique =
        !is_slice_cap(cap2) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data2)));

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
        // Propagate header from list2 (source of elements)
        unsafe { propagate_header(data2, cap2, new_data, n2 as i64) };
        dec_list_buffer(data1, cap1);
        dec_list_buffer(data2, cap2);
        unsafe { write_list_output(out_ptr, n2 as i64, new_cap as i64, new_data) };
        return;
    }

    // FAST PATH: list1 unique, non-slice — reuse its buffer
    let data1_unique =
        !is_slice_cap(cap1) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data1)));
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
    // Propagate header from either source (both same-typed)
    unsafe { propagate_header(data1, cap1, new_data, new_len as i64) };
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
    cow_mode: i32,
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
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique =
        !is_slice_cap(cap) && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        // Swap pairs from both ends working inward.
        // Stack buffer for common element sizes (8, 16, 24 bytes),
        // heap fallback for larger elements.
        let mut stack_buf = [0u8; STACK_MAX];
        let mut heap_buf = Vec::new();
        let tmp: &mut [u8] = if es <= STACK_MAX {
            &mut stack_buf[..es]
        } else {
            heap_buf.resize(es, 0);
            &mut heap_buf
        };
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

    // Propagate header from source
    unsafe { propagate_header(data, cap, new_data, n as i64) };

    // Release old buffer (slice-aware)
    dec_list_buffer(data, cap);

    unsafe {
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64); // cap = len (tight)
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}
