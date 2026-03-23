//! Tests for runtime iterator state machine.

use std::ptr;

use super::*;

// List iterator

#[test]
fn list_iter_basic() {
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 10);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 20);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 30);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

#[test]
fn list_iter_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Range iterator

#[test]
fn range_iter_exclusive() {
    let iter = ori_iter_from_range(0, 3, 1, false);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 0);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 1);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

#[test]
fn range_iter_inclusive() {
    let iter = ori_iter_from_range(1, 3, 1, true);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 1);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 3);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

#[test]
fn range_iter_empty() {
    let iter = ori_iter_from_range(5, 0, 1, false);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Take adapter

#[test]
fn take_from_range() {
    let iter = ori_iter_from_range(0, 100, 1, false);
    let iter = ori_iter_take(iter, 3);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 0);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 1);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Skip adapter

#[test]
fn skip_from_list() {
    let data: [i64; 5] = [10, 20, 30, 40, 50];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 5, 0, 8);
    let iter = ori_iter_skip(iter, 3);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 40);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 50);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Map adapter

extern "C" fn double_i64(env: *mut u8, in_ptr: *const u8, out_ptr: *mut u8) {
    let _ = env;
    unsafe {
        let val = in_ptr.cast::<i64>().read();
        out_ptr.cast::<i64>().write(val * 2);
    }
}

#[test]
fn map_doubles() {
    let data: [i64; 3] = [1, 2, 3];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);
    let iter = ori_iter_map(iter, double_i64, ptr::null_mut(), 8);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 4);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 6);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Filter adapter

extern "C" fn is_even(env: *mut u8, elem_ptr: *const u8) -> bool {
    let _ = env;
    unsafe {
        let val = elem_ptr.cast::<i64>().read();
        val % 2 == 0
    }
}

#[test]
fn filter_even() {
    let data: [i64; 6] = [1, 2, 3, 4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 6, 0, 8);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), 8);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 4);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 6);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Enumerate adapter

#[test]
fn enumerate_list() {
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);
    let iter = ori_iter_enumerate(iter);

    // Output: (i64 index, i64 element) = 16 bytes
    let mut out: [i64; 2] = [0, 0];
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [0, 10]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [1, 20]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [2, 30]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 0);

    ori_iter_drop(iter);
}

// Count consumer

#[test]
fn count_range() {
    let iter = ori_iter_from_range(0, 10, 1, false);
    assert_eq!(ori_iter_count(iter, 8), 10);
    // iter is consumed — no drop needed
}

#[test]
fn count_filtered() {
    let data: [i64; 6] = [1, 2, 3, 4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 6, 0, 8);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), 8);
    assert_eq!(ori_iter_count(iter, 8), 3);
}

// Collect consumer

#[test]
fn collect_range() {
    let iter = ori_iter_from_range(0, 5, 1, false);

    // OriList layout: { i64 len, i64 cap, ptr data }
    let mut out = [0u8; 24];
    ori_iter_collect(iter, 8, None, out.as_mut_ptr());

    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let data_ptr = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };

    assert_eq!(len, 5);
    for i in 0..5 {
        let val = unsafe { data_ptr.cast::<i64>().add(i).read() };
        assert_eq!(val, i as i64);
    }

    // Free the collected data (RC-managed via ori_list_alloc_data)
    if !data_ptr.is_null() {
        let cap = unsafe { out.as_ptr().cast::<i64>().add(1).read() };
        crate::ori_list_free_data(data_ptr, cap, 8);
    }
}

// Chained adapters

#[test]
fn map_filter_take() {
    // [1,2,3,4,5,6,7,8,9,10].iter().map(x -> x*2).filter(x -> x > 10).take(3)
    let data: [i64; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 10, 0, 8);
    let iter = ori_iter_map(iter, double_i64, ptr::null_mut(), 8);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), 8); // all doubled are even
    let iter = ori_iter_take(iter, 3);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 4);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 6);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// Any consumer

extern "C" fn gt_3(env: *mut u8, elem_ptr: *const u8) -> bool {
    let _ = env;
    unsafe { elem_ptr.cast::<i64>().read() > 3 }
}

#[test]
fn any_found() {
    let data: [i64; 5] = [1, 2, 3, 4, 5];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 5, 0, 8);
    assert_eq!(ori_iter_any(iter, gt_3, ptr::null_mut(), 8), 1);
}

#[test]
fn any_not_found() {
    let data: [i64; 3] = [1, 2, 3];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);
    assert_eq!(ori_iter_any(iter, gt_3, ptr::null_mut(), 8), 0);
}

#[test]
fn any_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8);
    assert_eq!(ori_iter_any(iter, gt_3, ptr::null_mut(), 8), 0);
}

// All consumer

#[test]
fn all_true() {
    let data: [i64; 3] = [4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);
    assert_eq!(ori_iter_all(iter, gt_3, ptr::null_mut(), 8), 1);
}

#[test]
fn all_false() {
    let data: [i64; 3] = [4, 2, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);
    assert_eq!(ori_iter_all(iter, gt_3, ptr::null_mut(), 8), 0);
}

#[test]
fn all_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8);
    assert_eq!(ori_iter_all(iter, gt_3, ptr::null_mut(), 8), 1); // vacuously true
}

// Find consumer

#[test]
fn find_found() {
    let data: [i64; 5] = [1, 2, 3, 4, 5];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 5, 0, 8);

    // Option<i64> = { i64 tag, i64 payload } = 16 bytes
    // ARC enum convention: Some=0, None=1
    let mut out = [0u8; 16];
    ori_iter_find(iter, gt_3, ptr::null_mut(), 8, out.as_mut_ptr());

    let tag = unsafe { out.as_ptr().cast::<i64>().read() };
    let payload = unsafe { out.as_ptr().add(8).cast::<i64>().read() };
    assert_eq!(tag, 0); // Some (ARC: first variant = 0)
    assert_eq!(payload, 4); // first > 3
}

#[test]
fn find_not_found() {
    let data: [i64; 3] = [1, 2, 3];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);

    // ARC enum convention: Some=0, None=1
    let mut out = [0u8; 16];
    ori_iter_find(iter, gt_3, ptr::null_mut(), 8, out.as_mut_ptr());

    let tag = unsafe { out.as_ptr().cast::<i64>().read() };
    assert_eq!(tag, 1); // None (ARC: second variant = 1)
}

// For Each consumer

extern "C" fn increment_counter(env: *mut u8, _elem_ptr: *const u8) {
    unsafe {
        let count = env.cast::<i64>();
        *count += 1;
    }
}

#[test]
fn for_each_counts() {
    let data: [i64; 4] = [10, 20, 30, 40];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 4, 0, 8);

    let mut counter: i64 = 0;
    ori_iter_for_each(iter, increment_counter, (&raw mut counter).cast(), 8);
    assert_eq!(counter, 4);
}

#[test]
fn for_each_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8);
    let mut counter: i64 = 0;
    ori_iter_for_each(iter, increment_counter, (&raw mut counter).cast(), 8);
    assert_eq!(counter, 0);
}

// Fold consumer

extern "C" fn sum_fold(env: *mut u8, acc_ptr: *const u8, elem_ptr: *const u8, out_ptr: *mut u8) {
    let _ = env;
    unsafe {
        let acc = acc_ptr.cast::<i64>().read();
        let elem = elem_ptr.cast::<i64>().read();
        out_ptr.cast::<i64>().write(acc + elem);
    }
}

#[test]
fn fold_sum() {
    let data: [i64; 4] = [1, 2, 3, 4];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 4, 0, 8);

    let init: i64 = 0;
    let mut result: i64 = 0;
    ori_iter_fold(
        iter,
        (&raw const init).cast(),
        sum_fold,
        ptr::null_mut(),
        8,
        8,
        (&raw mut result).cast(),
    );
    assert_eq!(result, 10);
}

#[test]
fn fold_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8);

    let init: i64 = 42;
    let mut result: i64 = 0;
    ori_iter_fold(
        iter,
        (&raw const init).cast(),
        sum_fold,
        ptr::null_mut(),
        8,
        8,
        (&raw mut result).cast(),
    );
    assert_eq!(result, 42); // returns init when empty
}

#[test]
fn fold_with_filter() {
    // [1,2,3,4,5,6].filter(even).fold(0, +) = 2+4+6 = 12
    let data: [i64; 6] = [1, 2, 3, 4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 6, 0, 8);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), 8);

    let init: i64 = 0;
    let mut result: i64 = 0;
    ori_iter_fold(
        iter,
        (&raw const init).cast(),
        sum_fold,
        ptr::null_mut(),
        8,
        8,
        (&raw mut result).cast(),
    );
    assert_eq!(result, 12);
}

// Zip adapter

#[test]
fn zip_equal_length() {
    let left: [i64; 3] = [1, 2, 3];
    let right: [i64; 3] = [10, 20, 30];
    let l = ori_iter_from_list(left.as_ptr() as *mut u8, 3, 0, 8);
    let r = ori_iter_from_list(right.as_ptr() as *mut u8, 3, 0, 8);
    let iter = ori_iter_zip(l, r, 8);

    // Output: (i64, i64) = 16 bytes
    let mut out: [i64; 2] = [0, 0];
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [1, 10]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [2, 20]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [3, 30]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 0);

    ori_iter_drop(iter);
}

#[test]
fn zip_unequal_length() {
    let left: [i64; 3] = [1, 2, 3];
    let right: [i64; 2] = [10, 20];
    let l = ori_iter_from_list(left.as_ptr() as *mut u8, 3, 0, 8);
    let r = ori_iter_from_list(right.as_ptr() as *mut u8, 2, 0, 8);
    let iter = ori_iter_zip(l, r, 8);

    let mut out: [i64; 2] = [0, 0];
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [1, 10]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 1);
    assert_eq!(out, [2, 20]);
    assert_eq!(ori_iter_next(iter, out.as_mut_ptr().cast(), 16), 0);

    ori_iter_drop(iter);
}

#[test]
fn zip_count() {
    let l = ori_iter_from_range(0, 3, 1, false);
    let r = ori_iter_from_range(10, 13, 1, false);
    let iter = ori_iter_zip(l, r, 8);
    assert_eq!(ori_iter_count(iter, 16), 3);
}

// Chain adapter

#[test]
fn chain_two_lists() {
    let left: [i64; 2] = [1, 2];
    let right: [i64; 3] = [3, 4, 5];
    let l = ori_iter_from_list(left.as_ptr() as *mut u8, 2, 0, 8);
    let r = ori_iter_from_list(right.as_ptr() as *mut u8, 3, 0, 8);
    let iter = ori_iter_chain(l, r);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 1);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 2);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 3);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 4);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 5);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

#[test]
fn chain_count() {
    let l = ori_iter_from_range(0, 3, 1, false);
    let r = ori_iter_from_range(10, 13, 1, false);
    let iter = ori_iter_chain(l, r);
    assert_eq!(ori_iter_count(iter, 8), 6);
}

// Seamless slice cleanup
//
// Regression: IterState::List Drop used `*cap > 0` which excluded seamless
// slices (negative cap due to SLICE_FLAG). The iterator failed to release
// its RC reference to the backing buffer, causing a memory leak.

#[test]
fn list_iter_drop_releases_slice_rc() {
    use crate::rc::{ori_rc_alloc, ori_rc_count, ori_rc_free, ori_rc_inc};
    use crate::slice_encoding::make_slice_cap;
    let _g = crate::test_helpers::lock_rc();

    // Allocate an RC-managed buffer for 5 × i64 = 40 bytes (RC starts at 1)
    let data = ori_rc_alloc(40, 8);
    assert!(!data.is_null());
    unsafe {
        for i in 0..5_i64 {
            data.cast::<i64>().add(i as usize).write(i * 10);
        }
    }
    assert_eq!(ori_rc_count(data.cast_const()), 1);

    // Simulate a slice taking a reference (like ori_list_slice_take does)
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data.cast_const()), 2);

    // Create an iterator over the "slice" — 2 elements starting at byte offset 0
    let slice_cap = make_slice_cap(0);
    let iter = ori_iter_from_list(data, 2, slice_cap, 8);

    // Drop the iterator — should release the slice's RC reference
    ori_iter_drop(iter);
    assert_eq!(
        ori_rc_count(data.cast_const()),
        1,
        "Iterator drop should have decremented RC from 2 to 1"
    );

    // Clean up the original allocation
    ori_rc_free(data, 40, 8);
}

#[test]
fn list_iter_drop_frees_slice_when_last_ref() {
    use crate::rc::{ori_rc_alloc, ori_rc_live_count};
    use crate::slice_encoding::make_slice_cap;
    let _g = crate::test_helpers::lock_rc();

    let before = ori_rc_live_count();

    // Allocate a buffer (RC=1) — the iterator is the sole owner via the slice
    let data = ori_rc_alloc(24, 8); // 3 × i64
    unsafe {
        for i in 0..3_i64 {
            data.cast::<i64>().add(i as usize).write(i + 1);
        }
    }
    assert_eq!(ori_rc_live_count(), before + 1);

    // Create an iterator with slice cap — sole owner (RC=1)
    let slice_cap = make_slice_cap(0);
    let iter = ori_iter_from_list(data, 3, slice_cap, 8);

    // Drop the iterator — should free the buffer entirely (RC 1→0)
    ori_iter_drop(iter);
    assert_eq!(
        ori_rc_live_count(),
        before,
        "Iterator drop should have freed the backing buffer"
    );
}

// Null safety

#[test]
fn null_iter_safety() {
    assert_eq!(ori_iter_next(ptr::null_mut(), ptr::null_mut(), 8), 0);
    assert_eq!(ori_iter_count(ptr::null_mut(), 8), 0);
    assert_eq!(ori_iter_any(ptr::null_mut(), gt_3, ptr::null_mut(), 8), 0);
    assert_eq!(ori_iter_all(ptr::null_mut(), gt_3, ptr::null_mut(), 8), 1);
    ori_iter_drop(ptr::null_mut()); // should not crash
}

// Output element size validation (TPR-01-005)
//
// Consumer functions and ori_iter_next are `extern "C"`, so panics abort
// rather than unwind. We test the assert_elem_size guard directly, then
// verify normal and boundary element sizes work through the full pipeline.

#[test]
#[should_panic(expected = "exceeds MAX_ELEM_SIZE")]
fn assert_elem_size_rejects_oversized() {
    state::assert_elem_size((state::MAX_ELEM_SIZE + 1) as i64, "test");
}

#[test]
#[should_panic(expected = "exceeds MAX_ELEM_SIZE")]
fn assert_elem_size_rejects_negative() {
    state::assert_elem_size(-1, "test");
}

#[test]
fn assert_elem_size_accepts_zero() {
    state::assert_elem_size(0, "test"); // should not panic
}

#[test]
fn assert_elem_size_accepts_max() {
    state::assert_elem_size(state::MAX_ELEM_SIZE as i64, "test");
}

#[test]
fn assert_elem_size_accepts_typical() {
    state::assert_elem_size(8, "test"); // int
    state::assert_elem_size(24, "test"); // str
    state::assert_elem_size(200, "test"); // large struct
}

// Semantic pin: normal-sized elements still work through consumers
#[test]
fn normal_sized_elem_passes_collect() {
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8);
    let mut out = [0u8; 24];
    ori_iter_collect(iter, 8, None, out.as_mut_ptr());
    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    assert_eq!(len, 3);
}

#[test]
fn max_sized_elem_passes_collect() {
    // Exactly MAX_ELEM_SIZE should be accepted (boundary test)
    let max_size = state::MAX_ELEM_SIZE as i64;
    let data = vec![0u8; state::MAX_ELEM_SIZE];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 1, 0, max_size);
    let mut out = [0u8; 24];
    ori_iter_collect(iter, max_size, None, out.as_mut_ptr());
    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    assert_eq!(len, 1);
}
