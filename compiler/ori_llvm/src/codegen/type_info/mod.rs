//! `TypeInfo` enum, `TypeInfoStore`, and `TypeLayoutResolver` for V2 codegen.
//!
//! Every Ori type category gets a [`TypeInfo`] variant that encapsulates its
//! LLVM representation, memory layout, and calling convention. Adding a new
//! type means adding one enum variant — not modifying match arms across the
//! codebase.
//!
//! Design from Swift's `TypeInfo` hierarchy, adapted as a Rust enum per Ori
//! coding guidelines ("enum for fixed sets — exhaustiveness, static dispatch").
//!
//! # Module Layout
//!
//! - **[`info`]** — `TypeInfo` enum + methods (`storage_type`, `size`, `alignment`, triviality)
//! - **[`store`]** — `TypeInfoStore` cached `Idx` → `TypeInfo` mapping
//! - **[`layout_resolver`]** — `TypeLayoutResolver`: recursive LLVM type resolution with cycle detection
//! - **[`enum_layout`]** — Enum-specific LLVM type resolution (tagless, niche, explicit)
//!
//! # References
//!
//! - Swift `lib/IRGen/TypeInfo.h` (hierarchy: `TypeInfo` > `FixedTypeInfo` > `LoadableTypeInfo`)
//! - Roc `gen_llvm/src/llvm/convert.rs` (`basic_type_from_layout`)
//! - Zig `src/codegen/llvm.zig` (`lowerType` with `TypeMap` cache)

mod enum_layout;
mod info;
mod layout_resolver;
mod repr_lowering;
mod store;
pub(crate) mod type_size;

pub use info::{EnumVariantInfo, TypeInfo};
pub use layout_resolver::TypeLayoutResolver;
pub use store::TypeInfoStore;

// Tests

#[cfg(test)]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::uninlined_format_args,
    clippy::doc_markdown,
    reason = "benchmark/test code — precision loss acceptable for display, style relaxed"
)]
mod tests;
