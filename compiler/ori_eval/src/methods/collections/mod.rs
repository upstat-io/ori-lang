//! Method dispatch for collection types (str, map, range, set).
//!
//! - [`string_range`]: free-function dispatch for `str` and `range` (no interpreter access)
//! - [`map_set`]: `Interpreter` methods for `map` and `set` — non-primitive
//!   (user-`Hashable`) keys require `@hash` + `@eq` calls, which need interpreter access

mod map_set;
mod string_range;

pub use string_range::{dispatch_range_method, dispatch_string_method};
