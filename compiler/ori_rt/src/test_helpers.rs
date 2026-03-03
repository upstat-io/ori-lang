//! Shared test utilities for `ori_rt`.
//!
//! `RC_LIVE_COUNT` is a process-global atomic counter modified by
//! `ori_rc_alloc` / `ori_rc_free`. Without serialization, tests that
//! assert on `ori_rc_live_count()` race with any concurrent test that
//! allocates or frees RC objects.
//!
//! Every `#[test]` that calls RC functions **must** acquire this lock:
//!
//! ```ignore
//! let _g = crate::test_helpers::lock_rc();
//! ```

use std::sync::MutexGuard;

/// Global mutex that serializes all RC-touching tests across every
/// module in `ori_rt`. Without this, parallel test threads cause
/// TOCTOU races on `RC_LIVE_COUNT`.
static RC_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire the RC test lock. Returns a guard that releases on drop.
///
/// Recovers from mutex poisoning (prior test panicked while holding
/// the lock) so that subsequent tests still run.
pub fn lock_rc() -> MutexGuard<'static, ()> {
    RC_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
