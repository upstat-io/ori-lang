//! Container Packing (Layer 2)
//!
//! Container packing decisions for how to format lists, args,
//! fields, and other containers.
//!
//! # Architecture
//!
//! This module implements the second layer of the 5-layer formatter architecture:
//!
//! 1. **`Packing`**: The strategy for packing items (fit on one line, one per line, etc.)
//! 2. **`ConstructKind`**: What type of container we're formatting
//! 3. **`determine_packing()`**: Decision function mapping construct → packing strategy
//!
//! Spec: Annex D §Width-Based Breaking, §Always-Stacked Constructs (simple vs complex items).

mod construct;
mod separator;
mod simple;
mod strategy;

pub use construct::ConstructKind;
pub use separator::{separator_for, Separator};
pub use simple::{all_items_simple, is_simple_item, list_construct_kind};
pub use strategy::{determine_packing, Packing};

#[cfg(test)]
mod tests;
