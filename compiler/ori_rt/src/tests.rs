//! Tests for `ori_rt` core functions (memory, refcounting, panic).
//!
//! All RC tests acquire `RC_TEST_LOCK` because `RC_LIVE_COUNT` is a global
//! atomic counter modified by `ori_rc_alloc`/`ori_rc_free`. Without
//! serialization, parallel tests cause TOCTOU races in live-count assertions.
#![expect(clippy::unwrap_used, reason = "test code uses unwrap for clarity")]
#![expect(clippy::expect_used, reason = "test code uses expect for clarity")]
#![expect(
    clippy::items_after_statements,
    reason = "inner helper fns in test functions are idiomatic"
)]

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
    ori_list_pop_cow(data, 3, 4, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_pop_cow(data, 3, 4, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_pop_cow(data, 1, 4, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_remove_cow(data, 3, 4, 1, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_remove_cow(data, 3, 4, 2, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_remove_cow(data, 1, 4, 0, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_remove_cow(data, 3, 4, 1, es as i64, 8, None, out.as_mut_ptr());

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

    // list2: [30, 40]
    let data2 = rc_alloc_i64_list(&[30, 40], 4);

    let mut out = [0u8; 24];
    ori_list_concat_cow(data1, 2, 8, data2, 2, es as i64, 8, None, out.as_mut_ptr());

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

    ori_rc_free(result_data, 8 * es, 8);
    ori_rc_free(data2, 4 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_unique_needs_growth() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1: [10, 20] with capacity 2 (no room)
    let data1 = rc_alloc_i64_list(&[10, 20], 2);
    // list2: [30, 40]
    let data2 = rc_alloc_i64_list(&[30, 40], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(data1, 2, 2, data2, 2, es as i64, 8, None, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert!(cap >= 4, "grew to fit: {cap}");

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 10);
        assert_eq!(*result_data.cast::<i64>().add(1), 20);
        assert_eq!(*result_data.cast::<i64>().add(2), 30);
        assert_eq!(*result_data.cast::<i64>().add(3), 40);
    }

    ori_rc_free(result_data, cap as usize * es, 8);
    ori_rc_free(data2, 2 * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_shared_copies() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1 shared (RC=2)
    let data1 = rc_alloc_i64_list(&[10, 20], 4);
    ori_rc_inc(data1);
    let data2 = rc_alloc_i64_list(&[30, 40], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(data1, 2, 4, data2, 2, es as i64, 8, None, out.as_mut_ptr());

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 4);
    assert_ne!(result_data, data1, "SLOW PATH: different pointer (shared)");

    // Old buffer untouched
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

    ori_rc_free(data1, 4 * es, 8);
    ori_rc_free(data2, 2 * es, 8);
    ori_rc_free(result_data, cap as usize * es, 8);
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_empty_lists() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // Both empty
    let mut out = [0u8; 24];
    ori_list_concat_cow(
        std::ptr::null_mut(),
        0,
        0,
        std::ptr::null(),
        0,
        es as i64,
        8,
        None,
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0);
    assert!(result_data.is_null());
    assert_eq!(ori_rc_live_count(), before, "no leaks");
}

#[test]
fn cow_concat_empty_list1() {
    let _g = lock_rc();
    let before = ori_rc_live_count();
    let es = std::mem::size_of::<i64>();

    // list1 empty, list2 has data
    let data2 = rc_alloc_i64_list(&[30, 40], 2);

    let mut out = [0u8; 24];
    ori_list_concat_cow(
        std::ptr::null_mut(),
        0,
        0,
        data2,
        2,
        es as i64,
        8,
        None,
        out.as_mut_ptr(),
    );

    let (len, cap, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 2);
    assert!(!result_data.is_null());
    assert_ne!(result_data, data2, "fresh copy, not same as list2");

    unsafe {
        assert_eq!(*result_data.cast::<i64>(), 30);
        assert_eq!(*result_data.cast::<i64>().add(1), 40);
    }

    ori_rc_free(data2, 2 * es, 8);
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
    ori_list_reverse_cow(data, 4, 4, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_reverse_cow(data, 3, 4, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_reverse_cow(data, 3, 4, es as i64, 8, None, out.as_mut_ptr());

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
    ori_list_reverse_cow(data, 1, 4, es as i64, 8, None, out.as_mut_ptr());

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
        out.as_mut_ptr(),
    );

    let (len, _, result_data) = unsafe { read_list_result(&out) };
    assert_eq!(len, 0);
    assert!(result_data.is_null());
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
