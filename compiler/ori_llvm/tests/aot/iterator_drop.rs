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

/// TPR-07-011 semantic pin: match-and-consume on a tagged-pointer
/// enum iterator payload must not double-free. The `Project`
/// codegen decodes the payload pointer from the enum, the match
/// arm consumes it via `.count()`, and the ARC pipeline must NOT
/// also emit a scope-exit drop on the original enum (which would
/// decode the same pointer and drop it a second time). Iteration-3
/// finding of the TPR-07-008 review: the iteration-1 matrix only
/// covered the "payload goes out of scope unused" case.
#[test]
fn tpr_07_011_enum_tagged_ptr_match_consume_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/enum_tagged_ptr_match_consume.ori"),
        "tpr_07_011_enum_tagged_ptr_match_consume",
    );
}

/// TPR-07-011 matrix pin: `Empty` branch selected at construction
/// time — no iterator ever exists, so nothing to drop. Ensures the
/// take-project suppression doesn't accidentally break the trivial
/// no-iterator path.
#[test]
fn tpr_07_011_enum_tagged_ptr_match_empty_path() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/enum_tagged_ptr_match_empty.ori"),
        "tpr_07_011_enum_tagged_ptr_match_empty",
    );
}

/// TPR-07-011 matrix pin: dynamic construction via a helper function
/// — the compiler cannot constant-fold the match, so both branches
/// are live and ARC must correctly handle the control-flow diamond.
/// Exercises the full path: Construct (via helper) → Switch →
/// Holds(project→count) | Empty(0) → merge → return.
#[test]
fn tpr_07_011_enum_tagged_ptr_match_consume_dynamic() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/enum_tagged_ptr_match_consume_dynamic.ori"),
        "tpr_07_011_enum_tagged_ptr_match_consume_dynamic",
    );
}

/// TPR-07-013 semantic pin: symmetric case of TPR-07-011. Matching
/// an iterator payload from a user sum type into a binding that is
/// NEVER consumed must drop the projected iterator at scope exit.
/// TPR-07-011 correctly suppressed the source enum's scope-exit
/// drop when a take-project fires (preventing double-free on the
/// "project-then-consume" path), but the symmetric "project-then-
/// drop-unused" path needs the projected iterator binding itself
/// to drop at its own scope exit — it's an ownership transfer, not
/// an alias.
#[test]
fn tpr_07_013_enum_match_unused_binding_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/enum_match_unused_binding.ori"),
        "tpr_07_013_enum_match_unused_binding",
    );
}

/// TPR-07-013 matrix pin: `Option<Iterator<int>>` case of the
/// project-unused pattern.
#[test]
fn tpr_07_013_option_match_unused_binding_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/option_match_unused_binding.ori"),
        "tpr_07_013_option_match_unused_binding",
    );
}

/// TPR-07-013 matrix pin: `Result<Iterator<int>, int>` case of the
/// project-unused pattern.
#[test]
fn tpr_07_013_result_match_unused_binding_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/result_match_unused_binding.ori"),
        "tpr_07_013_result_match_unused_binding",
    );
}

/// TPR-07-016 semantic pin: path-sensitive take-project suppression.
/// The TPR-07-011 fix suppressed the source enum's scope-exit drop
/// function-globally when a take-project reaches the source chain.
/// That's too coarse: on runtime paths that never reach the
/// projection (e.g., `if false then match x { Holds(it) -> ... }
/// else 0`), the source enum leaks because the global suppression
/// also disables the drop on the non-projecting path.
///
/// The correct behavior is path-sensitive: the drop is suppressed
/// on paths that actually execute the projection, retained on paths
/// that don't. Codex iteration 6 found this gap with an
/// `if flag then <match with consume> else 0` repro.
#[test]
fn tpr_07_016_enum_conditional_consume_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/enum_conditional_consume.ori"),
        "tpr_07_016_enum_conditional_consume",
    );
}

/// TPR-07-017 semantic pin: per-class take-project partitioning.
/// Two unrelated take-projects coexist in the same function on
/// different alias chains. With function-global bypass-safe block
/// computation, every block reachable from `match_b` is excluded from
/// `a`'s bypass-safe set, so `a` falls back to the alias-only
/// suppression and LEAKS its iterator payload on the runtime path
/// that bypasses `match_a`. Per-class partitioning fixes this by
/// computing bypass-safety against only the take-projects in `a`'s
/// own connected-component class, independent of unrelated `b`.
///
/// Codex iteration 7 found this gap during TPR review of the
/// TPR-07-016 fix. The `take_project.rs` source itself flagged
/// "two iterators moved on different branches" as the missing
/// partitioning case.
#[test]
fn tpr_07_017_two_unrelated_take_projects_no_leak() {
    assert_aot_success(
        include_str!("fixtures/iterator_drop/two_unrelated_take_projects.ori"),
        "tpr_07_017_two_unrelated_take_projects",
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
