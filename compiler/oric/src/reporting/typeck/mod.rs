//! Type error rendering.
//!
//! Re-exports from `ori_types::reporting` which owns the implementation.
//! This module exists for backward compatibility — all callers within `oric`
//! continue to import from `crate::reporting::typeck::*`.
//!
//! Tests live in `ori_types::reporting::tests`.

pub use ori_types::reporting::{render_type_errors, TypeErrorRenderer};
