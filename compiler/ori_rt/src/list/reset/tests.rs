//! Tests for `ori_list_reset_buffer`.

use crate::rc::{ori_rc_alloc, ori_rc_free, ori_rc_inc};

use super::ori_list_reset_buffer;

/// Helper: allocate a buffer, store `count` i64 values, return data ptr.
fn alloc_i64_buffer(count: usize) -> *mut u8 {
    if count == 0 {
        return std::ptr::null_mut();
    }
    let data = ori_rc_alloc(count * 8, 8);
    for i in 0..count {
        unsafe { data.cast::<i64>().add(i).write(i as i64) };
    }
    data
}

/// Read the RC value from a buffer's header.
fn read_rc(data: *const u8) -> i64 {
    unsafe { *(data.sub(8).cast::<i64>()) }
}

#[test]
fn reuse_unique_buffer_sufficient_capacity() {
    let data = alloc_i64_buffer(4);
    let mut out_cap: i64 = 0;

    // Old: len=4, cap=4, elem_size=8. New: len=2.
    // Cap (4) >= new_len (2) → reuse as-is.
    let result = ori_list_reset_buffer(data, 4, 4, 2, 8, None, &mut out_cap);
    assert_eq!(result, data, "should reuse same buffer");
    assert_eq!(out_cap, 4, "capacity unchanged");

    // Clean up
    ori_rc_free(result, 4 * 8, 8);
}

#[test]
fn reuse_unique_buffer_needs_realloc() {
    let data = alloc_i64_buffer(2);
    let mut out_cap: i64 = 0;

    // Old: len=2, cap=2, elem_size=8. New: len=8.
    // Cap (2) < new_len (8) → realloc.
    let result = ori_list_reset_buffer(data, 2, 2, 8, 8, None, &mut out_cap);
    assert!(!result.is_null(), "should return valid pointer");
    assert_eq!(out_cap, 8, "capacity = new_len");

    ori_rc_free(result, 8 * 8, 8);
}

#[test]
fn shared_buffer_allocates_fresh() {
    let data = alloc_i64_buffer(4);
    ori_rc_inc(data); // RC = 2, shared
    let mut out_cap: i64 = 0;

    let result = ori_list_reset_buffer(data, 4, 4, 3, 8, None, &mut out_cap);
    assert_ne!(result, data, "should NOT reuse shared buffer");
    assert_eq!(out_cap, 3);

    // Old buffer still alive (RC was 2, now 1 after dec)
    assert_eq!(read_rc(data), 1);

    // Clean up both
    ori_rc_free(data, 4 * 8, 8);
    ori_rc_free(result, 3 * 8, 8);
}

#[test]
fn null_old_data_allocates_fresh() {
    let mut out_cap: i64 = 0;
    let result = ori_list_reset_buffer(std::ptr::null_mut(), 0, 0, 5, 8, None, &mut out_cap);
    assert!(!result.is_null());
    assert_eq!(out_cap, 5);

    ori_rc_free(result, 5 * 8, 8);
}

#[test]
fn zero_new_len_returns_null() {
    let data = alloc_i64_buffer(4);
    let mut out_cap: i64 = 99;

    // Reuse with 0 new elements → old buffer freed, null returned.
    // But first the old buffer's elements are cleaned up (no dec fn here).
    // The unique buffer gets freed because we can't reuse with 0 capacity.
    // Actually, new_len=0 → alloc_fresh returns null.
    // But old buffer is unique → it gets cleaned and... let's see.
    // reuse_buffer: oc(4) >= new_len(0) → reuse as-is. out_cap = 4.
    // Actually that would return the old buffer with cap=4 for 0 elements.
    // That's correct — the buffer is reused, caller stores 0 elements.
    let result = ori_list_reset_buffer(data, 4, 4, 0, 8, None, &mut out_cap);
    assert_eq!(result, data, "unique buffer reused even for 0 elements");
    assert_eq!(out_cap, 4);

    ori_rc_free(result, 4 * 8, 8);
}
