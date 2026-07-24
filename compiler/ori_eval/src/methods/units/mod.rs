//! Method dispatch for unit types (Duration, Size).
//!
//! - [`duration`]: Duration accessors, operators, factory functions, formatting
//! - [`size`]: Size accessors, operators, factory functions, formatting

mod duration;
mod size;

pub use duration::{dispatch_duration_associated, dispatch_duration_method};
pub use size::{dispatch_size_associated, dispatch_size_method};

pub(in crate::methods) use duration::format_duration_debug;
pub(in crate::methods) use size::format_size_debug;
