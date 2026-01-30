//! Stack safety utilities for deep recursion.
//!
//! Prevents stack overflow in recursive parsing, type-checking, and evaluation
//! of deeply nested expressions by dynamically growing the stack when needed.
//!
//! # Platform Support
//!
//! - **Native targets**: Uses the `stacker` crate to grow the stack on demand.
//! - **WASM targets**: No-op passthrough (WASM has its own stack management).
//!
//! # Usage
//!
//! Wrap recursive calls that could overflow with [`ensure_sufficient_stack`]:
//!
//! ```text
//! fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
//!     ensure_sufficient_stack(|| {
//!         // ... recursive parsing logic ...
//!     })
//! }
//! ```
//!
//! # Configuration
//!
//! - **Red zone**: 100KB - If less than this remains, we grow the stack
//! - **Growth size**: 1MB - Each growth allocates this much additional space
//!
//! These values are chosen to handle deeply nested code (100k+ recursion depth)
//! while keeping memory usage reasonable.

/// Minimum stack space to keep available (100KB red zone).
///
/// If less than this amount remains, we'll grow the stack.
const RED_ZONE: usize = 100 * 1024;

/// Stack space to allocate when growing (1MB).
///
/// Each growth allocates this much additional stack space.
const STACK_PER_RECURSION: usize = 1024 * 1024;

/// Ensure sufficient stack space is available before executing `f`.
///
/// If the remaining stack is below the red zone threshold, this will
/// allocate additional stack space before calling `f`. This prevents
/// stack overflow in deeply recursive code paths.
///
/// # Example
///
/// ```text
/// fn recursive_operation(&mut self, depth: usize) -> Result<Value, Error> {
///     ensure_sufficient_stack(|| {
///         if depth == 0 {
///             Ok(base_case())
///         } else {
///             // Safe to recurse - stack will grow if needed
///             self.recursive_operation(depth - 1)
///         }
///     })
/// }
/// ```
///
/// # Platform Behavior
///
/// - **Native**: Uses `stacker::maybe_grow` to dynamically grow the stack
/// - **WASM**: Simply calls `f()` directly (WASM manages its own stack)
#[inline]
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}

/// WASM version - just call directly (WASM has its own stack management).
#[inline]
#[cfg(target_arch = "wasm32")]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shallow_recursion() {
        fn factorial(n: u64) -> u64 {
            ensure_sufficient_stack(|| if n <= 1 { 1 } else { n * factorial(n - 1) })
        }

        assert_eq!(factorial(10), 3_628_800);
    }

    #[test]
    fn test_deep_recursion() {
        // This would overflow without stack growth
        fn deep_recurse(n: u64) -> u64 {
            ensure_sufficient_stack(|| if n == 0 { 0 } else { deep_recurse(n - 1) + 1 })
        }

        // 100k recursions - would overflow a typical 8MB stack
        assert_eq!(deep_recurse(100_000), 100_000);
    }

    #[test]
    fn test_returns_closure_result() {
        let result = ensure_sufficient_stack(|| 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_works_with_result_type() {
        let result: Result<i32, &str> = ensure_sufficient_stack(|| Ok(123));
        assert_eq!(result, Ok(123));
    }
}
