//! Loop-carried struct self-rebuild RC accounting — burden-path AOT harness.
//!
//! A struct reassigned in a loop from two-or-more plain field-projections of
//! itself (`r = T { a: r.a, b: r.b }`) moves every field out via field-projection
//! transfers (RL-2 `ConstructArg`). The moved-out-field set must unify across the
//! sibling `Let { Var }` aliases the lowering creates for each projection, so the
//! field-grain partial-dec on each alias is recognized as a FULL move and no
//! spurious release is emitted. Without the unification the per-alias partial-dec
//! frees a field already transferred into the new struct and carried to the next
//! iteration — a double-free.
//!
//! Positive cells: the previously-double-freeing self-rebuild shapes across heap
//! field types. Negative cells: shapes that must keep their per-field lifecycle
//! (single field, one-field-replaced-fresh, explicit-alias kept-inc, no-loop).

use crate::util::assert_aot_success;

// Positive: two [int] fields self-forwarded in a loop (exit 134 double-free).
#[test]
#[ignore = "BUG-04-155: loop-carried struct self-rebuild from >=2 plain projections double-frees (AOT)"]
fn test_two_list_fields_loop_self_rebuild_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/two_list_fields_loop.ori"),
        "struct_self_rebuild_two_list_fields_loop",
    );
}

// Positive: mixed str + [int] fields (real heap str, not SSO) self-forwarded.
#[test]
#[ignore = "BUG-04-155: loop-carried struct self-rebuild from >=2 plain projections double-frees (AOT)"]
fn test_str_and_list_fields_loop_self_rebuild_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/str_and_list_fields_loop.ori"),
        "struct_self_rebuild_str_and_list_fields_loop",
    );
}

// Positive: three heap fields self-forwarded (union across three aliases).
#[test]
#[ignore = "BUG-04-155: loop-carried struct self-rebuild from >=2 plain projections double-frees (AOT)"]
fn test_three_heap_fields_loop_self_rebuild_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/three_heap_fields_loop.ori"),
        "struct_self_rebuild_three_heap_fields_loop",
    );
}

// Positive: one field plain-forwarded, the other pushed (Apply-consume +
// Construct-arg transfers both move out).
#[test]
fn test_heap_field_mutated_loop_self_rebuild_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/heap_field_mutated_loop.ori"),
        "struct_self_rebuild_heap_field_mutated_loop",
    );
}

// Negative: single heap field — nothing extra to unify; must not over-suppress.
#[test]
fn test_single_field_loop_self_rebuild_no_leak() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/single_field_loop_negative.ori"),
        "struct_self_rebuild_single_field_loop",
    );
}

// Negative: one field self-forwarded, the other built fresh — the fresh field
// did NOT move out of the loop-carried struct, so it keeps its own lifecycle.
#[test]
fn test_partial_replace_self_rebuild_no_leak() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/partial_replace_negative.ori"),
        "struct_self_rebuild_partial_replace",
    );
}

// Negative: explicit `let s = r` keeps the whole-struct inc (Many cardinality);
// the already-correct shape must be unperturbed.
#[test]
fn test_explicit_alias_loop_self_rebuild_no_leak() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/explicit_alias_loop_negative.ori"),
        "struct_self_rebuild_explicit_alias_loop",
    );
}

// Negative: two-field self-rebuild without a loop — single transfer, released
// once at scope exit.
#[test]
fn test_no_loop_self_rebuild_no_leak() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/no_loop_rebuild_negative.ori"),
        "struct_self_rebuild_no_loop",
    );
}
