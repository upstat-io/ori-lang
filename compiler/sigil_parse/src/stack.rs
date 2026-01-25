//! Stack safety for recursive parsing.
//!
//! Prevents stack overflow on deeply nested expressions.

const RED_ZONE: usize = 100 * 1024; // 100KB
const STACK_PER_RECURSION: usize = 1024 * 1024; // 1MB

/// Ensure sufficient stack space for recursive operations.
#[inline]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}
