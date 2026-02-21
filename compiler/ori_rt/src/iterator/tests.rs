//! Tests for runtime iterator state machine.

use super::*;

// ── List iterator ───────────────────────────────────────────────────────

#[test]
fn list_iter_basic() {
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr().cast(), 3, 8);

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
    let iter = ori_iter_from_list(ptr::null(), 0, 8);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// ── Range iterator ──────────────────────────────────────────────────────

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

// ── Take adapter ────────────────────────────────────────────────────────

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

// ── Skip adapter ────────────────────────────────────────────────────────

#[test]
fn skip_from_list() {
    let data: [i64; 5] = [10, 20, 30, 40, 50];
    let iter = ori_iter_from_list(data.as_ptr().cast(), 5, 8);
    let iter = ori_iter_skip(iter, 3);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 40);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 50);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

// ── Map adapter ─────────────────────────────────────────────────────────

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
    let iter = ori_iter_from_list(data.as_ptr().cast(), 3, 8);
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

// ── Filter adapter ──────────────────────────────────────────────────────

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
    let iter = ori_iter_from_list(data.as_ptr().cast(), 6, 8);
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

// ── Enumerate adapter ───────────────────────────────────────────────────

#[test]
fn enumerate_list() {
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr().cast(), 3, 8);
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

// ── Count consumer ──────────────────────────────────────────────────────

#[test]
fn count_range() {
    let iter = ori_iter_from_range(0, 10, 1, false);
    assert_eq!(ori_iter_count(iter, 8), 10);
    // iter is consumed — no drop needed
}

#[test]
fn count_filtered() {
    let data: [i64; 6] = [1, 2, 3, 4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr().cast(), 6, 8);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), 8);
    assert_eq!(ori_iter_count(iter, 8), 3);
}

// ── Collect consumer ────────────────────────────────────────────────────

#[test]
fn collect_range() {
    let iter = ori_iter_from_range(0, 5, 1, false);

    // OriList layout: { i64 len, i64 cap, ptr data }
    let mut out = [0u8; 24];
    ori_iter_collect(iter, 8, out.as_mut_ptr());

    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let data_ptr = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };

    assert_eq!(len, 5);
    for i in 0..5 {
        let val = unsafe { data_ptr.cast::<i64>().add(i).read() };
        assert_eq!(val, i as i64);
    }

    // Free the collected data
    if !data_ptr.is_null() {
        let cap = unsafe { out.as_ptr().cast::<i64>().add(1).read() };
        crate::ori_free(data_ptr, cap as usize * 8, 8);
    }
}

// ── Chained adapters ────────────────────────────────────────────────────

#[test]
fn map_filter_take() {
    // [1,2,3,4,5,6,7,8,9,10].iter().map(x -> x*2).filter(x -> x > 10).take(3)
    let data: [i64; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let iter = ori_iter_from_list(data.as_ptr().cast(), 10, 8);
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

// ── Null safety ─────────────────────────────────────────────────────────

#[test]
fn null_iter_safety() {
    assert_eq!(ori_iter_next(ptr::null_mut(), ptr::null_mut(), 8), 0);
    assert_eq!(ori_iter_count(ptr::null_mut(), 8), 0);
    ori_iter_drop(ptr::null_mut()); // should not crash
}
