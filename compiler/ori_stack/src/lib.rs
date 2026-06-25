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

mod grow;

pub use grow::ensure_sufficient_stack;

#[cfg(test)]
mod tests;
