//! Serialization for tests that observe the process-global RC counter.

use std::sync::MutexGuard;

static RC_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire the RC-test lock, recovering from a prior test panic.
pub(crate) fn lock_rc() -> MutexGuard<'static, ()> {
    RC_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
