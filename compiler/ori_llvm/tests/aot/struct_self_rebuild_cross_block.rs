//! Cross-block loop self-rebuild RC accounting — the BUG-04-156 RED matrix.
//!
//! Shapes that thread the loop-carried struct (or a field) across CFG-block
//! boundaries: branch-arm-exclusive rebuilds, sum-payload match block-param
//! handoffs, post-construct duplication reads, and alternating variant arms.
//! Both RC emission paths mis-account on these (the predicate-stack baseline
//! leaks too); the same-block sibling-union analysis deliberately declines
//! them. Eval is clean on every fixture — the family is AOT-only.

use crate::util::assert_aot_success;

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
