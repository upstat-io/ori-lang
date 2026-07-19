//! Method dispatch for collection types (str, map, range, set).
//!
//! - [`string`] / [`range`]: free-function dispatch (no interpreter access)
//! - [`map_set`]: `Interpreter` methods for `map` and `set` — non-primitive
//!   (user-`Hashable`) keys require `@hash` + `@eq` calls, which need interpreter access

mod map_set;
mod range;
mod string;

pub use range::dispatch_range_method;
pub use string::dispatch_string_method;
