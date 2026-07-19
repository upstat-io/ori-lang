//! Method dispatch for variant types (Option, Result, bool, char, byte, newtype).
//!
//! - [`scalar`]: scalar variants (bool, char, byte, newtype)
//! - [`wrapper`]: wrapper variants (Option, Result)

mod scalar;
mod wrapper;

pub(super) use scalar::{
    dispatch_bool_method, dispatch_byte_method, dispatch_char_method, dispatch_newtype_method,
};
pub(super) use wrapper::{dispatch_option_method, dispatch_result_method};
