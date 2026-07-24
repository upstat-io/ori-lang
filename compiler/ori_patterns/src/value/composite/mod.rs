//! Composite value types: structs, functions, and ranges.
//!
//! These types are more complex than primitive values and have
//! their own internal structure. Each type family lives in its own
//! submodule; this module re-exports the public surface.

mod function_value;
mod range_value;
mod struct_value;

pub use function_value::{FunctionValue, MemoizedFunctionValue};
pub use range_value::RangeValue;
pub use struct_value::{StructLayout, StructValue};

#[cfg(test)]
#[allow(
    clippy::cast_possible_wrap,
    reason = "tests use small literal values (0-10) that fit in i64 without wrapping"
)]
mod tests;
