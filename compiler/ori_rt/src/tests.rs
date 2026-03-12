//! Tests for `ori_rt` core functions (memory, refcounting, panic).
//!
//! All RC tests acquire `lock_rc()` (from `crate::test_helpers`) because
//! `RC_LIVE_COUNT` is a global atomic counter modified by
//! `ori_rc_alloc`/`ori_rc_free`. Without serialization, parallel tests
//! cause TOCTOU races in live-count assertions.
#![expect(clippy::unwrap_used, reason = "test code uses unwrap for clarity")]
#![expect(clippy::expect_used, reason = "test code uses expect for clarity")]
#![expect(
    clippy::items_after_statements,
    reason = "inner helper fns in test functions are idiomatic"
)]

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

use crate::test_helpers::lock_rc;

/// Free a heap `OriStr`'s RC allocation in tests.
///
/// Calls `ori_rc_free` with the capacity (= actual allocation size), which
/// decrements `RC_LIVE_COUNT` and frees memory. No-op for SSO strings.
fn free_heap_str(s: &OriStr) {
    if let Some(ptr) = s.heap_data_ptr() {
        let alloc_size = unsafe { s.heap.cap as usize };
        ori_rc_free(ptr, alloc_size, 1);
    }
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
fn rc_inc_skips_at_max_refcount() {
    // Immortal objects have refcount set to MAX_REFCOUNT.
    // ori_rc_inc should be a no-op — refcount stays at MAX_REFCOUNT.
    let ptr = ori_rc_alloc(16, 8);

    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        *rc_ptr = MAX_REFCOUNT;
    }

    // This should skip (no-op), not abort or increment.
    ori_rc_inc(ptr);

    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        assert_eq!(
            *rc_ptr, MAX_REFCOUNT,
            "refcount should remain at MAX_REFCOUNT after ori_rc_inc"
        );
    }

    // Clean up: reset refcount to 1 so we can free.
    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        *rc_ptr = 1;
    }
    ori_rc_dec(ptr, None);
}

#[test]
fn rc_dec_skips_at_max_refcount() {
    // Immortal objects have refcount set to MAX_REFCOUNT.
    // ori_rc_dec should be a no-op — refcount stays at MAX_REFCOUNT.
    let ptr = ori_rc_alloc(16, 8);

    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        *rc_ptr = MAX_REFCOUNT;
    }

    // This should skip (no-op), not decrement or free.
    ori_rc_dec(ptr, None);

    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        assert_eq!(
            *rc_ptr, MAX_REFCOUNT,
            "refcount should remain at MAX_REFCOUNT after ori_rc_dec"
        );
    }

    // Clean up: reset refcount to 1 so we can free.
    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        *rc_ptr = 1;
    }
    ori_rc_dec(ptr, None);
}

// Compile-time verification that MAX_REFCOUNT is correctly defined.
const _: () = {
    assert!(MAX_REFCOUNT == isize::MAX as i64);
    assert!(MAX_REFCOUNT > 0);
};

// ── RC Event Tracing ────────────────────────────────────────────────

#[test]
fn rc_trace_disabled_by_default() {
    // When ORI_TRACE_RC is not set, tracing should be disabled.
    assert!(
        !rc_trace_enabled(),
        "rc_trace_enabled() should return false when ORI_TRACE_RC is not set"
    );
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "subprocess test pattern requires infallible exe/output"
)]
fn rc_trace_produces_balanced_sequence() {
    use std::process::Command;

    let result =
        Command::new(std::env::current_exe().expect("could not determine test binary path"))
            .arg("--exact")
            .arg("tests::rc_trace_produces_balanced_sequence_child")
            .arg("--nocapture")
            .env("ORI_RC_TRACE_TEST", "1")
            .env("ORI_TRACE_RC", "1")
            .output()
            .expect("failed to spawn child process");

    assert!(
        result.status.success(),
        "child should exit successfully, stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stderr = String::from_utf8_lossy(&result.stderr);
    let rc_lines: Vec<&str> = stderr.lines().filter(|l| l.starts_with("[RC]")).collect();

    // Expected sequence: alloc, inc, dec, dec(FREE), free
    assert_eq!(
        rc_lines.len(),
        5,
        "expected 5 RC trace lines, got {}:\n{}",
        rc_lines.len(),
        rc_lines.join("\n")
    );

    // Verify event types in order
    assert!(rc_lines[0].contains("alloc"), "line 0: {}", rc_lines[0]);
    assert!(rc_lines[0].contains("rc=1"), "alloc rc=1: {}", rc_lines[0]);
    assert!(
        rc_lines[0].contains("size=16"),
        "alloc size: {}",
        rc_lines[0]
    );
    assert!(
        rc_lines[0].contains("live=1"),
        "alloc live=1: {}",
        rc_lines[0]
    );

    assert!(rc_lines[1].contains("inc"), "line 1: {}", rc_lines[1]);
    assert!(rc_lines[1].contains("rc=2"), "inc rc=2: {}", rc_lines[1]);

    assert!(rc_lines[2].contains("dec"), "line 2: {}", rc_lines[2]);
    assert!(rc_lines[2].contains("rc=1"), "dec rc=1: {}", rc_lines[2]);

    assert!(rc_lines[3].contains("dec"), "line 3: {}", rc_lines[3]);
    assert!(rc_lines[3].contains("rc=0"), "dec rc=0: {}", rc_lines[3]);
    assert!(rc_lines[3].contains("FREE"), "dec FREE: {}", rc_lines[3]);

    assert!(rc_lines[4].contains("free"), "line 4: {}", rc_lines[4]);
    assert!(
        rc_lines[4].contains("size=16"),
        "free size: {}",
        rc_lines[4]
    );
    assert!(
        rc_lines[4].contains("live=0"),
        "free live=0: {}",
        rc_lines[4]
    );

    // All lines reference the same pointer.
    // Extract the "0x..." substring up to the next space.
    let alloc_line = rc_lines[0];
    let ptr_start = alloc_line
        .find("0x")
        .expect("alloc line should contain pointer");
    let ptr_end = alloc_line[ptr_start..]
        .find(' ')
        .map_or(alloc_line.len(), |i| ptr_start + i);
    let ptr = &alloc_line[ptr_start..ptr_end];
    for (i, line) in rc_lines.iter().enumerate() {
        assert!(
            line.contains(ptr),
            "line {i} should reference same pointer {ptr}: {line}"
        );
    }
}

/// Subprocess helper for `rc_trace_produces_balanced_sequence`.
#[test]
fn rc_trace_produces_balanced_sequence_child() {
    if std::env::var("ORI_RC_TRACE_TEST").is_err() {
        return;
    }

    let ptr = ori_rc_alloc(16, 8); // alloc → rc=1
    ori_rc_inc(ptr); // inc → rc=2
    ori_rc_dec(ptr, None); // dec → rc=1
    ori_rc_dec(ptr, None); // dec → rc=0 FREE (no drop_fn)
    ori_rc_free(ptr, 16, 8); // free
}

// ── Leak Attribution ─────────────────────────────────────────────────

#[test]
#[expect(
    clippy::expect_used,
    reason = "subprocess test pattern requires infallible exe/output"
)]
fn leak_attribution_reports_unfreed_allocations() {
    use std::process::Command;

    let result =
        Command::new(std::env::current_exe().expect("could not determine test binary path"))
            .arg("--exact")
            .arg("tests::leak_attribution_child")
            .arg("--nocapture")
            .env("ORI_LEAK_ATTRIB_TEST", "1")
            .env("ORI_CHECK_LEAKS", "1")
            .output()
            .expect("failed to spawn child process");

    // ori_run_main returns exit code 2 when leaks detected
    assert_eq!(
        result.status.code(),
        Some(2),
        "child should exit with code 2 (leak detected), stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let stderr = String::from_utf8_lossy(&result.stderr);

    // Should report 2 leaked allocations
    assert!(
        stderr.contains("2 RC allocation(s) not freed"),
        "should report 2 leaks: {stderr}"
    );

    // In debug builds, attribution lines include alloc_id, ptr, size, align
    #[cfg(debug_assertions)]
    {
        // Attribution lines start with "  #"
        let attrib_lines: Vec<&str> = stderr.lines().filter(|l| l.starts_with("  #")).collect();
        assert_eq!(
            attrib_lines.len(),
            2,
            "expected 2 attribution lines, got {}:\n{}",
            attrib_lines.len(),
            attrib_lines.join("\n")
        );

        // Each line should contain ptr=, size=, align=, (unfreed)
        for line in &attrib_lines {
            assert!(line.contains("ptr=0x"), "missing ptr: {line}");
            assert!(line.contains("size="), "missing size: {line}");
            assert!(line.contains("align="), "missing align: {line}");
            assert!(line.contains("(unfreed)"), "missing (unfreed): {line}");
        }

        // First allocation (24 bytes), second allocation (16 bytes) — sorted by ID
        assert!(
            attrib_lines[0].contains("#0"),
            "first attribution should be #0: {}",
            attrib_lines[0]
        );
        assert!(
            attrib_lines[0].contains("size=24"),
            "first leak should be 24 bytes: {}",
            attrib_lines[0]
        );
        assert!(
            attrib_lines[1].contains("#1"),
            "second attribution should be #1: {}",
            attrib_lines[1]
        );
        assert!(
            attrib_lines[1].contains("size=16"),
            "second leak should be 16 bytes: {}",
            attrib_lines[1]
        );
    }
}

/// Subprocess helper for `leak_attribution_reports_unfreed_allocations`.
///
/// Deliberately leaks 2 allocations and frees 1, then exits via `ori_run_main`
/// so the leak report is printed.
#[test]
fn leak_attribution_child() {
    if std::env::var("ORI_LEAK_ATTRIB_TEST").is_err() {
        return;
    }

    extern "C" fn leaky_main() {
        let ptr1 = ori_rc_alloc(24, 8); // alloc #0 — leaked
        let ptr2 = ori_rc_alloc(16, 8); // alloc #1 — leaked
        let ptr3 = ori_rc_alloc(32, 8); // alloc #2 — freed

        // Free only ptr3 (via dec to zero + free)
        ori_rc_dec(ptr3, None);
        ori_rc_free(ptr3, 32, 8);

        // ptr1 and ptr2 are deliberately leaked
        let _ = (ptr1, ptr2);
    }

    let exit_code = ori_run_main(leaky_main);
    std::process::exit(exit_code);
}

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
    let _g = lock_rc();
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

    // Clean up — data buffers are RC-allocated via ori_list_alloc_data
    ori_list_free_data(list.data, list.cap, 8);
}

#[test]
fn list_ensure_capacity_noop_when_sufficient() {
    let _g = lock_rc();
    // Allocate a data buffer with capacity 8 (RC-allocated)
    let data = ori_list_alloc_data(8, 8); // 8 elements × 8 bytes
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

    ori_list_free_data(list.data, list.cap, 8);
}

#[test]
fn list_ensure_capacity_grows_buffer() {
    let _g = lock_rc();
    // Data buffers are RC-allocated via ori_list_alloc_data
    let data = ori_list_alloc_data(4, 8); // 4 elements × 8 bytes
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

    ori_list_free_data(list.data, list.cap, 8);
}

// ── Empty collection sentinels (COW foundation) ──────────────────────

#[test]
fn list_empty_sentinel_is_null() {
    let ptr = ori_list_empty();
    assert!(ptr.is_null(), "empty list sentinel should be null pointer");
}

// SSO (Small String Optimization) tests

#[test]
fn sso_layout_is_24_bytes() {
    assert_eq!(std::mem::size_of::<OriStr>(), 24);
    assert_eq!(std::mem::align_of::<OriStr>(), 8);
}

#[test]
fn sso_empty_string() {
    let s = OriStr::EMPTY;
    assert!(s.is_sso(), "empty string should be SSO");
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert_eq!(s.as_bytes(), &[]);
    assert_eq!(unsafe { s.as_str() }, "");
    assert!(s.heap_data_ptr().is_none(), "SSO has no heap data");
}

#[test]
fn sso_empty_sentinel_matches_const() {
    let s = ori_str_empty();
    assert!(s.is_sso());
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
}

#[test]
fn sso_one_byte_string() {
    let s = OriStr::from_sso(b"x");
    assert!(s.is_sso());
    assert_eq!(s.len(), 1);
    assert!(!s.is_empty());
    assert_eq!(s.as_bytes(), b"x");
    assert_eq!(unsafe { s.as_str() }, "x");
    assert!(s.heap_data_ptr().is_none());
}

#[test]
fn sso_max_length_23_bytes() {
    let data = b"abcdefghijklmnopqrstuvw"; // exactly 23 bytes
    assert_eq!(data.len(), 23);
    let s = OriStr::from_sso(data);
    assert!(s.is_sso());
    assert_eq!(s.len(), 23);
    assert_eq!(s.as_bytes(), data.as_slice());
    assert_eq!(unsafe { s.as_str() }, "abcdefghijklmnopqrstuvw");
    assert!(s.heap_data_ptr().is_none());
}

#[test]
fn sso_from_bytes_chooses_sso_for_short() {
    let s = OriStr::from_bytes(b"hello");
    assert!(s.is_sso());
    assert_eq!(s.len(), 5);
    assert_eq!(unsafe { s.as_str() }, "hello");
}

#[test]
fn sso_from_bytes_chooses_heap_for_long() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let data = b"this is a string that exceeds twenty-three bytes";
    assert!(data.len() > 23);
    let s = OriStr::from_bytes(data);
    assert!(!s.is_sso(), "long string should be heap");
    assert_eq!(s.len(), data.len());
    assert_eq!(s.as_bytes(), data.as_slice());
    assert_eq!(
        unsafe { s.as_str() },
        "this is a string that exceeds twenty-three bytes"
    );
    assert!(s.heap_data_ptr().is_some(), "heap string has data pointer");
    // Verify RC was allocated
    let after = ori_rc_live_count();
    assert_eq!(after, before + 1, "heap string should allocate 1 RC object");
    // Clean up
    free_heap_str(&s);
}

#[test]
fn sso_from_owned_short_string() {
    let s = OriStr::from_owned("hi");
    assert!(s.is_sso());
    assert_eq!(s.len(), 2);
    assert_eq!(unsafe { s.as_str() }, "hi");
}

#[test]
fn sso_from_owned_long_string() {
    let _g = lock_rc();
    let long = "a".repeat(50);
    let s = OriStr::from_owned(&long);
    assert!(!s.is_sso());
    assert_eq!(s.len(), 50);
    assert_eq!(unsafe { s.as_str() }, &long);
    // Clean up
    free_heap_str(&s);
}

#[test]
fn sso_from_raw_short() {
    let data = b"test";
    let s = ori_str_from_raw(data.as_ptr(), 4);
    assert!(s.is_sso(), "4-byte from_raw should use SSO");
    assert_eq!(s.len(), 4);
    assert_eq!(unsafe { s.as_str() }, "test");
}

#[test]
fn sso_from_raw_empty() {
    let s = ori_str_from_raw(std::ptr::null(), 0);
    assert!(s.is_sso());
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
}

#[test]
fn sso_from_raw_long() {
    let _g = lock_rc();
    let data = b"this is definitely longer than twenty three bytes!";
    let s = ori_str_from_raw(data.as_ptr(), data.len() as i64);
    assert!(!s.is_sso());
    assert_eq!(s.len(), data.len());
    assert_eq!(
        unsafe { s.as_str() },
        "this is definitely longer than twenty three bytes!"
    );
    free_heap_str(&s);
}

#[test]
fn sso_from_bool_uses_sso() {
    let t = ori_str_from_bool(true);
    let f = ori_str_from_bool(false);
    assert!(t.is_sso(), "'true' (4 bytes) should be SSO");
    assert!(f.is_sso(), "'false' (5 bytes) should be SSO");
    assert_eq!(unsafe { t.as_str() }, "true");
    assert_eq!(unsafe { f.as_str() }, "false");
}

#[test]
fn sso_from_int_short() {
    let s = ori_str_from_int(42);
    assert!(s.is_sso(), "'42' (2 bytes) should be SSO");
    assert_eq!(unsafe { s.as_str() }, "42");
}

#[test]
fn sso_heap_data_ptr_returns_valid_pointer() {
    let _g = lock_rc();
    let s = OriStr::from_heap(b"heap string that is long enough");
    assert!(!s.is_sso());
    let ptr = s.heap_data_ptr();
    assert!(ptr.is_some());
    // Verify the pointer is valid by checking RC
    assert_eq!(ori_rc_count(ptr.unwrap()), 1);
    free_heap_str(&s);
}

#[test]
fn sso_discriminator_is_reliable() {
    // SSO strings: byte 23 has high bit set
    let empty = OriStr::EMPTY;
    let short = OriStr::from_sso(b"a");
    let max = OriStr::from_sso(&[b'z'; 23]);
    assert!(empty.is_sso());
    assert!(short.is_sso());
    assert!(max.is_sso());

    // Heap strings: byte 23 has high bit clear (canonical addressing)
    let _g = lock_rc();
    let heap = OriStr::from_heap(b"long enough to be heap allocated!!");
    assert!(!heap.is_sso());
    free_heap_str(&heap);
}

#[test]
fn sso_utf8_multibyte() {
    // UTF-8 multibyte chars: "café" = 5 bytes (c, a, f, 0xC3, 0xA9)
    let s = OriStr::from_bytes("café".as_bytes());
    assert!(s.is_sso(), "5-byte UTF-8 string should be SSO");
    assert_eq!(s.len(), 5);
    assert_eq!(unsafe { s.as_str() }, "café");

    // Emoji: "🎉" = 4 bytes
    let s = OriStr::from_bytes("🎉".as_bytes());
    assert!(s.is_sso());
    assert_eq!(s.len(), 4);
    assert_eq!(unsafe { s.as_str() }, "🎉");
}

// Capacity tracking and SSO→heap promotion tests

#[test]
fn sso_promote_to_heap() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"hello");
    assert!(s.is_sso());
    let heap = s.promote_to_heap(32);
    assert!(!heap.is_sso(), "promoted string should be heap");
    assert_eq!(heap.len(), 5);
    assert_eq!(unsafe { heap.as_str() }, "hello");
    unsafe {
        assert!(heap.heap.cap >= 32, "capacity should be at least min_cap");
    }
    free_heap_str(&heap);
}

#[test]
fn sso_promote_empty_to_heap() {
    let _g = lock_rc();
    let s = OriStr::EMPTY;
    let heap = s.promote_to_heap(16);
    assert!(!heap.is_sso());
    assert_eq!(heap.len(), 0);
    unsafe {
        assert!(heap.heap.cap >= 16);
    }
    free_heap_str(&heap);
}

#[test]
fn heap_with_capacity() {
    let _g = lock_rc();
    let s = OriStr::with_capacity(64);
    assert!(!s.is_sso(), "with_capacity always creates heap");
    assert_eq!(s.len(), 0);
    unsafe {
        assert_eq!(s.heap.cap, 64);
    }
    free_heap_str(&s);
}

#[test]
fn heap_ensure_capacity_no_realloc_when_sufficient() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let mut s = OriStr::from_heap(&[b'a'; 30]); // 30-byte heap string
    assert!(!s.is_sso());
    let old_cap = unsafe { s.heap.cap };
    let old_ptr = s.heap_data_ptr().unwrap();
    s.ensure_capacity(30); // already have capacity
    let new_ptr = s.heap_data_ptr().unwrap();
    assert_eq!(old_ptr, new_ptr, "should not reallocate");
    assert_eq!(unsafe { s.heap.cap }, old_cap);
    free_heap_str(&s);
    assert_eq!(ori_rc_live_count(), before);
}

#[test]
fn heap_ensure_capacity_reallocs_when_needed() {
    let _g = lock_rc();
    let mut s = OriStr::from_heap(&[b'b'; 30]); // cap=30
    assert_eq!(s.len(), 30);
    s.ensure_capacity(100); // need more
    unsafe {
        assert!(s.heap.cap >= 100, "cap should grow to at least 100");
    }
    assert_eq!(s.len(), 30, "length should not change");
    assert_eq!(s.as_bytes(), &[b'b'; 30], "data should be preserved");
    free_heap_str(&s);
}

#[test]
fn heap_from_heap_sets_cap_equal_to_len() {
    let _g = lock_rc();
    let s = OriStr::from_heap(b"test data here!!");
    assert!(!s.is_sso());
    unsafe {
        assert_eq!(s.heap.cap, s.heap.len, "from_heap sets cap = len");
    }
    free_heap_str(&s);
}

// SSO + SSO → SSO (combined ≤ 23 bytes)
#[test]
fn concat_sso_sso_to_sso() {
    let _g = lock_rc();
    let a = OriStr::from_sso(b"hello ");
    let b = OriStr::from_sso(b"world");
    let result = ori_str_concat(&a, &b);
    assert!(result.is_sso(), "short concat should stay SSO");
    assert_eq!(unsafe { result.as_str() }, "hello world");
    assert_eq!(result.len(), 11);
}

// SSO + SSO → heap (combined > 23 bytes)
#[test]
fn concat_sso_sso_to_heap() {
    let _g = lock_rc();
    let a = OriStr::from_sso(b"twelve chars!"); // 12 bytes
    let b = OriStr::from_sso(b"twelve chars!"); // 12 bytes → combined 24 > 23
    let result = ori_str_concat(&a, &b);
    assert!(
        !result.is_sso(),
        "combined > 23 bytes should promote to heap"
    );
    assert_eq!(unsafe { result.as_str() }, "twelve chars!twelve chars!");
    assert_eq!(result.len(), 26);
    // Per-allocation RC check avoids races with concurrent tests in other modules.
    unsafe {
        assert_eq!(
            rc::ori_rc_count(result.heap.data),
            1,
            "freshly allocated heap string should have RC=1"
        );
    }
    free_heap_str(&result);
}

// Heap unique with capacity → in-place append
#[test]
fn concat_heap_unique_with_capacity_inplace() {
    let _g = lock_rc();
    // Content must be long enough that a + b > SSO_MAX_LEN (23),
    // otherwise Case 1 (SSO) triggers before Case 2 (in-place).
    let content = b"this exceeds sso limit!"; // 23 bytes
    let mut a = OriStr::with_capacity(64);
    assert!(!a.is_sso());
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), a.heap.data, content.len());
        a.heap.len = content.len() as i64;
    }
    // Per-allocation RC check avoids races with concurrent tests in other modules
    // (list/reset, list/slice) that allocate without RC_TEST_LOCK.
    unsafe {
        assert_eq!(
            rc::ori_rc_count(a.heap.data),
            1,
            "with_capacity should create one allocation with RC=1"
        );
    }

    let b = OriStr::from_sso(b"x"); // 23 + 1 = 24 > 23 → forces heap path
    let result = ori_str_concat(&a, &b);
    assert!(!result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "this exceeds sso limit!x");
    // Should reuse the same allocation — pointer equality proves in-place append.
    // RC=2 because both `a` and `result` reference the same buffer.
    unsafe {
        assert_eq!(result.heap.data, a.heap.data, "should reuse same buffer");
        assert_eq!(
            rc::ori_rc_count(result.heap.data),
            2,
            "both a and result should reference the same buffer"
        );
    }
    free_heap_str(&result);
}

// Heap unique without capacity → fresh allocation (borrow semantics)
#[test]
fn concat_heap_unique_no_capacity_realloc() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    // from_heap sets cap = len, so no spare capacity
    let a = OriStr::from_heap(b"hello from the heap side!");
    assert!(!a.is_sso());
    assert_eq!(ori_rc_live_count(), before + 1);

    let b = OriStr::from_sso(b" more!");
    let result = ori_str_concat(&a, &b);
    assert!(!result.is_sso());
    assert_eq!(
        unsafe { result.as_str() },
        "hello from the heap side! more!"
    );
    // Borrow semantics: concat doesn't consume `a`, so both are live
    assert_eq!(ori_rc_live_count(), before + 2);
    free_heap_str(&a);
    free_heap_str(&result);
}

// Heap shared + SSO → new allocation
#[test]
fn concat_heap_shared_new_alloc() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let a = OriStr::from_heap(b"shared string data here!");
    assert!(!a.is_sso());
    // Add a second reference so it's shared (not unique)
    unsafe {
        ori_rc_inc(a.heap.data);
    }
    assert!(!ori_rc_is_unique(unsafe { a.heap.data }));

    let b = OriStr::from_sso(b" extra");
    let result = ori_str_concat(&a, &b);
    assert!(!result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "shared string data here! extra");
    // Should be a new allocation (2 live: original shared + new)
    assert_eq!(ori_rc_live_count(), before + 2);

    // Clean up: dec the extra ref on a, then free both
    ori_rc_dec(unsafe { a.heap.data }, None);
    free_heap_str(&a);
    free_heap_str(&result);
}

// Empty + any → return other side (no allocation)
#[test]
fn concat_empty_returns_other() {
    let _g = lock_rc();
    let before = ori_rc_live_count();

    let a = OriStr::EMPTY;
    let b = OriStr::from_sso(b"hello");
    let result = ori_str_concat(&a, &b);
    assert_eq!(unsafe { result.as_str() }, "hello");
    assert_eq!(
        ori_rc_live_count(),
        before,
        "empty + sso should not allocate"
    );

    let result2 = ori_str_concat(&b, &a);
    assert_eq!(unsafe { result2.as_str() }, "hello");
    assert_eq!(
        ori_rc_live_count(),
        before,
        "sso + empty should not allocate"
    );
}

// Null pointers handled safely
#[test]
fn concat_null_pointers() {
    let _g = lock_rc();
    let a = OriStr::from_sso(b"test");
    let result = ori_str_concat(&a, std::ptr::null());
    assert_eq!(unsafe { result.as_str() }, "test");

    let result2 = ori_str_concat(std::ptr::null(), &a);
    assert_eq!(unsafe { result2.as_str() }, "test");

    let result3 = ori_str_concat(std::ptr::null(), std::ptr::null());
    assert_eq!(unsafe { result3.as_str() }, "");
}

// Chain of 100 concats on unique string with proper cleanup (borrow semantics)
#[test]
fn concat_chain_amortized_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    // Start > 23 bytes so every concat stays in heap territory
    let seed = b"this seed exceeds sso!!"; // 23 bytes
    let mut s = OriStr::from_heap(seed);
    let chunk = OriStr::from_sso(b"x");
    for _ in 0..100 {
        let old = s;
        s = ori_str_concat(&old, &chunk);
        // Borrow semantics: concat doesn't consume `old`, so we must release it.
        // Case 2 (in-place append) shares data pointer → RC was inc'd to 2,
        // so dec brings to 1. Case 4 (fresh alloc) leaves old at RC=1, so
        // free releases it.
        if let Some(old_ptr) = old.heap_data_ptr() {
            let new_ptr = s.heap_data_ptr();
            if new_ptr == Some(old_ptr) {
                // Case 2: shared allocation, just dec (RC: 2 → 1)
                ori_rc_dec(old_ptr, None);
            } else {
                // Case 4: different allocation, free the old one
                free_heap_str(&old);
            }
        }
    }
    assert_eq!(s.len(), seed.len() + 100);
    assert!(unsafe { s.as_str() }.starts_with("this seed exceeds sso!!"));
    assert_eq!(
        unsafe { &s.as_str()[seed.len()..] },
        "x".repeat(100).as_str()
    );
    // After proper cleanup, only the final string is live
    assert_eq!(
        ori_rc_live_count(),
        before + 1,
        "chain with cleanup should end with single allocation"
    );
    free_heap_str(&s);
}

// Verify capacity grows with amortized doubling
#[test]
fn concat_capacity_growth_is_amortized() {
    let _g = lock_rc();
    // Both sides must combine to > 23 bytes, so we exercise the heap path
    let a = OriStr::from_heap(b"seed data exceeding sso!"); // 24 bytes, cap = len = 24
    let b = OriStr::from_sso(b"!");
    let result = ori_str_concat(&a, &b);
    assert!(!result.is_sso());
    unsafe {
        // Fresh allocation uses next_capacity for amortized growth
        assert!(
            result.heap.cap > result.heap.len,
            "capacity {} should exceed len {} after growth",
            result.heap.cap,
            result.heap.len,
        );
    }
    // Borrow semantics: both `a` and `result` are live
    free_heap_str(&a);
    free_heap_str(&result);
}

// UTF-8 multibyte concat stays valid
#[test]
fn concat_utf8_multibyte() {
    let _g = lock_rc();
    let a = OriStr::from_sso("héllo".as_bytes());
    let b = OriStr::from_sso(" wörld".as_bytes());
    let result = ori_str_concat(&a, &b);
    assert_eq!(unsafe { result.as_str() }, "héllo wörld");
    // "héllo" = 6 bytes, " wörld" = 7 bytes → 13 ≤ 23 → SSO
    assert!(result.is_sso());
    assert_eq!(result.len(), 13);
}

// ── COW string transforms ──────────────────────────────────────────────

// to_uppercase: SSO ASCII → mutate SSO bytes in place
#[test]
fn uppercase_sso_ascii() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"hello");
    let result = ori_str_to_uppercase(&s);
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "HELLO");
}

// to_uppercase: heap unique ASCII → in-place mutation
#[test]
fn uppercase_heap_unique_inplace() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_heap(b"hello from the heap side!!");
    assert!(!s.is_sso());
    assert_eq!(ori_rc_live_count(), before + 1);

    let result = ori_str_to_uppercase(&s);
    assert!(!result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "HELLO FROM THE HEAP SIDE!!");
    // In-place: no new allocation
    assert_eq!(ori_rc_live_count(), before + 1);
    free_heap_str(&result);
}

// to_uppercase: heap shared ASCII → new allocation
#[test]
fn uppercase_heap_shared_new_alloc() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_heap(b"shared string on heap!!!");
    unsafe {
        ori_rc_inc(s.heap.data);
    } // make shared
    assert_eq!(ori_rc_live_count(), before + 1);

    let result = ori_str_to_uppercase(&s);
    assert_eq!(unsafe { result.as_str() }, "SHARED STRING ON HEAP!!!");
    // New allocation for the result
    assert_eq!(ori_rc_live_count(), before + 2);

    ori_rc_dec(unsafe { s.heap.data }, None);
    free_heap_str(&s);
    free_heap_str(&result);
}

// to_uppercase: non-ASCII falls through to Rust's to_uppercase
#[test]
fn uppercase_non_ascii() {
    let _g = lock_rc();
    let s = OriStr::from_sso("café".as_bytes());
    let result = ori_str_to_uppercase(&s);
    assert_eq!(unsafe { result.as_str() }, "CAFÉ");
}

// to_lowercase: SSO ASCII
#[test]
fn lowercase_sso_ascii() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"HELLO");
    let result = ori_str_to_lowercase(&s);
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "hello");
}

// to_lowercase: heap unique ASCII → in-place
#[test]
fn lowercase_heap_unique_inplace() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_heap(b"HELLO FROM THE HEAP SIDE!!");
    let result = ori_str_to_lowercase(&s);
    assert_eq!(unsafe { result.as_str() }, "hello from the heap side!!");
    assert_eq!(ori_rc_live_count(), before + 1);
    free_heap_str(&result);
}

// trim: removes whitespace, returns SSO for short results
#[test]
fn trim_removes_whitespace() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"  hello  ");
    let result = ori_str_trim(&s);
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "hello");
}

// trim: empty result
#[test]
fn trim_all_whitespace() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"   ");
    let result = ori_str_trim(&s);
    assert!(result.is_sso());
    assert_eq!(result.len(), 0);
}

// replace: same-length on unique heap → in-place
#[test]
fn replace_same_length_heap_inplace() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_heap(b"hello world, hello earth!");
    let from = OriStr::from_sso(b"hello");
    let to = OriStr::from_sso(b"HELLO");
    let result = ori_str_replace(&s, &from, &to);
    assert_eq!(unsafe { result.as_str() }, "HELLO world, HELLO earth!");
    assert_eq!(ori_rc_live_count(), before + 1);
    free_heap_str(&result);
}

// replace: different-length → new allocation
#[test]
fn replace_different_length() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"hello world");
    let from = OriStr::from_sso(b"world");
    let to = OriStr::from_sso(b"everyone");
    let result = ori_str_replace(&s, &from, &to);
    assert_eq!(unsafe { result.as_str() }, "hello everyone");
}

// repeat: SSO result
#[test]
fn repeat_sso_result() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_sso(b"ab");
    let result = ori_str_repeat(&s, 5);
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "ababababab");
    assert_eq!(
        ori_rc_live_count(),
        before,
        "SSO repeat should not allocate"
    );
}

// repeat: heap result with exact capacity
#[test]
fn repeat_heap_exact_capacity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_sso(b"abc");
    let result = ori_str_repeat(&s, 10); // 30 bytes > 23
    assert!(!result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "abcabcabcabcabcabcabcabcabcabc");
    assert_eq!(result.len(), 30);
    unsafe {
        assert_eq!(result.heap.cap, 30, "repeat should use exact capacity");
    }
    assert_eq!(ori_rc_live_count(), before + 1);
    free_heap_str(&result);
}

// repeat: count=0 → empty
#[test]
fn repeat_zero_count() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"hello");
    let result = ori_str_repeat(&s, 0);
    assert!(result.is_sso());
    assert_eq!(result.len(), 0);
}

// repeat: count=1 → return input
#[test]
fn repeat_one_count() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"hello");
    let result = ori_str_repeat(&s, 1);
    assert_eq!(unsafe { result.as_str() }, "hello");
}

// push_char: SSO + ASCII char → SSO
#[test]
fn push_char_sso_ascii() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let s = OriStr::from_sso(b"hello");
    let result = ori_str_push_char(&s, u32::from(b'!'));
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "hello!");
    assert_eq!(
        ori_rc_live_count(),
        before,
        "SSO push_char should not allocate"
    );
}

// push_char: SSO + multibyte char → SSO
#[test]
fn push_char_sso_multibyte() {
    let _g = lock_rc();
    let s = OriStr::from_sso(b"caf");
    let result = ori_str_push_char(&s, 0xE9); // 'é'
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "café");
    assert_eq!(result.len(), 5); // 3 + 2 bytes for é
}

// push_char: heap unique with capacity → in-place
#[test]
fn push_char_heap_inplace() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let content = b"this exceeds sso limit!"; // 23 bytes
    let mut s = OriStr::with_capacity(64);
    unsafe {
        std::ptr::copy_nonoverlapping(content.as_ptr(), s.heap.data, content.len());
        s.heap.len = content.len() as i64;
    }
    assert_eq!(ori_rc_live_count(), before + 1);

    let result = ori_str_push_char(&s, u32::from(b'x'));
    assert_eq!(unsafe { result.as_str() }, "this exceeds sso limit!x");
    assert_eq!(
        ori_rc_live_count(),
        before + 1,
        "in-place push should not allocate"
    );
    free_heap_str(&result);
}

// push_char: empty + char → SSO
#[test]
fn push_char_empty_string() {
    let _g = lock_rc();
    let s = OriStr::EMPTY;
    let result = ori_str_push_char(&s, u32::from(b'x'));
    assert!(result.is_sso());
    assert_eq!(unsafe { result.as_str() }, "x");
}

#[test]
fn map_empty_sentinel_is_null() {
    let m = map::ori_map_empty();
    assert_eq!(m.len, 0);
    assert_eq!(m.cap, 0);
    assert!(m.data.is_null(), "sentinel map data should be null");
}

#[test]
fn set_empty_sentinel_is_null() {
    let ptr = set::ori_set_empty();
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
    let data = ori_list_alloc_data(4, 8); // 4 × 8 bytes
    assert!(!data.is_null());

    let box_ptr = ori_list_box_new(3, 4, data);
    assert!(!box_ptr.is_null(), "box_new should return non-null");
    assert_eq!(ori_rc_count(box_ptr), 1, "fresh box has RC=1");

    // Read fields back via OriList cast
    let list = unsafe { &*box_ptr.cast::<OriList>() };
    assert_eq!(list.len, 3);
    assert_eq!(list.cap, 4);
    assert_eq!(list.data, data);

    // Clean up (RC-allocated data buffer)
    ori_list_free_data(data, 4, 8);
    ori_rc_free(
        box_ptr,
        std::mem::size_of::<OriList>(),
        std::mem::align_of::<OriList>(),
    );
}

#[test]
fn list_box_new_round_trip_with_data() {
    let _g = lock_rc();

    // Write known pattern to data buffer (RC-allocated)
    let data = ori_list_alloc_data(3, 8);
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

    ori_list_free_data(data, 3, 8);
    ori_rc_free(
        box_ptr,
        std::mem::size_of::<OriList>(),
        std::mem::align_of::<OriList>(),
    );
}

// ── COW list push (ori_list_push_cow) ────────────────────────────────

/// Helper: read an `OriList` from a raw byte buffer (sret pattern).
unsafe fn read_list_result(out: &[u8; 24]) -> (i64, i64, *mut u8) {
    let ptr = out.as_ptr();
    let len = ptr.cast::<i64>().read();
    let cap = ptr.cast::<i64>().add(1).read();
    let data = ptr.add(16).cast::<*mut u8>().read();
    (len, cap, data)
}

/// Helper: create an RC-allocated data buffer with `count` i64 values.
///
/// Returns `(data_ptr, byte_capacity)`. The data pointer is RC-allocated
/// via `ori_rc_alloc` and must be freed with `ori_rc_free`.
fn rc_alloc_i64_list(values: &[i64], capacity: usize) -> *mut u8 {
    let es = std::mem::size_of::<i64>();
    let cap = capacity.max(values.len());
    let data = ori_rc_alloc(cap * es, 8);
    assert!(!data.is_null());
    for (i, &val) in values.iter().enumerate() {
        unsafe {
            *data.cast::<i64>().add(i) = val;
        }
    }
    data
}

#[test]
fn cow_push_to_empty_sentinel() {
    let _g = lock_rc();
    let before = ori_rc_live_count();

    let elem: i64 = 42;
    let mut out = [0u8; 24];

    ori_list_push_cow(
        std::ptr::null_mut(), // empty sentinel
        0,
        0,
        std::ptr::from_ref(&elem).cast(),
        std::mem::size_of::<i64>() as i64,
        std::mem::align_of::<i64>() as i64,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 1, "should have 1 element after push");
    assert!(
        cap >= 4,
        "should get at least MIN_COLLECTION_CAPACITY (4), got {cap}"
    );
    assert!(!data.is_null(), "data should be non-null after push");
    assert_eq!(ori_rc_count(data), 1, "new buffer should have RC=1");

    // Verify the element was written
    unsafe {
        assert_eq!(*data.cast::<i64>(), 42, "element should be 42");
    }

    // Cleanup
    ori_rc_free(data, cap as usize * std::mem::size_of::<i64>(), 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_push_unique_with_capacity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create an RC-allocated buffer with 3 elements, capacity 8
    let data = rc_alloc_i64_list(&[10, 20, 30], 8);
    let original_ptr = data;

    let elem: i64 = 40;
    let mut out = [0u8; 24];

    ori_list_push_cow(
        data,
        3, // len
        8, // cap (room for 5 more)
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 4, "should have 4 elements");
    assert_eq!(cap, 8, "capacity unchanged (had room)");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: should return same pointer (mutated in place)"
    );
    assert_eq!(ori_rc_count(result_data), 1, "RC should still be 1");

    // Verify all elements
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    // Cleanup
    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_push_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create an RC-allocated buffer at full capacity (4 elements, cap 4)
    let data = rc_alloc_i64_list(&[10, 20, 30, 40], 4);

    let elem: i64 = 50;
    let mut out = [0u8; 24];

    ori_list_push_cow(
        data,
        4, // len
        4, // cap (FULL — needs growth)
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 5, "should have 5 elements");
    assert!(
        cap >= 8,
        "should have doubled capacity to at least 8, got {cap}"
    );
    assert!(!result_data.is_null(), "result data should be non-null");
    assert_eq!(ori_rc_count(result_data), 1, "new buffer should have RC=1");

    // Verify all elements survived the realloc
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
        assert_eq!(*result_data.cast::<i64>().add(4), 50);
    }

    // Cleanup — realloc freed old buffer, just free the new one
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_push_shared_list_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create an RC-allocated buffer with RC=2 (shared)
    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    ori_rc_inc(data); // RC=2 (simulate sharing)

    let elem: i64 = 40;
    let mut out = [0u8; 24];

    ori_list_push_cow(
        data,
        3,
        4,
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 4, "should have 4 elements");
    assert!(cap >= 4, "should have capacity >= 4, got {cap}");
    assert_ne!(
        result_data, data,
        "SLOW PATH: should return different pointer (copied)"
    );

    // Old buffer: was RC=2, push_cow consumed one ref (dec'd to 1)
    assert_eq!(
        ori_rc_count(data),
        1,
        "old buffer RC should be 1 (was 2, dec'd by push_cow)"
    );

    // New buffer: fresh allocation at RC=1
    assert_eq!(ori_rc_count(result_data), 1, "new buffer should have RC=1");

    // Verify old data is untouched
    unsafe {
        assert_eq!(*data.cast::<i64>(), 10);
        assert_eq!(*data.cast::<i64>().add(1), 20);
        assert_eq!(*data.cast::<i64>().add(2), 30);
    }

    // Verify new data has all elements
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    // Cleanup: free both buffers
    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_push_1000_sequential_amortized() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Start from empty sentinel, push 1000 elements
    let mut data: *mut u8 = std::ptr::null_mut();
    let mut len: i64 = 0;
    let mut cap: i64 = 0;
    let mut realloc_count = 0u32;

    for i in 0..1000i64 {
        let old_data = data;
        let mut out = [0u8; 24];

        ori_list_push_cow(
            data,
            len,
            cap,
            std::ptr::from_ref(&i).cast(),
            es as i64,
            8,
            None,
            0,
            out.as_mut_ptr(),
        );

        let (new_len, new_cap, new_data) = unsafe { read_list_result(&out) };

        if new_data != old_data {
            realloc_count += 1;
        }

        data = new_data;
        len = new_len;
        cap = new_cap;
    }

    assert_eq!(len, 1000, "should have 1000 elements");
    assert!(cap >= 1000, "capacity should be at least 1000, got {cap}");

    // With doubling growth (4, 8, 16, 32, 64, 128, 256, 512, 1024),
    // we expect roughly 10 allocations (1 initial + ~9 doublings)
    assert!(
        realloc_count <= 15,
        "amortized O(1): expected ~10 reallocations for 1000 pushes, got {realloc_count}"
    );

    // Verify first and last elements
    unsafe {
        assert_eq!(*data.cast::<i64>(), 0, "first element should be 0");
        assert_eq!(
            *data.cast::<i64>().add(999),
            999,
            "last element should be 999"
        );
    }

    // Verify RC is 1 (sole owner throughout — fast path always)
    assert_eq!(ori_rc_count(data), 1, "buffer should have RC=1");

    // Cleanup
    ori_rc_free(data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW list pop (ori_list_pop_cow) ──────────────────────────────────

#[test]
fn cow_pop_unique_decrements_len() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create a unique list [10, 20, 30] with capacity 4
    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_pop_cow(data, 3, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2, "should have 2 elements after pop");
    assert_eq!(cap, 4, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, just shrink len)"
    );
    assert_eq!(ori_rc_count(result_data), 1, "RC should still be 1");

    // Verify remaining elements untouched
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
    }

    // Cleanup
    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_pop_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create a shared list [10, 20, 30] (RC=2)
    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    ori_rc_inc(data); // RC=2

    let mut out = [0u8; 24];
    ori_list_pop_cow(data, 3, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2, "should have 2 elements after pop");
    assert_eq!(cap, 2, "new buffer capacity matches new length");
    assert_ne!(
        result_data, data,
        "SLOW PATH: different pointer (shared, copied)"
    );

    // Old buffer: was RC=2, pop_cow consumed one ref (dec'd to 1)
    assert_eq!(ori_rc_count(data), 1, "old buffer RC should be 1");
    // New buffer: fresh allocation at RC=1
    assert_eq!(ori_rc_count(result_data), 1, "new buffer RC should be 1");

    // Verify old data untouched
    unsafe {
        assert_eq!(*data.cast::<i64>(), 10);
        assert_eq!(*data.cast::<i64>().add(1), 20);
        assert_eq!(*data.cast::<i64>().add(2), 30);
    }

    // Verify new data has first 2 elements only
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
    }

    // Cleanup
    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_pop_to_empty_retains_buffer() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create a unique list [42] with capacity 4
    let data = rc_alloc_i64_list(&[42], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_pop_cow(data, 1, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0, "should be empty after popping last element");
    assert_eq!(cap, 4, "capacity retained (no auto-shrink)");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, capacity retained)"
    );

    // Cleanup
    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_pop_empty_list_returns_empty() {
    let mut out = [0u8; 24];
    ori_list_pop_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::mem::size_of::<i64>() as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0, "empty pop should return len=0");
    assert_eq!(cap, 0, "empty pop should return cap=0");
    assert!(data.is_null(), "empty pop should return null data");
}

// ── COW list set (ori_list_set_cow) ──────────────────────────────────

#[test]
fn cow_set_unique_overwrites_in_place() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create unique list [10, 20, 30] with capacity 4
    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    let original_ptr = data;

    // Set index 1 to 99
    let elem: i64 = 99;
    let mut out = [0u8; 24];
    ori_list_set_cow(
        data,
        3,
        4,
        1, // index
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3);
    assert_eq!(cap, 4);
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, in-place overwrite)"
    );

    // Verify element was replaced
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(
            *result_data.cast::<i64>().add(1),
            99,
            "index 1 should be 99"
        );
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create shared list [10, 20, 30] (RC=2)
    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    ori_rc_inc(data);

    // Set index 2 to 77
    let elem: i64 = 77;
    let mut out = [0u8; 24];
    ori_list_set_cow(
        data,
        3,
        4,
        2, // last index
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3);
    assert_ne!(result_data, data, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched
    assert_eq!(ori_rc_count(data), 1, "old buffer RC dec'd to 1");
    unsafe {
        assert_eq!(*data.cast::<i64>(), 10);
        assert_eq!(*data.cast::<i64>().add(1), 20);
        assert_eq!(*data.cast::<i64>().add(2), 30, "old buffer unchanged");
    }

    // New buffer has replacement
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(
            *result_data.cast::<i64>().add(2),
            77,
            "new buffer has replacement at index 2"
        );
    }

    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_at_index_zero() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);

    let elem: i64 = 55;
    let mut out = [0u8; 24];
    ori_list_set_cow(
        data,
        3,
        4,
        0, // first index
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 55, "index 0 should be replaced");
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW list insert (ori_list_insert_cow) ────────────────────────────

#[test]
fn cow_insert_unique_at_beginning() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create unique list [10, 20, 30] with capacity 8
    let data = rc_alloc_i64_list(&[10, 20, 30], 8);
    let original_ptr = data;

    // Insert 5 at index 0
    let elem: i64 = 5;
    let mut out = [0u8; 24];
    ori_list_insert_cow(
        data,
        3,
        8,
        0, // index
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 4);
    assert_eq!(cap, 8, "capacity unchanged — had room");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, had capacity)"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 5, "inserted at 0");
        assert_eq!(*result_data.cast::<i64>().add(1), 10, "shifted right");
        assert_eq!(*result_data.cast::<i64>().add(2), 20);
        assert_eq!(*result_data.cast::<i64>().add(3), 30);
    }

    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_insert_unique_at_middle() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 8);

    let elem: i64 = 15;
    let mut out = [0u8; 24];
    ori_list_insert_cow(
        data,
        3,
        8,
        1, // between 10 and 20
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 15, "inserted at 1");
        assert_eq!(*result_data.cast::<i64>().add(2), 20, "shifted right");
        assert_eq!(*result_data.cast::<i64>().add(3), 30);
    }

    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_insert_unique_at_end() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 8);

    let elem: i64 = 40;
    let mut out = [0u8; 24];
    ori_list_insert_cow(
        data,
        3,
        8,
        3, // index == len (append)
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40, "appended at end");
    }

    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_insert_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Create list [10, 20] with capacity exactly 2 (no room)
    let data = rc_alloc_i64_list(&[10, 20], 2);

    let elem: i64 = 15;
    let mut out = [0u8; 24];
    ori_list_insert_cow(
        data,
        2,
        2,
        1, // insert between
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3);
    assert!(cap >= 3, "capacity grew: {cap}");

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 15);
        assert_eq!(*result_data.cast::<i64>().add(2), 20);
    }

    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_insert_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Shared list (RC=2)
    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    ori_rc_inc(data);

    let elem: i64 = 15;
    let mut out = [0u8; 24];
    ori_list_insert_cow(
        data,
        3,
        4,
        1,
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_ne!(result_data, data, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched
    assert_eq!(ori_rc_count(data), 1, "old buffer RC dec'd to 1");
    unsafe {
        assert_eq!(*data.cast::<i64>(), 10);
        assert_eq!(*data.cast::<i64>().add(1), 20);
        assert_eq!(*data.cast::<i64>().add(2), 30, "old unchanged");
    }

    // New buffer has insert
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 15, "inserted");
        assert_eq!(*result_data.cast::<i64>().add(2), 20);
        assert_eq!(*result_data.cast::<i64>().add(3), 30);
    }

    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_insert_into_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let elem: i64 = 42;
    let mut out = [0u8; 24];
    ori_list_insert_cow(
        std::ptr::null_mut(),
        0,
        0,
        0,
        std::ptr::from_ref(&elem).cast(),
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 1);
    assert!(cap >= 1);
    assert!(!result_data.is_null());

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 42);
    }

    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW list remove (ori_list_remove_cow) ────────────────────────────

#[test]
fn cow_remove_unique_at_beginning() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_remove_cow(
        data,
        3,
        4,
        0, // remove first
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert_eq!(cap, 4, "capacity retained");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique)"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 20, "shifted left");
        assert_eq!(*result_data.cast::<i64>().add(1), 30);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_remove_unique_at_middle() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);

    let mut out = [0u8; 24];
    ori_list_remove_cow(data, 3, 4, 1, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(
            *result_data.cast::<i64>().add(1),
            30,
            "20 removed, 30 shifted"
        );
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_remove_unique_at_end() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);

    let mut out = [0u8; 24];
    ori_list_remove_cow(data, 3, 4, 2, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_remove_unique_last_element_frees() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[42], 4);

    let mut out = [0u8; 24];
    ori_list_remove_cow(data, 1, 4, 0, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0, "empty after removing last");
    assert_eq!(cap, 0);
    assert!(result_data.is_null(), "buffer freed");

    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_remove_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    ori_rc_inc(data);

    let mut out = [0u8; 24];
    ori_list_remove_cow(data, 3, 4, 1, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert_ne!(result_data, data, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched
    assert_eq!(ori_rc_count(data), 1);
    unsafe {
        assert_eq!(*data.cast::<i64>(), 10);
        assert_eq!(*data.cast::<i64>().add(1), 20);
        assert_eq!(*data.cast::<i64>().add(2), 30);
    }

    // New buffer has removal
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 30, "20 removed");
    }

    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, 2 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW list concat (ori_list_concat_cow) ────────────────────────────

#[test]
fn cow_concat_unique_with_capacity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1: [10, 20] with capacity 8 (room for list2)
    let data1 = rc_alloc_i64_list(&[10, 20], 8);
    let original_ptr = data1;

    // list2: [30, 40] — both unique (RC=1), runtime consumes both
    let data2 = rc_alloc_i64_list(&[30, 40], 4);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        2,
        8,
        data2,
        2,
        4,
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_eq!(cap, 8, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, had capacity)"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30, "from list2");
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    // data2 consumed by runtime (unique → freed). Only free result.
    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1: [10, 20] with capacity 2 (no room)
    let data1 = rc_alloc_i64_list(&[10, 20], 2);
    // list2: [30, 40] — both unique, runtime consumes both
    let data2 = rc_alloc_i64_list(&[30, 40], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        2,
        2,
        data2,
        2,
        2,
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert!(cap >= 4, "grew to fit: {cap}");

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    // data2 consumed by runtime (unique → freed). Only free result.
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_shared_list1_unique_list2() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1 shared (RC=2), list2 unique (RC=1)
    let data1 = rc_alloc_i64_list(&[10, 20], 4);
    ori_rc_inc(data1);
    let data2 = rc_alloc_i64_list(&[30, 40], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        2,
        4,
        data2,
        2,
        2,
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_ne!(result_data, data1, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched, RC went from 2→1 (runtime dec'd)
    assert_eq!(ori_rc_count(data1), 1);
    unsafe {
        assert_eq!(*data1.cast::<i64>(), 10);
        assert_eq!(*data1.cast::<i64>().add(1), 20);
    }

    // New buffer has both lists
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    // data2 consumed by runtime (unique → freed). Free data1 (our remaining ref) and result.
    ori_rc_free(data1, 4 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_empty_lists() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Both empty — runtime decs both (no-op for null)
    let mut out = [0u8; 24];
    ori_list_concat_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::null_mut(),
        0,
        0,
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0);
    assert!(result_data.is_null());
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_empty_list1_unique_list2_takeover() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1 empty, list2 unique → TAKEOVER (zero-copy)
    let data2 = rc_alloc_i64_list(&[30, 40], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        std::ptr::null_mut(),
        0,
        0,
        data2,
        2,
        2,
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert_eq!(cap, 2);
    assert_eq!(
        result_data, data2,
        "TAKEOVER: result IS data2's buffer (zero-copy)"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 30);
        assert_eq!(*result_data.cast::<i64>().add(1), 40);
    }

    // data2 ownership transferred to result — free once
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_empty_list1_shared_list2() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1 empty, list2 shared (RC=2) → fresh copy
    let data2 = rc_alloc_i64_list(&[30, 40], 2);
    ori_rc_inc(data2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        std::ptr::null_mut(),
        0,
        0,
        data2,
        2,
        2,
        es as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert!(!result_data.is_null());
    assert_ne!(result_data, data2, "shared: fresh copy, not same as list2");

    // data2 still alive (RC was 2, runtime dec'd to 1)
    assert_eq!(ori_rc_count(data2), 1);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 30);
        assert_eq!(*result_data.cast::<i64>().add(1), 40);
    }

    // Free data2 (our remaining ref) and result
    ori_rc_free(data2, 2 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW concat: inc_fn counting tests ────────────────────────────────
//
// These tests use a counting inc_fn to verify that the runtime correctly
// skips RC increments when list2 is uniquely owned (moved, not copied).

static INC_COUNT: AtomicUsize = AtomicUsize::new(0);

extern "C" fn counting_inc_fn(_ptr: *mut u8) {
    INC_COUNT.fetch_add(1, Ordering::Relaxed);
}

#[test]
fn cow_concat_both_unique_no_inc() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();
    INC_COUNT.store(0, Ordering::Relaxed);

    // Both lists unique — no element RC increments should happen
    let data1 = rc_alloc_i64_list(&[10, 20], 8);
    let data2 = rc_alloc_i64_list(&[30, 40], 4);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        2,
        8,
        data2,
        2,
        4,
        es as i64,
        8,
        Some(counting_inc_fn),
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_eq!(
        INC_COUNT.load(Ordering::Relaxed),
        0,
        "both unique: ZERO inc_fn calls (elements moved, not copied)"
    );

    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_list1_unique_list2_shared_inc_n2() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();
    INC_COUNT.store(0, Ordering::Relaxed);

    // list1 unique, list2 shared (RC=2) — should inc N2 elements
    let data1 = rc_alloc_i64_list(&[10, 20], 8);
    let data2 = rc_alloc_i64_list(&[30, 40, 50], 3);
    ori_rc_inc(data2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        2,
        8,
        data2,
        3,
        3,
        es as i64,
        8,
        Some(counting_inc_fn),
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 5);
    assert_eq!(
        INC_COUNT.load(Ordering::Relaxed),
        3,
        "list2 shared: inc_fn called N2=3 times"
    );

    // data2 still alive (RC was 2, runtime dec'd to 1)
    assert_eq!(ori_rc_count(data2), 1);
    ori_rc_free(data2, 3 * es, 8);
    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_list1_shared_list2_unique_inc_n1_only() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();
    INC_COUNT.store(0, Ordering::Relaxed);

    // list1 shared (RC=2), list2 unique — should inc N1 elements only
    let data1 = rc_alloc_i64_list(&[10, 20, 30], 3);
    ori_rc_inc(data1);
    let data2 = rc_alloc_i64_list(&[40, 50], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        3,
        3,
        data2,
        2,
        2,
        es as i64,
        8,
        Some(counting_inc_fn),
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 5);
    assert_eq!(
        INC_COUNT.load(Ordering::Relaxed),
        3,
        "list1 shared, list2 unique: inc_fn called N1=3 times (list2 moved)"
    );

    // data1 still alive (RC was 2, runtime dec'd to 1)
    assert_eq!(ori_rc_count(data1), 1);
    ori_rc_free(data1, 3 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_both_shared_inc_n1_plus_n2() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();
    INC_COUNT.store(0, Ordering::Relaxed);

    // Both shared — should inc N1 + N2 elements
    let data1 = rc_alloc_i64_list(&[10, 20], 2);
    ori_rc_inc(data1);
    let data2 = rc_alloc_i64_list(&[30, 40, 50], 3);
    ori_rc_inc(data2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data1,
        2,
        2,
        data2,
        3,
        3,
        es as i64,
        8,
        Some(counting_inc_fn),
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 5);
    assert_eq!(
        INC_COUNT.load(Ordering::Relaxed),
        5,
        "both shared: inc_fn called N1+N2=5 times"
    );

    // Both originals still alive (RC 2→1 each)
    assert_eq!(ori_rc_count(data1), 1);
    assert_eq!(ori_rc_count(data2), 1);
    ori_rc_free(data1, 2 * es, 8);
    ori_rc_free(data2, 3 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_self_same_buffer() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();
    INC_COUNT.store(0, Ordering::Relaxed);

    // Self-concat: x + x. ARC pipeline inc's for multi-use → RC=2 at call.
    let data = rc_alloc_i64_list(&[10, 20], 4);
    ori_rc_inc(data); // Simulate ARC pipeline inc for multi-use

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        data,
        2,
        4,
        data,
        2,
        4,
        es as i64,
        8,
        Some(counting_inc_fn),
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    // Both args shared (RC=2) → case 4: inc all elements (N1 + N2 = 4)
    assert_eq!(
        INC_COUNT.load(Ordering::Relaxed),
        4,
        "self-concat: both shared, inc N1+N2=4"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 10, "same data");
        assert_eq!(*result_data.cast::<i64>().add(3), 20);
    }

    // Original buffer: RC was 2, runtime dec'd twice (data1 + data2) → 0 → freed
    // Result is the only live allocation
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW list reverse (ori_list_reverse_cow) ──────────────────────────

#[test]
fn cow_reverse_unique_in_place() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30, 40], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_reverse_cow(data, 4, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_eq!(cap, 4);
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, in-place swap)"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 40);
        assert_eq!(*result_data.cast::<i64>().add(1), 30);
        assert_eq!(*result_data.cast::<i64>().add(2), 20);
        assert_eq!(*result_data.cast::<i64>().add(3), 10);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_reverse_unique_odd_count() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);

    let mut out = [0u8; 24];
    ori_list_reverse_cow(data, 3, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 30);
        assert_eq!(*result_data.cast::<i64>().add(1), 20, "middle unchanged");
        assert_eq!(*result_data.cast::<i64>().add(2), 10);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_reverse_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30], 4);
    ori_rc_inc(data);

    let mut out = [0u8; 24];
    ori_list_reverse_cow(data, 3, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3);
    assert_ne!(result_data, data, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched
    assert_eq!(ori_rc_count(data), 1);
    unsafe {
        assert_eq!(*data.cast::<i64>(), 10);
        assert_eq!(*data.cast::<i64>().add(1), 20);
        assert_eq!(*data.cast::<i64>().add(2), 30, "original unchanged");
    }

    // New buffer reversed
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 30);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 10);
    }

    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, 3 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_reverse_single_element_unchanged() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[42], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_reverse_cow(data, 1, 4, es as i64, 8, None, 0, out.as_mut_ptr());

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 1);
    assert_eq!(
        result_data, original_ptr,
        "single element: returned unchanged"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 42);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_reverse_empty_unchanged() {
    let mut out = [0u8; 24];
    ori_list_reverse_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::mem::size_of::<i64>() as i64,
        8,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0);
    assert!(result_data.is_null());
}

// ── COW list sort (ori_list_sort_cow) ────────────────────────────────

/// Comparison function for i64 elements (ascending order).
extern "C" fn compare_i64_asc(a: *const u8, b: *const u8) -> i32 {
    let va = unsafe { *a.cast::<i64>() };
    let vb = unsafe { *b.cast::<i64>() };
    match va.cmp(&vb) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

#[test]
fn cow_sort_unique_in_place() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[30, 10, 40, 20], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_sort_cow(
        data,
        4,
        4,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_eq!(cap, 4);
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (unique, in-place sort)"
    );

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[30, 10, 40, 20], 4);
    ori_rc_inc(data);

    let mut out = [0u8; 24];
    ori_list_sort_cow(
        data,
        4,
        4,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_ne!(result_data, data, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched
    assert_eq!(ori_rc_count(data), 1);
    unsafe {
        assert_eq!(*data.cast::<i64>(), 30);
        assert_eq!(*data.cast::<i64>().add(1), 10);
        assert_eq!(*data.cast::<i64>().add(2), 40);
        assert_eq!(*data.cast::<i64>().add(3), 20, "original unsorted");
    }

    // New buffer sorted
    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    ori_rc_free(data, 4 * es, 8);
    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_already_sorted() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[10, 20, 30, 40], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_sort_cow(
        data,
        4,
        4,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (_, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(result_data, original_ptr, "same pointer");

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40, "already sorted");
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_reverse_sorted() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[40, 30, 20, 10], 4);

    let mut out = [0u8; 24];
    ori_list_sort_cow(
        data,
        4,
        4,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (_, _, result_data) = unsafe { read_list_result(&out) };

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_with_duplicates() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[30, 10, 30, 20, 10], 8);

    let mut out = [0u8; 24];
    ori_list_sort_cow(
        data,
        5,
        8,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 5);

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 10);
        assert_eq!(*result_data.cast::<i64>().add(2), 20);
        assert_eq!(*result_data.cast::<i64>().add(3), 30);
        assert_eq!(
            *result_data.cast::<i64>().add(4),
            30,
            "duplicates preserved"
        );
    }

    ori_rc_free(result_data, 8 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_single_element_unchanged() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[42], 4);
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_sort_cow(
        data,
        1,
        4,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 1);
    assert_eq!(result_data, original_ptr, "single element: unchanged");

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_empty_unchanged() {
    let mut out = [0u8; 24];
    ori_list_sort_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::mem::size_of::<i64>() as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0);
    assert!(result_data.is_null());
}

// ── COW list stable sort (ori_list_sort_stable_cow) ──────────────────

/// Stability test: elements with equal keys preserve their original order.
/// We sort pairs (key, id) by key — ids should stay in insertion order for equal keys.
/// Encode as i64: high 32 bits = key, low 32 bits = id.
fn encode_pair(key: i32, id: i32) -> i64 {
    (i64::from(key) << 32) | (i64::from(id) & 0xFFFF_FFFF)
}
fn decode_id(val: i64) -> i32 {
    val as i32
}

/// Compare only the high 32 bits (the "key" portion).
extern "C" fn compare_by_key(a: *const u8, b: *const u8) -> i32 {
    let a_val = unsafe { a.cast::<i64>().read() };
    let b_val = unsafe { b.cast::<i64>().read() };
    let a_key = (a_val >> 32) as i32;
    let b_key = (b_val >> 32) as i32;
    a_key.cmp(&b_key) as i32
}

#[test]
fn cow_sort_stable_preserves_equal_element_order() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Elements: (key=1,id=0), (key=2,id=1), (key=1,id=2), (key=3,id=3), (key=2,id=4)
    // Stable sort by key should give: (1,0),(1,2),(2,1),(2,4),(3,3)
    let elems = [
        encode_pair(1, 0),
        encode_pair(2, 1),
        encode_pair(1, 2),
        encode_pair(3, 3),
        encode_pair(2, 4),
    ];
    let data = rc_alloc_i64_list(&elems, 5);

    let mut out = [0u8; 24];
    ori_list_sort_stable_cow(
        data,
        5,
        5,
        es as i64,
        8,
        compare_by_key,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 5);

    // Verify stability: ids within each key group are in original insertion order
    let result_ids: Vec<i32> = (0..5)
        .map(|i| unsafe { decode_id(result_data.cast::<i64>().add(i).read()) })
        .collect();
    assert_eq!(
        result_ids,
        vec![0, 2, 1, 4, 3],
        "stable sort preserves equal-element order"
    );

    ori_rc_free(result_data, 5 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_unstable_may_not_preserve_order() {
    // This test verifies that sort_stable and sort_unstable CAN differ on ordering
    // of equal elements. We don't assert that unstable MUST reorder (it may or may not),
    // but we verify stable sort is deterministic.
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let elems = [
        encode_pair(1, 0),
        encode_pair(1, 1),
        encode_pair(1, 2),
        encode_pair(1, 3),
    ];
    let data = rc_alloc_i64_list(&elems, 4);

    let mut out = [0u8; 24];
    ori_list_sort_stable_cow(
        data,
        4,
        4,
        es as i64,
        8,
        compare_by_key,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);

    // All keys equal — stable sort MUST preserve original order
    let result_ids: Vec<i32> = (0..4)
        .map(|i| unsafe { decode_id(result_data.cast::<i64>().add(i).read()) })
        .collect();
    assert_eq!(
        result_ids,
        vec![0, 1, 2, 3],
        "stable sort: all-equal keys preserve order"
    );

    ori_rc_free(result_data, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_sort_stable_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    let data = rc_alloc_i64_list(&[30, 10, 20], 4);
    ori_rc_inc(data); // RC=2, shared
    let original_ptr = data;

    let mut out = [0u8; 24];
    ori_list_sort_stable_cow(
        data,
        3,
        4,
        es as i64,
        8,
        compare_i64_asc,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3);
    assert_ne!(
        result_data, original_ptr,
        "SLOW PATH: different pointer (shared)"
    );

    // Verify sorted
    let vals: Vec<i64> = (0..3)
        .map(|i| unsafe { result_data.cast::<i64>().add(i).read() })
        .collect();
    assert_eq!(vals, vec![10, 20, 30]);

    ori_rc_free(original_ptr, 4 * es, 8); // free original (RC was dec'd to 1 by runtime)
    ori_rc_free(result_data, 3 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── ORI_RT_DEBUG assertion mode ───────────────────────────────────────

/// Enable debug assertions for the duration of a closure, then clean up.
fn with_rt_debug<F: FnOnce()>(f: F) {
    let _g = lock_rc();
    reset_freed_set();
    RT_DEBUG_FORCE.store(true, Ordering::Relaxed);
    f();
    RT_DEBUG_FORCE.store(false, Ordering::Relaxed);
    reset_freed_set();
}

#[test]
fn freed_set_tracks_freed_pointers() {
    with_rt_debug(|| {
        let ptr = ori_rc_alloc(16, 8);
        let addr = ptr as usize;

        // Not in freed set yet
        assert!(!freed_set().lock().unwrap().contains(&addr));

        // Free — should register in freed set
        ori_rc_free(ptr, 16, 8);
        assert!(freed_set().lock().unwrap().contains(&addr));
    });
}

#[test]
fn freed_set_reset_clears_state() {
    with_rt_debug(|| {
        let ptr = ori_rc_alloc(16, 8);
        let addr = ptr as usize;
        ori_rc_free(ptr, 16, 8);
        assert!(freed_set().lock().unwrap().contains(&addr));

        reset_freed_set();
        assert!(!freed_set().lock().unwrap().contains(&addr));
    });
}

#[test]
fn debug_validate_rc_accepts_valid_refcount() {
    with_rt_debug(|| {
        let ptr = ori_rc_alloc(16, 8);

        // Refcount is 1, which is valid (1..999999)
        let rc = unsafe { ptr.sub(8).cast::<i64>().read() };
        assert_eq!(rc, 1);
        assert!(rc > 0 && rc < 1_000_000);

        // rt_debug_validate_rc should not abort on a valid refcount
        rt_debug_validate_rc(ptr.cast_const(), "test");

        ori_rc_free(ptr, 16, 8);
    });
}

#[test]
fn debug_validate_rc_accepts_incremented_refcount() {
    with_rt_debug(|| {
        let ptr = ori_rc_alloc(16, 8);

        // ori_rc_inc should succeed (validated internally)
        ori_rc_inc(ptr);
        let rc = unsafe { ptr.sub(8).cast::<i64>().read() };
        assert_eq!(rc, 2);

        // Clean up
        ori_rc_dec(ptr, None);
        ori_rc_dec(ptr, None);
        // After dec to 0, ptr is logically freed but drop_fn is None
        // so ori_rc_free wasn't called — free explicitly
        ori_rc_free(ptr, 16, 8);
    });
}

#[test]
fn debug_mode_detects_freed_pointer_in_set() {
    with_rt_debug(|| {
        let ptr = ori_rc_alloc(16, 8);
        let addr = ptr as usize;

        // Simulate: free the pointer
        ori_rc_free(ptr, 16, 8);

        // Verify detection: the freed set should contain this pointer
        assert!(
            freed_set().lock().unwrap().contains(&addr),
            "freed pointer should be tracked in freed set"
        );

        // In a real scenario, calling ori_rc_inc(ptr) would now abort
        // because rt_debug_check_not_freed detects the pointer in the
        // freed set. We can't test the abort in-process, but we verify
        // the detection mechanism works.
    });
}

#[test]
fn debug_check_not_freed_passes_for_live_pointer() {
    with_rt_debug(|| {
        let ptr = ori_rc_alloc(16, 8);

        // Should not abort — pointer is live
        rt_debug_check_not_freed(ptr.cast_const(), "test");

        ori_rc_free(ptr, 16, 8);
    });
}

// ── Release-mode underflow detection ──────────────────────────────────

#[test]
fn rc_underflow_aborts_process() {
    // Verify that decrementing a zero-refcount allocation aborts.
    // Must be tested in a subprocess because abort() kills the process.
    use std::process::Command;

    let result =
        Command::new(std::env::current_exe().expect("could not determine test binary path"))
            .arg("--exact")
            .arg("tests::rc_underflow_aborts_process_child")
            .env("ORI_RC_UNDERFLOW_TEST", "1")
            .output()
            .expect("failed to spawn child process");

    // The child should have been killed by abort (SIGABRT) or exited non-zero
    assert!(
        !result.status.success(),
        "child process should have aborted on underflow, but exited successfully"
    );
}

/// Helper test only run as a subprocess by `rc_underflow_aborts_process`.
///
/// Allocates an RC object, manually zeroes the refcount, then calls
/// `ori_rc_dec` which should trigger the underflow abort.
#[test]
fn rc_underflow_aborts_process_child() {
    if std::env::var("ORI_RC_UNDERFLOW_TEST").is_err() {
        // Only run when invoked as a subprocess
        return;
    }

    let ptr = ori_rc_alloc(16, 8);

    // Directly write 0 into the refcount header (simulate already-freed)
    unsafe {
        let rc_ptr = ptr.sub(8).cast::<i64>();
        *rc_ptr = 0;
    }

    // This should trigger the underflow abort
    ori_rc_dec(ptr, None);

    // Should never reach here
    unreachable!("ori_rc_dec should have aborted on zero refcount");
}

// ── COW map insert tests ─────────────────────────────────────────────

/// i64 key equality comparator for COW map tests.
extern "C" fn i64_key_eq(a: *const u8, b: *const u8) -> bool {
    unsafe { *a.cast::<i64>() == *b.cast::<i64>() }
}

/// i64 key hash function for COW map tests (identity hash).
extern "C" fn i64_key_hash(a: *const u8) -> i64 {
    unsafe { *a.cast::<i64>() }
}

/// Helper: allocate an RC'd hash table map buffer with i64 keys and i64 values.
///
/// Returns `(data, actual_cap)`. The capacity is a power-of-two from the hash
/// table, which may be larger than `min_capacity`. Entries are inserted using
/// hash probing.
fn rc_alloc_i64_map(keys: &[i64], values: &[i64], min_capacity: usize) -> (*mut u8, usize) {
    assert_eq!(keys.len(), values.len());
    let len = keys.len();
    let ks = std::mem::size_of::<i64>();
    let vs = std::mem::size_of::<i64>();
    let cap = map::hash_table::next_hash_capacity(min_capacity.max(len));
    if cap == 0 {
        return (std::ptr::null_mut(), 0);
    }
    let data = map::OriMap::alloc_hash_buffer(cap, ks, vs);
    assert!(!data.is_null());
    let layout = map::hash_table::HashTableLayout::for_map(cap, ks, vs);
    for i in 0..len {
        let hash = keys[i]; // identity hash
        let bucket = unsafe { map::hash_table::probe_find_slot(data, cap, hash) };
        unsafe {
            let key_ptr = data.add(layout.keys_offset + bucket * ks);
            *key_ptr.cast::<i64>() = keys[i];
            let val_ptr = data.add(layout.vals_offset + bucket * vs);
            *val_ptr.cast::<i64>() = values[i];
            map::hash_table::set_meta(data, bucket, map::hash_table::META_OCCUPIED);
        }
    }
    (data, cap)
}

/// Helper: look up a key's value in a hash table map buffer.
unsafe fn map_lookup_val(data: *const u8, cap: usize, key: i64) -> Option<i64> {
    let ks = std::mem::size_of::<i64>();
    let vs = std::mem::size_of::<i64>();
    let layout = map::hash_table::HashTableLayout::for_map(cap, ks, vs);
    let hash = key; // identity hash
    let bucket = map::hash_table::probe_find(
        data,
        cap,
        layout.keys_offset,
        std::ptr::from_ref(&key).cast(),
        hash,
        ks,
        i64_key_eq,
    )?;
    let val_ptr = data.add(layout.vals_offset + bucket * vs);
    Some(*val_ptr.cast::<i64>())
}

/// Helper: collect all key-value pairs from a hash table map buffer.
///
/// Scans metadata for OCCUPIED buckets and returns all `(key, val)` pairs.
/// The order is bucket order (not insertion order).
unsafe fn map_collect_entries(data: *const u8, cap: usize) -> Vec<(i64, i64)> {
    let ks = std::mem::size_of::<i64>();
    let vs = std::mem::size_of::<i64>();
    let layout = map::hash_table::HashTableLayout::for_map(cap, ks, vs);
    let mut entries = Vec::new();
    for bucket in 0..cap {
        if map::hash_table::get_meta(data, bucket) == map::hash_table::META_OCCUPIED {
            let key = *data.add(layout.keys_offset + bucket * ks).cast::<i64>();
            let val = *data.add(layout.vals_offset + bucket * vs).cast::<i64>();
            entries.push((key, val));
        }
    }
    entries
}

/// Helper: free a hash table map buffer.
fn free_i64_map(data: *mut u8, cap: usize) {
    let ks = std::mem::size_of::<i64>();
    let vs = std::mem::size_of::<i64>();
    let layout = map::hash_table::HashTableLayout::for_map(cap, ks, vs);
    ori_rc_free(data, layout.total_size, 8);
}

#[test]
fn cow_map_insert_into_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let key: i64 = 100;
    let val: i64 = 200;
    let mut out = [0u8; 24];

    map::cow::ori_map_insert_cow(
        std::ptr::null_mut(), // empty sentinel
        0,
        0,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 1, "should have 1 entry");
    assert!(
        cap >= 4,
        "should get at least MIN_HASH_CAPACITY (4), got {cap}"
    );
    assert!(
        (cap as usize).is_power_of_two(),
        "capacity must be power of two"
    );
    assert!(!data.is_null(), "data should be non-null");
    assert_eq!(ori_rc_count(data), 1, "new buffer should have RC=1");

    // Verify key and value via hash lookup
    unsafe {
        assert_eq!(map_lookup_val(data, cap as usize, 100), Some(200));
        let entries = map_collect_entries(data, cap as usize);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], (100, 200));
    }

    // Cleanup
    free_i64_map(data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_insert_unique_new_key_with_capacity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Create map with 2 entries
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20], &[100, 200], 8);
    let original_ptr = data;

    let key: i64 = 30;
    let val: i64 = 300;
    let mut out = [0u8; 24];

    map::cow::ori_map_insert_cow(
        data,
        2, // len
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "should have 3 entries");
    assert_eq!(cap, alloc_cap as i64, "capacity unchanged (had room)");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: should return same pointer (mutated in place)"
    );
    assert_eq!(ori_rc_count(result_data), 1, "RC should still be 1");

    // Verify all entries via hash lookup
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(map_lookup_val(result_data, cap as usize, 20), Some(200));
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
        let entries = map_collect_entries(result_data, cap as usize);
        assert_eq!(entries.len(), 3);
    }

    // Cleanup
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_insert_unique_existing_key_overwrites() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Create map with 3 entries
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    // Overwrite key 20's value from 200 to 999
    let key: i64 = 20;
    let val: i64 = 999;
    let mut out = [0u8; 24];

    map::cow::ori_map_insert_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(
        len, 3,
        "should still have 3 entries (overwrite, not append)"
    );
    assert_eq!(cap, alloc_cap as i64, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: should return same pointer (overwritten in place)"
    );

    // Verify value at key 20 was updated, others unchanged
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 20),
            Some(999),
            "should be overwritten"
        );
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
    }

    // Cleanup
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_insert_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Create a map with 2 entries, then share it (RC=2)
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20], &[100, 200], 4);
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2, "should be shared");

    let key: i64 = 30;
    let val: i64 = 300;
    let mut out = [0u8; 24];

    map::cow::ori_map_insert_cow(
        data,
        2,
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "should have 3 entries");
    assert!(cap >= 3, "capacity should fit 3 entries");
    assert_ne!(
        result_data, data,
        "SLOW PATH: should return different pointer (new allocation)"
    );
    assert_eq!(ori_rc_count(result_data), 1, "new buffer should have RC=1");
    assert_eq!(
        ori_rc_count(data),
        1,
        "old buffer should have RC=1 (was 2, decremented by cow)"
    );

    // Verify new buffer has all entries
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(map_lookup_val(result_data, cap as usize, 20), Some(200));
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
    }

    // Verify original buffer is unchanged
    unsafe {
        assert_eq!(map_lookup_val(data, alloc_cap, 10), Some(100));
        assert_eq!(map_lookup_val(data, alloc_cap, 20), Some(200));
    }

    // Cleanup both buffers
    free_i64_map(data, alloc_cap);
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_insert_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Create a map with 4 entries (hash table will pick capacity based on load factor)
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30, 40], &[100, 200, 300, 400], 4);

    // Insert a 5th key — may or may not trigger growth depending on alloc_cap
    let key: i64 = 50;
    let val: i64 = 500;
    let mut out = [0u8; 24];

    map::cow::ori_map_insert_cow(
        data,
        4,
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 5, "should have 5 entries");
    assert!(cap >= 5, "capacity should fit 5 entries, got {cap}");
    assert!(
        (cap as usize).is_power_of_two(),
        "capacity must be power of two, got {cap}"
    );
    assert!(!result_data.is_null(), "data should not be null");
    assert_eq!(ori_rc_count(result_data), 1, "RC should be 1");

    // Verify all 5 entries via hash lookup
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(map_lookup_val(result_data, cap as usize, 20), Some(200));
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
        assert_eq!(map_lookup_val(result_data, cap as usize, 40), Some(400));
        assert_eq!(map_lookup_val(result_data, cap as usize, 50), Some(500));
        let entries = map_collect_entries(result_data, cap as usize);
        assert_eq!(entries.len(), 5);
    }

    // Cleanup
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_insert_1000_sequential_amortized() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let mut out = [0u8; 24];
    let mut current_data: *mut u8 = std::ptr::null_mut();
    let mut current_len: i64 = 0;
    let mut current_cap: i64 = 0;

    for i in 0..1000_i64 {
        let key = i;
        let val = i * 10;

        map::cow::ori_map_insert_cow(
            current_data,
            current_len,
            current_cap,
            std::ptr::from_ref(&key).cast(),
            std::ptr::from_ref(&val).cast(),
            ks,
            vs,
            i64_key_eq,
            i64_key_hash,
            None,
            None,
            0,
            out.as_mut_ptr(),
        );

        let (len, cap, data) = unsafe { read_list_result(&out) };
        current_data = data;
        current_len = len;
        current_cap = cap;
    }

    assert_eq!(current_len, 1000, "should have 1000 entries");
    assert!(
        current_cap >= 1000,
        "capacity should be >= 1000, got {current_cap}"
    );
    assert!(
        (current_cap as usize).is_power_of_two(),
        "capacity must be power of two"
    );
    assert!(!current_data.is_null());
    assert_eq!(ori_rc_count(current_data), 1, "final buffer RC=1");

    // Spot-check entries via hash lookup
    unsafe {
        assert_eq!(
            map_lookup_val(current_data, current_cap as usize, 0),
            Some(0)
        );
        assert_eq!(
            map_lookup_val(current_data, current_cap as usize, 500),
            Some(5000)
        );
        assert_eq!(
            map_lookup_val(current_data, current_cap as usize, 999),
            Some(9990)
        );
        let entries = map_collect_entries(current_data, current_cap as usize);
        assert_eq!(entries.len(), 1000);
    }

    // Cleanup
    free_i64_map(current_data, current_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW map remove tests ────────────────────────────────────────────

#[test]
fn cow_map_remove_key_not_found() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    let needle: i64 = 999; // not in map
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "should still have 3 entries");
    assert_eq!(cap, alloc_cap as i64, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "should return same pointer (no-op)"
    );

    // Cleanup
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_remove_unique_middle() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Map: {10: 100, 20: 200, 30: 300}
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    // Remove key 20
    let needle: i64 = 20;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2, "should have 2 entries");
    assert_eq!(
        cap, alloc_cap as i64,
        "capacity unchanged (in-place tombstone)"
    );
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (tombstoned in place)"
    );

    // Verify: key 20 is gone, keys 10 and 30 remain
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 20),
            None,
            "key 20 removed"
        );
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
        let entries = map_collect_entries(result_data, cap as usize);
        assert_eq!(entries.len(), 2);
    }

    // Cleanup
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_remove_unique_first() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    // Remove key 10
    let needle: i64 = 10;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2);
    assert_eq!(
        cap, alloc_cap as i64,
        "capacity unchanged (in-place tombstone)"
    );
    assert_eq!(result_data, original_ptr, "FAST PATH: in-place tombstone");

    unsafe {
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 10),
            None,
            "key 10 removed"
        );
        assert_eq!(map_lookup_val(result_data, cap as usize, 20), Some(200));
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
    }

    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_remove_unique_last() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    // Remove key 30
    let needle: i64 = 30;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2);
    assert_eq!(
        cap, alloc_cap as i64,
        "capacity unchanged (in-place tombstone)"
    );
    assert_eq!(result_data, original_ptr, "FAST PATH: in-place tombstone");

    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(map_lookup_val(result_data, cap as usize, 20), Some(200));
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 30),
            None,
            "key 30 removed"
        );
    }

    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_remove_unique_last_entry_frees() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Single-entry map
    let (data, alloc_cap) = rc_alloc_i64_map(&[42], &[999], 4);
    assert_eq!(ori_rc_count(data), 1);

    let needle: i64 = 42;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        1,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0, "should be empty");
    assert_eq!(cap, 0, "capacity should be 0");
    assert!(result_data.is_null(), "should return null (empty sentinel)");
    // Buffer was freed by remove (unique + new_len == 0)
    assert_eq!(ori_rc_live_count(), before, "no leaks — buffer was freed");
}

#[test]
fn cow_map_remove_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Map: {10: 100, 20: 200, 30: 300}, shared (RC=2)
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2);

    // Remove key 20
    let needle: i64 = 20;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2, "should have 2 entries");
    assert!(
        (cap as usize).is_power_of_two(),
        "new capacity must be power of two"
    );
    assert_ne!(
        result_data, data,
        "SLOW PATH: should return different pointer"
    );
    assert_eq!(ori_rc_count(result_data), 1, "new buffer RC=1");
    assert_eq!(
        ori_rc_count(data),
        1,
        "old buffer RC=1 (was 2, decremented)"
    );

    // Verify new buffer: keys 10 and 30, no key 20
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 20),
            None,
            "key 20 removed"
        );
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
        let entries = map_collect_entries(result_data, cap as usize);
        assert_eq!(entries.len(), 2);
    }

    // Verify original unchanged: all 3 keys still present
    unsafe {
        assert_eq!(map_lookup_val(data, alloc_cap, 10), Some(100));
        assert_eq!(map_lookup_val(data, alloc_cap, 20), Some(200));
        assert_eq!(map_lookup_val(data, alloc_cap, 30), Some(300));
    }

    // Cleanup both buffers
    free_i64_map(data, alloc_cap);
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_remove_shared_last_entry_decs() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Single-entry shared map (RC=2)
    let (data, alloc_cap) = rc_alloc_i64_map(&[42], &[999], 4);
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2);

    let needle: i64 = 42;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        data,
        1,
        alloc_cap as i64,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0, "should be empty");
    assert_eq!(cap, 0, "capacity should be 0");
    assert!(result_data.is_null(), "should return null (empty sentinel)");
    assert_eq!(
        ori_rc_count(data),
        1,
        "old buffer RC decremented (was 2, now 1)"
    );

    // Cleanup original buffer (still alive, RC=1)
    free_i64_map(data, alloc_cap);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_remove_from_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let needle: i64 = 42;
    let mut out = [0u8; 24];

    map::cow::ori_map_remove_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::from_ref(&needle).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0, "still empty");
    assert_eq!(cap, 0, "still empty");
    assert!(result_data.is_null(), "still null");
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW map update tests ────────────────────────────────────────────

#[test]
fn cow_map_update_key_not_found() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    let key: i64 = 999; // not in map
    let val: i64 = 42;
    let mut out = [0u8; 24];

    map::cow::ori_map_update_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "should still have 3 entries (no insertion)");
    assert_eq!(cap, alloc_cap as i64, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "should return same pointer (no-op)"
    );

    // Values unchanged
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(map_lookup_val(result_data, cap as usize, 20), Some(200));
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
    }

    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_update_unique_overwrites_in_place() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    let original_ptr = data;

    // Update key 20's value from 200 to 999
    let key: i64 = 20;
    let val: i64 = 999;
    let mut out = [0u8; 24];

    map::cow::ori_map_update_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "should still have 3 entries");
    assert_eq!(cap, alloc_cap as i64, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "FAST PATH: same pointer (overwritten in place)"
    );

    // Verify only key 20's value changed
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 20),
            Some(999),
            "should be updated"
        );
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
    }

    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_update_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    // Shared map (RC=2)
    let (data, alloc_cap) = rc_alloc_i64_map(&[10, 20, 30], &[100, 200, 300], 4);
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2);

    // Update key 20's value
    let key: i64 = 20;
    let val: i64 = 999;
    let mut out = [0u8; 24];

    map::cow::ori_map_update_cow(
        data,
        3,
        alloc_cap as i64,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "should still have 3 entries");
    assert_ne!(
        result_data, data,
        "SLOW PATH: different pointer (new allocation)"
    );
    assert_eq!(ori_rc_count(result_data), 1, "new buffer RC=1");
    assert_eq!(ori_rc_count(data), 1, "old buffer RC=1 (was 2)");

    // Verify new buffer has updated value
    unsafe {
        assert_eq!(map_lookup_val(result_data, cap as usize, 10), Some(100));
        assert_eq!(
            map_lookup_val(result_data, cap as usize, 20),
            Some(999),
            "updated"
        );
        assert_eq!(map_lookup_val(result_data, cap as usize, 30), Some(300));
    }

    // Verify original unchanged
    unsafe {
        assert_eq!(map_lookup_val(data, alloc_cap, 10), Some(100));
        assert_eq!(
            map_lookup_val(data, alloc_cap, 20),
            Some(200),
            "original should be 200"
        );
        assert_eq!(map_lookup_val(data, alloc_cap, 30), Some(300));
    }

    // Cleanup both
    free_i64_map(data, alloc_cap);
    free_i64_map(result_data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_map_update_on_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let ks = std::mem::size_of::<i64>() as i64;
    let vs = std::mem::size_of::<i64>() as i64;

    let key: i64 = 42;
    let val: i64 = 999;
    let mut out = [0u8; 24];

    map::cow::ori_map_update_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::from_ref(&key).cast(),
        std::ptr::from_ref(&val).cast(),
        ks,
        vs,
        i64_key_eq,
        i64_key_hash,
        None,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0, "still empty (update doesn't insert)");
    assert_eq!(cap, 0, "still empty");
    assert!(result_data.is_null(), "still null");
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW set insert tests ────────────────────────────────────────────

/// Helper: allocate an RC'd hash table set buffer with i64 elements.
///
/// Returns `(data, actual_cap)`. The capacity is a power-of-two from the hash
/// table, which may be larger than `min_capacity`. Elements are inserted using
/// hash probing (identity hash for i64).
fn rc_alloc_i64_set(elems: &[i64], min_capacity: usize) -> (*mut u8, usize) {
    let len = elems.len();
    let es = std::mem::size_of::<i64>();
    let cap = map::hash_table::next_hash_capacity(min_capacity.max(len));
    if cap == 0 {
        return (std::ptr::null_mut(), 0);
    }
    let data = set::alloc_set_hash_buffer(cap, es);
    assert!(!data.is_null());
    let layout = map::hash_table::HashTableLayout::for_set(cap, es);
    for &e in elems {
        let hash = e; // identity hash
        let bucket = unsafe { map::hash_table::probe_find_slot(data, cap, hash) };
        unsafe {
            *data.add(layout.keys_offset + bucket * es).cast::<i64>() = e;
            map::hash_table::set_meta(data, bucket, map::hash_table::META_OCCUPIED);
        }
    }
    (data, cap)
}

/// Helper: check if an element exists in a hash table set buffer.
unsafe fn set_lookup_elem(data: *const u8, cap: usize, elem: i64) -> bool {
    let es = std::mem::size_of::<i64>();
    let layout = map::hash_table::HashTableLayout::for_set(cap, es);
    let hash = elem; // identity hash
    map::hash_table::probe_find(
        data,
        cap,
        layout.keys_offset,
        std::ptr::from_ref(&elem).cast(),
        hash,
        es,
        i64_key_eq,
    )
    .is_some()
}

/// Helper: collect all elements from a hash table set buffer (scan OCCUPIED buckets).
///
/// Returns elements in bucket order. Sort the result for deterministic comparison.
unsafe fn set_collect_elements(data: *const u8, cap: usize) -> Vec<i64> {
    let es = std::mem::size_of::<i64>();
    let layout = map::hash_table::HashTableLayout::for_set(cap, es);
    let mut elems = Vec::new();
    for bucket in 0..cap {
        if map::hash_table::get_meta(data, bucket) == map::hash_table::META_OCCUPIED {
            let val = *data.add(layout.keys_offset + bucket * es).cast::<i64>();
            elems.push(val);
        }
    }
    elems
}

/// Helper: free a hash table set buffer using the correct layout size.
fn free_i64_set(data: *mut u8, cap: usize) {
    let es = std::mem::size_of::<i64>();
    let layout = map::hash_table::HashTableLayout::for_set(cap, es);
    ori_rc_free(data, layout.total_size, 8);
}

#[test]
fn cow_set_insert_into_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let elem: i64 = 42;
    let mut out = [0u8; 24];

    set::cow::ori_set_insert_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::from_ref(&elem).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 1);
    assert!(cap >= 4, "should get at least MIN_HASH_CAPACITY, got {cap}");
    assert!(!data.is_null());
    assert_eq!(ori_rc_count(data), 1);

    unsafe { assert!(set_lookup_elem(data, cap as usize, 42), "42 must be in set") };

    free_i64_set(data, cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_insert_unique_new_elem() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // Allocate {10, 20} with min_capacity=4 → actual cap=8 (load factor headroom)
    let (data, cap) = rc_alloc_i64_set(&[10, 20], 4);
    let original_ptr = data;

    let elem: i64 = 30;
    let mut out = [0u8; 24];

    set::cow::ori_set_insert_cow(
        data,
        2,
        cap as i64,
        std::ptr::from_ref(&elem).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3);
    assert_eq!(result_cap, cap as i64, "capacity unchanged (had room)");
    assert_eq!(result_data, original_ptr, "FAST PATH: same pointer");

    unsafe {
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 10),
            "10 must be in result"
        );
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 20),
            "20 must be in result"
        );
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 30),
            "30 must be in result"
        );
    }

    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_insert_existing_elem_noop() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let original_ptr = data;

    let elem: i64 = 20; // already in set
    let mut out = [0u8; 24];

    set::cow::ori_set_insert_cow(
        data,
        3,
        cap as i64,
        std::ptr::from_ref(&elem).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3, "no change — element already exists");
    assert_eq!(result_cap, cap as i64, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "should return same pointer (no-op)"
    );

    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_insert_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[10, 20], 4);
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2);

    let elem: i64 = 30;
    let mut out = [0u8; 24];

    set::cow::ori_set_insert_cow(
        data,
        2,
        cap as i64,
        std::ptr::from_ref(&elem).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3);
    assert!(result_cap >= 3);
    assert_ne!(result_data, data, "SLOW PATH: different pointer");
    assert_eq!(ori_rc_count(result_data), 1, "new buffer RC=1");
    assert_eq!(ori_rc_count(data), 1, "old buffer RC decremented");

    unsafe {
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 20),
            "20 in result"
        );
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 30),
            "30 in result"
        );
        // Verify original unchanged
        assert!(set_lookup_elem(data, cap, 10), "10 still in original");
        assert!(set_lookup_elem(data, cap, 20), "20 still in original");
    }

    free_i64_set(data, cap);
    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_insert_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // Fill a set to load-factor threshold: 6 elements in cap=8 triggers rehash
    let (data, cap) = rc_alloc_i64_set(&[10, 20, 30, 40, 50, 60], 6);
    assert_eq!(cap, 8, "next_hash_capacity(6) = 8");

    let elem: i64 = 70;
    let mut out = [0u8; 24];

    set::cow::ori_set_insert_cow(
        data,
        6,
        cap as i64,
        std::ptr::from_ref(&elem).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 7);
    assert!(
        result_cap > cap as i64,
        "should have grown beyond {cap}, got {result_cap}"
    );
    assert!(!result_data.is_null());

    unsafe {
        for &v in &[10i64, 20, 30, 40, 50, 60, 70] {
            assert!(
                set_lookup_elem(result_data, result_cap as usize, v),
                "{v} must be in result"
            );
        }
    }

    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW set remove tests ────────────────────────────────────────────

#[test]
fn cow_set_remove_not_found() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let original_ptr = data;

    let needle: i64 = 999;
    let mut out = [0u8; 24];

    set::cow::ori_set_remove_cow(
        data,
        3,
        cap as i64,
        std::ptr::from_ref(&needle).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 3);
    assert_eq!(result_cap, cap as i64, "capacity unchanged");
    assert_eq!(
        result_data, original_ptr,
        "should return same pointer (no-op)"
    );

    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_remove_unique_middle() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let original_ptr = data;

    let needle: i64 = 20;
    let mut out = [0u8; 24];

    set::cow::ori_set_remove_cow(
        data,
        3,
        cap as i64,
        std::ptr::from_ref(&needle).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2);
    assert_eq!(
        result_cap, cap as i64,
        "capacity unchanged (tombstone in place)"
    );
    assert_eq!(result_data, original_ptr, "FAST PATH: same pointer");

    unsafe {
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 10),
            "10 remains"
        );
        assert!(
            !set_lookup_elem(result_data, result_cap as usize, 20),
            "20 removed"
        );
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 30),
            "30 remains"
        );
    }

    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_remove_unique_last_entry_frees() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[42], 1);
    assert_eq!(ori_rc_count(data), 1);

    let needle: i64 = 42;
    let mut out = [0u8; 24];

    set::cow::ori_set_remove_cow(
        data,
        1,
        cap as i64,
        std::ptr::from_ref(&needle).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0);
    assert_eq!(result_cap, 0);
    assert!(result_data.is_null(), "empty sentinel");
    assert_eq!(ori_rc_live_count(), before, "buffer freed, no leaks");
}

#[test]
fn cow_set_remove_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[10, 20, 30], 4);
    ori_rc_inc(data);
    assert_eq!(ori_rc_count(data), 2);

    let needle: i64 = 20;
    let mut out = [0u8; 24];

    set::cow::ori_set_remove_cow(
        data,
        3,
        cap as i64,
        std::ptr::from_ref(&needle).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 2);
    assert!(result_cap >= 2, "new buffer sized for 2 elements");
    assert_ne!(result_data, data, "SLOW PATH: different pointer");
    assert_eq!(ori_rc_count(result_data), 1, "new buffer RC=1");
    assert_eq!(ori_rc_count(data), 1, "old buffer RC decremented");

    unsafe {
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            !set_lookup_elem(result_data, result_cap as usize, 20),
            "20 removed"
        );
        assert!(
            set_lookup_elem(result_data, result_cap as usize, 30),
            "30 in result"
        );
        // Verify original unchanged
        assert!(set_lookup_elem(data, cap, 10), "10 still in original");
        assert!(set_lookup_elem(data, cap, 20), "20 still in original");
        assert!(set_lookup_elem(data, cap, 30), "30 still in original");
    }

    free_i64_set(data, cap);
    free_i64_set(result_data, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_remove_from_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let needle: i64 = 42;
    let mut out = [0u8; 24];

    set::cow::ori_set_remove_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::from_ref(&needle).cast(),
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };

    assert_eq!(len, 0);
    assert_eq!(cap, 0);
    assert!(result_data.is_null());
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW set union tests ────────────────────────────────────────────────

#[test]
fn cow_set_union_both_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;
    let mut out = [0u8; 24];

    set::cow::ori_set_union_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::null(),
        0,
        0,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0);
    assert_eq!(cap, 0);
    assert!(data.is_null());
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_union_set2_empty_identity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (data, cap) = rc_alloc_i64_set(&[10, 20], 4);
    let original_ptr = data;
    let mut out = [0u8; 24];

    // A ∪ ∅ = A (should return set1 unchanged)
    set::cow::ori_set_union_cow(
        data,
        2,
        cap as i64,
        std::ptr::null(),
        0,
        0,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert_eq!(result_cap, cap as i64, "capacity unchanged");
    assert_eq!(result, original_ptr, "identity: same pointer");

    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_union_set1_empty_copies_set2() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (d2, cap2) = rc_alloc_i64_set(&[30, 40], 2);
    let mut out = [0u8; 24];

    // ∅ ∪ B = B (should allocate new buffer with B's elements)
    set::cow::ori_set_union_cow(
        std::ptr::null_mut(),
        0,
        0,
        d2.cast_const(),
        2,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert!(result_cap >= 2);
    assert!(!result.is_null());
    assert_ne!(result, d2, "new allocation, not same as d2");

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 30),
            "30 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 40),
            "40 in result"
        );
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_union_unique_extends_in_place() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // set1 = {10, 20} with plenty of headroom, set2 = {20, 30, 40}
    // Use min_capacity=4 for d1 → cap=8; union of 4 unique elems fits (4 <= 8*3/4=6)
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 4);
    let original_ptr = d1;
    let (d2, cap2) = rc_alloc_i64_set(&[20, 30, 40], 4);
    let mut out = [0u8; 24];

    set::cow::ori_set_union_cow(
        d1,
        2,
        cap1 as i64,
        d2.cast_const(),
        3,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4, "union: {{10,20}} ∪ {{20,30,40}} = {{10,20,30,40}}");
    assert_eq!(result_cap, cap1 as i64, "capacity unchanged (had room)");
    assert_eq!(result, original_ptr, "FAST PATH: same pointer");

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 20),
            "20 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 30),
            "30 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 40),
            "40 in result"
        );
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_union_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // set1 = {10, 20} at load factor threshold (cap=4, 2 elems → needs_rehash(3,4)=true)
    // set2 = {30} — union of 3 elems forces growth
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 2);
    let (d2, cap2) = rc_alloc_i64_set(&[30], 1);
    let mut out = [0u8; 24];

    set::cow::ori_set_union_cow(
        d1,
        2,
        cap1 as i64,
        d2.cast_const(),
        1,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3);
    assert!(result_cap >= 4, "grown to fit 3 elements: cap={result_cap}");

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 20),
            "20 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 30),
            "30 in result"
        );
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_union_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 4);
    ori_rc_inc(d1);
    assert_eq!(ori_rc_count(d1), 2);

    let (d2, cap2) = rc_alloc_i64_set(&[20, 30], 2);
    let mut out = [0u8; 24];

    set::cow::ori_set_union_cow(
        d1,
        2,
        cap1 as i64,
        d2.cast_const(),
        2,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3, "union: {{10,20}} ∪ {{20,30}} = {{10,20,30}}");
    assert!(result_cap >= 3);
    assert_ne!(result, d1, "SLOW PATH: new allocation");
    assert_eq!(ori_rc_count(result), 1, "new buffer RC=1");
    assert_eq!(ori_rc_count(d1), 1, "old buffer RC decremented");

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 20),
            "20 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 30),
            "30 in result"
        );
        // Verify original unchanged
        assert!(set_lookup_elem(d1, cap1, 10), "10 still in original");
        assert!(set_lookup_elem(d1, cap1, 20), "20 still in original");
    }

    free_i64_set(d1, cap1);
    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_union_superset_noop() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // A ⊇ B → A ∪ B = A (no-op)
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let original_ptr = d1;
    let (d2, cap2) = rc_alloc_i64_set(&[10, 20], 2);
    let mut out = [0u8; 24];

    set::cow::ori_set_union_cow(
        d1,
        3,
        cap1 as i64,
        d2.cast_const(),
        2,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3, "superset: no change");
    assert_eq!(result_cap, cap1 as i64, "capacity unchanged");
    assert_eq!(result, original_ptr, "same pointer (no-op)");

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW set intersection tests ─────────────────────────────────────────

#[test]
fn cow_set_intersection_either_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // A ∩ ∅ = ∅
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 4);
    let mut out = [0u8; 24];

    set::cow::ori_set_intersection_cow(
        d1,
        2,
        cap1 as i64,
        std::ptr::null(),
        0,
        0,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0, "A ∩ ∅ = ∅");
    assert_eq!(cap, 0);
    assert!(data.is_null());
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_intersection_unique_compacts() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // {10, 20, 30} ∩ {20, 30, 40} = {20, 30}
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let original_ptr = d1;
    let (d2, cap2) = rc_alloc_i64_set(&[20, 30, 40], 4);
    let mut out = [0u8; 24];

    set::cow::ori_set_intersection_cow(
        d1,
        3,
        cap1 as i64,
        d2.cast_const(),
        3,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert_eq!(
        result_cap, cap1 as i64,
        "capacity preserved (in-place tombstone)"
    );
    assert_eq!(result, original_ptr, "FAST PATH: same pointer");

    unsafe {
        assert!(
            !set_lookup_elem(result, result_cap as usize, 10),
            "10 removed"
        );
        assert!(set_lookup_elem(result, result_cap as usize, 20), "20 kept");
        assert!(set_lookup_elem(result, result_cap as usize, 30), "30 kept");

        let mut elems = set_collect_elements(result, result_cap as usize);
        elems.sort_unstable();
        assert_eq!(elems, vec![20, 30]);
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_intersection_unique_all_removed() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // {10, 20} ∩ {30, 40} = ∅ (disjoint)
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 4);
    let (d2, cap2) = rc_alloc_i64_set(&[30, 40], 2);
    let mut out = [0u8; 24];

    set::cow::ori_set_intersection_cow(
        d1,
        2,
        cap1 as i64,
        d2.cast_const(),
        2,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0, "disjoint sets → empty");
    assert_eq!(cap, 0);
    assert!(result.is_null());

    free_i64_set(d2, cap2);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_intersection_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30], 4);
    ori_rc_inc(d1);
    assert_eq!(ori_rc_count(d1), 2);

    let (d2, cap2) = rc_alloc_i64_set(&[20, 30, 40], 4);
    let mut out = [0u8; 24];

    set::cow::ori_set_intersection_cow(
        d1,
        3,
        cap1 as i64,
        d2.cast_const(),
        3,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2, "{{10,20,30}} ∩ {{20,30,40}} = {{20,30}}");
    assert!(result_cap >= 2, "new buffer sized for 2 elements");
    assert_ne!(result, d1, "SLOW PATH: new allocation");
    assert_eq!(ori_rc_count(result), 1, "new buffer RC=1");
    assert_eq!(ori_rc_count(d1), 1, "old buffer RC decremented");

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 20),
            "20 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 30),
            "30 in result"
        );
        // Original unchanged
        assert!(set_lookup_elem(d1, cap1, 10), "10 still in original");
        assert!(set_lookup_elem(d1, cap1, 20), "20 still in original");
        assert!(set_lookup_elem(d1, cap1, 30), "30 still in original");
    }

    free_i64_set(d1, cap1);
    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_intersection_self_identity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // A ∩ A = A (all elements match)
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let original_ptr = d1;
    // Use a separate copy for d2 (d1 is consumed by the call)
    let (d2, cap2) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let mut out = [0u8; 24];

    set::cow::ori_set_intersection_cow(
        d1,
        3,
        cap1 as i64,
        d2.cast_const(),
        3,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 3, "A ∩ A = A");
    assert_eq!(result, original_ptr, "FAST PATH: all kept in place");

    unsafe {
        let mut elems = set_collect_elements(result, result_cap as usize);
        elems.sort_unstable();
        assert_eq!(elems, vec![10, 20, 30]);
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

// ── COW set difference tests ───────────────────────────────────────────

#[test]
fn cow_set_difference_set1_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;
    let mut out = [0u8; 24];

    // ∅ \ B = ∅
    set::cow::ori_set_difference_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::null(),
        0,
        0,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0, "∅ \\ B = ∅");
    assert_eq!(cap, 0);
    assert!(data.is_null());
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_difference_set2_empty_identity() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // A \ ∅ = A
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 4);
    let original_ptr = d1;
    let mut out = [0u8; 24];

    set::cow::ori_set_difference_cow(
        d1,
        2,
        cap1 as i64,
        std::ptr::null(),
        0,
        0,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2, "A \\ ∅ = A");
    assert_eq!(result_cap, cap1 as i64, "capacity unchanged");
    assert_eq!(result, original_ptr, "identity: same pointer");

    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_difference_unique_compacts() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // {10, 20, 30, 40} \ {20, 40} = {10, 30}
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30, 40], 4);
    let original_ptr = d1;
    let (d2, cap2) = rc_alloc_i64_set(&[20, 40], 2);
    let mut out = [0u8; 24];

    set::cow::ori_set_difference_cow(
        d1,
        4,
        cap1 as i64,
        d2.cast_const(),
        2,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert_eq!(
        result_cap, cap1 as i64,
        "capacity preserved (tombstone in place)"
    );
    assert_eq!(result, original_ptr, "FAST PATH: same pointer");

    unsafe {
        assert!(set_lookup_elem(result, result_cap as usize, 10), "10 kept");
        assert!(
            !set_lookup_elem(result, result_cap as usize, 20),
            "20 removed"
        );
        assert!(set_lookup_elem(result, result_cap as usize, 30), "30 kept");
        assert!(
            !set_lookup_elem(result, result_cap as usize, 40),
            "40 removed"
        );

        let mut elems = set_collect_elements(result, result_cap as usize);
        elems.sort_unstable();
        assert_eq!(elems, vec![10, 30]);
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_difference_self_yields_empty() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // A \ A = ∅
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let (d2, cap2) = rc_alloc_i64_set(&[10, 20, 30], 4);
    let mut out = [0u8; 24];

    set::cow::ori_set_difference_cow(
        d1,
        3,
        cap1 as i64,
        d2.cast_const(),
        3,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0, "A \\ A = ∅");
    assert_eq!(cap, 0);
    assert!(result.is_null());

    free_i64_set(d2, cap2);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_difference_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    let (d1, cap1) = rc_alloc_i64_set(&[10, 20, 30], 4);
    ori_rc_inc(d1);
    assert_eq!(ori_rc_count(d1), 2);

    let (d2, cap2) = rc_alloc_i64_set(&[20], 1);
    let mut out = [0u8; 24];

    set::cow::ori_set_difference_cow(
        d1,
        3,
        cap1 as i64,
        d2.cast_const(),
        1,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2, "{{10,20,30}} \\ {{20}} = {{10,30}}");
    assert!(result_cap >= 2, "new buffer sized for 2 elements");
    assert_ne!(result, d1, "SLOW PATH: new allocation");
    assert_eq!(ori_rc_count(result), 1, "new buffer RC=1");
    assert_eq!(ori_rc_count(d1), 1, "old buffer RC decremented");

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            !set_lookup_elem(result, result_cap as usize, 20),
            "20 removed"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 30),
            "30 in result"
        );
        // Original unchanged
        assert!(set_lookup_elem(d1, cap1, 10), "10 still in original");
        assert!(set_lookup_elem(d1, cap1, 20), "20 still in original");
        assert!(set_lookup_elem(d1, cap1, 30), "30 still in original");
    }

    free_i64_set(d1, cap1);
    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_set_difference_disjoint_noop() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>() as i64;

    // {10, 20} \ {30, 40} = {10, 20} (no overlap)
    let (d1, cap1) = rc_alloc_i64_set(&[10, 20], 4);
    let original_ptr = d1;
    let (d2, cap2) = rc_alloc_i64_set(&[30, 40], 2);
    let mut out = [0u8; 24];

    set::cow::ori_set_difference_cow(
        d1,
        2,
        cap1 as i64,
        d2.cast_const(),
        2,
        cap2 as i64,
        es,
        8,
        i64_key_eq,
        i64_key_hash,
        None,
        0,
        out.as_mut_ptr(),
    );

    let (len, result_cap, result) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2, "disjoint: no elements removed");
    assert_eq!(result_cap, cap1 as i64, "capacity unchanged");
    assert_eq!(
        result, original_ptr,
        "FAST PATH: same pointer (nothing removed)"
    );

    unsafe {
        assert!(
            set_lookup_elem(result, result_cap as usize, 10),
            "10 in result"
        );
        assert!(
            set_lookup_elem(result, result_cap as usize, 20),
            "20 in result"
        );
    }

    free_i64_set(d2, cap2);
    free_i64_set(result, result_cap as usize);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}
