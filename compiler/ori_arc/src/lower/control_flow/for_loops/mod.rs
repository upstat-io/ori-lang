//! For-loop variant lowering — iterator, option, and range.
//!
//! Each variant produces a distinct block structure optimized for its
//! iteration pattern. Mutable variables from the enclosing scope flow
//! through the loop as SSA block parameters.
//!
//! - **Iterator**: `__iter_next` loop with tag/element projection.
//! - **Option**: 0-or-1 element branch (Some/None).
//! - **Range**: Direct counter-based loop with `start..end` projection.

mod for_iterator;
mod for_option;
mod for_range;
