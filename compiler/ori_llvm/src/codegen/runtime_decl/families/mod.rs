//! Per-family runtime-function declaration slices.
//!
//! Each submodule holds one contiguous family of [`RtFn`] entries;
//! `super::runtime_functions::RT_FUNCTIONS` concatenates them into the
//! single lookup surface (SSOT preserved — one table, one index, one
//! `jit_allowed` partition test).
//!
//! [`RtFn`]: super::types::RtFn

pub(super) mod base;
pub(super) mod cow_args;
pub(super) mod eh;
pub(super) mod format;
pub(super) mod iterator;
pub(super) mod list;
pub(super) mod map;
pub(super) mod rc;
pub(super) mod set;
pub(super) mod slice;
pub(super) mod strings;
