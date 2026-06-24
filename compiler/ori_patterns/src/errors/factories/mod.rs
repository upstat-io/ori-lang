//! Error-factory functions for pattern evaluation.
//!
//! Each factory populates an [`EvalError`](crate::errors::EvalError) from a
//! structured [`EvalErrorKind`](crate::errors::EvalErrorKind) (where one
//! exists) or a `Custom` message. The factory functions are the public API;
//! downstream crates call them rather than constructing `EvalError` directly.

mod collections;
mod construction;

pub use collections::*;
pub use construction::*;
