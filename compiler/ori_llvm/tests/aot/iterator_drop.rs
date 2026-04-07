//! AOT tests for TPR-07-008: iterators in compound containers must be
//! dropped via `ori_iter_drop` at scope exit.
//!
//! Before the fix, `classify_triviality()` marked `Tag::Iterator` and
//! `Tag::DoubleEndedIterator` as `Trivial`, so the ARC pipeline never
//! emitted any drop for iterator values outside of `for`-loops (which
//! emit `ori_iter_drop` explicitly during lowering). Iterators stored
//! in enum variants, struct fields, tuple elements, or bare `let`
//! bindings that were never consumed by `iter_next` leaked their
//! Box-allocated state.
//!
//! The fix classifies iterators as non-trivial at the SSOT
//! (`ori_types::triviality`), adds `RcStrategy::Iterator` to the ARC
//! emitter for direct `RcDec` on iterator variables, adds a
//! `Tag::Iterator` arm in `dec_value_rc_inner` for iterator fields
//! inside compound types, and upgrades `ProtocolBuiltin::IterDrop`'s
//! ownership contract from `Borrowed` to `Owned` so the explicit
//! for-loop drop is correctly modeled as consuming the iterator
//! (preventing a double-free from the ARC pipeline also inserting a
//! scope-exit drop).
//!
//! # Matrix coverage
//!
//! - **`enum_tagged_ptr_iter`**: exact Codex repro — iterator payload
//!   in a tagged-pointer-eligible enum. Semantic pin.
//! - **`struct_iter_field`**: iterator field in a user struct.
//! - **`tuple_iter_element`**: iterator element in a tuple.
//! - **`bare_unused_iter`**: bare `let` iterator never consumed.
//! - **`for_loop_still_works`**: regression guard — for-loop iterator
//!   cleanup must continue to work (no double-free, no leak) under
//!   the new IterDrop=Owned ownership contract.
//!
//! Spec: `plans/repr-opt/section-07-enum-repr.md` §07.R TPR-07-008.

use crate::util::assert_aot_success;

/// Semantic pin: the exact Codex repro. An enum with an iterator
/// payload that goes out of scope without being consumed must not
/// leak. This is the canonical regression guard — reverting the fix
/// would cause this test to fail with exit code 2 (leak detected).
#[test]
fn tpr_07_008_enum_tagged_ptr_iter_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/enum_tagged_ptr_iter.ori"),
        "tpr_07_008_enum_tagged_ptr_iter",
    );
}

// Note: the explicit-tag enum case (≥9 variants carrying an
// iterator payload) is blocked by BUG-04-044 — the Construct path
// emits `insertvalue [N x i64], ptr` without ptrtoint casting the
// iterator pointer to i64 for the slot array. That's a pre-existing
// codegen bug in the explicit-tag enum lowering, unrelated to the
// triviality SSOT fix here. The struct and tuple matrix pins below
// exercise the same `dec_value_rc` compound-drop dispatch path that
// would fire for an explicit-tag enum variant field, so the
// compound-drop coverage is preserved through a different shape.
// When BUG-04-044 is fixed, the explicit-tag enum test should be
// added back here as an additional matrix pin.

/// Matrix pin: iterator as a struct field. The struct's generated
/// drop function must traverse the field via `dec_value_rc` and emit
/// an `ori_iter_drop` call.
#[test]
fn tpr_07_008_struct_iter_field_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/struct_iter_field.ori"),
        "tpr_07_008_struct_iter_field",
    );
}

/// Matrix pin: iterator as a tuple element. Same shape as struct
/// field but through the tuple dispatch path.
#[test]
fn tpr_07_008_tuple_iter_element_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/tuple_iter_element.ori"),
        "tpr_07_008_tuple_iter_element",
    );
}

/// Matrix pin: bare iterator variable never consumed. Direct
/// `RcDec(iter_var)` at function-end scope, handled by
/// `RcStrategy::Iterator` dispatch in the ARC emitter.
#[test]
fn tpr_07_008_bare_unused_iter_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/bare_unused_iter.ori"),
        "tpr_07_008_bare_unused_iter",
    );
}

/// Regression guard: for-loops already emit explicit `ori_iter_drop`
/// during ARC lowering. After upgrading `IterDrop` to `Owned`
/// ownership, ARC must not insert a second drop at loop exit
/// (double-free) and must still release the iterator (not leak).
#[test]
fn tpr_07_008_for_loop_still_works() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/for_loop_still_works.ori"),
        "tpr_07_008_for_loop_still_works",
    );
}
