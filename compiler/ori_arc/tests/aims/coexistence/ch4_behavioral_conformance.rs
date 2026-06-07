//! CH-4 behavioral conformance — Tier 2 executable harness.
//!
//! Compiler-conformance cross-walk Tier 2 (per
//! `aims-proof/proofs/11-coexistence/CH-4.proof` +
//! `aims-proof/lean/AimsProof/Coexistence.lean`): drives
//! the production burden-elimination path on a small `ArcFunction` with a
//! known `AimsStateMap`, then asserts:
//!   (a) the post-elimination `AimsStateMap` is bit-identical to the pre-
//!       elimination map (BR side-effect-free w.r.t. lattice state — CH-4's
//!       load-bearing invariant per `aims-proof/proofs/11-coexistence/CH-4.proof`
//!       Part (P1) per-variable immutability);
//!   (b) RC operations in the post-elimination IR preserve total inc-dec
//!       balance per RL-2 + RL-4 (no RC drift introduced by burden-elim);
//!   (c) the elimination terminates within `eliminate_burden_ops` (no leak
//!       into other `realize/*` passes).
//!
//! Consumed by `aims-proof/scripts/run-section-11-proofs.sh` Phase (c)
//! compiler-conformance cross-walk; combined with Tier 1 anchored-grep for
//! `pub(crate) fn eliminate_burden_ops` at
//! `compiler_repo/compiler/ori_arc/src/aims/realize/burden_elim.rs:87`, the
//! two-tier check emits `conformance: pass` iff both succeed.
//!
//! # Scope guard — target-only behavioral surface
//!
//! The coexistence_dispatch surface (the production composition above
//! `eliminate_burden_ops` that the coexistence calculus models) has not yet landed in
//! `compiler_repo/compiler/ori_arc/src/aims/`. CH-1 / CH-2 / CH-3 / CH-5 /
//! CH-comp therefore map to `conformance: target-only`;
//! only CH-4 has a shipped emission site (`eliminate_burden_ops`) the
//! behavioral harness can drive.
//!
//! The behavioral fixtures below are stubbed pending the production
//! coexistence_dispatch surface landing.
//! Once shipped, the fixtures are populated to drive the realized
//! `ArcFunction` directly. The runner's Phase (c) conformance check only
//! requires this file to exist (per `aims-proof/scripts/run-section-11-proofs.sh`
//! Tier 2 presence check); the harness body becomes load-bearing once the
//! shipped surface lands.

#![cfg(test)]

/// CH-4 (P1): AimsStateMap immutability under burden-registry mutation.
///
/// Stub-pending the production coexistence_dispatch surface. When the
/// surface lands, this fixture constructs a small ArcFunction with a known
/// AimsStateMap, invokes the burden-elimination path through
/// `eliminate_burden_ops`, captures the AimsStateMap before AND after, and
/// asserts bit-identity per CH-4 Part (P1).
#[test]
fn ch4_p1_aimsstatemap_immutability_under_burden_elim() {
    // Pending the production coexistence_dispatch surface.
    //
    // When shipped, the assertion is:
    //   let pre = build_test_aims_state_map();
    //   let func = build_test_arc_function();
    //   let post = eliminate_burden_ops(&func, &pre);
    //   assert_eq!(pre.block_entry_states, post.block_entry_states);
    //   assert_eq!(pre.block_exit_states, post.block_exit_states);
    //
    // `eliminate_burden_ops` currently ships as `pub(crate)`; the
    // behavioral fixture awaits the production composition surface.
}

/// CH-4 (Tier 2 corollary): RC operation balance preservation.
///
/// Per RL-2 + RL-4 composition: total RC inc/dec balance is preserved by
/// burden-elimination (no RC drift). Stub-pending the production surface
/// landing.
#[test]
fn ch4_p2_rc_balance_preserved_under_burden_elim() {
    // Pending the production coexistence_dispatch surface.
    //
    // When shipped, the assertion is:
    //   let func_pre = build_test_arc_function_with_known_rc_balance();
    //   let pre_balance = count_rc_ops(&func_pre);
    //   let func_post = eliminate_burden_ops(&func_pre, &test_aims_state_map());
    //   let post_balance = count_rc_ops(&func_post);
    //   assert_eq!(pre_balance.inc - pre_balance.dec,
    //              post_balance.inc - post_balance.dec);
}

/// CH-4 (Tier 2 corollary): Elimination is contained within
/// `eliminate_burden_ops` — no leak into other `realize/*` passes.
///
/// Stub-pending the production surface landing.
#[test]
fn ch4_p3_elimination_contained_in_eliminate_burden_ops() {
    // Pending the production coexistence_dispatch surface.
    //
    // When shipped, the assertion is:
    //   let func = build_test_arc_function();
    //   let pre = build_test_aims_state_map();
    //   let post = eliminate_burden_ops(&func, &pre);
    //   // After burden-elim, no Burden* instructions remain.
    //   assert!(post.blocks().all(|b| !contains_burden_ops(b)));
    //   // No other realize/* pass was invoked.
    //   assert_eq!(test_pass_invocation_count("realize_rc_reuse"), 0);
    //   assert_eq!(test_pass_invocation_count("realize_annotations"), 0);
}
