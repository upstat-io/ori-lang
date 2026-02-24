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
