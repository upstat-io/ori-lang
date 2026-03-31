#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_h1_drop_hint_unique_list() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h1_drop_hint_unique_list.ori"),
        "h1_drop_hint_unique_list",
    );
}

#[test]
fn test_h3_precise_death_points_once_cardinality() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h3_precise_death_points_once_cardinality.ori"),
        "h3_precise_death_points_once_cardinality",
    );
}

#[test]
fn test_h4_fip_certified_recursive_rebuild() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h4_fip_certified_recursive_rebuild.ori"),
        "h4_fip_certified_recursive_rebuild",
    );
}

#[test]
fn test_h6_callee_returns_unique_for_caller_reuse() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h6_callee_returns_unique_for_caller_reuse.ori"),
        "h6_callee_returns_unique_for_caller_reuse",
    );
}

#[test]
fn test_h7_pure_callee_then_static_unique_cow_mutation() {
    assert_aot_success(
        include_str!(
            "fixtures/aims_interactions/h7_pure_callee_then_static_unique_cow_mutation.ori"
        ),
        "h7_pure_callee_then_static_unique_cow_mutation",
    );
}

#[test]
fn test_h13_reuse_priority_over_drop_hint() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h13_reuse_priority_over_drop_hint.ori"),
        "h13_reuse_priority_over_drop_hint",
    );
}

#[test]
fn test_h15_reuse_survives_block_merge() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h15_reuse_survives_block_merge.ori"),
        "h15_reuse_survives_block_merge",
    );
}

// H2: Lattice × TRMC — Canonicalize Rule 4 fires on context var in TRMC loop.
// Context var is StaticUnique for in-place Set (no Dynamic COW check).

#[test]
fn test_h2_lattice_trmc_context_var_unique() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h2_lattice_trmc_context_var_unique.ori"),
        "h2_lattice_trmc_context_var_unique",
    );
}

// H5: Intra × TRMC — Analysis reconverges on TRMC-rewritten IR.
// Loop back-edge creates cycle in dataflow; context vars get Unique+FunctionLocal.

#[test]
fn test_h5_intra_trmc_reconvergence() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h5_intra_trmc_reconvergence.ori"),
        "h5_intra_trmc_reconvergence",
    );
}

// H8: Inter × TRMC — Callee is TRMC-rewritten; caller sees refreshed contract.
// Caller uses has_unbounded_stack=false from updated contract.

#[test]
fn test_h8_inter_trmc_refreshed_contract() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h8_inter_trmc_refreshed_contract.ori"),
        "h8_inter_trmc_refreshed_contract",
    );
}

// H9: RC × FIP — FIP Certified function has zero RcInc; all RcDec matched by reuse.

#[test]
fn test_h9_rc_fip_zero_net_rc() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h9_rc_fip_zero_net_rc.ori"),
        "h9_rc_fip_zero_net_rc",
    );
}

// H10: RC × TRMC — RcInc/RcDec correct for context vars in TRMC loop body.
// Context root: no RcDec until base case; context hole_obj: Set only.

#[test]
fn test_h10_rc_trmc_context_var_lifecycle() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h10_rc_trmc_context_var_lifecycle.ori"),
        "h10_rc_trmc_context_var_lifecycle",
    );
}

// H11: RC × TailCall — Tail-call rewrite removes self-call; no phantom RcInc.
// Args transferred via Jump, not call.

#[test]
fn test_h11_rc_tailcall_no_phantom_inc() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h11_rc_tailcall_no_phantom_inc.ori"),
        "h11_rc_tailcall_no_phantom_inc",
    );
}

// H12: RC × Merge — Block merge doesn't invalidate RC operations.
// Trampoline blocks cleaned up correctly.

#[test]
fn test_h12_rc_merge_preserves_rc_ops() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h12_rc_merge_preserves_rc_ops.ori"),
        "h12_rc_merge_preserves_rc_ops",
    );
}

// H14: Reuse × TRMC — Pattern match in TRMC-rewritten loop: scrutinee
// death + same-type construct → reuse.

#[test]
fn test_h14_reuse_trmc_loop_reuse() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h14_reuse_trmc_loop_reuse.ori"),
        "h14_reuse_trmc_loop_reuse",
    );
}

// H16: COW × FIP — FIP Certified function: all COW ops are StaticUnique (auto-FBIP).

#[test]
fn test_h16_cow_fip_auto_fbip() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h16_cow_fip_auto_fbip.ori"),
        "h16_cow_fip_auto_fbip",
    );
}

// H17: COW × TRMC — COW mutation in TRMC loop body on context var → StaticUnique.

#[test]
fn test_h17_cow_trmc_static_unique_context() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h17_cow_trmc_static_unique_context.ori"),
        "h17_cow_trmc_static_unique_context",
    );
}

// H18: Drop × TRMC — RcDec on context root at base-case return → drop hint
// if collection type. Context root is unique by construction.

#[test]
fn test_h18_drop_trmc_context_root_hint() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h18_drop_trmc_context_root_hint.ori"),
        "h18_drop_trmc_context_root_hint",
    );
}

// H19: FIP × TRMC — Contract upgrades from Never to Certified post-rewrite.
// Before TRMC: unbounded stack (recursive calls). After: constant stack.

#[test]
fn test_h19_fip_trmc_contract_upgrade() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h19_fip_trmc_contract_upgrade.ori"),
        "h19_fip_trmc_contract_upgrade",
    );
}

// H20: TRMC × TailCall — TRMC produces loop back-edge; tail-call pass
// runs after; must not double-loopify.

#[test]
fn test_h20_trmc_tailcall_no_double_loop() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h20_trmc_tailcall_no_double_loop.ori"),
        "h20_trmc_tailcall_no_double_loop",
    );
}

// H21: TRMC × Merge — TRMC prologue→header→helper topology survives merge.
// Prologue retained (or inlined correctly); context init preserved.

#[test]
fn test_h21_trmc_merge_topology_preserved() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h21_trmc_merge_topology_preserved.ori"),
        "h21_trmc_merge_topology_preserved",
    );
}

// H22: TRMC × Verify — Verify pass checks rewritten IR: no undefined vars,
// no unreachable blocks, RC balanced.

#[test]
fn test_h22_trmc_verify_clean_ir() {
    assert_aot_success(
        include_str!("fixtures/aims_interactions/h22_trmc_verify_clean_ir.ori"),
        "h22_trmc_verify_clean_ir",
    );
}
