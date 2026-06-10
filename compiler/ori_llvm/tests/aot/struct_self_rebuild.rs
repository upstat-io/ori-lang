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
fn test_two_list_fields_loop_self_rebuild_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/two_list_fields_loop.ori"),
        "struct_self_rebuild_two_list_fields_loop",
    );
}

// Positive: mixed str + [int] fields (real heap str, not SSO) self-forwarded.
#[test]
fn test_str_and_list_fields_loop_self_rebuild_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/str_and_list_fields_loop.ori"),
        "struct_self_rebuild_str_and_list_fields_loop",
    );
}

// Positive: three heap fields self-forwarded (union across three aliases).
#[test]
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

// Positive: rebuild on one branch arm only, read on the other — the per-alias
// partial-dec still double-frees on the rebuild arm.
#[test]
#[ignore = "BUG-04-156: branch-exclusive rebuild — cross-arm sibling field move; needs cross-block same-allocation accounting (predicate-stack path also leaks), distinct from the same-block sibling-union cure"]
fn test_branch_exclusive_rebuild_no_overfire() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/branch_exclusive_rebuild.ori"),
        "struct_self_rebuild_branch_exclusive",
    );
}

// Positive: sum-payload field extracted through a match feeding the rebuild
// (pre-fix SIGSEGV, exit 139).
#[test]
#[ignore = "BUG-04-156: sum-payload match rebuild — field threads through a match block-param handoff (distinct chain root); needs cross-block same-allocation accounting forbidden to this Phase-5 same-block cure"]
fn test_sum_payload_sibling_fields_no_overfire() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/sum_payload_match_rebuild.ori"),
        "struct_self_rebuild_sum_payload_match",
    );
}

// Negative: one field through an identity-forwarder Apply (Owned ttr param),
// the other plainly self-projected — the forwarder transfer already accounts
// correctly; the sibling-union fix must not perturb it.
#[test]
fn test_apply_transferred_field_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/apply_transferred_field_loop.ori"),
        "struct_self_rebuild_apply_transferred_field",
    );
}

// Positive: explicit alias read AFTER the construct site in the iteration —
// the alias's release set must keep what it still backs.
#[test]
#[ignore = "BUG-04-156: late-use alias — the post-construct read makes the moved field a genuine duplication (kept inc), not a pure move; distinct from the same-block sibling-union full-move suppression"]
fn test_late_use_alias_keeps_uncovered_dec() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/late_use_alias_loop.ori"),
        "struct_self_rebuild_late_use_alias",
    );
}

// Positive: mixed coverage — three heap fields, only two self-projected, the
// third replaced fresh; pins the per-FIELD verdict + single-releaser ledger.
#[test]
fn test_three_fields_two_self_rebuilt_mixed_coverage() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/three_fields_two_self_rebuilt.ori"),
        "struct_self_rebuild_three_fields_two_self_rebuilt",
    );
}

// Positive: projections through DIFFERENT variant arms across iterations —
// nondeterministic misaligned-pointer dec (type confusion) pre-fix; the
// cross-variant DECLINE must also leave the shape leak-clean.
#[test]
#[ignore = "BUG-04-156: cross-variant projection — moved-field attribution mis-types across variant arms (type-confusion dec); distinct cross-variant defect, declined by the sibling-union multivariant-sum gate"]
fn test_cross_variant_projection_declines_unification() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/cross_variant_projection_loop.ori"),
        "struct_self_rebuild_cross_variant_projection",
    );
}

// Positive: heap + SCALAR fields both self-projected — the scalar projection
// counts as the second sibling alias, so the heap field's partial-dec still
// double-frees (the >=2 trigger is projected FIELDS, not heap fields).
#[test]
fn test_mixed_heap_scalar_fields_no_overwiden() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/mixed_heap_scalar_fields_loop.ori"),
        "struct_self_rebuild_mixed_heap_scalar_fields",
    );
}

// Positive: nested heap-carrying struct field — top-level grain accounting;
// the outer drop-glue must release nested owned fields exactly once.
#[test]
fn test_nested_heap_field_loop_self_rebuild() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/nested_heap_field_loop.ori"),
        "struct_self_rebuild_nested_heap_field",
    );
}
