//! Loop-based method codegen for collection types (list, map, set).
//!
//! Unlike tuple/option/result methods which unroll at compile-time,
//! collection methods require runtime loops with phi-merged accumulators
//! because element count is dynamic. Extracted from `lower_builtin_methods/`
//! to keep files under 500 lines.
//!
//! # Supported operations
//!
//! - **List**: `compare`, `hash`, `equals` (in `list.rs`)
//! - **Set**: `equals`, `hash` (in `set.rs`)
//! - **Map**: `equals`, `hash` (in `map.rs`)

mod list;
mod map;
mod set;
