//! Collection and constructor lowering.
//!
//! Spread variants and template strings are eliminated during canonicalization;
//! this module dispatches the remaining canonical collection forms.

mod list_pattern;
mod lowering;
mod propagation;

pub(crate) use list_pattern::{emit_list_element, emit_list_rest_slice};

#[cfg(test)]
mod tests;
