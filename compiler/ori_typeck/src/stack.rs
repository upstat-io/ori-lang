//! Stack safety utilities for recursive type checking.
//!
//! Uses the `stacker` crate to ensure sufficient stack space for deeply nested
//! type inference operations.

/// Ensure sufficient stack space for recursive operations.
///
/// Grows the stack if remaining space is less than 256KB, allocating up to 2MB.
/// This prevents stack overflows in deeply nested type inference.
pub fn ensure_sufficient_stack<R, F: FnOnce() -> R>(f: F) -> R {
    stacker::maybe_grow(256 * 1024, 2 * 1024 * 1024, f)
}
