//! Ori AOT runtime C ABI.
//!
//! The crate owns allocation, reference counting, strings, collections,
//! formatting, I/O, exception boundaries, and iterator execution for compiled
//! programs. Collection mutations use copy-on-write buffers; strings use SSO
//! for payloads of at most 23 bytes.

#![warn(clippy::allow_attributes_without_reason)]
#![allow(
    unsafe_code,
    reason = "C-ABI runtime functions require unsafe for raw pointer operations"
)]
#![allow(
    clippy::not_unsafe_ptr_arg_deref,
    reason = "FFI entry points receive compiler-generated valid pointers"
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_ptr_alignment,
    reason = "FFI code uses i64 for ABI compatibility"
)]
#![allow(
    clippy::manual_let_else,
    reason = "explicit matches preserve FFI error-path readability"
)]
#![allow(
    clippy::borrow_as_ptr,
    clippy::ptr_cast_constness,
    clippy::cast_slice_from_raw_parts,
    reason = "runtime tests intentionally form raw pointers"
)]

mod abi;
mod allocation;
mod entry;
pub mod format;
mod integer;
pub mod io;
pub mod iterator;
pub mod list;
pub mod map;
pub mod prelude;
pub mod rc;
pub mod set;
pub(crate) mod slice_encoding;
pub mod string;

// Preserve the established flat C-ABI surface while each implementation stays
// owned by its domain module. Map and set remain module-qualified because both
// expose a `cow` child module.
pub use prelude::*;

pub(crate) use abi::{OPTION_TAG_NONE, OPTION_TAG_SOME};
pub(crate) use allocation::next_capacity;
pub(crate) use rc::{check_leaks_enabled, RC_LIVE_COUNT};

#[cfg(all(test, debug_assertions))]
pub(crate) use rc::{freed_set, rt_debug_check_not_freed};
#[cfg(test)]
pub(crate) use rc::{rc_trace_enabled, MAX_REFCOUNT};
#[cfg(all(test, debug_assertions))]
pub(crate) use rc::{rt_debug_validate_rc, RT_DEBUG_FORCE};

#[cfg(all(
    test,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod forced_unwind_tests;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;
