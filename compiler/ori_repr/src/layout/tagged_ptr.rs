//! Tagged pointer analysis for enum representation optimization (§07.3).
//!
//! On 64-bit systems, heap pointers have alignment ≥8, meaning the low 3 bits
//! are always zero. These bits can store a 3-bit tag (up to 8 variants).
//!
//! Only single-word pointer types are taggable:
//! - `RcPointer` (heap-allocated, 8-byte aligned)
//! - `OpaquePtr` (C interop pointer)
//! - `UnmanagedPtr` (raw pointer)
//!
//! Multi-word types (`FatPointer`, `Closure`, `Struct`, `Tuple`) are NOT
//! taggable — they occupy more than 8 bytes.
//!
//! This module provides [`can_use_tagged_pointer`] to check if an enum layout
//! is eligible for tagged pointer optimization.
