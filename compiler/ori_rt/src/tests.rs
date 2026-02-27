//! Tests for `ori_rt` core functions (memory, refcounting, panic).
//!
//! All RC tests acquire `RC_TEST_LOCK` because `RC_LIVE_COUNT` is a global
//! atomic counter modified by `ori_rc_alloc`/`ori_rc_free`. Without
//! serialization, parallel tests cause TOCTOU races in live-count assertions.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::MutexGuard;

use super::*;

/// Serializes all RC tests to prevent TOCTOU races on `RC_LIVE_COUNT`.
static RC_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_rc() -> MutexGuard<'static, ()> {
    RC_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ── Basic RC lifecycle ──────────────────────────────────────────────────

#[test]
fn rc_alloc_initializes_count_to_one() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    assert!(!ptr.is_null());
    assert_eq!(ori_rc_count(ptr), 1);
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_inc_increments_count() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    ori_rc_inc(ptr);
    assert_eq!(ori_rc_count(ptr), 2);
    ori_rc_inc(ptr);
    assert_eq!(ori_rc_count(ptr), 3);
    // Clean up: dec back to 0, then free the allocation.
    // Using None for drop_fn since we handle cleanup explicitly.
    ori_rc_dec(ptr, None);
    ori_rc_dec(ptr, None);
    ori_rc_dec(ptr, None);
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_dec_decrements_count() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    ori_rc_inc(ptr);
    ori_rc_inc(ptr);
    assert_eq!(ori_rc_count(ptr), 3);

    ori_rc_dec(ptr, None);
    assert_eq!(ori_rc_count(ptr), 2);

    ori_rc_dec(ptr, None);
    assert_eq!(ori_rc_count(ptr), 1);

    // Final dec reaches 0 (no drop_fn), then free explicitly.
    ori_rc_dec(ptr, None);
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_null_pointer_is_noop() {
    let _g = lock_rc();
    // These should not crash
    ori_rc_inc(std::ptr::null_mut());
    ori_rc_dec(std::ptr::null_mut(), None);
    assert_eq!(ori_rc_count(std::ptr::null()), 0);
}

// ── Drop function called exactly once ───────────────────────────────────

/// Global counter for tracking drop function calls.
static DROP_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Test drop function that increments the global counter and frees the allocation.
///
/// Real drop functions always call `ori_rc_free` as their final step.
/// Test drop functions must do the same to keep `RC_LIVE_COUNT` accurate.
extern "C" fn test_drop_fn(data_ptr: *mut u8) {
    DROP_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    ori_rc_free(data_ptr, 16, 8);
}

#[test]
fn drop_function_called_once_at_zero() {
    let _g = lock_rc();
    DROP_CALL_COUNT.store(0, Ordering::SeqCst);

    let ptr = ori_rc_alloc(16, 8);
    ori_rc_inc(ptr); // count = 2
    ori_rc_inc(ptr); // count = 3

    ori_rc_dec(ptr, Some(test_drop_fn)); // count = 2, no drop
    assert_eq!(DROP_CALL_COUNT.load(Ordering::SeqCst), 0);

    ori_rc_dec(ptr, Some(test_drop_fn)); // count = 1, no drop
    assert_eq!(DROP_CALL_COUNT.load(Ordering::SeqCst), 0);

    ori_rc_dec(ptr, Some(test_drop_fn)); // count = 0, DROP!
    assert_eq!(DROP_CALL_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn drop_function_not_called_above_zero() {
    let _g = lock_rc();
    DROP_CALL_COUNT.store(0, Ordering::SeqCst);

    let ptr = ori_rc_alloc(16, 8);
    ori_rc_inc(ptr); // count = 2

    // Dec from 2 to 1 — should NOT call drop
    ori_rc_dec(ptr, Some(test_drop_fn));
    assert_eq!(DROP_CALL_COUNT.load(Ordering::SeqCst), 0);
    assert_eq!(ori_rc_count(ptr), 1);

    // Final dec triggers drop
    ori_rc_dec(ptr, Some(test_drop_fn));
    assert_eq!(DROP_CALL_COUNT.load(Ordering::SeqCst), 1);
}

// ── Concurrent refcount operations ──────────────────────────────────────

#[test]
fn concurrent_increments_are_correct() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    let data_ptr = ptr as usize; // Send across threads

    let num_threads = 8;
    let incs_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            std::thread::spawn(move || {
                let ptr = data_ptr as *mut u8;
                for _ in 0..incs_per_thread {
                    ori_rc_inc(ptr);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    // Initial 1 + (num_threads * incs_per_thread) increments
    let expected = 1 + i64::from(num_threads * incs_per_thread);
    assert_eq!(ori_rc_count(ptr), expected);

    // Clean up: decrement back to zero, then free.
    for _ in 0..expected {
        ori_rc_dec(ptr, None);
    }
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn concurrent_inc_and_dec_are_correct() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    let data_ptr = ptr as usize;

    // Start with extra refs so decrements don't hit zero mid-test
    let extra_refs = 10_000;
    for _ in 0..extra_refs {
        ori_rc_inc(ptr);
    }
    // Count is now 1 + extra_refs = 10_001

    let num_threads = 8;
    let ops_per_thread = 1000;

    // Half the threads increment, half decrement
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            std::thread::spawn(move || {
                let ptr = data_ptr as *mut u8;
                for _ in 0..ops_per_thread {
                    if i % 2 == 0 {
                        ori_rc_inc(ptr);
                    } else {
                        ori_rc_dec(ptr, None);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    // 4 threads increment (4 * 1000 = 4000), 4 threads decrement (4 * 1000 = 4000)
    // Net change = 0, so count should still be 1 + extra_refs
    let expected = 1 + i64::from(extra_refs);
    assert_eq!(ori_rc_count(ptr), expected);

    // Clean up: decrement back to zero, then free.
    for _ in 0..expected {
        ori_rc_dec(ptr, None);
    }
    ori_rc_free(ptr, 16, 8);
}

/// Global counter for concurrent drop tracking.
static CONCURRENT_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

extern "C" fn concurrent_test_drop_fn(data_ptr: *mut u8) {
    CONCURRENT_DROP_COUNT.fetch_add(1, Ordering::SeqCst);
    ori_rc_free(data_ptr, 16, 8);
}

// ── Leak detection (RC_LIVE_COUNT) ────────────────────────────────────

#[test]
fn rc_live_count_tracks_alloc_and_free() {
    let _g = lock_rc();
    let before = ori_rc_live_count();

    let ptr = ori_rc_alloc(16, 8);
    assert_eq!(ori_rc_live_count(), before + 1);

    ori_rc_free(ptr, 16, 8);
    assert_eq!(ori_rc_live_count(), before);
}

#[test]
fn rc_live_count_nonzero_after_alloc_without_free() {
    let _g = lock_rc();
    let before = ori_rc_live_count();

    let ptr = ori_rc_alloc(16, 8);
    assert!(
        ori_rc_live_count() > before,
        "live count should increase after allocation"
    );

    // Clean up so we don't pollute other tests
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_reset_live_count_zeroes_counter() {
    let _g = lock_rc();

    // Allocate without freeing
    let _ptr = ori_rc_alloc(16, 8);

    ori_rc_reset_live_count();
    assert_eq!(
        ori_rc_live_count(),
        0,
        "live count should be zero after reset"
    );

    // Note: the allocation is now leaked — this is intentional for testing
    // the reset mechanism. The leaked memory is small and the test process
    // exits shortly after.
}

#[test]
fn concurrent_dec_triggers_drop_exactly_once() {
    let _g = lock_rc();
    CONCURRENT_DROP_COUNT.store(0, Ordering::SeqCst);

    let ptr = ori_rc_alloc(16, 8);
    let data_ptr = ptr as usize;

    let num_threads = 8;
    // Give each thread exactly 1 ref to release
    for _ in 0..num_threads - 1 {
        ori_rc_inc(ptr);
    }
    // Count is now num_threads (8)

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            std::thread::spawn(move || {
                let ptr = data_ptr as *mut u8;
                ori_rc_dec(ptr, Some(concurrent_test_drop_fn));
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    // Exactly one thread should have triggered the drop
    assert_eq!(CONCURRENT_DROP_COUNT.load(Ordering::SeqCst), 1);
}

// ── Overflow detection ────────────────────────────────────────────────

#[test]
fn rc_inc_does_not_overflow_under_normal_use() {
    // Verify that many increments (but far below MAX_REFCOUNT) work fine.
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    assert_eq!(ori_rc_count(ptr), 1);

    // Increment 1000 times — well within bounds
    for _ in 0..1000 {
        ori_rc_inc(ptr);
    }
    assert_eq!(ori_rc_count(ptr), 1001);

    // Clean up
    for _ in 0..1001 {
        ori_rc_dec(ptr, None);
    }
    ori_rc_free(ptr, 16, 8);
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "subprocess test pattern requires infallible exe/output"
)]
fn rc_overflow_aborts_process() {
    // We can't actually increment to isize::MAX (would take years), but we
    // can verify the mechanism by directly setting the refcount near the
    // limit and confirming that ori_rc_inc aborts the child process.
    use std::process::Command;

    let result =
        Command::new(std::env::current_exe().expect("could not determine test binary path"))
            .arg("--exact")
            .arg("tests::rc_overflow_aborts_process_child")
            .env("ORI_RC_OVERFLOW_TEST", "1")
            .output()
            .expect("failed to spawn child process");

    // The child should have been killed by abort (signal) or exited non-zero
    assert!(
        !result.status.success(),
        "child process should have aborted on overflow, but exited successfully"
    );
}

/// Helper test that is only run as a subprocess by `rc_overflow_aborts_process`.
///
/// Directly manipulates the refcount header to near `MAX_REFCOUNT`, then
/// calls `ori_rc_inc` which should trigger abort.
#[test]
fn rc_overflow_aborts_process_child() {
    if std::env::var("ORI_RC_OVERFLOW_TEST").is_err() {
        // Only run when invoked as a subprocess
        return;
    }

    let ptr = ori_rc_alloc(16, 8);

    // Directly write MAX_REFCOUNT into the refcount header
    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        *rc_ptr = MAX_REFCOUNT;
    }

    // This should trigger the overflow abort
    ori_rc_inc(ptr);

    // Should never reach here
    unreachable!("ori_rc_inc should have aborted");
}

// Compile-time verification that MAX_REFCOUNT is correctly defined.
const _: () = {
    assert!(MAX_REFCOUNT == isize::MAX as i64);
    assert!(MAX_REFCOUNT > 0);
};

// ── Uniqueness check (COW foundation) ────────────────────────────────

#[test]
fn rc_is_unique_freshly_allocated() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    assert!(
        ori_rc_is_unique(ptr),
        "freshly allocated block should be unique (RC=1)"
    );
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_is_unique_false_after_inc() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    ori_rc_inc(ptr);
    assert!(
        !ori_rc_is_unique(ptr),
        "should not be unique after increment (RC=2)"
    );

    // Dec back to 1 — should be unique again
    ori_rc_dec(ptr, None);
    assert!(
        ori_rc_is_unique(ptr),
        "should be unique again after decrement back to RC=1"
    );
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_is_unique_null_returns_false() {
    // Null pointers (empty collection sentinels) are never unique
    assert!(
        !ori_rc_is_unique(std::ptr::null()),
        "null pointer should never be unique"
    );
}

#[test]
fn rc_is_unique_or_null_null_returns_true() {
    // Sentinels: null is "unique or null" → true
    assert!(
        ori_rc_is_unique_or_null(std::ptr::null()),
        "null pointer should return true for is_unique_or_null"
    );
}

#[test]
fn rc_is_unique_or_null_unique_returns_true() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    assert!(
        ori_rc_is_unique_or_null(ptr),
        "unique allocation should return true for is_unique_or_null"
    );
    ori_rc_free(ptr, 16, 8);
}

#[test]
fn rc_is_unique_or_null_shared_returns_false() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);
    ori_rc_inc(ptr);
    assert!(
        !ori_rc_is_unique_or_null(ptr),
        "shared allocation (RC=2) should return false for is_unique_or_null"
    );
    ori_rc_dec(ptr, None);
    ori_rc_free(ptr, 16, 8);
}

// ── Capacity management primitives (COW foundation) ──────────────────

#[test]
fn rc_realloc_preserves_data_on_growth() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(16, 8);

    // Write a known pattern to the data area
    unsafe {
        for i in 0..16u8 {
            *ptr.add(i as usize) = i + 1;
        }
    }
    assert_eq!(ori_rc_count(ptr), 1);

    // Grow from 16 to 64 bytes
    let new_ptr = ori_rc_realloc(ptr, 16, 64, 8);
    assert!(!new_ptr.is_null());

    // Refcount should be preserved
    assert_eq!(ori_rc_count(new_ptr), 1);

    // Original data should be preserved
    unsafe {
        for i in 0..16u8 {
            assert_eq!(
                *new_ptr.add(i as usize),
                i + 1,
                "data at offset {i} should be preserved after realloc"
            );
        }
    }

    ori_rc_free(new_ptr, 64, 8);
}

#[test]
fn rc_realloc_preserves_data_on_shrink() {
    let _g = lock_rc();
    let ptr = ori_rc_alloc(64, 8);

    // Write pattern to the first 16 bytes
    unsafe {
        for i in 0..16u8 {
            *ptr.add(i as usize) = 0xAA + i;
        }
    }

    // Shrink from 64 to 16 bytes
    let new_ptr = ori_rc_realloc(ptr, 64, 16, 8);
    assert!(!new_ptr.is_null());

    // First 16 bytes should still be intact
    unsafe {
        for i in 0..16u8 {
            assert_eq!(
                *new_ptr.add(i as usize),
                0xAA + i,
                "data at offset {i} should survive shrink"
            );
        }
    }

    assert_eq!(ori_rc_count(new_ptr), 1);
    ori_rc_free(new_ptr, 16, 8);
}

#[test]
fn rc_realloc_null_returns_null() {
    assert!(ori_rc_realloc(std::ptr::null_mut(), 0, 64, 8).is_null());
}

#[test]
fn memcpy_elements_copies_correctly() {
    let src: [u64; 4] = [10, 20, 30, 40];
    let mut dst: [u64; 4] = [0; 4];

    ori_memcpy_elements(
        dst.as_mut_ptr().cast(),
        src.as_ptr().cast(),
        4,
        std::mem::size_of::<u64>(),
    );

    assert_eq!(dst, [10, 20, 30, 40]);
}

#[test]
fn memcpy_elements_zero_count_is_noop() {
    let src: [u64; 4] = [10, 20, 30, 40];
    let mut dst: [u64; 4] = [0; 4];

    ori_memcpy_elements(
        dst.as_mut_ptr().cast(),
        src.as_ptr().cast(),
        0,
        std::mem::size_of::<u64>(),
    );

    assert_eq!(dst, [0, 0, 0, 0], "zero count should not copy anything");
}

#[test]
fn memmove_elements_handles_overlap() {
    // Simulate shifting elements right: [1, 2, 3, 4, 0] → [1, 1, 2, 3, 0]
    let mut buf: [u64; 5] = [1, 2, 3, 4, 0];
    let elem_size = std::mem::size_of::<u64>();

    // Move 3 elements from index 0 to index 1 (overlapping)
    let src = buf.as_ptr().cast::<u8>();
    let dst = unsafe { buf.as_mut_ptr().add(1).cast::<u8>() };

    ori_memmove_elements(dst, src, 3, elem_size);

    assert_eq!(
        buf,
        [1, 1, 2, 3, 0],
        "overlapping move should work correctly"
    );
}

#[test]
fn memmove_elements_zero_count_is_noop() {
    let mut buf: [u64; 4] = [1, 2, 3, 4];
    let elem_size = std::mem::size_of::<u64>();

    ori_memmove_elements(buf.as_mut_ptr().cast(), buf.as_ptr().cast(), 0, elem_size);

    assert_eq!(buf, [1, 2, 3, 4], "zero count should not move anything");
}

// ── Growth strategy (COW foundation) ─────────────────────────────────

#[test]
fn next_capacity_from_zero() {
    // From empty, requesting 1 → should get MIN_COLLECTION_CAPACITY (4)
    assert_eq!(next_capacity(0, 1), 4);
}

#[test]
fn next_capacity_doubles() {
    assert_eq!(next_capacity(4, 5), 8);
    assert_eq!(next_capacity(8, 9), 16);
    assert_eq!(next_capacity(16, 17), 32);
}

#[test]
fn next_capacity_required_exceeds_double() {
    // required (100) > doubled (16), so use required directly
    assert_eq!(next_capacity(8, 100), 100);
}

#[test]
fn next_capacity_overflow_saturates() {
    let huge = usize::MAX / 2 + 1;
    // Doubling huge would overflow — saturating_mul returns usize::MAX
    let result = next_capacity(huge, 1);
    assert!(
        result >= huge,
        "should not wrap around: got {result}, expected >= {huge}"
    );
}

#[test]
fn list_ensure_capacity_grows_from_sentinel() {
    let mut list = OriList {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    };

    // Request capacity for 1 element (8 bytes each, 8-byte aligned)
    ori_list_ensure_capacity(&mut list, 1, 8, 8);

    assert!(
        !list.data.is_null(),
        "sentinel should be replaced with real allocation"
    );
    assert!(
        list.cap >= 4,
        "should get at least MIN_COLLECTION_CAPACITY (4), got {}",
        list.cap
    );

    // Clean up — data buffers are plain-allocated (ori_alloc), not RC-allocated
    ori_free(list.data, list.cap as usize * 8, 8);
}

#[test]
fn list_ensure_capacity_noop_when_sufficient() {
    // Allocate a data buffer with capacity 8 (plain alloc, not RC)
    let data = ori_alloc(8 * 8, 8); // 8 elements × 8 bytes
    let mut list = OriList {
        len: 3,
        cap: 8,
        data,
    };

    let old_data = list.data;

    // Request capacity for 5 — already have 8, should be no-op
    ori_list_ensure_capacity(&mut list, 5, 8, 8);

    assert_eq!(
        list.data, old_data,
        "should not reallocate when capacity is sufficient"
    );
    assert_eq!(list.cap, 8);

    ori_free(list.data, 8 * 8, 8);
}

#[test]
fn list_ensure_capacity_grows_buffer() {
    // Data buffers are plain-allocated (ori_alloc), not RC-allocated
    let data = ori_alloc(4 * 8, 8); // 4 elements × 8 bytes
    let mut list = OriList {
        len: 4,
        cap: 4,
        data,
    };

    // Write known data
    unsafe {
        for i in 0..4i64 {
            *list.data.cast::<i64>().add(i as usize) = (i + 1) * 10;
        }
    }

    // Request capacity for 5 — should double to 8
    ori_list_ensure_capacity(&mut list, 5, 8, 8);

    assert!(
        list.cap >= 8,
        "should have grown to at least 8, got {}",
        list.cap
    );

    // Verify data survived the realloc
    unsafe {
        for i in 0..4i64 {
            let val = *(list.data as *const i64).add(i as usize);
            assert_eq!(val, (i + 1) * 10, "data at index {i} should survive growth");
        }
    }

    ori_free(list.data, list.cap as usize * 8, 8);
}

// ── Empty collection sentinels (COW foundation) ──────────────────────

#[test]
fn list_empty_sentinel_is_null() {
    let ptr = ori_list_empty();
    assert!(ptr.is_null(), "empty list sentinel should be null pointer");
}

#[test]
fn str_empty_sentinel_is_null() {
    let s = ori_str_empty();
    assert_eq!(s.len, 0);
    assert!(s.data.is_null(), "sentinel str data should be null");
}

#[test]
fn map_empty_sentinel_is_null() {
    let m = ori_map_empty();
    assert_eq!(m.len, 0);
    assert_eq!(m.cap, 0);
    assert!(m.keys.is_null(), "sentinel map keys should be null");
    assert!(m.values.is_null(), "sentinel map values should be null");
}

#[test]
fn set_empty_sentinel_is_null() {
    let ptr = ori_set_empty();
    assert!(ptr.is_null(), "empty set sentinel should be null pointer");
}

#[test]
fn sentinel_rc_operations_are_noop() {
    // Verify that null pointers (sentinels) are safe with all RC ops
    let null: *mut u8 = std::ptr::null_mut();

    // These must not crash
    ori_rc_inc(null);
    ori_rc_dec(null, None);

    // Uniqueness: null is never unique (forces allocation on first mutation)
    assert!(!ori_rc_is_unique(null as *const u8));
    assert_eq!(ori_rc_count(null as *const u8), 0);
}

// ── Boxed list functions (ori_list_box_new) ──────────────────────────

#[test]
fn list_box_new_creates_rc_struct() {
    let _g = lock_rc();
    let data = ori_alloc(32, 8); // 4 × 8 bytes
    assert!(!data.is_null());

    let box_ptr = ori_list_box_new(3, 4, data);
    assert!(!box_ptr.is_null(), "box_new should return non-null");
    assert_eq!(ori_rc_count(box_ptr), 1, "fresh box has RC=1");

    // Read fields back via OriList cast
    let list = unsafe { &*box_ptr.cast::<OriList>() };
    assert_eq!(list.len, 3);
    assert_eq!(list.cap, 4);
    assert_eq!(list.data, data);

    // Clean up
    ori_free(data, 32, 8);
    ori_rc_free(
        box_ptr,
        std::mem::size_of::<OriList>(),
        std::mem::align_of::<OriList>(),
    );
}

#[test]
fn list_box_new_round_trip_with_data() {
    let _g = lock_rc();

    // Write known pattern to data buffer
    let data = ori_alloc(3 * 8, 8);
    unsafe {
        *data.cast::<i64>() = 10;
        *data.cast::<i64>().add(1) = 20;
        *data.cast::<i64>().add(2) = 30;
    }

    let box_ptr = ori_list_box_new(3, 3, data);
    let list = unsafe { &*box_ptr.cast::<OriList>() };

    // Verify data is accessible through the box
    unsafe {
        assert_eq!(*(list.data as *const i64), 10);
        assert_eq!(*(list.data as *const i64).add(1), 20);
        assert_eq!(*(list.data as *const i64).add(2), 30);
    }

    ori_free(data, 3 * 8, 8);
    ori_rc_free(
        box_ptr,
        std::mem::size_of::<OriList>(),
        std::mem::align_of::<OriList>(),
    );
}
