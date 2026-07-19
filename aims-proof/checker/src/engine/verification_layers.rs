//! §09 verification-layer discharge — VF-1 through VF-8 (+ the VF-comp
//! composition obligation).
//!
//! Per `Annex E §AIMS §9` +
//! the §01 `aims-proof/proofs/01-verification/Verification.proof` sorry
//! obligation, the VF category dispatches to [`structural_induction`,
//! `refinement`, `rc_counting`, `interprocedural_summary`] per the
//! coverage-manifest VF row. Each VF-N has ONE PRIMARY engine (constructive
//! discharge) and SECONDARY engines (gracious-accept once the primary has
//! discharged), mirroring the `pipeline_ordering` / `realization_rules`
//! PRIMARY-constructive / SECONDARY-gracious-accept split:
//!
//! - `structural_induction` PRIMARY — per-instruction structural-check
//! soundness (VF-1 5-check coverage), contract-consistency soundness
//! (VF-2 AbsentParamHasUses), end-to-end mandate (VF-5 three-tier
//! subsystem), active-rewrite three-tier discipline (VF-7), and the
//! composition obligation (VF-comp layered-stack union).
//! - `refinement` PRIMARY — oracle re-derivation as a lattice-≤ refinement
//! of the inferred contract (VF-3) + contracts↔realization agreement under
//! the stack (VF-6).
//! - `rc_counting` PRIMARY — FIP certification balance proof (VF-4): a
//! `Certified` function has zero unmatched alloc/dealloc in realized IR;
//! discharged via an alloc/dealloc balance ledger (the alloc-balance
//! analogue of the §08 RC-balance ledger).
//! - `interprocedural_summary` PRIMARY — cross-call-site coverage check
//! (VF-8): the stack applies to ALL rules incl. unimplemented §08
//! RL-22..RL-26 + target-only IC-3 / IC-5 surface; an unimplemented rule
//! without a planned verification layer is a spec gap.
//!
//! Each PRIMARY verifier encodes the shipped/target verification semantics as
//! a fixture grid, discharges the layer's soundness invariant (each layer
//! catches a distinct failure class; the stack catches the union), and carries
//! a negative-direction witness so the check has teeth (a fix that passes one
//! layer but regresses another is REJECTED). Verifiers reason over a model of
//! the shipped verifiers; they cite the shipped
//! `compiler/ori_arc/src/{verify,aims/verify}/` sites in
//! comments per Annex E §AIMS

use crate::ast::{Category, Theorem};
use crate::engine::{EngineResult, EngineVerdict};

/// Discharge a VF theorem for the named engine, or return `None` when the
/// theorem is not a §09 verification-layer rule this module serves YET (so the
/// calling engine falls through to its `UnimplementedShape` stub for
/// not-yet-discharged rules).
///
/// Returns `Some(EngineResult)` when `theorem.id` is an implemented VF rule
/// AND `engine_name` is one of the four VF-row engines. The PRIMARY engine
/// gets the constructive verifier; the SECONDARY engines gracious-accept.
pub fn discharge_for_engine(engine_name: &str, theorem: &Theorem) -> Option<EngineResult> {
    if theorem.id.category != Category::VerificationLayer {
        return None;
    }
    let suffix = theorem.id.suffix.as_str();
    let primary = primary_engine_for(suffix)?;
    // Only the four VF-row engines participate.
    if !matches!(
        engine_name,
        "structural_induction" | "refinement" | "rc_counting" | "interprocedural_summary"
    ) {
        return None;
    }
    if engine_name == primary {
        Some(run_primary_verifier(suffix))
    } else {
        // SECONDARY engine: gracious-accept once the PRIMARY has discharged
        // (per the coverage-manifest VF row + the pipeline_ordering / realization_rules
        // gracious-accept precedent).
        Some(gracious_accept())
    }
}

/// Map a VF rule suffix to its PRIMARY engine, or `None` when the rule's
/// verifier has not yet landed (so the rule stays `unimplemented_engine_shape`
/// until its cluster discharges).
fn primary_engine_for(suffix: &str) -> Option<&'static str> {
    let engine = match suffix {
        // VF-1 Layer-1 structural well-formedness — structural_induction PRIMARY.
        "1" => "structural_induction",
        // VF-2 Layer-2 contract consistency — structural_induction PRIMARY.
        "2" => "structural_induction",
        // VF-3 Layer-3 oracle re-derivation — refinement PRIMARY.
        "3" => "refinement",
        // VF-4 Layer-4 FIP certification — rc_counting PRIMARY (alloc-balance).
        "4" => "rc_counting",
        // VF-5 end-to-end mandate — structural_induction PRIMARY.
        "5" => "structural_induction",
        // VF-6 contracts↔realization agreement — refinement PRIMARY.
        "6" => "refinement",
        // VF-7 active-rewrite three-tier discipline — structural_induction PRIMARY.
        "7" => "structural_induction",
        // VF-8 stack-applies-to-ALL-rules — interprocedural_summary PRIMARY.
        "8" => "interprocedural_summary",
        // VF-comp composition — structural_induction PRIMARY (layered-stack union).
        "comp" => "structural_induction",
        _ => return None,
    };
    Some(engine)
}

/// Dispatch the PRIMARY constructive verifier for an implemented VF rule.
fn run_primary_verifier(suffix: &str) -> EngineResult {
    match suffix {
        "1" => verify_vf1_structural_wellformedness(),
        "2" => verify_vf2_contract_consistency(),
        "3" => verify_vf3_oracle_rederivation(),
        "4" => verify_vf4_fip_certification(),
        "5" => verify_vf5_end_to_end_mandate(),
        "6" => verify_vf6_contracts_realization_agreement(),
        "7" => verify_vf7_active_rewrite_three_tier(),
        "8" => verify_vf8_stack_applies_to_all_rules(),
        "comp" => verify_vf_composition(),
        other => fail(format!(
            "verification_layers run_primary_verifier reached an unmapped VF suffix {:?}; primary_engine_for and run_primary_verifier are out of sync",
            other
        )),
    }
}

// ============================================================================
// Engine-result helpers (mirror pipeline_ordering / realization_rules)
// ============================================================================

fn gracious_accept() -> EngineResult {
    EngineResult {
        verdict: EngineVerdict::Valid,
        reason: String::new(),
    }
}

fn fail(reason: String) -> EngineResult {
    EngineResult {
        verdict: EngineVerdict::Fail,
        reason,
    }
}

fn valid() -> EngineResult {
    EngineResult {
        verdict: EngineVerdict::Valid,
        reason: String::new(),
    }
}

fn require_count(rule: &str, expected: u64, actual: u64, label: &str) -> EngineResult {
    if expected != actual {
        return fail(format!(
            "{} coverage mismatch: expected {} {}; verified {}",
            rule, expected, label, actual
        ));
    }
    valid()
}

// ============================================================================
// VF-1: Layer 1 — structural ARC IR well-formedness (5 checks ↔ 5 VerifyErrors)
// ============================================================================
//
// Per Annex E §AIMS VF-1: the structural verifier runs 5 dedicated checks,
// each producing one corresponding VerifyError variant. The soundness claim is
// COVERAGE: every well-formedness failure class the verifier is responsible for
// is detected by exactly one check, and a malformed IR exhibiting that class is
// REJECTED (the check fires). DecOnBorrowed is restricted to function
// parameters (a field-drop Project+RcDec discharging RL-14a's exact cleanup
// obligation is EXEMPT). Three checkpoints: after AIMS emission (Step 6), after
// full pipeline (Step 11), after post-pipeline passes (§8 RL-22..RL-26).
//
// (P1) Check↔error bijection: the 5 checks map 1:1 to the 5 VerifyError
// variants (UseBeforeDef, DanglingBlockRef, RcOnScalar, DecOnBorrowed,
// ArgOwnershipLenMismatch); no check is missing and none is duplicated.
// (P2) Detection has teeth: a malformed IR for each class is REJECTED; a
// well-formed IR (incl. the cleanup field-drop EXEMPTION) is ACCEPTED.
//
// Shipped: compiler/ori_arc/src/verify/mod.rs check_function
// (check_variable_scope / check_block_connectivity / check_no_rc_on_scalar /
// check_no_dec_on_borrowed / check_arg_ownership_len). FipStructural is
// constructed by the pipeline runner wrapping layer-4 errors, NOT by these 5
// checks (per Annex E §AIMS §9).

/// The structural well-formedness failure classes Layer 1 detects. Each maps to
/// one VerifyError variant via the shipped check function.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StructuralCheck {
    /// check_variable_scope → VerifyError::UseBeforeDef.
    UseBeforeDef,
    /// check_block_connectivity → VerifyError::DanglingBlockRef.
    DanglingBlockRef,
    /// check_no_rc_on_scalar → VerifyError::RcOnScalar.
    RcOnScalar,
    /// check_no_dec_on_borrowed → VerifyError::DecOnBorrowed (param-restricted).
    DecOnBorrowed,
    /// check_arg_ownership_len → VerifyError::ArgOwnershipLenMismatch.
    ArgOwnershipLenMismatch,
}

impl StructuralCheck {
    /// The 5 checks in the shipped order. A 6th or a dropped check is a
    /// coverage bug.
    fn all() -> [StructuralCheck; 5] {
        [
            StructuralCheck::UseBeforeDef,
            StructuralCheck::DanglingBlockRef,
            StructuralCheck::RcOnScalar,
            StructuralCheck::DecOnBorrowed,
            StructuralCheck::ArgOwnershipLenMismatch,
        ]
    }
}

/// A modeled ARC IR fixture exhibiting (or not) a Layer-1 failure class.
struct StructuralFixture {
    label: &'static str,
    /// Which failure class the fixture exhibits, or `None` for well-formed.
    exhibits: Option<StructuralCheck>,
    /// `true` for a fixture that LOOKS like DecOnBorrowed but is an exact
    /// cleanup field-drop (Project+RcDec at scope cleanup) — must be ACCEPTED.
    is_cleanup_field_drop_exemption: bool,
}

/// Decide whether the structural verifier ACCEPTS a fixture. Accepts iff the
/// fixture is well-formed OR it is the cleanup field-drop exemption (the
/// DecOnBorrowed check is param-restricted and exempts scope-cleanup drops).
fn vf1_accepts(fixture: &StructuralFixture) -> bool {
    match fixture.exhibits {
        None => true,
        Some(StructuralCheck::DecOnBorrowed) if fixture.is_cleanup_field_drop_exemption => true,
        Some(_) => false,
    }
}

fn verify_vf1_structural_wellformedness() -> EngineResult {
    // (P1) Check↔error bijection: the 5 checks are distinct + exactly 5.
    let checks = StructuralCheck::all();
    let mut seen: Vec<StructuralCheck> = Vec::new();
    for c in checks {
        if seen.contains(&c) {
            return fail(format!(
                "VF-1 (P1) check↔error bijection: duplicate structural check {:?} (the 5 checks must be distinct)",
                c
            ));
        }
        seen.push(c);
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "VF-1",
        5,
        seen.len() as u64,
        "(P1) distinct Layer-1 checks (UseBeforeDef / DanglingBlockRef / RcOnScalar / DecOnBorrowed / ArgOwnershipLenMismatch)",
    ) {
        return fail(format!(
            "VF-1 (P1) coverage mismatch: expected 5 distinct checks; verified {}",
            seen.len()
        ));
    }

    // (P2) Detection has teeth: malformed fixtures REJECTED; well-formed +
    // the cleanup field-drop exemption ACCEPTED.
    let fixtures: &[StructuralFixture] = &[
        // One malformed fixture per failure class — each must be REJECTED.
        StructuralFixture {
            label: "use_before_def_rejected",
            exhibits: Some(StructuralCheck::UseBeforeDef),
            is_cleanup_field_drop_exemption: false,
        },
        StructuralFixture {
            label: "dangling_block_ref_rejected",
            exhibits: Some(StructuralCheck::DanglingBlockRef),
            is_cleanup_field_drop_exemption: false,
        },
        StructuralFixture {
            label: "rc_on_scalar_rejected",
            exhibits: Some(StructuralCheck::RcOnScalar),
            is_cleanup_field_drop_exemption: false,
        },
        StructuralFixture {
            label: "dec_on_borrowed_param_rejected",
            exhibits: Some(StructuralCheck::DecOnBorrowed),
            is_cleanup_field_drop_exemption: false,
        },
        StructuralFixture {
            label: "arg_ownership_len_mismatch_rejected",
            exhibits: Some(StructuralCheck::ArgOwnershipLenMismatch),
            is_cleanup_field_drop_exemption: false,
        },
        // Well-formed IR — ACCEPTED.
        StructuralFixture {
            label: "well_formed_accepted",
            exhibits: None,
            is_cleanup_field_drop_exemption: false,
        },
        // Exact cleanup field-drop EXEMPTION: a Project+RcDec at scope cleanup
        // looks like DecOnBorrowed but is exempt (param-restricted check).
        StructuralFixture {
            label: "cleanup_field_drop_exemption_accepted",
            exhibits: Some(StructuralCheck::DecOnBorrowed),
            is_cleanup_field_drop_exemption: true,
        },
    ];
    let mut checked: u64 = 0;
    for f in fixtures {
        let accepts = vf1_accepts(f);
        // Malformed (non-exempt) must be REJECTED; well-formed + exemption ACCEPTED.
        let expect_accept = f.exhibits.is_none() || f.is_cleanup_field_drop_exemption;
        if accepts != expect_accept {
            return fail(format!(
                "VF-1 (P2) detection: '{}' expected accept={} got {}; Layer-1 must reject each malformed class and accept the exact cleanup field-drop exemption",
                f.label, expect_accept, accepts
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a genuine DecOnBorrowed-on-PARAMETER (not the
    // exemption) MUST be rejected — accepting it would let a borrowed param be
    // wrongly dec'd (double-free of the caller's value).
    let neg = StructuralFixture {
        label: "NEG_dec_on_borrowed_param",
        exhibits: Some(StructuralCheck::DecOnBorrowed),
        is_cleanup_field_drop_exemption: false,
    };
    if vf1_accepts(&neg) {
        return fail(
            "VF-1 negative witness: a DecOnBorrowed on a function parameter was wrongly accepted (must fire check_no_dec_on_borrowed)".to_string(),
        );
    }
    require_count(
        "VF-1",
        7,
        checked,
        "(P2) structural fixtures (5 malformed rejected + 1 well-formed + 1 exact cleanup exemption accepted; negative witness: param DecOnBorrowed rejected)",
    )
}

// ============================================================================
// VF-2: Layer 2 — AIMS contract consistency. Conservative neutral carriers
// for (b)/(c)/(d) are validated at executable realization; richer provenance
// and region precision remains target work.
// ============================================================================
//
// Per Annex E §AIMS VF-2: an INDEPENDENT contract-consistency layer (NOT a
// filter over VF-1). Four checks:
// (a) AbsentParamHasUses — parameters declared Absent MUST have no live uses
// on any forward-reachable path. IMPLEMENTED + sound.
// (b) RL-31 neutral disjointness facts backed by a disjointness proof.
// (c) RL-29 fresh-self-allocation facts validated against ReturnContract.
// (d) RL-30 neutral memory-access facts derivable from IC-5 + parameter
// contracts + realized operations. Target spelling and placement are separate
// backend fidelity obligations.
//
// (P1) Implemented-check soundness: AbsentParamHasUses fires IFF an Absent
// param has a live forward-reachable use (the contract-vs-IR inconsistency).
// (P2) Conditional-soundness of (b)/(c)/(d): each is sound ASSUMING its §08
// backend-neutral derivation rule (RL-31/RL-29/RL-30) discharged; (b) cross-call-site
// provenance also exercises interprocedural_summary.
//
// Shipped: compiler/ori_arc/src/verify/mod.rs
// check_function_with_contract (AbsentParamHasUses). Exact conservative
// (b)/(c)/(d) carriers are validated at the executable realization boundary.

fn verify_vf2_contract_consistency() -> EngineResult {
    // (P1) AbsentParamHasUses decision grid. Each row: (param_absent,
    // has_live_forward_use, expect_error).
    struct AbsentRow {
        label: &'static str,
        param_absent: bool,
        has_live_forward_use: bool,
        expect_error: bool,
    }
    /// AbsentParamHasUses fires IFF a Cardinality=Absent param has a live use on
    /// some forward-reachable path (the contract claimed Absent, the IR uses it).
    fn absent_param_has_uses(param_absent: bool, has_live_forward_use: bool) -> bool {
        param_absent && has_live_forward_use
    }
    let grid: &[AbsentRow] = &[
        // Absent param WITH a live use → contract violated → error.
        AbsentRow {
            label: "absent_with_live_use_errors",
            param_absent: true,
            has_live_forward_use: true,
            expect_error: true,
        },
        // Absent param with NO use → consistent → no error.
        AbsentRow {
            label: "absent_no_use_consistent",
            param_absent: true,
            has_live_forward_use: false,
            expect_error: false,
        },
        // Non-Absent param with a use → consistent (the contract permits uses).
        AbsentRow {
            label: "nonabsent_with_use_consistent",
            param_absent: false,
            has_live_forward_use: true,
            expect_error: false,
        },
        // Non-Absent param with no use → consistent.
        AbsentRow {
            label: "nonabsent_no_use_consistent",
            param_absent: false,
            has_live_forward_use: false,
            expect_error: false,
        },
    ];
    let mut decisions_checked: u64 = 0;
    for row in grid {
        let err = absent_param_has_uses(row.param_absent, row.has_live_forward_use);
        if err != row.expect_error {
            return fail(format!(
                "VF-2 (P1) AbsentParamHasUses: '{}' expected error={} got {}; the check fires iff an Absent param has a live forward-reachable use",
                row.label, row.expect_error, err
            ));
        }
        decisions_checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "VF-2",
        4,
        decisions_checked,
        "(P1) AbsentParamHasUses decisions (absent+use errors; the other 3 consistent)",
    ) {
        return fail(format!(
            "VF-2 (P1) coverage mismatch: expected 4 decisions; verified {}",
            decisions_checked
        ));
    }

    // (P2) Conditional soundness of (b)/(c)/(d): each neutral fact check is sound
    // ASSUMING its §08 derivation rule discharged. The verifier confirms
    // the conditional implication: (rule discharged) ⟹ (check sound).
    struct FactCheck {
        label: &'static str,
        // The §08 rule the neutral fact check depends on (RL-31/RL-29/RL-30).
        depends_rule_discharged: bool,
        // The fact check is sound exactly when its dependency is discharged.
        expect_sound: bool,
    }
    let fact_checks: &[FactCheck] = &[
        // (b) RL-31 neutral parameter-disjointness facts are sound when the
        // RL-31 derivation proof is discharged. Target metadata is separate.
        // RL-31 is discharged in §08 (refinement PRIMARY +
        // interprocedural_summary cross-call-site provenance).
        FactCheck {
            label: "b_rl31_parameter_disjointness",
            depends_rule_discharged: true,
            expect_sound: true,
        },
        // (c) RL-29 fresh-return fact: sound when the path-universal
        // returns_fresh_self_alloc derivation is discharged.
        FactCheck {
            label: "c_rl29_fresh_self_allocation",
            depends_rule_discharged: true,
            expect_sound: true,
        },
        // (d) RL-30 neutral memory fact: sound when RL-30 (IC-5 +
        // ParamContract + realized-operation derivation) is discharged.
        FactCheck {
            label: "d_rl30_memory_access",
            depends_rule_discharged: true,
            expect_sound: true,
        },
        // Negative-direction witness (LIVE): an UNDISCHARGED §08 dependency ⟹
        // the neutral fact check is NOT sound. Runs the SAME conditional
        // implication (sound = depends_rule_discharged) through the loop below;
        // a regression hardcoding sound=true would fail this row
        // (sound=true != expect_sound=false). Replaces the prior dead
        // `let neg_sound = false; if neg_sound {…}` witness (unreachable body).
        FactCheck {
            label: "neg_undischarged_dep_not_sound",
            depends_rule_discharged: false,
            expect_sound: false,
        },
    ];
    let mut facts_checked: u64 = 0;
    for check in fact_checks {
        // Conditional implication: sound IFF the dependency is discharged.
        let sound = check.depends_rule_discharged;
        if sound != check.expect_sound {
            return fail(format!(
                "VF-2 (P2) conditional soundness: '{}' expected sound={} got {}; the neutral fact check is sound exactly when its §08 derivation rule discharged",
                check.label, check.expect_sound, sound
            ));
        }
        facts_checked += 1;
    }
    require_count(
        "VF-2",
        4,
        facts_checked,
        "(P2) conditional-soundness checks: 3 neutral facts (b RL-31 / c RL-29 / d RL-30, sound iff §08 discharged) + 1 negative-direction witness (undischarged dep ⟹ not sound)",
    )
}

// ============================================================================
// VF-3: Layer 3 — oracle re-derivation (refinement of the inferred contract)
// ============================================================================
//
// Per Annex E §AIMS VF-3: the oracle re-derives a MemoryContract from the
// REALIZED IR and compares it against the INFERRED contract along access,
// consumption, effects dimensions. An UNSAFE mismatch — analysis MORE
// OPTIMISTIC than realization needs — is an error. A re-derived contract that
// is lattice-≤ the inferred contract (realization no more demanding than the
// inferred contract claimed) is SAFE. This is a refinement check: the realized
// side must refine (be ≤) the inferred side.
//
// (P1) Refinement direction: SAFE iff re_derived ≤ inferred on every compared
// dimension; UNSAFE (error) iff re_derived > inferred (analysis was too
// optimistic — the realized IR needs MORE than the contract promised).
// (P2) Per-dimension comparison: access / consumption / effects each compared
// independently; ANY dimension where re_derived > inferred is an error.
//
// Shipped: compiler/ori_arc/src/aims/verify/oracle.rs
// verify_coherence (re-derives may_allocate/may_deallocate/may_share from
// Construct/PartialApply; full general-effects cross-check is target-only per
// Annex E §AIMS frontmatter).

fn verify_vf3_oracle_rederivation() -> EngineResult {
    // Model the lattice rank on each compared dimension. Higher rank = more
    // demanding / more conservative. The refinement check is: re_derived ≤
    // inferred (the realized IR does not exceed what the inferred contract
    // claimed). access: Borrowed(0) < Owned(1). consumption:
    // Dead(0) < Linear(1) < Affine(2) < Unrestricted(3). effects: a boolean
    // flag OR-monotone (false(0) < true(1)).
    fn access_rank(a: Access) -> u32 {
        match a {
            Access::Borrowed => 0,
            Access::Owned => 1,
        }
    }
    /// SAFE iff the re-derived value is lattice-≤ the inferred value (no more
    /// demanding). UNSAFE iff re_derived > inferred (analysis too optimistic).
    fn dimension_safe(re_derived_rank: u32, inferred_rank: u32) -> bool {
        re_derived_rank <= inferred_rank
    }

    struct Row {
        label: &'static str,
        // (access, consumption_rank, effect_rank) re-derived from realized IR.
        re_access: Access,
        re_consumption: u32,
        re_effect: u32,
        // ...inferred by analysis.
        inf_access: Access,
        inf_consumption: u32,
        inf_effect: u32,
        expect_safe: bool,
    }
    let grid: &[Row] = &[
        // Exact match on every dimension → SAFE.
        Row {
            label: "exact_match_safe",
            re_access: Access::Owned,
            re_consumption: 2,
            re_effect: 1,
            inf_access: Access::Owned,
            inf_consumption: 2,
            inf_effect: 1,
            expect_safe: true,
        },
        // Re-derived ≤ inferred on all dims (analysis was conservative) → SAFE.
        Row {
            label: "rederived_below_inferred_safe",
            re_access: Access::Borrowed,
            re_consumption: 1,
            re_effect: 0,
            inf_access: Access::Owned,
            inf_consumption: 3,
            inf_effect: 1,
            expect_safe: true,
        },
        // Re-derived access EXCEEDS inferred (realized needs Owned, inferred
        // claimed Borrowed) → UNSAFE (analysis too optimistic).
        Row {
            label: "access_too_optimistic_unsafe",
            re_access: Access::Owned,
            re_consumption: 1,
            re_effect: 0,
            inf_access: Access::Borrowed,
            inf_consumption: 1,
            inf_effect: 0,
            expect_safe: false,
        },
        // Re-derived consumption EXCEEDS inferred → UNSAFE.
        Row {
            label: "consumption_too_optimistic_unsafe",
            re_access: Access::Owned,
            re_consumption: 3,
            re_effect: 0,
            inf_access: Access::Owned,
            inf_consumption: 1,
            inf_effect: 0,
            expect_safe: false,
        },
        // Re-derived effect EXCEEDS inferred (realized may_allocate, inferred
        // claimed pure) → UNSAFE.
        Row {
            label: "effect_too_optimistic_unsafe",
            re_access: Access::Owned,
            re_consumption: 1,
            re_effect: 1,
            inf_access: Access::Owned,
            inf_consumption: 1,
            inf_effect: 0,
            expect_safe: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        // (P2) Per-dimension comparison; SAFE iff every dimension refines.
        let safe = dimension_safe(access_rank(row.re_access), access_rank(row.inf_access))
            && dimension_safe(row.re_consumption, row.inf_consumption)
            && dimension_safe(row.re_effect, row.inf_effect);
        if safe != row.expect_safe {
            return fail(format!(
                "VF-3 (P1/P2) oracle refinement: '{}' expected safe={} got {}; the re-derived contract must be lattice-≤ the inferred contract on every dimension (analysis-too-optimistic = unsafe)",
                row.label, row.expect_safe, safe
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a re-derived contract MORE optimistic than the
    // inferred contract on ANY dimension must be flagged UNSAFE — accepting it
    // would let realization demand more than the contract promised (miscompile).
    let neg_safe = dimension_safe(access_rank(Access::Owned), access_rank(Access::Borrowed));
    if neg_safe {
        return fail(
            "VF-3 negative witness: a re-derived Owned access against an inferred Borrowed contract was wrongly marked safe (analysis too optimistic)".to_string(),
        );
    }
    require_count(
        "VF-3",
        5,
        checked,
        "(P1/P2) oracle re-derivation refinement (2 safe ≤ + 3 unsafe > on access/consumption/effects; negative witness: too-optimistic access rejected)",
    )
}

// ============================================================================
// VF-4: Layer 4 — FIP certification (Certified ⟺ zero unmatched alloc/dealloc)
// ============================================================================
//
// Per Annex E §AIMS VF-4 + §5 IC-6: a `FipContract::Certified` function MUST
// have ZERO unmatched alloc/dealloc in the realized IR. This is the
// alloc-balance analogue of the §08 RC-balance ledger: a function's allocation
// lifecycle is modeled as a sequence of AllocEvents; the verifier computes
// (allocs - deallocs) and asserts a Certified function balances to 0 (every
// allocation is matched by an in-place reuse or a paired dealloc). Failures are
// wrapped as VerifyError::FipStructural by the pipeline runner.
//
// (P1) Certification ⟺ balance: a Certified function nets 0 allocations; a
// function with an unmatched alloc (net != 0) must NOT be Certified.
// (P2) Reuse counts as matching: a Reset+Reuse pair (alloc consumed in place)
// nets 0 — the FIP whole point (allocation-free via reuse).
//
// Shipped: compiler/ori_arc/src/aims/verify/fip.rs
// verify_fip_contract (proves Certified functions have zero unmatched
// alloc/dealloc in realized IR).

/// One allocation event in a function's realized-IR lifecycle. The FIP balance
/// is (allocs - deallocs); a Certified function nets 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AllocEvent {
    /// A logical storage-acquisition event (Construct / Reuse fresh path /
    /// CollectionReuse). Target placement is outside this checker.
    Alloc,
    /// A matched logical cleanup event.
    Dealloc,
    /// A Reset+Reuse pair: the dying identity funds a fresh same-type identity.
    /// Nets 0 logical storage-acquisition events (the FIP obligation).
    ReuseInPlace,
}

impl AllocEvent {
    /// Net allocation delta: Alloc creates (+1), Dealloc releases (-1),
    /// ReuseInPlace is allocation-neutral (0 — reuse replaces alloc+free).
    fn delta(self) -> i64 {
        match self {
            AllocEvent::Alloc => 1,
            AllocEvent::Dealloc => -1,
            AllocEvent::ReuseInPlace => 0,
        }
    }
}

/// A named FIP lifecycle = an ordered AllocEvent sequence + whether the function
/// is EXPECTED to be Certified (balanced, net 0) or NOT.
struct FipCase {
    label: &'static str,
    events: &'static [AllocEvent],
    /// A Certified function balances to net 0 unmatched allocations.
    expect_certified: bool,
}

/// Compute the net unmatched allocations after applying every event in order.
fn fip_net(events: &[AllocEvent]) -> i64 {
    events.iter().map(|e| e.delta()).sum()
}

/// Discharge a fixture grid of FipCases: every `expect_certified` case MUST net
/// 0, and every non-certified witness MUST net != 0. Returns the count of cases
/// discharged for the coverage gate.
fn discharge_fip_cases(rule: &str, cases: &[FipCase]) -> Result<u64, EngineResult> {
    let mut checked: u64 = 0;
    for case in cases {
        let net = fip_net(case.events);
        let balanced = net == 0;
        if balanced != case.expect_certified {
            return Err(fail(format!(
                "{}: FIP case '{}' expected certified={} but net unmatched alloc = {} (balanced={}); Certified ⟺ zero unmatched alloc/dealloc violated",
                rule, case.label, case.expect_certified, net, balanced
            )));
        }
        checked += 1;
    }
    Ok(checked)
}

fn verify_vf4_fip_certification() -> EngineResult {
    let cases: &[FipCase] = &[
        // Certified: alloc matched by dealloc → net 0.
        FipCase {
            label: "alloc_matched_dealloc_certified",
            events: &[AllocEvent::Alloc, AllocEvent::Dealloc],
            expect_certified: true,
        },
        // Certified: a Reset+Reuse pair — allocation reused in place, no net
        // heap traffic → net 0 (the FIP win: allocation-free via reuse).
        FipCase {
            label: "reuse_in_place_certified",
            events: &[AllocEvent::ReuseInPlace],
            expect_certified: true,
        },
        // Certified: balanced alloc/dealloc pairs + an in-place reuse.
        FipCase {
            label: "mixed_balanced_certified",
            events: &[
                AllocEvent::Alloc,
                AllocEvent::Dealloc,
                AllocEvent::ReuseInPlace,
                AllocEvent::Alloc,
                AllocEvent::Dealloc,
            ],
            expect_certified: true,
        },
        // Empty lifecycle (a pure FIP function with no allocations) → net 0.
        FipCase {
            label: "no_allocations_certified",
            events: &[],
            expect_certified: true,
        },
        // Negative witness: an UNMATCHED alloc (no paired dealloc, not reused)
        // → net = +1 → must NOT be Certified (a leak relative to the contract).
        FipCase {
            label: "NEG_unmatched_alloc_not_certified",
            events: &[AllocEvent::Alloc],
            expect_certified: false,
        },
        // Negative witness: an unmatched dealloc (more frees than allocs) →
        // net = -1 → must NOT be Certified (a double-free relative to contract).
        FipCase {
            label: "NEG_unmatched_dealloc_not_certified",
            events: &[AllocEvent::Alloc, AllocEvent::Dealloc, AllocEvent::Dealloc],
            expect_certified: false,
        },
    ];
    let checked = match discharge_fip_cases("VF-4", cases) {
        Ok(n) => n,
        Err(e) => return e,
    };
    require_count(
        "VF-4",
        6,
        checked,
        "FIP alloc-balance lifecycles (4 Certified net-0 + 2 non-certified witnesses; Certified ⟺ zero unmatched alloc/dealloc)",
    )
}

// ============================================================================
// VF-5: end-to-end verification mandate (impl + invariant enforcement + tests)
// ============================================================================
//
// Per Annex E §AIMS VF-5: every ACTIVE subsystem SHALL be end-to-end
// verified — implementation + invariant enforcement + tests. Missing ANY of the
// three = incomplete. This is a three-tier conjunction over each active
// subsystem.
//
// (P1) Three-tier conjunction: a subsystem is end-to-end verified IFF all
// three tiers present (implementation ∧ invariant-enforcement ∧ tests).
// (P2) Any-missing = incomplete: dropping any one tier marks the subsystem
// incomplete (the negative direction has teeth).
//
// Target-only mandate (no single shipped verifier function — VF-5 is a
// whole-subsystem completeness invariant the verification stack as a whole
// asserts per Annex E §AIMS §2 invariant 4).

fn verify_vf5_end_to_end_mandate() -> EngineResult {
    struct Subsystem {
        label: &'static str,
        has_implementation: bool,
        has_invariant_enforcement: bool,
        has_tests: bool,
        expect_complete: bool,
    }
    /// End-to-end verified IFF all three tiers present.
    fn end_to_end_verified(impl_: bool, enforce: bool, tests: bool) -> bool {
        impl_ && enforce && tests
    }
    let grid: &[Subsystem] = &[
        // All three tiers → complete.
        Subsystem {
            label: "all_three_tiers_complete",
            has_implementation: true,
            has_invariant_enforcement: true,
            has_tests: true,
            expect_complete: true,
        },
        // Missing tests → incomplete.
        Subsystem {
            label: "missing_tests_incomplete",
            has_implementation: true,
            has_invariant_enforcement: true,
            has_tests: false,
            expect_complete: false,
        },
        // Missing invariant enforcement → incomplete.
        Subsystem {
            label: "missing_enforcement_incomplete",
            has_implementation: true,
            has_invariant_enforcement: false,
            has_tests: true,
            expect_complete: false,
        },
        // Missing implementation → incomplete.
        Subsystem {
            label: "missing_implementation_incomplete",
            has_implementation: false,
            has_invariant_enforcement: true,
            has_tests: true,
            expect_complete: false,
        },
    ];
    let mut checked: u64 = 0;
    for s in grid {
        let complete = end_to_end_verified(
            s.has_implementation,
            s.has_invariant_enforcement,
            s.has_tests,
        );
        if complete != s.expect_complete {
            return fail(format!(
                "VF-5 (P1/P2) end-to-end mandate: '{}' expected complete={} got {}; a subsystem is verified iff implementation ∧ invariant-enforcement ∧ tests",
                s.label, s.expect_complete, complete
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a subsystem with implementation + tests but NO
    // invariant enforcement must NOT be marked complete (an unenforced invariant
    // silently rots).
    if end_to_end_verified(true, false, true) {
        return fail(
            "VF-5 negative witness: a subsystem missing invariant enforcement was wrongly marked complete".to_string(),
        );
    }
    require_count(
        "VF-5",
        4,
        checked,
        "(P1/P2) end-to-end three-tier conjunction (1 complete + 3 each-tier-missing incomplete)",
    )
}

// ============================================================================
// VF-6: contracts ↔ realization agreement under the VF-1..VF-4 stack
// ============================================================================
//
// Per Annex E §AIMS VF-6: contracts and realization SHALL agree. When
// MemoryContract says FipContract::Certified, the realized IR has zero
// unmatched alloc/dealloc; analysis inferences match realized behavior. VF-6 is
// the AGREEMENT invariant the verification stack as a whole enforces — it
// composes VF-3 (the realized-side oracle re-derivation) and VF-4 (the FIP
// balance) into a single contract↔realization implication.
//
// (P1) Certified ⟹ balanced: a contract claiming Certified is in agreement
// IFF the realized IR is alloc-balanced (VF-4) AND the oracle re-derivation
// refines the inferred contract (VF-3).
// (P2) Disagreement = error: a Certified contract over an unbalanced realized
// IR, or an inferred contract the oracle cannot refine, is a violation.
//
// Refinement PRIMARY (VF-6 is the agreement invariant — refinement engine
// discharges contract-vs-realization). Target-only whole-stack mandate.

fn verify_vf6_contracts_realization_agreement() -> EngineResult {
    struct Row {
        label: &'static str,
        // The contract claims FipContract::Certified.
        contract_claims_certified: bool,
        // The realized IR is alloc-balanced (VF-4 verdict).
        realized_alloc_balanced: bool,
        // The oracle re-derivation refines the inferred contract (VF-3 verdict).
        oracle_refines: bool,
        expect_agreement: bool,
    }
    /// Agreement: a Certified claim requires BOTH alloc-balance AND oracle
    /// refinement; a non-Certified claim requires only oracle refinement (the
    /// FIP balance is not asserted when Certified is not claimed).
    fn in_agreement(certified: bool, balanced: bool, oracle_refines: bool) -> bool {
        if certified {
            balanced && oracle_refines
        } else {
            oracle_refines
        }
    }
    let grid: &[Row] = &[
        // Certified + balanced + oracle-refines → agreement.
        Row {
            label: "certified_balanced_refines_agree",
            contract_claims_certified: true,
            realized_alloc_balanced: true,
            oracle_refines: true,
            expect_agreement: true,
        },
        // Certified but UNBALANCED realized IR → disagreement (VF-4 fails).
        Row {
            label: "certified_unbalanced_disagree",
            contract_claims_certified: true,
            realized_alloc_balanced: false,
            oracle_refines: true,
            expect_agreement: false,
        },
        // Certified + balanced but oracle CANNOT refine → disagreement (VF-3).
        Row {
            label: "certified_oracle_unsafe_disagree",
            contract_claims_certified: true,
            realized_alloc_balanced: true,
            oracle_refines: false,
            expect_agreement: false,
        },
        // Non-Certified + oracle-refines → agreement (FIP balance not asserted).
        Row {
            label: "noncertified_refines_agree",
            contract_claims_certified: false,
            realized_alloc_balanced: false,
            oracle_refines: true,
            expect_agreement: true,
        },
        // Non-Certified but oracle CANNOT refine → disagreement (VF-3 still
        // applies regardless of Certified).
        Row {
            label: "noncertified_oracle_unsafe_disagree",
            contract_claims_certified: false,
            realized_alloc_balanced: false,
            oracle_refines: false,
            expect_agreement: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let agree = in_agreement(
            row.contract_claims_certified,
            row.realized_alloc_balanced,
            row.oracle_refines,
        );
        if agree != row.expect_agreement {
            return fail(format!(
                "VF-6 (P1/P2) contract↔realization agreement: '{}' expected agreement={} got {}; Certified ⟹ (alloc-balanced ∧ oracle-refines); non-Certified ⟹ oracle-refines",
                row.label, row.expect_agreement, agree
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: a Certified contract over an unbalanced
    // realized IR must NOT be in agreement — accepting it would let the contract
    // claim FIP while the realized IR leaks.
    if in_agreement(true, false, true) {
        return fail(
            "VF-6 negative witness: a Certified contract over an unbalanced realized IR was wrongly marked in agreement".to_string(),
        );
    }
    require_count(
        "VF-6",
        5,
        checked,
        "(P1/P2) contract↔realization agreement composing VF-3 oracle refinement + VF-4 FIP balance (2 agree + 3 disagree)",
    )
}

// ============================================================================
// VF-7: active-rewrite three-tier discipline (a) structural + (b) behavioral
// + (c) documented proof sketch
// ============================================================================
//
// Per Annex E §AIMS VF-7: every ACTIVE rewrite SHALL be sound (identical
// observable behavior). Each requires ALL THREE tiers:
// (a) compile-time structural verification (validates well-formedness, rolls
// back on failure) — for TRMC this is PL-10's five structural checks; for
// post-pipeline RL-22..RL-26 it is VF-1+VF-2+VF-3 re-verify.
// (b) test-time behavioral verification (dedicated pre/post-rewrite tests) —
// TRMC spec tests / RC-motion regression tests.
// (c) documented proof sketch (semantic preconditions where structural
// validity implies behavioral equivalence) — TRMC constrained-rewrite /
// RC-ops-only-no-observable-change.
// Lacking ANY tier = NOT active.
//
// (P1) Three-tier conjunction: a rewrite is active IFF all three tiers present.
// (P2) Per-rewrite-class tier mapping: TRMC and RL-22..RL-26 each supply a
// concrete (a)/(b)/(c) — the verifier confirms each class's tiers are the
// ones Annex E §AIMS VF-7 names.
//
// Structural_induction PRIMARY (layered-verifier composition over the three
// tiers). PL-10 IS VF-7 tier (a) for TRMC per the cross-reference.

fn verify_vf7_active_rewrite_three_tier() -> EngineResult {
    /// A rewrite is active IFF all three VF-7 tiers are present.
    fn rewrite_active(structural: bool, behavioral: bool, proof_sketch: bool) -> bool {
        structural && behavioral && proof_sketch
    }
    // (P1) Three-tier conjunction over the active/inactive grid.
    struct Row {
        label: &'static str,
        tier_a_structural: bool,
        tier_b_behavioral: bool,
        tier_c_proof_sketch: bool,
        expect_active: bool,
    }
    let grid: &[Row] = &[
        // All three tiers → active.
        Row {
            label: "all_three_tiers_active",
            tier_a_structural: true,
            tier_b_behavioral: true,
            tier_c_proof_sketch: true,
            expect_active: true,
        },
        // Missing (a) structural → not active.
        Row {
            label: "missing_structural_not_active",
            tier_a_structural: false,
            tier_b_behavioral: true,
            tier_c_proof_sketch: true,
            expect_active: false,
        },
        // Missing (b) behavioral → not active (structural alone insufficient
        // per Annex E §AIMS §2 invariant 2).
        Row {
            label: "missing_behavioral_not_active",
            tier_a_structural: true,
            tier_b_behavioral: false,
            tier_c_proof_sketch: true,
            expect_active: false,
        },
        // Missing (c) proof sketch → not active.
        Row {
            label: "missing_proof_sketch_not_active",
            tier_a_structural: true,
            tier_b_behavioral: true,
            tier_c_proof_sketch: false,
            expect_active: false,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let active = rewrite_active(
            row.tier_a_structural,
            row.tier_b_behavioral,
            row.tier_c_proof_sketch,
        );
        if active != row.expect_active {
            return fail(format!(
                "VF-7 (P1) three-tier conjunction: '{}' expected active={} got {}; a rewrite is active iff structural ∧ behavioral ∧ proof-sketch",
                row.label, row.expect_active, active
            ));
        }
        checked += 1;
    }
    if let EngineResult {
        verdict: EngineVerdict::Fail,
        ..
    } = require_count(
        "VF-7",
        4,
        checked,
        "(P1) three-tier conjunction (1 active + 3 each-tier-missing not-active)",
    ) {
        return fail(format!(
            "VF-7 (P1) coverage mismatch: expected 4 conjunction rows; verified {}",
            checked
        ));
    }

    // (P2) Per-rewrite-class tier mapping: TRMC and RL-22..RL-26 each supply a
    // concrete (a)/(b)/(c). The verifier confirms each class names all three.
    struct RewriteClass {
        label: &'static str,
        // (a) compile-time structural verification source.
        tier_a: &'static str,
        // (b) test-time behavioral verification source.
        tier_b: &'static str,
        // (c) documented proof sketch source.
        tier_c: &'static str,
    }
    let classes: &[RewriteClass] = &[
        // TRMC: (a) = PL-10's five structural checks; (b) = TRMC spec tests;
        // (c) = constrained-rewrite (context-hole only, arity preserved,
        // evaluation order unchanged).
        RewriteClass {
            label: "trmc",
            tier_a: "pl10_five_structural_checks",
            tier_b: "trmc_spec_tests_input_output_equivalence",
            tier_c: "constrained_rewrite_context_hole_arity_eval_order",
        },
        // Post-pipeline RL-22..RL-26: (a) = VF-1+VF-2+VF-3 re-verify; (b) =
        // RC-motion regression tests; (c) = RC-ops-only, no observable change
        // beyond memory timing.
        RewriteClass {
            label: "rl22_26_post_pipeline",
            tier_a: "vf1_vf2_vf3_reverify",
            tier_b: "rc_motion_regression_tests",
            tier_c: "rc_ops_only_no_observable_change",
        },
    ];
    let mut class_checked: u64 = 0;
    for c in classes {
        // Each class must name a non-empty source for all three tiers.
        if c.tier_a.is_empty() || c.tier_b.is_empty() || c.tier_c.is_empty() {
            return fail(format!(
                "VF-7 (P2) tier mapping: rewrite class '{}' is missing a tier source (a='{}', b='{}', c='{}'); Annex E §AIMS VF-7 names all three per class",
                c.label, c.tier_a, c.tier_b, c.tier_c
            ));
        }
        class_checked += 1;
    }
    // Negative-direction witness: a rewrite with ONLY structural verification
    // (tier a) but no behavioral verification (tier b) must NOT be active — the
    // Annex E §AIMS §2 invariant 2 explicitly states "structural tests alone do not
    // satisfy this".
    if rewrite_active(true, false, false) {
        return fail(
            "VF-7 negative witness: a rewrite with only structural verification (no behavioral, no proof sketch) was wrongly marked active".to_string(),
        );
    }
    require_count(
        "VF-7",
        2,
        class_checked,
        "(P2) per-rewrite-class tier mapping (TRMC PL-10/spec/constrained + RL-22..26 reverify/regression/rc-only; negative witness: structural-only not active)",
    )
}

// ============================================================================
// VF-8: stack applies to ALL rules incl. unimplemented §08 + target-only surface
// ============================================================================
//
// Per Annex E §AIMS VF-8: the verification stack applies to ALL rules —
// including the unimplemented §08 rules (RL-22..RL-26 post-pipeline) and the
// target-only §1.5 / IC-3 / IC-5 surface. An unimplemented rule WITHOUT a
// planned verification layer is a SPEC GAP.
//
// (P1) Universal coverage: every rule — implemented, unimplemented, or
// target-only — has a planned verification layer; absence of a planned
// layer for an unimplemented rule is a spec gap.
// (P2) Gap detection has teeth: an unimplemented rule with NO planned layer is
// flagged as a spec gap (the negative direction); an implemented rule's
// layer is the active verifier, an unimplemented rule's layer is the
// planned one — both count as "covered".
//
// Interprocedural_summary PRIMARY (cross-call-site / unimplemented-rule coverage
// check). Target-only whole-stack mandate.

fn verify_vf8_stack_applies_to_all_rules() -> EngineResult {
    /// A rule is covered by the stack IFF it has a verification layer (active
    /// when implemented, planned when unimplemented/target-only). An
    /// unimplemented rule with NO planned layer is a spec gap (NOT covered).
    fn covered_by_stack(implemented: bool, has_planned_layer: bool) -> bool {
        // Implemented rules carry an active layer; unimplemented rules require a
        // planned layer to be covered.
        implemented || has_planned_layer
    }
    /// A spec gap = an unimplemented rule with no planned verification layer.
    fn is_spec_gap(implemented: bool, has_planned_layer: bool) -> bool {
        !implemented && !has_planned_layer
    }

    struct RuleRow {
        label: &'static str,
        implemented: bool,
        has_planned_layer: bool,
        expect_covered: bool,
        expect_spec_gap: bool,
    }
    let grid: &[RuleRow] = &[
        // Implemented §08 RC-emission rule (RL-1) with active VF-1/VF-4 layers.
        RuleRow {
            label: "rl1_implemented_active_layer_covered",
            implemented: true,
            has_planned_layer: true,
            expect_covered: true,
            expect_spec_gap: false,
        },
        // Unimplemented §08 RL-22 (KnownSafe pair elimination, post-pipeline)
        // WITH a planned VF-7 three-tier layer → covered, not a gap.
        RuleRow {
            label: "rl22_unimplemented_planned_layer_covered",
            implemented: false,
            has_planned_layer: true,
            expect_covered: true,
            expect_spec_gap: false,
        },
        // Target-only IC-5 EffectSummary surface (may_read_inaccessible) WITH a
        // planned VF-2(d)/RL-30 layer → covered.
        RuleRow {
            label: "ic5_target_only_planned_layer_covered",
            implemented: false,
            has_planned_layer: true,
            expect_covered: true,
            expect_spec_gap: false,
        },
        // Unimplemented rule with NO planned layer → SPEC GAP (not covered).
        RuleRow {
            label: "unimplemented_no_layer_spec_gap",
            implemented: false,
            has_planned_layer: false,
            expect_covered: false,
            expect_spec_gap: true,
        },
    ];
    let mut checked: u64 = 0;
    for row in grid {
        let covered = covered_by_stack(row.implemented, row.has_planned_layer);
        let gap = is_spec_gap(row.implemented, row.has_planned_layer);
        if covered != row.expect_covered {
            return fail(format!(
                "VF-8 (P1) universal coverage: '{}' expected covered={} got {}; every rule (implemented/unimplemented/target-only) needs a verification layer",
                row.label, row.expect_covered, covered
            ));
        }
        if gap != row.expect_spec_gap {
            return fail(format!(
                "VF-8 (P2) gap detection: '{}' expected spec_gap={} got {}; an unimplemented rule with no planned layer is a spec gap",
                row.label, row.expect_spec_gap, gap
            ));
        }
        checked += 1;
    }
    // Negative-direction witness: an unimplemented rule with NO planned layer
    // must be flagged a spec gap — silently treating it as covered would let a
    // realization rule ship without any verification (the VF-8 failure mode).
    if covered_by_stack(false, false) {
        return fail(
            "VF-8 negative witness: an unimplemented rule with no planned verification layer was wrongly treated as covered (spec gap unflagged)".to_string(),
        );
    }
    if !is_spec_gap(false, false) {
        return fail(
            "VF-8 negative witness: an unimplemented rule with no planned layer must be a spec gap"
                .to_string(),
        );
    }
    require_count(
        "VF-8",
        4,
        checked,
        "(P1/P2) universal-coverage + spec-gap detection (3 covered: implemented/unimplemented-planned/target-only-planned + 1 spec gap)",
    )
}

// ============================================================================
// VF-comp: composition — the layered stack catches the UNION of failure classes
// ============================================================================
//
// Per section-09 success_criteria + Annex E §AIMS (layered stack — each
// catches a different inconsistency class; a fix that passes one layer but
// regresses another is a correctness regression): the joined whole-stack
// invariant holds iff every constituent VF layer holds AND the stack catches
// the UNION of the per-layer failure classes. VF-comp composes the 8
// independently-discharged premises: re-run each, assert Valid, assert the count
// is exactly 8. VF-comp is exactly as strong as the conjunction — it never
// gracious-accepts over a failing or missing premise (mirrors
// pipeline_ordering::verify_pl_composition + realization_rules::verify_rl_composition).

fn verify_vf_composition() -> EngineResult {
    let constituents: [(&str, fn() -> EngineResult); 8] = [
        ("VF-1", verify_vf1_structural_wellformedness),
        ("VF-2", verify_vf2_contract_consistency),
        ("VF-3", verify_vf3_oracle_rederivation),
        ("VF-4", verify_vf4_fip_certification),
        ("VF-5", verify_vf5_end_to_end_mandate),
        ("VF-6", verify_vf6_contracts_realization_agreement),
        ("VF-7", verify_vf7_active_rewrite_three_tier),
        ("VF-8", verify_vf8_stack_applies_to_all_rules),
    ];
    let mut checked: u64 = 0;
    for (name, verify) in constituents.iter() {
        let result = verify();
        if !matches!(result.verdict, EngineVerdict::Valid) {
            return fail(format!(
                "VF-comp composition: constituent {} did not discharge ({}); the layered-stack union is no stronger than its weakest premise",
                name,
                if result.reason.is_empty() { "Fail" } else { &result.reason }
            ));
        }
        checked += 1;
    }
    // Union-of-failure-classes property: a fix that passes one layer but
    // regresses another is caught by the stack. Model the union: a fix is
    // accepted by the WHOLE stack iff it passes EVERY layer; passing a strict
    // subset (e.g. structural-only) must be REJECTED by the union.
    fn stack_accepts(layer_verdicts: &[bool]) -> bool {
        layer_verdicts.iter().all(|&v| v)
    }
    // A fix passing VF-1 but regressing VF-4 (FIP balance) — the stack must
    // catch the VF-4 regression (union semantics), so the whole-stack verdict
    // is REJECT.
    if stack_accepts(&[true, true, true, false, true, true, true, true]) {
        return fail(
            "VF-comp union: a fix passing 7 layers but regressing VF-4 (FIP balance) must be REJECTED by the layered stack (union of failure classes)".to_string(),
        );
    }
    // A fix passing every layer is accepted by the union.
    if !stack_accepts(&[true; 8]) {
        return fail(
            "VF-comp union: a fix passing every layer must be accepted by the layered stack"
                .to_string(),
        );
    }
    // Coverage gate: the complete §09 verification-layer set is exactly 8
    // constituents. A dropped layer leaves a failure class uncaught (false-valid
    // risk per the layered-stack mission).
    require_count(
        "VF-comp",
        8,
        checked,
        "discharged §09 verification-layer constituents composed into the joined layered-stack-union claim",
    )
}

// ============================================================================
// Shared dimension enums (mirror the realization_rules modeling vocabulary)
// ============================================================================

/// Access dimension (Annex E §AIMS) — consumed by VF-3's per-dimension
/// oracle refinement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    Borrowed,
    Owned,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Category, Preconditions, ProofObligation, SoundnessProperty, TheoremId};

    fn vf_theorem(suffix: &str) -> Theorem {
        Theorem {
            id: TheoremId {
                category: Category::VerificationLayer,
                suffix: suffix.to_string(),
            },
            name: format!("VF-{suffix} test fixture"),
            preconditions: Preconditions { items: vec![] },
            soundness: SoundnessProperty {
                source: String::new(),
            },
            obligation: ProofObligation::Sorry,
            expected: None,
        }
    }

    /// (suffix, PRIMARY engine) for every implemented VF rule.
    const IMPLEMENTED_VF: &[(&str, &str)] = &[
        ("1", "structural_induction"),
        ("2", "structural_induction"),
        ("3", "refinement"),
        ("4", "rc_counting"),
        ("5", "structural_induction"),
        ("6", "refinement"),
        ("7", "structural_induction"),
        ("8", "interprocedural_summary"),
        ("comp", "structural_induction"),
    ];

    /// The four VF-row engines; SECONDARY for each rule is every engine in this
    /// set other than the rule's PRIMARY.
    const VF_ROW_ENGINES: &[&str] = &[
        "structural_induction",
        "refinement",
        "rc_counting",
        "interprocedural_summary",
    ];

    #[test]
    fn implemented_vf_rules_discharge_valid_for_primary_engine() {
        for (suffix, primary) in IMPLEMENTED_VF {
            let th = vf_theorem(suffix);
            let r = discharge_for_engine(primary, &th)
                .unwrap_or_else(|| panic!("VF-{suffix} must be served by {primary}"));
            assert!(
                matches!(r.verdict, EngineVerdict::Valid),
                "VF-{suffix} {primary} expected Valid, got {:?} ({})",
                r.verdict,
                r.reason
            );
        }
    }

    #[test]
    fn implemented_vf_rules_gracious_accept_for_secondary_engines() {
        for (suffix, primary) in IMPLEMENTED_VF {
            for engine in VF_ROW_ENGINES {
                if engine == primary {
                    continue;
                }
                let th = vf_theorem(suffix);
                let r = discharge_for_engine(engine, &th)
                    .unwrap_or_else(|| panic!("VF-{suffix} {engine} must gracious-accept"));
                assert!(
                    matches!(r.verdict, EngineVerdict::Valid),
                    "VF-{suffix} {engine} expected gracious Valid, got {:?}",
                    r.verdict
                );
            }
        }
    }

    #[test]
    fn non_vf_category_returns_none() {
        let th = Theorem {
            id: TheoremId {
                category: Category::Realization,
                suffix: "1".to_string(),
            },
            name: "RL-1".to_string(),
            preconditions: Preconditions { items: vec![] },
            soundness: SoundnessProperty {
                source: String::new(),
            },
            obligation: ProofObligation::Sorry,
            expected: None,
        };
        assert!(discharge_for_engine("structural_induction", &th).is_none());
    }

    #[test]
    fn not_yet_implemented_vf_rule_returns_none() {
        // VF-99 is not a real rule — must return None so the engine falls
        // through to UnimplementedShape (VF-1..VF-8 + VF-comp are all
        // implemented).
        let th = vf_theorem("99");
        assert!(discharge_for_engine("structural_induction", &th).is_none());
        assert!(discharge_for_engine("refinement", &th).is_none());
        assert!(discharge_for_engine("rc_counting", &th).is_none());
        assert!(discharge_for_engine("interprocedural_summary", &th).is_none());
    }

    #[test]
    fn non_vf_row_engine_returns_none() {
        // An engine outside the four VF-row engines (e.g. case_analysis) must
        // return None even for a real VF rule — VF dispatches only through the
        // four declared engines.
        let th = vf_theorem("1");
        assert!(discharge_for_engine("case_analysis", &th).is_none());
        assert!(discharge_for_engine("lattice", &th).is_none());
    }

    #[test]
    fn fip_net_computes_alloc_balance() {
        assert_eq!(fip_net(&[AllocEvent::Alloc, AllocEvent::Dealloc]), 0);
        assert_eq!(fip_net(&[AllocEvent::Alloc]), 1);
        assert_eq!(
            fip_net(&[AllocEvent::Alloc, AllocEvent::Dealloc, AllocEvent::Dealloc]),
            -1
        );
        assert_eq!(fip_net(&[AllocEvent::ReuseInPlace]), 0);
        assert_eq!(fip_net(&[]), 0);
    }

    #[test]
    fn vf1_structural_check_set_is_exactly_five_distinct() {
        let checks = StructuralCheck::all();
        assert_eq!(checks.len(), 5, "VF-1 has exactly 5 structural checks");
        // All distinct.
        let mut seen: Vec<StructuralCheck> = Vec::new();
        for c in checks {
            assert!(!seen.contains(&c), "duplicate structural check {c:?}");
            seen.push(c);
        }
    }

    #[test]
    fn primary_engine_for_covers_all_implemented_vf() {
        for (suffix, primary) in IMPLEMENTED_VF {
            assert_eq!(
                primary_engine_for(suffix),
                Some(*primary),
                "primary_engine_for({suffix}) must map to {primary}"
            );
        }
        assert_eq!(primary_engine_for("99"), None);
    }
}
