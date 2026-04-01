#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — panics provide clear failure messages"
)]

//! Cross-crate sync enforcement tests.
//!
//! These tests verify that all consuming crates stay in sync with the
//! canonical `DerivedTrait` definitions in `ori_ir`. They complement
//! the per-crate unit tests with integration-level
//! validation that reads actual source files.
//!
//! # Running
//!
//! ```bash
//! cargo test -p oric --test sync
//! ```

#[path = "sync/prelude_functions.rs"]
mod prelude_functions;
#[path = "sync/prelude_traits.rs"]
mod prelude_traits;

/// Cross-phase invariant: `TypeId::FIRST_COMPOUND` must equal `Idx::FIRST_DYNAMIC`.
///
/// These constants define the boundary between pre-interned primitive types and
/// dynamically-allocated compound types. A mismatch would cause raw index values
/// to be interpreted as wrong types across the `ori_ir`/`ori_types` boundary.
#[test]
fn typeid_first_compound_matches_idx_first_dynamic() {
    assert_eq!(
        ori_ir::TypeId::FIRST_COMPOUND,
        ori_types::Idx::FIRST_DYNAMIC,
        "TypeId::FIRST_COMPOUND and Idx::FIRST_DYNAMIC must be equal \
         for raw index interchangeability"
    );
}
