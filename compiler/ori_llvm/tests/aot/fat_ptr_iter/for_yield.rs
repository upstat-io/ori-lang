//! For-yield tests — F3 (transform), F10 (identity yield), break/continue in yield.
//!
//! Yield identity: the loop variable `w` is borrowed from the iterator. When yielded
//! directly (not transformed to a scalar), the element escapes the iterator's borrow
//! scope. The ARC pipeline must emit `RcInc` on `w` before passing it to `ori_list_push`.

use crate::util::assert_aot_success;

// Yield identity — for w in words yield w

#[test]
fn test_yield_identity_str_list() {
    // `for w in words yield w` — yields the actual string element
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/yield_identity_str_list.ori"),
        "yield_identity_str_list",
    );
}

#[test]
fn test_yield_identity_str_list_borrowed_param() {
    // Borrowed param + yield identity: the borrowed [str] is iterated
    // and each element is yielded into a new list.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/for_yield/yield_identity_str_list_borrowed_param.ori"
        ),
        "yield_identity_str_list_borrowed_param",
    );
}

#[test]
fn test_yield_identity_str_list_two_calls() {
    // Two calls to a function that does yield identity on borrowed [str].
    // Stresses RC: original must survive both clones.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/yield_identity_str_list_two_calls.ori"),
        "yield_identity_str_list_two_calls",
    );
}

// For-yield with non-scalar elements — elem_dec_fn correctness

/// `[str]` for-yield identity — borrowed str elements yielded into new list.
/// Verifies `elem_dec_fn` correctly cleans up source list elements when the
/// iterator drops, while the result list owns its own copies.
#[test]
fn test_for_yield_str_identity() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_str_identity.ori"),
        "for_yield_str_identity",
    );
}

/// [str] for-yield with scalar transformation — str elements borrowed,
/// lengths (int) yielded. Verifies no leak on source str elements.
#[test]
fn test_for_yield_str_to_lengths() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_str_to_lengths.ori"),
        "for_yield_str_to_lengths",
    );
}

/// `[[int]]` for-yield — nested list elements borrowed from outer list,
/// inner sums yielded as scalars. Verifies `elem_dec_fn` on nested `[int]`.
#[test]
fn test_for_yield_nested_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_nested_list.ori"),
        "for_yield_nested_list",
    );
}

/// `[Option<str>]` for-yield with match — `Option<str>` elements borrowed,
/// pattern-matched to extract lengths. Verifies `elem_dec_fn` on `Option<str>`.
#[test]
fn test_for_yield_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_option_str.ori"),
        "for_yield_option_str",
    );
}

// For-yield mutable variable threading (TPR-02-002 regression)

/// Outer mutable variable mutation inside for-yield body — the body
/// assigns to `sum` which is declared outside the for-yield. Verifies
/// that the assignment is correctly propagated through the loop's SSA
/// block parameters. Regression test for TPR-02-002 where
/// `clear_mutable_names()` silently dropped the assignment in AOT.
#[test]
fn test_for_yield_outer_mutable_mutation() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_outer_mutable_mutation.ori"),
        "for_yield_outer_mutable_mutation",
    );
}

/// Nested for-do inside for-yield body — inner loop mutates an outer
/// variable. Verifies mutable threading works with nested control flow.
#[test]
fn test_for_yield_nested_for_do_mutation() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_nested_for_do_mutation.ori"),
        "for_yield_nested_for_do_mutation",
    );
}

/// For-yield with str elements and outer mutable counter — combines
/// fat pointer iteration with mutable variable threading and leak check.
#[test]
fn test_for_yield_str_with_mutable_counter() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_str_with_mutable_counter.ori"),
        "for_yield_str_with_mutable_counter",
    );
}

// For-yield RC balance tests (Section 03.4)

/// For-yield with closure elements — closures applied to argument in body.
#[test]
fn test_for_yield_closure_elements() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_closure_elements.ori"),
        "for_yield_closure_elements",
    );
}

/// For-yield with struct elements containing str fields — struct field
/// access in body, verifies element RC through struct aggregates.
#[test]
fn test_for_yield_struct_elements() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_struct_elements.ori"),
        "for_yield_struct_elements",
    );
}

/// For-yield with guard on `[str]` — filters short strings, yields only
/// long ones. Verifies `guard_skip` path threads mutable params correctly.
#[test]
fn test_for_yield_guard_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_guard_str.ori"),
        "for_yield_guard_str",
    );
}

/// For-yield on `[[str]]` — nested list of str lists. Verifies element
/// cleanup for nested fat pointer types.
#[test]
fn test_for_yield_nested_str_loops() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_nested_str_loops.ori"),
        "for_yield_nested_str_loops",
    );
}

/// For-yield on empty `[str]` — zero iterations, empty result list.
/// Verifies no leak from allocated-but-empty growable list.
#[test]
fn test_for_yield_empty_str_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_empty_str_list.ori"),
        "for_yield_empty_str_list",
    );
}

// For-yield break/continue lowering

/// `break` in for-yield: stop early, return accumulated list.
#[test]
fn test_for_yield_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_break.ori"),
        "for_yield_break",
    );
}

/// `break value` in for-yield: push value then stop.
#[test]
fn test_for_yield_break_value() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_break_value.ori"),
        "for_yield_break_value",
    );
}

/// `continue` in for-yield: skip element, don't push.
#[test]
fn test_for_yield_continue() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_continue.ori"),
        "for_yield_continue",
    );
}

/// `continue value` in for-yield: push substituted value.
#[test]
fn test_for_yield_continue_value() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_continue_value.ori"),
        "for_yield_continue_value",
    );
}

/// `break` in for-yield over str list: RC correctness with fat pointers.
#[test]
fn test_for_yield_break_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_break_str.ori"),
        "for_yield_break_str",
    );
}

/// `continue` in for-yield over str list: skip without leaking.
#[test]
fn test_for_yield_continue_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_continue_str.ori"),
        "for_yield_continue_str",
    );
}

/// `break` + mutable var: mutable variable threading preserved across break.
#[test]
fn test_for_yield_break_mutable() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_break_mutable.ori"),
        "for_yield_break_mutable",
    );
}

/// `continue value` + mutable var: mutable variable threading through continue.
#[test]
fn test_for_yield_continue_value_mutable() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/for_yield/for_yield_continue_value_mutable.ori"),
        "for_yield_continue_value_mutable",
    );
}
