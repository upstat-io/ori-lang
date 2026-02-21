#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code — panics provide clear failure messages"
)]

//! Cross-crate sync enforcement tests.
//!
//! These tests verify that all consuming crates stay in sync with the
//! canonical `DerivedTrait` definitions in `ori_ir`. They complement
//! the per-crate unit tests (Section 05.1) with integration-level
//! validation that reads actual source files.
//!
//! # Running
//!
//! ```bash
//! cargo test -p oric --test sync
//! ```

#[path = "sync/prelude_traits.rs"]
mod prelude_traits;
