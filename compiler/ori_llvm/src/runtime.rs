//! Runtime functions for LLVM-compiled Ori code.
//!
//! This module re-exports runtime functions from the `ori_rt` crate.
//! The actual implementations live in `ori_rt`, which can be built as
//! both an rlib (for JIT) and a staticlib (for AOT linking).
//!
//! # Safety
//!
//! This module re-exports `#[no_mangle]` functions for FFI compatibility with LLVM JIT.
//! All functions are safe Rust but need stable symbol names.

// Re-export everything from ori_rt
pub use ori_rt::*;
