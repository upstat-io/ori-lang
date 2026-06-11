//! Cross-block loop self-rebuild RC accounting — the BUG-04-156 RED matrix.
//!
//! Shapes that thread the loop-carried struct (or a field) across CFG-block
//! boundaries: branch-arm-exclusive rebuilds, sum-payload match block-param
//! handoffs, post-construct duplication reads, and alternating variant arms.
//! Both RC emission paths mis-account on these (the predicate-stack baseline
//! leaks too); the same-block sibling-union analysis deliberately declines
//! them. Eval is clean on every fixture — the family is AOT-only.

use crate::util::{assert_aot_success, compile_and_run_with_build_env};

// Positive: rebuild on one branch arm only, read on the other. The read-arm
// borrow aliases of the loop-carried param are pure borrow-views — their
// whole-var inc/dec pairs move WHOLE in the per-var elision (loop-carried
// pair atomicity); splitting them released the loop lineage's still-live
// reference on the read arm (already-freed double-free).
#[test]
fn test_branch_exclusive_rebuild_no_overfire() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/branch_exclusive_rebuild.ori"),
        "struct_self_rebuild_branch_exclusive",
    );
}

// Positive: sum-payload field extracted through a match feeding the rebuild
// (pre-fix SIGSEGV, exit 139).
#[test]
#[ignore = "BUG-04-156: sum-payload match rebuild — the bb-resident carrier's partial dec releases the sum field whose PAYLOAD was extracted through the match block-param handoff and re-wrapped into the new struct (misaligned-pointer dec); needs variant-tag-aware extract-transfer attribution (every switch arm either transfers the field payload to the rebuild construct or is a payload-less-variant arm) before the field counts as moved"]
fn test_sum_payload_sibling_fields_no_overfire() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/sum_payload_match_rebuild.ori"),
        "struct_self_rebuild_sum_payload_match",
    );
}

// Positive: explicit alias read AFTER the construct site in the iteration —
// the post-construct read makes the intermediate alias a genuine duplication
// (kept inc); the assigned KEEPER sibling's whole dec balances it (its own
// fresh-site inc is suppressed), and the loop's final value gets its RL-5
// dead-at-entry release at the loop-exit dead block-param (the loop-carried
// var is unused after the loop).
#[test]
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
#[ignore = "BUG-04-156: cross-variant projection — moved-field attribution mis-types across variant arms (misaligned-pointer dec); the per-arm extract is CONDITIONALLY transferred (inner branch inside the Some arm), so the variant-tag-aware extract-transfer attribution must DECLINE here while still releasing the dropped payload on the None-result path; residual mis-typed dec persists on both RC paths"]
fn test_cross_variant_projection_declines_unification() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/cross_variant_projection_loop.ori"),
        "struct_self_rebuild_cross_variant_projection",
    );
}

// Positive: the loop-carried struct re-threaded through TWO Jump-arg hops
// (if-merge block-param + loop latch) before the rebuild; the per-edge
// attribution resolves the merged name back to the loop lineage.
#[test]
fn test_two_jump_hop_rethreaded_rebuild_no_leak() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/two_hop_alias_loop.ori"),
        "struct_self_rebuild_two_hop_alias",
    );
}

// Negative: a genuine loop-header merge (forward INIT edge vs back-edge
// ITERATION value of one lineage) must NOT unify per-edge; the pre-rebuild
// read sees the current iteration's struct and the lineage stays leak-clean.
#[test]
fn test_loop_header_init_vs_back_edge_keeps_per_edge_identity() {
    assert_aot_success(
        include_str!("fixtures/struct_self_rebuild/loop_header_merge_read.ori"),
        "struct_self_rebuild_loop_header_merge_read",
    );
}

// Toggle semantic pin: disabling the RL-5 rebuild-lineage dead-param release
// restores the loop-exit leak on the late-use shape (the loop-carried var is
// unused after the loop; the union suppressed the in-loop releases).
#[test]
fn test_rebuild_lineage_release_toggle_restores_leak() {
    let source = include_str!("fixtures/struct_self_rebuild/late_use_alias_loop.ori");
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        source,
        &[("ORI_DISABLE_REBUILD_LINEAGE_DEAD_PARAM_RELEASE", "1")],
    );
    assert!(
        exit != 0 || stderr.contains("not freed"),
        "toggle ON must restore the loop-exit leak; exit={exit} stderr={stderr}"
    );
}
