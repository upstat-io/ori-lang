//! Tests for seamless list slice operations.

use crate::rc::{
    ori_buffer_rc_dec, ori_list_rc_inc, ori_rc_alloc, ori_rc_count, ori_rc_data_size, ori_rc_free,
};
use crate::slice_encoding::{is_slice_cap, make_slice_cap, slice_byte_offset, slice_original_data};

use super::*;

/// Size of an i64 element (8 bytes).
const ELEM_SIZE: i64 = 8;

/// Helper: allocate an RC-managed buffer with `n` i64 elements and fill them.
fn alloc_list(values: &[i64]) -> (*mut u8, i64, i64) {
    let n = values.len();
    if n == 0 {
        return (std::ptr::null_mut(), 0, 0);
    }
    let data = ori_rc_alloc(n * ELEM_SIZE as usize, 8);
    for (i, &v) in values.iter().enumerate() {
        unsafe { data.cast::<i64>().add(i).write(v) };
    }
    (data, n as i64, n as i64)
}

/// Helper: read the i64 elements from a list result written to `out_ptr`.
fn read_result(out: &[u8; 24]) -> (i64, i64, *mut u8) {
    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let cap = unsafe { out.as_ptr().cast::<i64>().add(1).read() };
    let data = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };
    (len, cap, data)
}

/// Helper: read i64 elements from a data pointer.
fn read_elements(data: *const u8, count: usize) -> Vec<i64> {
    (0..count)
        .map(|i| unsafe { data.cast::<i64>().add(i).read() })
        .collect()
}

// ── ori_list_slice ──────────────────────────────────────────────────────

#[test]
fn slice_of_regular_list() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap), "result should be a slice");
    assert!(!s_data.is_null());

    // Elements should be [20, 30, 40]
    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![20, 30, 40]);

    // Slice data should point into the original buffer
    let expected_data = unsafe { data.add(ELEM_SIZE as usize) };
    assert_eq!(s_data, expected_data);

    // Byte offset should be start * elem_size
    assert_eq!(slice_byte_offset(s_cap), ELEM_SIZE as usize);

    // Original data recoverable
    assert_eq!(slice_original_data(s_data, s_cap), data);

    // RC should be 2 (original + slice)
    assert_eq!(ori_rc_count(data), 2);

    // Clean up: dec for slice reference, then free
    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_full_view() {
    // Slicing [0, len) creates a full view — still a slice (separate reference)
    let (data, len, cap) = alloc_list(&[1, 2, 3]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 0, len, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(s_data, data); // Same data pointer (offset 0)
    assert_eq!(slice_byte_offset(s_cap), 0); // Zero offset
    assert_eq!(ori_rc_count(data), 2);

    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![1, 2, 3]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_empty_range() {
    // start == end → empty result
    let (data, len, cap) = alloc_list(&[1, 2, 3]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 2, 2, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert_eq!(s_cap, 0);
    assert!(s_data.is_null());

    // RC unchanged (no slice created)
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_of_empty_list() {
    let mut out = [0u8; 24];

    ori_list_slice(
        std::ptr::null_mut(),
        0,
        0,
        0,
        0,
        ELEM_SIZE,
        out.as_mut_ptr(),
    );
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert_eq!(s_cap, 0);
    assert!(s_data.is_null());
}

#[test]
fn slice_of_slice_accumulates_offsets() {
    // Create [10, 20, 30, 40, 50], slice [1,4) → [20, 30, 40],
    // then slice [1,2) of that → [30]
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out1 = [0u8; 24];

    // First slice: [1, 4) → [20, 30, 40]
    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, out1.as_mut_ptr());
    let (s1_len, s1_cap, s1_data) = read_result(&out1);
    assert_eq!(s1_len, 3);
    assert!(is_slice_cap(s1_cap));
    assert_eq!(slice_byte_offset(s1_cap), ELEM_SIZE as usize);

    // Second slice: [1, 2) of the first slice → [30]
    let mut out2 = [0u8; 24];
    ori_list_slice(s1_data, s1_len, s1_cap, 1, 2, ELEM_SIZE, out2.as_mut_ptr());
    let (s2_len, s2_cap, s2_data) = read_result(&out2);

    assert_eq!(s2_len, 1);
    assert!(is_slice_cap(s2_cap));

    // Total offset: 1 * 8 (first) + 1 * 8 (second) = 16
    assert_eq!(slice_byte_offset(s2_cap), 2 * ELEM_SIZE as usize);

    // Should point to element [30] in the original buffer
    let val = unsafe { s2_data.cast::<i64>().read() };
    assert_eq!(val, 30);

    // Original data should be recoverable
    assert_eq!(slice_original_data(s2_data, s2_cap), data);

    // RC should be 3 (original + first slice + second slice)
    assert_eq!(ori_rc_count(data), 3);

    // Clean up
    crate::rc::ori_rc_dec(data, None); // first slice
    crate::rc::ori_rc_dec(data, None); // second slice
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_rc_lifecycle() {
    // Verify: create, inc from slice, dec from slice drop
    let (data, _, cap) = alloc_list(&[1, 2, 3, 4]);
    assert_eq!(ori_rc_count(data), 1);

    let mut out = [0u8; 24];
    ori_list_slice(data, 4, cap, 0, 2, ELEM_SIZE, out.as_mut_ptr());
    assert_eq!(ori_rc_count(data), 2); // Slice incremented RC

    // Simulating slice drop: dec RC on original
    crate::rc::ori_rc_dec(data, None);
    assert_eq!(ori_rc_count(data), 1); // Back to original only

    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_start_clamped_to_zero() {
    // Negative start should be treated as 0 (defensive clamping)
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, -5, 2, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 2);
    let elems = read_elements(s_data, 2);
    assert_eq!(elems, vec![10, 20]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_end_clamped_to_len() {
    // end > len should be clamped to len
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 100, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 2);
    let elems = read_elements(s_data, 2);
    assert_eq!(elems, vec![20, 30]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

// ── ori_list_slice_take ─────────────────────────────────────────────────

#[test]
fn take_first_n() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = [0u8; 24];

    ori_list_slice_take(data, len, cap, 3, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(slice_byte_offset(s_cap), 0); // Starts at offset 0
    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![10, 20, 30]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn take_more_than_len() {
    // Taking more than available → clamps to len
    let (data, len, cap) = alloc_list(&[10, 20]);
    let mut out = [0u8; 24];

    ori_list_slice_take(data, len, cap, 100, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 2);
    let elems = read_elements(s_data, 2);
    assert_eq!(elems, vec![10, 20]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 2 * ELEM_SIZE as usize, 8);
}

#[test]
fn take_zero() {
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = [0u8; 24];

    ori_list_slice_take(data, len, cap, 0, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert!(s_data.is_null());
    assert_eq!(ori_rc_count(data), 1); // No RC change

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

// ── ori_list_slice_drop ─────────────────────────────────────────────────

#[test]
fn drop_first_n() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = [0u8; 24];

    ori_list_slice_drop(data, len, cap, 2, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(slice_byte_offset(s_cap), 2 * ELEM_SIZE as usize);
    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![30, 40, 50]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn drop_all() {
    // Dropping all elements → empty result
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = [0u8; 24];

    ori_list_slice_drop(data, len, cap, 3, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert!(s_data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn drop_more_than_len() {
    // Dropping more than available → clamps, returns empty
    let (data, len, cap) = alloc_list(&[10, 20]);
    let mut out = [0u8; 24];

    ori_list_slice_drop(data, len, cap, 100, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert!(s_data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 2 * ELEM_SIZE as usize, 8);
}

// ── Multiple slice references ───────────────────────────────────────────

#[test]
fn multiple_slices_share_buffer() {
    // Multiple slices of the same list share the buffer
    let (data, len, cap) = alloc_list(&[1, 2, 3, 4, 5]);
    assert_eq!(ori_rc_count(data), 1);

    let mut out1 = [0u8; 24];
    let mut out2 = [0u8; 24];
    let mut out3 = [0u8; 24];

    ori_list_slice(data, len, cap, 0, 2, ELEM_SIZE, out1.as_mut_ptr());
    ori_list_slice(data, len, cap, 2, 4, ELEM_SIZE, out2.as_mut_ptr());
    ori_list_slice(data, len, cap, 4, 5, ELEM_SIZE, out3.as_mut_ptr());

    assert_eq!(ori_rc_count(data), 4); // original + 3 slices

    let e1 = read_elements(read_result(&out1).2, 2);
    let e2 = read_elements(read_result(&out2).2, 2);
    let e3 = read_elements(read_result(&out3).2, 1);

    assert_eq!(e1, vec![1, 2]);
    assert_eq!(e2, vec![3, 4]);
    assert_eq!(e3, vec![5]);

    // Clean up all slice references
    crate::rc::ori_rc_dec(data, None);
    crate::rc::ori_rc_dec(data, None);
    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

// ── COW on Slice Mutation (05.5) ─────────────────────────────────────

#[test]
fn cow_push_on_slice_materializes() {
    // Push to a slice → should materialize into an owned list
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(ori_rc_count(data), 2);

    // Push element 99 to the slice
    let new_elem: i64 = 99;
    let mut push_out = [0u8; 24];
    crate::list::cow::ori_list_push_cow(
        s_data,
        s_len,
        s_cap,
        (&raw const new_elem).cast(),
        ELEM_SIZE,
        8,
        None,
        push_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&push_out);
    assert_eq!(r_len, 4); // 3 slice elements + 1 pushed
    assert!(
        !is_slice_cap(r_cap),
        "result should be a regular list, not a slice"
    );
    assert!(!r_data.is_null());

    // Result should contain [20, 30, 40, 99]
    let elems = read_elements(r_data, 4);
    assert_eq!(elems, vec![20, 30, 40, 99]);

    // Original buffer should be unmodified
    let original_elems = read_elements(data, 5);
    assert_eq!(original_elems, vec![10, 20, 30, 40, 50]);

    // Original RC should be 1 (slice reference was consumed by push_cow)
    assert_eq!(ori_rc_count(data), 1);

    // Clean up
    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_pop_on_slice_materializes() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 0, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));

    // Pop from the slice
    let mut pop_out = [0u8; 24];
    crate::list::cow::ori_list_pop_cow(
        s_data,
        s_len,
        s_cap,
        ELEM_SIZE,
        8,
        None,
        pop_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&pop_out);
    assert_eq!(r_len, 2);
    assert!(!is_slice_cap(r_cap), "result should be owned");
    let elems = read_elements(r_data, 2);
    assert_eq!(elems, vec![10, 20]);

    // Original unmodified
    assert_eq!(read_elements(data, 4), vec![10, 20, 30, 40]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_set_on_slice_materializes() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);

    // Set index 1 (element 30) to 999
    let new_val: i64 = 999;
    let mut set_out = [0u8; 24];
    crate::list::cow::ori_list_set_cow(
        s_data,
        s_len,
        s_cap,
        1,
        (&raw const new_val).cast(),
        ELEM_SIZE,
        8,
        None,
        set_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&set_out);
    assert_eq!(r_len, 3);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 3);
    assert_eq!(elems, vec![20, 999, 40]);

    // Original unmodified
    assert_eq!(read_elements(data, 5), vec![10, 20, 30, 40, 50]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_len as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_insert_on_slice_materializes() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 2); // [20, 30]

    // Insert 55 at index 1
    let new_val: i64 = 55;
    let mut insert_out = [0u8; 24];
    crate::list::cow::ori_list_insert_cow(
        s_data,
        s_len,
        s_cap,
        1,
        (&raw const new_val).cast(),
        ELEM_SIZE,
        8,
        None,
        insert_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&insert_out);
    assert_eq!(r_len, 3);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 3);
    assert_eq!(elems, vec![20, 55, 30]);

    assert_eq!(read_elements(data, 4), vec![10, 20, 30, 40]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_remove_on_slice_materializes() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 3); // [20, 30, 40]

    // Remove index 1 (element 30)
    let mut remove_out = [0u8; 24];
    crate::list::cow::ori_list_remove_cow(
        s_data,
        s_len,
        s_cap,
        1,
        ELEM_SIZE,
        8,
        None,
        remove_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&remove_out);
    assert_eq!(r_len, 2);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 2);
    assert_eq!(elems, vec![20, 40]);

    assert_eq!(read_elements(data, 5), vec![10, 20, 30, 40, 50]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_len as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_reverse_on_slice_materializes() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);

    let mut rev_out = [0u8; 24];
    crate::list::cow_sort::ori_list_reverse_cow(
        s_data,
        s_len,
        s_cap,
        ELEM_SIZE,
        8,
        None,
        rev_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&rev_out);
    assert_eq!(r_len, 3);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 3);
    assert_eq!(elems, vec![40, 30, 20]);

    assert_eq!(read_elements(data, 5), vec![10, 20, 30, 40, 50]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_len as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_concat_with_slice_list1() {
    // Concatenate slice + regular list
    let (data1, len1, cap1) = alloc_list(&[10, 20, 30, 40]);
    let (data2, len2, cap2) = alloc_list(&[50, 60]);

    let mut slice_out = [0u8; 24];
    ori_list_slice(data1, len1, cap1, 1, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 2); // [20, 30]

    let mut concat_out = [0u8; 24];
    crate::list::cow_sort::ori_list_concat_cow(
        s_data,
        s_len,
        s_cap,
        data2,
        len2,
        cap2,
        ELEM_SIZE,
        8,
        None,
        concat_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&concat_out);
    assert_eq!(r_len, 4);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 4);
    assert_eq!(elems, vec![20, 30, 50, 60]);

    // Original buffer1 RC decremented (slice consumed)
    assert_eq!(ori_rc_count(data1), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data1, 4 * ELEM_SIZE as usize, 8);
    // data2 was consumed by concat (unique → moved)
}

#[test]
fn materialize_slice_produces_owned_list() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(ori_rc_count(data), 2);

    // Materialize
    let mut mat_out = [0u8; 24];
    crate::list::slice::ori_list_materialize_slice(
        s_data,
        s_len,
        s_cap,
        ELEM_SIZE,
        8,
        None,
        mat_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&mat_out);
    assert_eq!(r_len, 3);
    assert!(!is_slice_cap(r_cap), "materialized list should be owned");
    assert!(r_cap >= 3, "capacity should be >= len");

    let elems = read_elements(r_data, 3);
    assert_eq!(elems, vec![20, 30, 40]);

    // Original RC decremented (slice consumed by materialize)
    assert_eq!(ori_rc_count(data), 1);

    // New buffer has its own RC
    assert_eq!(ori_rc_count(r_data), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn materialize_non_slice_is_noop() {
    // Materialize on a regular list should return it unchanged
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut mat_out = [0u8; 24];

    crate::list::slice::ori_list_materialize_slice(
        data,
        len,
        cap,
        ELEM_SIZE,
        8,
        None,
        mat_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&mat_out);
    assert_eq!(r_len, len);
    assert_eq!(r_cap, cap);
    assert_eq!(r_data, data); // Same pointer — no copy

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_push_on_slice_rc_lifecycle() {
    // Verify RC lifecycle: original RC=2 after slice, RC=1 after push_cow
    let (data, len, cap) = alloc_list(&[1, 2, 3]);
    let mut slice_out = [0u8; 24];

    ori_list_slice(data, len, cap, 0, 2, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(ori_rc_count(data), 2);

    let val: i64 = 42;
    let mut push_out = [0u8; 24];
    crate::list::cow::ori_list_push_cow(
        s_data,
        s_len,
        s_cap,
        (&raw const val).cast(),
        ELEM_SIZE,
        8,
        None,
        push_out.as_mut_ptr(),
    );

    let (r_len, _, r_data) = read_result(&push_out);
    assert_eq!(r_len, 3);
    // Slice reference was consumed → original RC back to 1
    assert_eq!(ori_rc_count(data), 1);
    // New buffer has RC 1
    assert_eq!(ori_rc_count(r_data), 1);

    let (_, r_cap, _) = read_result(&push_out);
    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

// ── Slice-Aware RC (05.4) ───────────────────────────────────────────

#[test]
fn ori_list_rc_inc_on_regular_list() {
    let (data, _, cap) = alloc_list(&[10, 20, 30]);
    assert_eq!(ori_rc_count(data), 1);

    // ori_list_rc_inc on a regular list should inc the data pointer's RC
    ori_list_rc_inc(data, cap);
    assert_eq!(ori_rc_count(data), 2);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_list_rc_inc_on_slice() {
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 3, ELEM_SIZE, out.as_mut_ptr());
    let (_, s_cap, s_data) = read_result(&out);
    assert_eq!(ori_rc_count(data), 2); // original + slice

    // ori_list_rc_inc on a slice should inc the ORIGINAL buffer's RC
    ori_list_rc_inc(s_data, s_cap);
    assert_eq!(ori_rc_count(data), 3);

    // Clean up
    crate::rc::ori_rc_dec(data, None); // undo the manual inc
    crate::rc::ori_rc_dec(data, None); // slice reference
    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_list_rc_inc_null_is_noop() {
    // ori_list_rc_inc on null should be a no-op (no crash)
    ori_list_rc_inc(std::ptr::null_mut(), 0);
    ori_list_rc_inc(std::ptr::null_mut(), make_slice_cap(0));
}

#[test]
fn ori_buffer_rc_dec_on_slice_decs_original() {
    // Create list, slice it, then dec the slice via ori_buffer_rc_dec
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);
    assert_eq!(ori_rc_count(data), 2);

    // Dec the slice → should dec the ORIGINAL's RC
    ori_buffer_rc_dec(s_data, s_len, s_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 1); // Back to original only

    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_buffer_rc_dec_slice_last_ref_frees() {
    // Create list, slice it, drop original (dec), drop slice (last ref → free)
    let (data, len, cap) = alloc_list(&[100, 200, 300, 400]);
    let mut out = [0u8; 24];

    ori_list_slice(data, len, cap, 1, 3, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);
    assert_eq!(ori_rc_count(data), 2);

    // "Drop" the original list — dec RC but don't free (RC still > 0)
    crate::rc::ori_rc_dec(data, None);
    assert_eq!(ori_rc_count(data), 1); // Slice is now the sole reference

    // Now drop the slice — this should be the last reference.
    // ori_buffer_rc_dec should free the buffer using stored data_size.
    // (No crash = buffer freed correctly using data_size from header)
    ori_buffer_rc_dec(s_data, s_len, s_cap, ELEM_SIZE, None);
    // After free, accessing data is UB, but we verify no crash above.
}

#[test]
fn ori_buffer_rc_dec_slice_of_slice_decs_original() {
    // Slice-of-slice: both should target the original allocation's RC
    let (data, len, cap) = alloc_list(&[1, 2, 3, 4, 5, 6]);
    let mut out1 = [0u8; 24];
    let mut out2 = [0u8; 24];

    // First slice: [1, 5) → [2, 3, 4, 5]
    ori_list_slice(data, len, cap, 1, 5, ELEM_SIZE, out1.as_mut_ptr());
    let (s1_len, s1_cap, s1_data) = read_result(&out1);
    assert_eq!(ori_rc_count(data), 2);

    // Slice of slice: [1, 3) → [3, 4]
    ori_list_slice(s1_data, s1_len, s1_cap, 1, 3, ELEM_SIZE, out2.as_mut_ptr());
    let (s2_len, s2_cap, s2_data) = read_result(&out2);
    assert_eq!(ori_rc_count(data), 3);

    // Dec the second slice → should dec ORIGINAL's RC
    ori_buffer_rc_dec(s2_data, s2_len, s2_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 2);

    // Dec the first slice → should dec ORIGINAL's RC
    ori_buffer_rc_dec(s1_data, s1_len, s1_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 6 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_rc_data_size_returns_allocation_size() {
    // Verify that ori_rc_data_size reads the stored allocation size
    let size = 5 * ELEM_SIZE as usize; // 40 bytes
    let data = ori_rc_alloc(size, 8);
    assert!(!data.is_null());

    assert_eq!(ori_rc_data_size(data.cast_const()), size as i64);

    ori_rc_free(data, size, 8);
}

#[test]
fn ori_rc_data_size_null_returns_zero() {
    assert_eq!(ori_rc_data_size(std::ptr::null()), 0);
}

#[test]
fn slice_rc_full_lifecycle() {
    // Comprehensive lifecycle: create list → multiple slices → drop in order
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    assert_eq!(ori_rc_count(data), 1);

    // Create 3 slices
    let mut out_a = [0u8; 24];
    let mut out_b = [0u8; 24];
    let mut out_c = [0u8; 24];

    ori_list_slice(data, len, cap, 0, 2, ELEM_SIZE, out_a.as_mut_ptr());
    let (a_len, a_cap, a_data) = read_result(&out_a);
    assert_eq!(ori_rc_count(data), 2);

    ori_list_slice(data, len, cap, 2, 4, ELEM_SIZE, out_b.as_mut_ptr());
    let (b_len, b_cap, b_data) = read_result(&out_b);
    assert_eq!(ori_rc_count(data), 3);

    ori_list_slice(data, len, cap, 4, 5, ELEM_SIZE, out_c.as_mut_ptr());
    let (c_len, c_cap, c_data) = read_result(&out_c);
    assert_eq!(ori_rc_count(data), 4);

    // Drop slice A via ori_buffer_rc_dec
    ori_buffer_rc_dec(a_data, a_len, a_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 3);

    // Drop original via ori_buffer_rc_dec (non-slice path)
    ori_buffer_rc_dec(data, len, cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 2);

    // Drop slice B
    ori_buffer_rc_dec(b_data, b_len, b_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 1);

    // Drop slice C — last reference → frees buffer
    ori_buffer_rc_dec(c_data, c_len, c_cap, ELEM_SIZE, None);
    // No crash = success (buffer freed using stored data_size)
}
