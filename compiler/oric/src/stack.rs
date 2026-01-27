//! Stack safety utilities for deep recursion.
//!
//! Uses the `stacker` crate to prevent stack overflow in recursive
//! parsing, type-checking, and evaluation of deeply nested expressions.
//!
//! # Usage
//!
//! Wrap recursive calls that could overflow with `ensure_sufficient_stack`:
//!
//! ```ignore
//! fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
//!     ensure_sufficient_stack(|| {
//!         // ... recursive parsing logic ...
//!     })
//! }
//! ```

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
/// ```ignore
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
#[inline]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shallow_recursion() {
        fn factorial(n: u64) -> u64 {
            ensure_sufficient_stack(|| {
                if n <= 1 { 1 } else { n * factorial(n - 1) }
            })
        }

        assert_eq!(factorial(10), 3_628_800);
    }

    #[test]
    fn test_deep_recursion() {
        // This would overflow without stack growth
        fn deep_recurse(n: u64) -> u64 {
            ensure_sufficient_stack(|| {
                if n == 0 { 0 } else { deep_recurse(n - 1) + 1 }
            })
        }

        // 100k recursions - would overflow a typical 8MB stack
        assert_eq!(deep_recurse(100_000), 100_000);
    }
}
