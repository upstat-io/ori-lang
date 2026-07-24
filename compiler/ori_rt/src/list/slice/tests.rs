use crate::rc::{
    ori_buffer_rc_dec, ori_list_rc_inc, ori_rc_alloc, ori_rc_count, ori_rc_data_size, ori_rc_free,
    ori_rc_live_count,
};
use crate::slice_encoding::{is_slice_cap, make_slice_cap, slice_byte_offset, slice_original_data};

use super::*;

const ELEM_SIZE: i64 = 8;

#[derive(Default)]
#[repr(align(8))]
struct OutputBytes([u8; 24]);

impl OutputBytes {
    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
}

fn alloc_list(values: &[i64]) -> (*mut u8, i64, i64) {
    let n = values.len();
    if n == 0 {
        return (std::ptr::null_mut(), 0, 0);
    }
    let data = ori_rc_alloc(n * ELEM_SIZE as usize, 8);
    for (i, &v) in values.iter().enumerate() {
        // SAFETY: The allocation holds `n` aligned `i64` elements, and `i < n`.
        unsafe { data.cast::<i64>().add(i).write(v) };
    }
    (data, n as i64, n as i64)
}

fn read_result(out: &OutputBytes) -> (i64, i64, *mut u8) {
    // SAFETY:
    // - `OutputBytes` provides the alignment and size required by the list ABI.
    // - Each caller initializes the entire `{ len, cap, data }` result before reading it.
    unsafe {
        let len = out.as_ptr().cast::<i64>().read();
        let cap = out.as_ptr().cast::<i64>().add(1).read();
        let data = out.as_ptr().add(16).cast::<*mut u8>().read();
        (len, cap, data)
    }
}

fn read_elements(data: *const u8, count: usize) -> Vec<i64> {
    (0..count)
        .map(|i| {
            // SAFETY: Test callers supply an aligned list buffer containing `count` i64 values.
            unsafe { data.cast::<i64>().add(i).read() }
        })
        .collect()
}

#[test]
fn slice_of_regular_list() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap), "result should be a slice");
    assert!(!s_data.is_null());

    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![20, 30, 40]);

    // SAFETY: `data` holds five elements, so the one-element offset is in bounds.
    let expected_data = unsafe { data.add(ELEM_SIZE as usize) };
    assert_eq!(s_data, expected_data);

    assert_eq!(slice_byte_offset(s_cap), ELEM_SIZE as usize);

    assert_eq!(slice_original_data(s_data, s_cap), data);

    assert_eq!(ori_rc_count(data), 2);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_full_view() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[1, 2, 3]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 0, len, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(s_data, data);
    assert_eq!(slice_byte_offset(s_cap), 0);
    assert_eq!(ori_rc_count(data), 2);

    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![1, 2, 3]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_empty_range() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[1, 2, 3]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 2, 2, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert_eq!(s_cap, 0);
    assert!(s_data.is_null());

    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_of_empty_list() {
    let mut out = OutputBytes::default();

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
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out1 = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, out1.as_mut_ptr());
    let (s1_len, s1_cap, s1_data) = read_result(&out1);
    assert_eq!(s1_len, 3);
    assert!(is_slice_cap(s1_cap));
    assert_eq!(slice_byte_offset(s1_cap), ELEM_SIZE as usize);

    let mut out2 = OutputBytes::default();
    ori_list_slice(s1_data, s1_len, s1_cap, 1, 2, ELEM_SIZE, out2.as_mut_ptr());
    let (s2_len, s2_cap, s2_data) = read_result(&out2);

    assert_eq!(s2_len, 1);
    assert!(is_slice_cap(s2_cap));

    assert_eq!(slice_byte_offset(s2_cap), 2 * ELEM_SIZE as usize);

    // SAFETY: The asserted one-element slice contains one initialized i64.
    let val = unsafe { s2_data.cast::<i64>().read() };
    assert_eq!(val, 30);

    assert_eq!(slice_original_data(s2_data, s2_cap), data);

    assert_eq!(ori_rc_count(data), 3);

    crate::rc::ori_rc_dec(data, None);
    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_rc_lifecycle() {
    let _g = crate::test_support::lock_rc();
    let (data, _, cap) = alloc_list(&[1, 2, 3, 4]);
    assert_eq!(ori_rc_count(data), 1);

    let mut out = OutputBytes::default();
    ori_list_slice(data, 4, cap, 0, 2, ELEM_SIZE, out.as_mut_ptr());
    assert_eq!(ori_rc_count(data), 2);

    crate::rc::ori_rc_dec(data, None);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn slice_start_clamped_to_zero() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = OutputBytes::default();

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
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 100, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 2);
    let elems = read_elements(s_data, 2);
    assert_eq!(elems, vec![20, 30]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn take_first_n() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = OutputBytes::default();

    ori_list_slice_take(data, len, cap, 3, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);

    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(slice_byte_offset(s_cap), 0);
    let elems = read_elements(s_data, 3);
    assert_eq!(elems, vec![10, 20, 30]);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn take_more_than_len() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20]);
    let mut out = OutputBytes::default();

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
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = OutputBytes::default();

    ori_list_slice_take(data, len, cap, 0, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert!(s_data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn drop_first_n() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = OutputBytes::default();

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
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut out = OutputBytes::default();

    ori_list_slice_drop(data, len, cap, 3, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert!(s_data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn drop_more_than_len() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20]);
    let mut out = OutputBytes::default();

    ori_list_slice_drop(data, len, cap, 100, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, _, s_data) = read_result(&out);

    assert_eq!(s_len, 0);
    assert!(s_data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 2 * ELEM_SIZE as usize, 8);
}

#[test]
fn multiple_slices_share_buffer() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[1, 2, 3, 4, 5]);
    assert_eq!(ori_rc_count(data), 1);

    let mut out1 = OutputBytes::default();
    let mut out2 = OutputBytes::default();
    let mut out3 = OutputBytes::default();

    ori_list_slice(data, len, cap, 0, 2, ELEM_SIZE, out1.as_mut_ptr());
    ori_list_slice(data, len, cap, 2, 4, ELEM_SIZE, out2.as_mut_ptr());
    ori_list_slice(data, len, cap, 4, 5, ELEM_SIZE, out3.as_mut_ptr());

    assert_eq!(ori_rc_count(data), 4);

    let e1 = read_elements(read_result(&out1).2, 2);
    let e2 = read_elements(read_result(&out2).2, 2);
    let e3 = read_elements(read_result(&out3).2, 1);

    assert_eq!(e1, vec![1, 2]);
    assert_eq!(e2, vec![3, 4]);
    assert_eq!(e3, vec![5]);

    crate::rc::ori_rc_dec(data, None);
    crate::rc::ori_rc_dec(data, None);
    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_push_on_slice_materializes() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));
    assert_eq!(ori_rc_count(data), 2);

    let new_elem: i64 = 99;
    let mut push_out = OutputBytes::default();
    crate::list::cow::ori_list_push_cow(
        s_data,
        s_len,
        s_cap,
        (&raw const new_elem).cast(),
        ELEM_SIZE,
        8,
        None,
        0,
        push_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&push_out);
    assert_eq!(r_len, 4);
    assert!(
        !is_slice_cap(r_cap),
        "result should be a regular list, not a slice"
    );
    assert!(!r_data.is_null());

    let elems = read_elements(r_data, 4);
    assert_eq!(elems, vec![20, 30, 40, 99]);

    let original_elems = read_elements(data, 5);
    assert_eq!(original_elems, vec![10, 20, 30, 40, 50]);

    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_pop_on_slice_materializes() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 0, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 3);
    assert!(is_slice_cap(s_cap));

    let mut pop_out = OutputBytes::default();
    crate::list::cow::ori_list_pop_cow(
        s_data,
        s_len,
        s_cap,
        ELEM_SIZE,
        8,
        None,
        0,
        pop_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&pop_out);
    assert_eq!(r_len, 2);
    assert!(!is_slice_cap(r_cap), "result should be owned");
    let elems = read_elements(r_data, 2);
    assert_eq!(elems, vec![10, 20]);

    assert_eq!(read_elements(data, 4), vec![10, 20, 30, 40]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_set_on_slice_materializes() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);

    let new_val: i64 = 999;
    let mut set_out = OutputBytes::default();
    crate::list::cow::ori_list_set_cow(
        s_data,
        s_len,
        s_cap,
        1,
        (&raw const new_val).cast(),
        ELEM_SIZE,
        8,
        None,
        0,
        set_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&set_out);
    assert_eq!(r_len, 3);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 3);
    assert_eq!(elems, vec![20, 999, 40]);

    assert_eq!(read_elements(data, 5), vec![10, 20, 30, 40, 50]);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(r_data, r_len as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_insert_on_slice_materializes() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 2);

    let new_val: i64 = 55;
    let mut insert_out = OutputBytes::default();
    crate::list::cow_structural::ori_list_insert_cow(
        s_data,
        s_len,
        s_cap,
        1,
        (&raw const new_val).cast(),
        ELEM_SIZE,
        8,
        None,
        0,
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
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 3);

    let mut remove_out = OutputBytes::default();
    crate::list::cow_structural::ori_list_remove_cow(
        s_data,
        s_len,
        s_cap,
        1,
        ELEM_SIZE,
        8,
        None,
        0,
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
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);

    let mut rev_out = OutputBytes::default();
    crate::list::cow_sort::ori_list_reverse_cow(
        s_data,
        s_len,
        s_cap,
        ELEM_SIZE,
        8,
        None,
        0,
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
    let _g = crate::test_support::lock_rc();
    let (data1, len1, cap1) = alloc_list(&[10, 20, 30, 40]);
    let (data2, len2, cap2) = alloc_list(&[50, 60]);

    let mut slice_out = OutputBytes::default();
    ori_list_slice(data1, len1, cap1, 1, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(s_len, 2);

    let mut concat_out = OutputBytes::default();
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
        0,
        concat_out.as_mut_ptr(),
    );

    let (r_len, r_cap, r_data) = read_result(&concat_out);
    assert_eq!(r_len, 4);
    assert!(!is_slice_cap(r_cap));
    let elems = read_elements(r_data, 4);
    assert_eq!(elems, vec![20, 30, 50, 60]);

    assert_eq!(ori_rc_count(data1), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data1, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_concat_frees_last_slice_receiver_owner() {
    let _g = crate::test_support::lock_rc();
    let before = ori_rc_live_count();
    let (data1, len1, cap1) = alloc_list(&[10, 20, 30, 40]);
    let (data2, len2, cap2) = alloc_list(&[50]);
    let mut slice_out = OutputBytes::default();
    ori_list_slice(data1, len1, cap1, 1, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (slice_len, slice_cap, slice_data) = read_result(&slice_out);
    ori_buffer_rc_dec(data1, len1, cap1, ELEM_SIZE, None);

    let mut concat_out = OutputBytes::default();
    crate::list::cow_sort::ori_list_concat_cow(
        slice_data,
        slice_len,
        slice_cap,
        data2,
        len2,
        cap2,
        ELEM_SIZE,
        8,
        None,
        0,
        concat_out.as_mut_ptr(),
    );

    let (result_len, result_cap, result_data) = read_result(&concat_out);
    assert_eq!(read_elements(result_data, 3), vec![20, 30, 50]);
    ori_buffer_rc_dec(result_data, result_len, result_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_live_count(), before, "slice receiver must be freed");
}

#[test]
fn cow_concat_frees_last_slice_argument_owner() {
    let _g = crate::test_support::lock_rc();
    let before = ori_rc_live_count();
    let (data1, len1, cap1) = alloc_list(&[1, 2]);
    let (data2, len2, cap2) = alloc_list(&[10, 20, 30, 40]);
    let mut slice_out = OutputBytes::default();
    ori_list_slice(data2, len2, cap2, 1, 3, ELEM_SIZE, slice_out.as_mut_ptr());
    let (slice_len, slice_cap, slice_data) = read_result(&slice_out);
    ori_buffer_rc_dec(data2, len2, cap2, ELEM_SIZE, None);

    let mut concat_out = OutputBytes::default();
    crate::list::cow_sort::ori_list_concat_cow(
        data1,
        len1,
        cap1,
        slice_data,
        slice_len,
        slice_cap,
        ELEM_SIZE,
        8,
        None,
        0,
        concat_out.as_mut_ptr(),
    );

    let (result_len, result_cap, result_data) = read_result(&concat_out);
    assert_eq!(read_elements(result_data, 4), vec![1, 2, 20, 30]);
    ori_buffer_rc_dec(result_data, result_len, result_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_live_count(), before, "slice argument must be freed");
}

#[test]
fn materialize_slice_produces_owned_list() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(ori_rc_count(data), 2);

    let mut mat_out = OutputBytes::default();
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

    assert_eq!(ori_rc_count(data), 1);

    assert_eq!(ori_rc_count(r_data), 1);

    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn materialize_non_slice_is_noop() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30]);
    let mut mat_out = OutputBytes::default();

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
    assert_eq!(r_data, data);

    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn cow_push_on_slice_rc_lifecycle() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[1, 2, 3]);
    let mut slice_out = OutputBytes::default();

    ori_list_slice(data, len, cap, 0, 2, ELEM_SIZE, slice_out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&slice_out);
    assert_eq!(ori_rc_count(data), 2);

    let val: i64 = 42;
    let mut push_out = OutputBytes::default();
    crate::list::cow::ori_list_push_cow(
        s_data,
        s_len,
        s_cap,
        (&raw const val).cast(),
        ELEM_SIZE,
        8,
        None,
        0,
        push_out.as_mut_ptr(),
    );

    let (r_len, _, r_data) = read_result(&push_out);
    assert_eq!(r_len, 3);
    assert_eq!(ori_rc_count(data), 1);
    assert_eq!(ori_rc_count(r_data), 1);

    let (_, r_cap, _) = read_result(&push_out);
    ori_rc_free(r_data, r_cap as usize * ELEM_SIZE as usize, 8);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_list_rc_inc_on_regular_list() {
    let _g = crate::test_support::lock_rc();
    let (data, _, cap) = alloc_list(&[10, 20, 30]);
    assert_eq!(ori_rc_count(data), 1);

    ori_list_rc_inc(data, cap);
    assert_eq!(ori_rc_count(data), 2);

    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 3 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_list_rc_inc_on_slice() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 3, ELEM_SIZE, out.as_mut_ptr());
    let (_, s_cap, s_data) = read_result(&out);
    assert_eq!(ori_rc_count(data), 2);

    ori_list_rc_inc(s_data, s_cap);
    assert_eq!(ori_rc_count(data), 3);

    crate::rc::ori_rc_dec(data, None);
    crate::rc::ori_rc_dec(data, None);
    ori_rc_free(data, 4 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_list_rc_inc_null_is_noop() {
    let _g = crate::test_support::lock_rc();
    ori_list_rc_inc(std::ptr::null_mut(), 0);
    ori_list_rc_inc(std::ptr::null_mut(), make_slice_cap(0));
}

#[test]
fn ori_buffer_rc_dec_on_slice_decs_original() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 4, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);
    assert_eq!(ori_rc_count(data), 2);

    ori_buffer_rc_dec(s_data, s_len, s_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 5 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_buffer_rc_dec_slice_last_ref_frees() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[100, 200, 300, 400]);
    let mut out = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 3, ELEM_SIZE, out.as_mut_ptr());
    let (s_len, s_cap, s_data) = read_result(&out);
    assert_eq!(ori_rc_count(data), 2);

    crate::rc::ori_rc_dec(data, None);
    assert_eq!(ori_rc_count(data), 1);

    ori_buffer_rc_dec(s_data, s_len, s_cap, ELEM_SIZE, None);
}

#[test]
fn ori_buffer_rc_dec_slice_of_slice_decs_original() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[1, 2, 3, 4, 5, 6]);
    let mut out1 = OutputBytes::default();
    let mut out2 = OutputBytes::default();

    ori_list_slice(data, len, cap, 1, 5, ELEM_SIZE, out1.as_mut_ptr());
    let (s1_len, s1_cap, s1_data) = read_result(&out1);
    assert_eq!(ori_rc_count(data), 2);

    ori_list_slice(s1_data, s1_len, s1_cap, 1, 3, ELEM_SIZE, out2.as_mut_ptr());
    let (s2_len, s2_cap, s2_data) = read_result(&out2);
    assert_eq!(ori_rc_count(data), 3);

    ori_buffer_rc_dec(s2_data, s2_len, s2_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 2);

    ori_buffer_rc_dec(s1_data, s1_len, s1_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 1);

    ori_rc_free(data, 6 * ELEM_SIZE as usize, 8);
}

#[test]
fn ori_rc_data_size_returns_allocation_size() {
    let _g = crate::test_support::lock_rc();
    let size = 5 * ELEM_SIZE as usize;
    let data = ori_rc_alloc(size, 8);
    assert!(!data.is_null());

    assert_eq!(ori_rc_data_size(data.cast_const()), size as i64);

    ori_rc_free(data, size, 8);
}

#[test]
fn ori_rc_data_size_null_returns_zero() {
    let _g = crate::test_support::lock_rc();
    assert_eq!(ori_rc_data_size(std::ptr::null()), 0);
}

#[test]
fn slice_rc_full_lifecycle() {
    let _g = crate::test_support::lock_rc();
    let (data, len, cap) = alloc_list(&[10, 20, 30, 40, 50]);
    assert_eq!(ori_rc_count(data), 1);

    let mut out_a = OutputBytes::default();
    let mut out_b = OutputBytes::default();
    let mut out_c = OutputBytes::default();

    ori_list_slice(data, len, cap, 0, 2, ELEM_SIZE, out_a.as_mut_ptr());
    let (a_len, a_cap, a_data) = read_result(&out_a);
    assert_eq!(ori_rc_count(data), 2);

    ori_list_slice(data, len, cap, 2, 4, ELEM_SIZE, out_b.as_mut_ptr());
    let (b_len, b_cap, b_data) = read_result(&out_b);
    assert_eq!(ori_rc_count(data), 3);

    ori_list_slice(data, len, cap, 4, 5, ELEM_SIZE, out_c.as_mut_ptr());
    let (c_len, c_cap, c_data) = read_result(&out_c);
    assert_eq!(ori_rc_count(data), 4);

    ori_buffer_rc_dec(a_data, a_len, a_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 3);

    ori_buffer_rc_dec(data, len, cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 2);

    ori_buffer_rc_dec(b_data, b_len, b_cap, ELEM_SIZE, None);
    assert_eq!(ori_rc_count(data), 1);

    ori_buffer_rc_dec(c_data, c_len, c_cap, ELEM_SIZE, None);
}
