use std::mem::{align_of, size_of};
use std::ptr;

use super::*;
use crate::test_support::AbiOutput;
use crate::{OPTION_TAG_NONE, OPTION_TAG_SOME};

fn read_i64_at<const N: usize>(bytes: &AbiOutput<N>, offset: usize) -> i64 {
    let Some(end) = offset.checked_add(size_of::<i64>()) else {
        panic!("iterator output offset overflows at {offset}");
    };
    let Some(field) = bytes.as_slice().get(offset..end) else {
        panic!("iterator output lacks an i64 at offset {offset}");
    };
    let mut raw = [0; size_of::<i64>()];
    raw.copy_from_slice(field);
    i64::from_ne_bytes(raw)
}

fn read_pointer_at(bytes: &AbiOutput<24>, offset: usize) -> *mut u8 {
    let Some(end) = offset.checked_add(size_of::<usize>()) else {
        panic!("iterator output offset overflows at {offset}");
    };
    assert!(
        end <= bytes.as_slice().len() && offset.is_multiple_of(align_of::<*mut u8>()),
        "iterator output lacks an aligned pointer at offset {offset}"
    );
    // SAFETY: `AbiOutput` provides pointer alignment, the bounds check covers
    // one pointer-sized field, and the runtime initialized that field.
    unsafe { bytes.as_ptr().add(offset).cast::<*mut u8>().read() }
}

#[test]
fn list_iterator_yields_elements_in_order_then_ends() {
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);

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
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8, true);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

#[test]
fn repeat_yields_same_value_indefinitely() {
    let value: i64 = 7;
    let iter = ori_iter_repeat((&raw const value).cast(), 8, None);

    let mut out: i64 = 0;
    for _ in 0..5 {
        assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
        assert_eq!(out, 7);
    }

    ori_iter_drop(iter);
}

#[test]
fn repeat_bounded_by_take() {
    let value: i64 = 99;
    let src = ori_iter_repeat((&raw const value).cast(), 8, None);
    let iter = ori_iter_take(src, 3);

    let mut out: i64 = 0;
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 99);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 99);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 99);
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

#[test]
fn cycle_replays_buffer_after_source_exhaustion() {
    let source = ori_iter_from_range(1, 3, 1, false);
    let iter = ori_iter_cycle(source, 8, None, None);
    let iter = ori_iter_take(iter, 5);

    let mut out: i64 = 0;
    for expected in [1, 2, 1, 2, 1] {
        assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 1);
        assert_eq!(out, expected);
    }
    assert_eq!(ori_iter_next(iter, (&raw mut out).cast(), 8), 0);

    ori_iter_drop(iter);
}

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
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 5, 0, 8, true);
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

extern "C-unwind" fn double_i64(env: *mut u8, in_ptr: *const u8, out_ptr: *mut u8) {
    let _ = env;
    // SAFETY: The iterator ABI supplies aligned `i64` input and output storage.
    unsafe {
        let val = in_ptr.cast::<i64>().read();
        out_ptr.cast::<i64>().write(val * 2);
    }
}

#[test]
fn map_doubles() {
    let data: [i64; 3] = [1, 2, 3];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);
    let iter = ori_iter_map(iter, double_i64, ptr::null_mut(), None, None, 8, None);

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

static MAPPED_OUTPUT_DEC_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static MAPPED_OUTPUT_INC_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static CALLBACK_ENV_INC_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static CALLBACK_ENV_DEC_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

extern "C" fn count_mapped_output_dec(_elem: *mut u8) {
    MAPPED_OUTPUT_DEC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

extern "C" fn count_mapped_output_inc(_elem: *mut u8) {
    MAPPED_OUTPUT_INC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

extern "C" fn count_callback_env_inc(_env: *mut u8) {
    CALLBACK_ENV_INC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

extern "C" fn count_callback_env_dec(_env: *mut u8) {
    CALLBACK_ENV_DEC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

extern "C-unwind" fn add_copied_env_word(env: *mut u8, in_ptr: *const u8, out_ptr: *mut u8) {
    // SAFETY: This test passes a copied two-word callback environment and
    // aligned `i64` input/output storage.
    unsafe {
        let captured = env.cast::<[*mut u8; 2]>().read()[1].cast::<usize>();
        let offset = captured.read() as i64;
        out_ptr
            .cast::<i64>()
            .write(in_ptr.cast::<i64>().read() + offset);
    }
}

#[test]
fn mapped_iterator_owns_a_copied_callback_environment() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    CALLBACK_ENV_INC_COUNT.store(0, SeqCst);
    CALLBACK_ENV_DEC_COUNT.store(0, SeqCst);

    let mut captured = 7_usize;
    let mut callback_env: [*mut u8; 2] =
        [ptr::null_mut(), std::ptr::from_mut(&mut captured).cast()];
    let source = ori_iter_from_range(0, 1, 1, false);
    let mapped = ori_iter_map(
        source,
        add_copied_env_word,
        callback_env.as_mut_ptr().cast(),
        Some(count_callback_env_inc),
        Some(count_callback_env_dec),
        8,
        None,
    );
    callback_env = [ptr::null_mut(); 2];

    let mut out = 0_i64;
    assert_eq!(ori_iter_next(mapped, (&raw mut out).cast(), 8), 1);
    assert_eq!(out, 7, "lazy callback must read the owned environment copy");
    assert_eq!(CALLBACK_ENV_INC_COUNT.load(SeqCst), 1);
    assert_eq!(CALLBACK_ENV_DEC_COUNT.load(SeqCst), 0);

    ori_iter_drop(mapped);
    assert_eq!(CALLBACK_ENV_DEC_COUNT.load(SeqCst), 1);
    assert_eq!(callback_env, [ptr::null_mut(); 2]);
}

fn mapped_range_with_counted_output(end: i64) -> *mut u8 {
    let source = ori_iter_from_range(0, end, 1, false);
    ori_iter_map(
        source,
        double_i64,
        ptr::null_mut(),
        None,
        None,
        8,
        Some(count_mapped_output_dec),
    )
}

extern "C-unwind" fn reject_all(_env: *mut u8, _elem_ptr: *const u8) -> bool {
    false
}

#[test]
fn nested_map_releases_each_consumed_intermediate() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let source = ori_iter_from_range(0, 3, 1, false);
    let inner = ori_iter_map(
        source,
        double_i64,
        ptr::null_mut(),
        None,
        None,
        8,
        Some(count_mapped_output_dec),
    );
    let outer = ori_iter_map(inner, double_i64, ptr::null_mut(), None, None, 8, None);

    let mut out = 0_i64;
    while ori_iter_next(outer, (&raw mut out).cast(), 8) != 0 {}
    ori_iter_drop(outer);

    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn count_releases_each_mapped_output_before_advancing() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    assert_eq!(ori_iter_count(mapped_range_with_counted_output(3), 8), 3);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn any_releases_each_mapped_output_including_the_match() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    assert_eq!(
        ori_iter_any(
            mapped_range_with_counted_output(3),
            gt_3,
            ptr::null_mut(),
            8,
        ),
        1
    );
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn all_releases_the_short_circuiting_mapped_output() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    assert_eq!(
        ori_iter_all(
            mapped_range_with_counted_output(3),
            gt_3,
            ptr::null_mut(),
            8,
        ),
        0
    );
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 1);
}

#[test]
fn filter_releases_each_rejected_mapped_output() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let source = ori_iter_from_range(0, 3, 1, false);
    let mapped = ori_iter_map(
        source,
        double_i64,
        ptr::null_mut(),
        None,
        None,
        8,
        Some(count_mapped_output_dec),
    );
    let filtered = ori_iter_filter(mapped, reject_all, ptr::null_mut(), None, None, 8);

    let mut out = 0_i64;
    assert_eq!(ori_iter_next(filtered, (&raw mut out).cast(), 8), 0);
    ori_iter_drop(filtered);

    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn skip_releases_discarded_mapped_outputs_and_forwards_the_survivor() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let source = ori_iter_from_range(0, 3, 1, false);
    let mapped = ori_iter_map(
        source,
        double_i64,
        ptr::null_mut(),
        None,
        None,
        8,
        Some(count_mapped_output_dec),
    );
    let skipped = ori_iter_skip(mapped, 2);

    let mut out = 0_i64;
    assert_eq!(ori_iter_next(skipped, (&raw mut out).cast(), 8), 1);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 2);
    // The surviving value and its ownership obligation are forwarded. Mimic a
    // terminal consumer releasing that value through the dynamic state.
    // SAFETY: `skipped` is the live iterator state returned by `ori_iter_skip`.
    unsafe {
        (*skipped.cast::<IterState>()).release_last_yield((&raw mut out).cast());
    }
    ori_iter_drop(skipped);

    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

// Filter adapter

extern "C-unwind" fn is_even(env: *mut u8, elem_ptr: *const u8) -> bool {
    let _ = env;
    // SAFETY: The iterator ABI supplies an aligned `i64` element pointer.
    unsafe {
        let val = elem_ptr.cast::<i64>().read();
        val % 2 == 0
    }
}

#[test]
fn filter_even() {
    let data: [i64; 6] = [1, 2, 3, 4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 6, 0, 8, true);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), None, None, 8);

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
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);
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

#[test]
fn count_range() {
    let iter = ori_iter_from_range(0, 10, 1, false);
    assert_eq!(ori_iter_count(iter, 8), 10);
}

#[test]
fn count_filtered() {
    let data: [i64; 6] = [1, 2, 3, 4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 6, 0, 8, true);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), None, None, 8);
    assert_eq!(ori_iter_count(iter, 8), 3);
}

#[test]
fn collect_range() {
    let _g = crate::test_support::lock_rc();
    let iter = ori_iter_from_range(0, 5, 1, false);

    // INVARIANT: An `OriList` result stores length, capacity, and data pointer in order.
    let mut out = AbiOutput::<24>::default();
    ori_iter_collect(iter, 8, None, None, out.as_mut_ptr());

    let len = read_i64_at(&out, 0);
    let data_ptr = read_pointer_at(&out, 16);

    assert_eq!(len, 5);
    for i in 0..5 {
        // SAFETY: `collect` returned a live allocation containing `len` aligned `i64` values.
        let val = unsafe { data_ptr.cast::<i64>().add(i).read() };
        assert_eq!(val, i as i64);
    }

    if !data_ptr.is_null() {
        let cap = read_i64_at(&out, 8);
        crate::ori_list_free_data(data_ptr, cap, 8);
    }
}

#[test]
fn map_filter_take_yields_first_three_doubled_values() {
    let data: [i64; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 10, 0, 8, true);
    let iter = ori_iter_map(iter, double_i64, ptr::null_mut(), None, None, 8, None);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), None, None, 8);
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

extern "C-unwind" fn gt_3(env: *mut u8, elem_ptr: *const u8) -> bool {
    let _ = env;
    // SAFETY: The iterator ABI supplies an aligned `i64` element pointer.
    unsafe { elem_ptr.cast::<i64>().read() > 3 }
}

#[test]
fn any_found() {
    let data: [i64; 5] = [1, 2, 3, 4, 5];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 5, 0, 8, true);
    assert_eq!(ori_iter_any(iter, gt_3, ptr::null_mut(), 8), 1);
}

#[test]
fn any_not_found() {
    let data: [i64; 3] = [1, 2, 3];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);
    assert_eq!(ori_iter_any(iter, gt_3, ptr::null_mut(), 8), 0);
}

#[test]
fn any_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8, true);
    assert_eq!(ori_iter_any(iter, gt_3, ptr::null_mut(), 8), 0);
}

#[test]
fn all_true() {
    let data: [i64; 3] = [4, 5, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);
    assert_eq!(ori_iter_all(iter, gt_3, ptr::null_mut(), 8), 1);
}

#[test]
fn all_false() {
    let data: [i64; 3] = [4, 2, 6];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);
    assert_eq!(ori_iter_all(iter, gt_3, ptr::null_mut(), 8), 0);
}

#[test]
fn all_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8, true);
    assert_eq!(ori_iter_all(iter, gt_3, ptr::null_mut(), 8), 1);
}

#[test]
fn find_found() {
    let data: [i64; 5] = [1, 2, 3, 4, 5];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 5, 0, 8, true);

    let mut out = AbiOutput::<16>::default();
    ori_iter_find(iter, gt_3, ptr::null_mut(), 8, None, out.as_mut_ptr());

    let tag = read_i64_at(&out, 0);
    let payload = read_i64_at(&out, 8);
    assert_eq!(tag, 0);
    assert_eq!(payload, 4);
}

#[test]
fn find_retains_output_then_releases_the_mapped_yield() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let mut out = AbiOutput::<16>::default();
    ori_iter_find(
        mapped_range_with_counted_output(3),
        gt_3,
        ptr::null_mut(),
        8,
        Some(count_mapped_output_inc),
        out.as_mut_ptr(),
    );

    assert_eq!(read_i64_at(&out, 0), OPTION_TAG_SOME);
    assert_eq!(read_i64_at(&out, 8), 4);
    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 1);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn find_not_found() {
    let data: [i64; 3] = [1, 2, 3];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);

    let mut out = AbiOutput::<16>::default();
    ori_iter_find(iter, gt_3, ptr::null_mut(), 8, None, out.as_mut_ptr());

    let tag = read_i64_at(&out, 0);
    assert_eq!(tag, 1);
}

#[test]
fn find_without_match_releases_all_mapped_yields_without_retaining() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let mut out = AbiOutput::<16>::default();
    ori_iter_find(
        mapped_range_with_counted_output(2),
        gt_3,
        ptr::null_mut(),
        8,
        Some(count_mapped_output_inc),
        out.as_mut_ptr(),
    );

    assert_eq!(read_i64_at(&out, 0), OPTION_TAG_NONE);
    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 0);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 2);
}

extern "C-unwind" fn increment_counter(env: *mut u8, _elem_ptr: *const u8) {
    // SAFETY: The test passes `env` as a unique pointer to a live `i64` counter.
    unsafe {
        let count = env.cast::<i64>();
        *count += 1;
    }
}

#[test]
fn for_each_counts() {
    let data: [i64; 4] = [10, 20, 30, 40];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 4, 0, 8, true);

    let mut counter: i64 = 0;
    ori_iter_for_each(iter, increment_counter, (&raw mut counter).cast(), 8);
    assert_eq!(counter, 4);
}

#[test]
fn for_each_releases_each_mapped_output_after_the_callback() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let mut counter = 0_i64;
    ori_iter_for_each(
        mapped_range_with_counted_output(3),
        increment_counter,
        (&raw mut counter).cast(),
        8,
    );

    assert_eq!(counter, 3);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn for_each_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8, true);
    let mut counter: i64 = 0;
    ori_iter_for_each(iter, increment_counter, (&raw mut counter).cast(), 8);
    assert_eq!(counter, 0);
}

// Fold consumer

extern "C-unwind" fn sum_fold(
    env: *mut u8,
    acc_ptr: *const u8,
    elem_ptr: *const u8,
    out_ptr: *mut u8,
) {
    let _ = env;
    // SAFETY: The fold ABI supplies aligned live `i64` input and output storage.
    unsafe {
        let acc = acc_ptr.cast::<i64>().read();
        let elem = elem_ptr.cast::<i64>().read();
        out_ptr.cast::<i64>().write(acc + elem);
    }
}

extern "C-unwind" fn append_digit_fold(
    env: *mut u8,
    acc_ptr: *const u8,
    elem_ptr: *const u8,
    out_ptr: *mut u8,
) {
    let _ = env;
    // SAFETY: The fold ABI supplies aligned live `i64` input and output storage.
    unsafe {
        let acc = acc_ptr.cast::<i64>().read();
        let elem = elem_ptr.cast::<i64>().read();
        out_ptr.cast::<i64>().write(acc * 10 + elem);
    }
}

extern "C" fn i64_to_ori_str(_env: *mut u8, elem_ptr: *const u8, out_ptr: *mut u8) {
    // SAFETY: The join conversion ABI supplies an aligned initialized `i64`.
    let value = unsafe { elem_ptr.cast::<i64>().read() };
    let rendered = value.to_string();
    let output = crate::string::OriStr::from_owned(&rendered);
    // SAFETY: The join conversion ABI supplies writable storage for one OriStr.
    unsafe { out_ptr.cast::<crate::string::OriStr>().write(output) };
}

#[test]
fn fold_sum() {
    let data: [i64; 4] = [1, 2, 3, 4];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 4, 0, 8, true);

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
fn fold_releases_each_mapped_output_after_the_callback() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let init = 0_i64;
    let mut result = 0_i64;
    ori_iter_fold(
        mapped_range_with_counted_output(3),
        (&raw const init).cast(),
        sum_fold,
        ptr::null_mut(),
        8,
        8,
        (&raw mut result).cast(),
    );

    assert_eq!(result, 6);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn fold_empty() {
    let iter = ori_iter_from_list(ptr::null_mut(), 0, 0, 8, true);

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
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 6, 0, 8, true);
    let iter = ori_iter_filter(iter, is_even, ptr::null_mut(), None, None, 8);

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

#[test]
fn last_uses_one_back_yield_and_transfers_its_mapped_output() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let mut out = AbiOutput::<16>::default();
    ori_iter_last(
        mapped_range_with_counted_output(3),
        8,
        Some(count_mapped_output_inc),
        out.as_mut_ptr(),
    );

    assert_eq!(read_i64_at(&out, 0), OPTION_TAG_SOME);
    assert_eq!(read_i64_at(&out, 8), 4);
    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 1);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 1);
}

#[test]
fn rfind_searches_from_back_and_transfers_the_first_matching_yield() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let mut out = AbiOutput::<16>::default();
    ori_iter_rfind(
        mapped_range_with_counted_output(3),
        gt_3,
        ptr::null_mut(),
        8,
        Some(count_mapped_output_inc),
        out.as_mut_ptr(),
    );

    assert_eq!(read_i64_at(&out, 0), OPTION_TAG_SOME);
    assert_eq!(read_i64_at(&out, 8), 4);
    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 1);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 1);
}

#[test]
fn rfind_without_match_releases_each_back_yield_without_retaining() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let mut out = AbiOutput::<16>::default();
    ori_iter_rfind(
        mapped_range_with_counted_output(2),
        gt_3,
        ptr::null_mut(),
        8,
        Some(count_mapped_output_inc),
        out.as_mut_ptr(),
    );

    assert_eq!(read_i64_at(&out, 0), OPTION_TAG_NONE);
    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 0);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 2);
}

#[test]
fn rfold_advances_from_back_and_releases_each_mapped_yield() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let init = 0_i64;
    let mut result = 0_i64;
    ori_iter_rfold(
        mapped_range_with_counted_output(3),
        (&raw const init).cast(),
        append_digit_fold,
        ptr::null_mut(),
        8,
        8,
        (&raw mut result).cast(),
    );

    assert_eq!(result, 420);
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
}

#[test]
fn join_releases_mapped_inputs_through_iterator_state() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let separator = crate::string::OriStr::from_bytes(b",");
    // SAFETY: OriStr is a raw-layout union; join accepts those three fields for
    // both heap and SSO values and reconstructs the same bits.
    let separator_fields = unsafe { separator.heap };
    let mut out = AbiOutput::<24>::default();
    ori_iter_join(
        mapped_range_with_counted_output(3),
        separator_fields.len,
        separator_fields.cap,
        separator_fields.data,
        Some(i64_to_ori_str),
        ptr::null_mut(),
        8,
        out.as_mut_ptr(),
    );

    // SAFETY: join initialized the complete, aligned OriStr output slot.
    let result = unsafe { out.as_ptr().cast::<crate::string::OriStr>().read() };
    assert_eq!(unsafe { result.as_str() }, "0,2,4");
    assert_eq!(MAPPED_OUTPUT_DEC_COUNT.load(SeqCst), 3);
    // SAFETY: Reading the heap view recovers the raw ABI fields even for SSO;
    // ori_str_rc_dec recognizes the SSO flag and performs no release.
    let result_fields = unsafe { result.heap };
    crate::rc::ori_str_rc_dec(
        result_fields.data,
        result_fields.cap,
        Some(crate::rc::ori_str_drop_buffer),
    );
}

// Zip adapter

#[test]
fn zip_equal_length() {
    let left: [i64; 3] = [1, 2, 3];
    let right: [i64; 3] = [10, 20, 30];
    let l = ori_iter_from_list(left.as_ptr() as *mut u8, 3, 0, 8, true);
    let r = ori_iter_from_list(right.as_ptr() as *mut u8, 3, 0, 8, true);
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
    let l = ori_iter_from_list(left.as_ptr() as *mut u8, 3, 0, 8, true);
    let r = ori_iter_from_list(right.as_ptr() as *mut u8, 2, 0, 8, true);
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
    let l = ori_iter_from_list(left.as_ptr() as *mut u8, 2, 0, 8, true);
    let r = ori_iter_from_list(right.as_ptr() as *mut u8, 3, 0, 8, true);
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

// A list iterator owns its backing-buffer reference for both ordinary and
// negative-cap slice encodings.

#[test]
fn list_iter_drop_releases_slice_rc() {
    use crate::rc::{ori_rc_alloc, ori_rc_count, ori_rc_free, ori_rc_inc};
    use crate::slice_encoding::make_slice_cap;
    let _g = crate::test_support::lock_rc();

    // Allocate an RC-managed buffer for 5 × i64 = 40 bytes (RC starts at 1)
    let data = ori_rc_alloc(40, 8);
    assert!(!data.is_null());
    // SAFETY: `ori_rc_alloc` returned at least 40 bytes aligned for `i64`.
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
    let iter = ori_iter_from_list(data, 2, slice_cap, 8, true);

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
    let _g = crate::test_support::lock_rc();

    let before = ori_rc_live_count();

    // Allocate three i64 elements; the iterator is the sole buffer owner.
    let data = ori_rc_alloc(24, 8);
    // SAFETY: `ori_rc_alloc` returned at least 24 bytes aligned for `i64`.
    unsafe {
        for i in 0..3_i64 {
            data.cast::<i64>().add(i as usize).write(i + 1);
        }
    }
    assert_eq!(ori_rc_live_count(), before + 1);

    // Create an iterator with slice cap — sole owner (RC=1)
    let slice_cap = make_slice_cap(0);
    let iter = ori_iter_from_list(data, 3, slice_cap, 8, true);

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
    ori_iter_drop(ptr::null_mut());
}

extern "C-unwind" fn panic_after_one_transform(env: *mut u8, in_ptr: *const u8, out_ptr: *mut u8) {
    // SAFETY: The test passes `env` as a unique pointer to a live `usize` counter.
    let calls = unsafe { &mut *env.cast::<usize>() };
    assert_ne!(*calls, 1, "iterator transform panic");
    *calls += 1;
    // SAFETY: The iterator ABI supplies aligned live `i64` input and output storage.
    unsafe { out_ptr.cast::<i64>().write(in_ptr.cast::<i64>().read()) };
}

/// A consumer owns both the opaque iterator and its in-progress output buffer.
/// A user panic after at least one successful yield must unwind across every
/// callback/runtime ABI frame and release both allocations before reaching the
/// caller's catch boundary.
#[test]
fn collect_releases_iterator_and_partial_buffer_when_transform_panics() {
    let _g = crate::test_support::lock_rc();
    let before = crate::ori_rc_live_count();
    let data = [10_i64, 20];
    let source = ori_iter_from_list(data.as_ptr().cast_mut().cast(), 2, 0, 8, true);
    let mut calls = 0_usize;
    let mapped = ori_iter_map(
        source,
        panic_after_one_transform,
        std::ptr::from_mut(&mut calls).cast(),
        None,
        None,
        8,
        None,
    );
    let mut out = AbiOutput::<24>::default();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ori_iter_collect(mapped, 8, None, None, out.as_mut_ptr());
    }));

    assert!(
        result.is_err(),
        "transform panic must reach the catch boundary"
    );
    assert_eq!(calls, 1, "one element must be initialized before the panic");
    assert_eq!(
        crate::ori_rc_live_count(),
        before,
        "panic cleanup must release the partial collect buffer"
    );
}

extern "C" fn eq_i64_ptr(left: *const u8, right: *const u8) -> bool {
    // SAFETY: The map ABI supplies aligned pointers to initialized `i64` keys.
    unsafe { left.cast::<i64>().read() == right.cast::<i64>().read() }
}

extern "C" fn hash_i64_ptr(value: *const u8) -> i64 {
    // SAFETY: The map ABI supplies an aligned pointer to an initialized `i64` key.
    unsafe { value.cast::<i64>().read() }
}

static COLLECT_TRANSFER_DEC_COUNT: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(0);

extern "C" fn count_collect_transfer_dec(_element: *mut u8) {
    COLLECT_TRANSFER_DEC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

fn owned_counted_i64_list(values: &[i64]) -> *mut u8 {
    let byte_len = std::mem::size_of_val(values);
    let data = crate::ori_rc_alloc(byte_len, 8);
    assert!(!data.is_null());
    unsafe {
        ptr::copy_nonoverlapping(values.as_ptr().cast::<u8>(), data, byte_len);
        crate::rc::store_elem_count(data, values.len() as i64);
        crate::rc::store_elem_dec_fn(data, Some(count_collect_transfer_dec));
    }
    data
}

#[test]
fn collect_list_transfers_direct_source_buffer_without_duplicate_teardown() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    let before = crate::ori_rc_live_count();
    COLLECT_TRANSFER_DEC_COUNT.store(0, SeqCst);
    let values = [10_i64, 20];
    let source_data = owned_counted_i64_list(&values);
    let iter = ori_iter_from_list(
        source_data,
        values.len() as i64,
        values.len() as i64,
        8,
        true,
    );
    let mut out = AbiOutput::<24>::default();

    ori_iter_collect(
        iter,
        8,
        None,
        Some(count_collect_transfer_dec),
        out.as_mut_ptr(),
    );

    let result_data = read_pointer_at(&out, 16);
    assert_eq!(
        result_data, source_data,
        "direct List collect must hand off its buffer"
    );
    assert_eq!(COLLECT_TRANSFER_DEC_COUNT.load(SeqCst), 0);

    crate::ori_buffer_rc_dec(result_data, 2, 2, 8, Some(count_collect_transfer_dec));
    assert_eq!(COLLECT_TRANSFER_DEC_COUNT.load(SeqCst), 2);
    assert_eq!(crate::ori_rc_live_count(), before);
}

#[test]
fn collect_set_moves_unique_source_elements_and_drops_duplicates_once() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    let before = crate::ori_rc_live_count();
    COLLECT_TRANSFER_DEC_COUNT.store(0, SeqCst);
    let values = [7_i64, 7, 9];
    let source_data = owned_counted_i64_list(&values);
    let iter = ori_iter_from_list(
        source_data,
        values.len() as i64,
        values.len() as i64,
        8,
        true,
    );
    let mut out = AbiOutput::<24>::default();

    ori_iter_collect_set(
        iter,
        8,
        eq_i64_ptr,
        hash_i64_ptr,
        None,
        Some(count_collect_transfer_dec),
        out.as_mut_ptr(),
    );

    let len = read_i64_at(&out, 0);
    let cap = read_i64_at(&out, 8);
    let result_data = read_pointer_at(&out, 16);
    assert_eq!(len, 2);
    assert_eq!(
        COLLECT_TRANSFER_DEC_COUNT.load(SeqCst),
        1,
        "only the discarded duplicate tears down during collection"
    );

    crate::ori_set_buffer_rc_dec(result_data, cap, len, 8, Some(count_collect_transfer_dec));
    assert_eq!(COLLECT_TRANSFER_DEC_COUNT.load(SeqCst), 3);
    assert_eq!(crate::ori_rc_live_count(), before);
}

#[test]
fn collect_set_releases_iterator_and_partial_buffer_when_transform_panics() {
    let _g = crate::test_support::lock_rc();
    let before = crate::ori_rc_live_count();
    let data = [10_i64, 20];
    let source = ori_iter_from_list(data.as_ptr().cast_mut().cast(), 2, 0, 8, true);
    let mut calls = 0_usize;
    let mapped = ori_iter_map(
        source,
        panic_after_one_transform,
        std::ptr::from_mut(&mut calls).cast(),
        None,
        None,
        8,
        None,
    );
    let mut out = AbiOutput::<24>::default();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ori_iter_collect_set(
            mapped,
            8,
            eq_i64_ptr,
            hash_i64_ptr,
            None,
            None,
            out.as_mut_ptr(),
        );
    }));

    assert!(
        result.is_err(),
        "transform panic must reach the catch boundary"
    );
    assert_eq!(
        calls, 1,
        "one set element must be inserted before the panic"
    );
    assert_eq!(
        crate::ori_rc_live_count(),
        before,
        "panic cleanup must release the partial collect_set buffer"
    );
}

#[test]
fn rev_retains_buffer_masters_and_releases_each_source_yield() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let reversed = ori_iter_rev(
        mapped_range_with_counted_output(3),
        8,
        Some(count_mapped_output_inc),
        Some(count_mapped_output_dec),
    );
    assert_eq!(ori_iter_count(reversed, 8), 3);

    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 3);
    assert_eq!(
        MAPPED_OUTPUT_DEC_COUNT.load(SeqCst),
        6,
        "three source yields and three retained reverse-buffer masters must release"
    );
}

#[test]
fn rev_unwind_releases_the_retained_prefix_and_consumed_source_yield() {
    use std::sync::atomic::Ordering::SeqCst;

    let _g = crate::test_support::lock_rc();
    MAPPED_OUTPUT_INC_COUNT.store(0, SeqCst);
    MAPPED_OUTPUT_DEC_COUNT.store(0, SeqCst);

    let source = ori_iter_from_range(10, 12, 1, false);
    let mut calls = 0_usize;
    let mapped = ori_iter_map(
        source,
        panic_after_one_transform,
        std::ptr::from_mut(&mut calls).cast(),
        None,
        None,
        8,
        Some(count_mapped_output_dec),
    );

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ori_iter_rev(
            mapped,
            8,
            Some(count_mapped_output_inc),
            Some(count_mapped_output_dec),
        )
    }));

    if let Ok(reversed) = result {
        ori_iter_drop(reversed);
        panic!("source callback panic must cross ori_iter_rev");
    }
    assert_eq!(calls, 1);
    assert_eq!(MAPPED_OUTPUT_INC_COUNT.load(SeqCst), 1);
    assert_eq!(
        MAPPED_OUTPUT_DEC_COUNT.load(SeqCst),
        2,
        "the consumed mapped yield and retained reverse prefix must both release"
    );
}

#[test]
#[should_panic(expected = "exceeds MAX_ELEM_SIZE")]
fn assert_elem_size_rejects_oversized() {
    state::assert_elem_size((state::ElemBuf::MAX_SIZE + 1) as i64, "test");
}

#[test]
#[should_panic(expected = "exceeds MAX_ELEM_SIZE")]
fn assert_elem_size_rejects_negative() {
    state::assert_elem_size(-1, "test");
}

#[test]
fn assert_elem_size_accepts_zero() {
    state::assert_elem_size(0, "test");
}

#[test]
fn assert_elem_size_accepts_max() {
    state::assert_elem_size(state::ElemBuf::MAX_SIZE as i64, "test");
}

#[test]
fn assert_elem_size_accepts_typical() {
    state::assert_elem_size(8, "test");
    state::assert_elem_size(24, "test");
    state::assert_elem_size(200, "test");
}

#[test]
fn normal_sized_elem_passes_collect() {
    let _g = crate::test_support::lock_rc();
    let data: [i64; 3] = [10, 20, 30];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 3, 0, 8, true);
    let mut out = AbiOutput::<24>::default();
    ori_iter_collect(iter, 8, None, None, out.as_mut_ptr());
    let len = read_i64_at(&out, 0);
    assert_eq!(len, 3);

    let data_ptr = read_pointer_at(&out, 16);
    if !data_ptr.is_null() {
        let cap = read_i64_at(&out, 8);
        crate::ori_list_free_data(data_ptr, cap, 8);
    }
}

#[test]
fn max_sized_elem_passes_collect() {
    let _g = crate::test_support::lock_rc();
    let max_size = state::ElemBuf::MAX_SIZE as i64;
    let data = vec![0u8; state::ElemBuf::MAX_SIZE];
    let iter = ori_iter_from_list(data.as_ptr() as *mut u8, 1, 0, max_size, true);
    let mut out = AbiOutput::<24>::default();
    ori_iter_collect(iter, max_size, None, None, out.as_mut_ptr());
    let len = read_i64_at(&out, 0);
    assert_eq!(len, 1);

    let data_ptr = read_pointer_at(&out, 16);
    if !data_ptr.is_null() {
        let cap = read_i64_at(&out, 8);
        crate::ori_list_free_data(data_ptr, cap, max_size);
    }
}

static REVERSED_DEC_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

extern "C" fn counting_dec(_elem: *mut u8) {
    REVERSED_DEC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn reversed_drop_decs_all_masters_regardless_of_pos() {
    use std::sync::atomic::Ordering::SeqCst;

    REVERSED_DEC_COUNT.store(0, SeqCst);
    drop(IterState::Reversed {
        elements: vec![0u8; 3 * 8],
        pos: 3,
        front: 0,
        elem_size: 8,
        elem_dec_fn: Some(counting_dec),
    });
    assert_eq!(
        REVERSED_DEC_COUNT.load(SeqCst),
        3,
        "Drop arm must dec all 3 stored masters"
    );

    REVERSED_DEC_COUNT.store(0, SeqCst);
    drop(IterState::Reversed {
        elements: vec![0u8; 3 * 8],
        pos: 1,
        front: 0,
        elem_size: 8,
        elem_dec_fn: Some(counting_dec),
    });
    assert_eq!(
        REVERSED_DEC_COUNT.load(SeqCst),
        3,
        "Drop arm decs all masters regardless of pos, never the un-yielded subset"
    );

    REVERSED_DEC_COUNT.store(0, SeqCst);
    drop(IterState::Reversed {
        elements: vec![0u8; 3 * 8],
        pos: 3,
        front: 0,
        elem_size: 8,
        elem_dec_fn: None,
    });
    assert_eq!(
        REVERSED_DEC_COUNT.load(SeqCst),
        0,
        "null elem_dec_fn (scalar element) decs nothing"
    );
}

static CYCLED_DEC_COUNT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

extern "C" fn cycled_counting_dec(_elem: *mut u8) {
    CYCLED_DEC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn cycled_drop_decs_all_masters_regardless_of_buf_pos() {
    use std::sync::atomic::Ordering::SeqCst;

    // Full buffer (buf_pos = count): Drop decs all 3 stored masters exactly once.
    CYCLED_DEC_COUNT.store(0, SeqCst);
    drop(IterState::Cycled {
        source: state::CycleSource::Replaying,
        buffer: vec![0u8; 3 * 8],
        buf_pos: 3,
        elem_size: 8,
        elem_inc_fn: None,
        elem_dec_fn: Some(cycled_counting_dec),
    });
    assert_eq!(
        CYCLED_DEC_COUNT.load(SeqCst),
        3,
        "Drop arm must dec all 3 stored buffer masters"
    );

    // Partially replayed (buf_pos mid-buffer): Drop STILL decs ALL 3 masters —
    // NOT the consumed/un-replayed subset. A replay re-yields a master without
    // moving the buffer's ownership; the consumer's collect-inc covers that alias.
    CYCLED_DEC_COUNT.store(0, SeqCst);
    drop(IterState::Cycled {
        source: state::CycleSource::Replaying,
        buffer: vec![0u8; 3 * 8],
        buf_pos: 1,
        elem_size: 8,
        elem_inc_fn: None,
        elem_dec_fn: Some(cycled_counting_dec),
    });
    assert_eq!(
        CYCLED_DEC_COUNT.load(SeqCst),
        3,
        "Drop arm decs all masters regardless of buf_pos, never the un-replayed subset"
    );

    // Negative pin: a scalar element (null dec fn) decs nothing — no-op.
    CYCLED_DEC_COUNT.store(0, SeqCst);
    drop(IterState::Cycled {
        source: state::CycleSource::Replaying,
        buffer: vec![0u8; 3 * 8],
        buf_pos: 3,
        elem_size: 8,
        elem_inc_fn: None,
        elem_dec_fn: None,
    });
    assert_eq!(
        CYCLED_DEC_COUNT.load(SeqCst),
        0,
        "null elem_dec_fn (scalar element) decs nothing"
    );
}
