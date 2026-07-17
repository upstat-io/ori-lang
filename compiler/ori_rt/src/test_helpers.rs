//! Shared synchronization and ABI-buffer utilities for `ori_rt` tests.
//!
//! RC tests serialize access to the process-global allocation counter.
//! [`AbiOutput`] gives raw sret storage the alignment required by runtime ABIs.

use std::ops::{Deref, DerefMut};
use std::sync::MutexGuard;

/// Zeroed, 16-byte-aligned storage for an ABI result.
#[repr(C, align(16))]
pub(crate) struct AbiOutput<const N: usize>([u8; N]);

impl<const N: usize> Default for AbiOutput<N> {
    fn default() -> Self {
        Self([0; N])
    }
}

impl<const N: usize> Deref for AbiOutput<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> DerefMut for AbiOutput<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

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
