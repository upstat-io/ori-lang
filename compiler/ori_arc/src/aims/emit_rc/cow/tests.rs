//! Tests for `is_borrow_disjoint_from_siblings` — the helper-layer
//! regression surface introduced by
//!
//! The helper now requires source `Uniqueness::Unique` exclusively — the former
//! former cross-dimensional `Owned + Linear + Once` fallback (via
//! `is_cow_aware_unique`) was removed as unsound per `aims-rules.md` §DP-10
//! removal rationale.
//!
//! Full state-map construction for the helper requires building an
//! ``AimsStateMap`` with `borrow_source` entries + sibling borrows tracking +
//! source uniqueness state. That machinery is heavyweight for a unit
//! test — the integration coverage comes from the AIMS snapshot tests
//! (`aims_snapshots::*`) and from the `decide_cow()` pure-function tests
//! in `realize/tests.rs` that exercise `ctx.is_borrow_disjoint=true`.
//!
//! This file establishes the test-home per
//! `compiler/ori_arc/src/aims/emit_rc/cow.rs`'s `#[cfg(test)] mod tests;`
//! declaration. Pure-function coverage for the uniqueness-gate logic lives
//! in the integration test `decide_cow_maybe_shared_with_unique_source_disjoint_borrow_stays_static_unique`
//! in `realize/tests.rs` (verifies that when `ctx.is_borrow_disjoint` is
//! `true` the `MaybeShared` → `StaticUnique` promotion fires — this proxies the
//! preserved-positive case since `is_borrow_disjoint_from_siblings()` is
//! the upstream setter for that flag).

// No pure-function unit tests in this file — see module doc above.
// Integration coverage via `realize/tests.rs` + AIMS snapshot corpus.
