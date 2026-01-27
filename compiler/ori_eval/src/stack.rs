//! Stack safety utilities for deep recursion.
//!
//! Uses the `stacker` crate to prevent stack overflow in recursive
//! evaluation of deeply nested expressions.
//!
//! For WASM targets where stacker isn't available, the function
//! just calls the closure directly (WASM has its own stack management).

/// Ensure sufficient stack space is available before executing `f`.
///
/// On native targets, uses `stacker` to grow the stack if needed.
/// On WASM targets, just calls the closure directly.
#[inline]
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    /// Minimum stack space to keep available (100KB red zone).
    const RED_ZONE: usize = 100 * 1024;

    /// Stack space to allocate when growing (1MB).
    const STACK_PER_RECURSION: usize = 1024 * 1024;

    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}

/// WASM version - just call directly (WASM has its own stack management).
#[inline]
#[cfg(target_arch = "wasm32")]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    f()
}
