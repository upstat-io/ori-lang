//! Tests for FIP contract verification.

use ori_ir::Name;

use super::*;
use crate::aims::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ReturnContract,
};
use crate::aims::realize::FipEvidence;

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

fn make_contract(fip: FipContract, effects: EffectSummary) -> MemoryContract {
    MemoryContract {
        params: vec![],
        return_info: ReturnContract::CONSERVATIVE,
        effects,
        context_behavior: ContextBehavior::default(),
        fip,
        is_fbip: !effects.may_allocate,
    }
}

// Certified verification

#[test]
fn verify_certified_no_allocations_passes() {
    let contract = make_contract(FipContract::Certified, EffectSummary::OPTIMISTIC);
    let evidence = FipEvidence::default();

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert!(errors.is_empty(), "clean Certified should pass: {errors:?}");
}

#[test]
fn verify_certified_with_missed_reuse_fails() {
    let contract = make_contract(FipContract::Certified, EffectSummary::OPTIMISTIC);
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 2,
    };

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0],
        FipVerificationError::CertifiedButHasMissedReuses {
            function: name(1),
            missed_count: 2,
        }
    );
}

#[test]
fn verify_certified_with_balanced_allocations_passes() {
    // FIP Certified allows may_allocate == true when token-balanced
    // (all allocations matched by reuses). FBIP (allocation-free) is
    // tracked separately by MemoryContract::is_fbip.
    let effects = EffectSummary {
        may_allocate: true,
        ..EffectSummary::OPTIMISTIC
    };
    let contract = make_contract(FipContract::Certified, effects);
    let evidence = FipEvidence::default();

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert!(
        errors.is_empty(),
        "token-balanced Certified should pass: {errors:?}"
    );
}

#[test]
fn verify_certified_with_unbounded_stack_fails() {
    let effects = EffectSummary {
        has_unbounded_stack: true,
        ..EffectSummary::OPTIMISTIC
    };
    let contract = make_contract(FipContract::Certified, effects);
    let evidence = FipEvidence::default();

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0],
        FipVerificationError::CertifiedButUnboundedStack { function: name(1) }
    );
}

#[test]
fn verify_certified_multiple_violations() {
    let effects = EffectSummary {
        may_allocate: true,
        has_unbounded_stack: true,
        ..EffectSummary::OPTIMISTIC
    };
    let contract = make_contract(FipContract::Certified, effects);
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 1,
    };

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    // 2 violations: missed reuses + unbounded stack
    // (may_allocate is OK for token-balanced Certified)
    assert_eq!(
        errors.len(),
        2,
        "should report missed reuses + unbounded stack: {errors:?}"
    );
}

// Bounded verification

#[test]
fn verify_bounded_within_limit_passes() {
    let contract = make_contract(
        FipContract::Bounded(2),
        EffectSummary {
            may_allocate: true,
            ..EffectSummary::OPTIMISTIC
        },
    );
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 2,
    };

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert!(errors.is_empty(), "within bound should pass: {errors:?}");
}

#[test]
fn verify_bounded_exceeded_fails() {
    let contract = make_contract(
        FipContract::Bounded(1),
        EffectSummary {
            may_allocate: true,
            ..EffectSummary::OPTIMISTIC
        },
    );
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 3,
    };

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0],
        FipVerificationError::BoundedExceeded {
            function: name(1),
            declared: 1,
            actual: 3,
        }
    );
}

#[test]
fn verify_bounded_exact_limit_passes() {
    let contract = make_contract(
        FipContract::Bounded(5),
        EffectSummary {
            may_allocate: true,
            ..EffectSummary::OPTIMISTIC
        },
    );
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 5,
    };

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert!(errors.is_empty(), "exact bound should pass: {errors:?}");
}

// Conditional and Never

#[test]
fn verify_conditional_params_match() {
    let contract = make_contract(
        FipContract::Conditional {
            requires_unique_params: vec![true, false],
        },
        EffectSummary {
            may_allocate: true,
            ..EffectSummary::OPTIMISTIC
        },
    );
    let evidence = FipEvidence::default();

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert!(
        errors.is_empty(),
        "Conditional passes without evidence checks: {errors:?}"
    );
}

#[test]
fn verify_never_always_passes() {
    let contract = make_contract(FipContract::Never, EffectSummary::CONSERVATIVE);
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 100,
    };

    let errors = verify_fip_contract(name(1), &contract, &evidence);
    assert!(errors.is_empty(), "Never should always pass: {errors:?}");
}

// recompute_fip_for_may_deallocate

#[test]
fn recompute_certified_with_may_deallocate_downgrades_to_never() {
    let mut contract = make_contract(FipContract::Certified, EffectSummary::OPTIMISTIC);
    contract.effects.may_deallocate = true;

    let changed = recompute_fip_for_may_deallocate(&mut contract);
    assert!(changed, "should downgrade Certified with may_deallocate");
    assert_eq!(contract.fip, FipContract::Never);
}

#[test]
fn recompute_bounded_with_may_deallocate_downgrades_to_never() {
    let mut contract = make_contract(
        FipContract::Bounded(3),
        EffectSummary {
            may_allocate: true,
            ..EffectSummary::OPTIMISTIC
        },
    );
    contract.effects.may_deallocate = true;

    let changed = recompute_fip_for_may_deallocate(&mut contract);
    assert!(changed, "should downgrade Bounded with may_deallocate");
    assert_eq!(contract.fip, FipContract::Never);
}

#[test]
fn recompute_conditional_with_may_deallocate_unchanged() {
    let mut contract = make_contract(
        FipContract::Conditional {
            requires_unique_params: vec![true],
        },
        EffectSummary::OPTIMISTIC,
    );
    contract.effects.may_deallocate = true;

    let changed = recompute_fip_for_may_deallocate(&mut contract);
    assert!(!changed, "Conditional should not be downgraded");
    assert!(matches!(contract.fip, FipContract::Conditional { .. }));
}

#[test]
fn recompute_never_with_may_deallocate_unchanged() {
    let mut contract = make_contract(FipContract::Never, EffectSummary::CONSERVATIVE);
    contract.effects.may_deallocate = true;

    let changed = recompute_fip_for_may_deallocate(&mut contract);
    assert!(!changed, "Never should not change");
    assert_eq!(contract.fip, FipContract::Never);
}

#[test]
fn recompute_certified_without_may_deallocate_unchanged() {
    let mut contract = make_contract(FipContract::Certified, EffectSummary::OPTIMISTIC);
    // may_deallocate is false (default)
    assert!(!contract.effects.may_deallocate);

    let changed = recompute_fip_for_may_deallocate(&mut contract);
    assert!(!changed, "should not change when may_deallocate is false");
    assert_eq!(contract.fip, FipContract::Certified);
}

// Second-pass recomputation scenarios (Section 12.1)
//
// These tests validate the functions used by `run_aims_pipeline_all()`'s
// second pass: `verify_fip_contract()` and `recompute_fip_for_may_deallocate()`.
// They simulate the data flow:
// 1. Stale detection: verifier catches Certified + missed_reuses mismatch
//    (step 5a pre-check, before recomputation)
// 2. Recomputation: contract.fip downgraded → re-verification passes cleanly
//    (second pass, after may_deallocate update)

#[test]
fn verify_fip_detects_stale_may_deallocate() {
    // Simulate step 5a: contract still has optimistic may_deallocate=false
    // from extract_contract(), but realization produced missed reuses.
    // The verifier should detect this as a CertifiedButHasMissedReuses error.
    let contract = make_contract(FipContract::Certified, EffectSummary::OPTIMISTIC);
    // may_deallocate is false (optimistic default from interprocedural analysis).
    assert!(!contract.effects.may_deallocate);

    // Realization evidence: 3 missed reuses => function actually deallocates.
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 3,
    };

    // Step 5a runs with the stale contract — it should detect the mismatch
    // between Certified and missed_reuses > 0.
    let errors = verify_fip_contract(name(42), &contract, &evidence);
    assert_eq!(
        errors.len(),
        1,
        "stale Certified with missed reuses should produce exactly 1 error"
    );
    assert_eq!(
        errors[0],
        FipVerificationError::CertifiedButHasMissedReuses {
            function: name(42),
            missed_count: 3,
        }
    );
}

#[test]
fn fip_recomputed_after_may_deallocate_update() {
    // Simulate the full second-pass flow from run_aims_pipeline_all():
    // 1. Start with Certified contract (optimistic may_deallocate=false)
    // 2. Update may_deallocate to true (from FipEvidence.missed_reuses > 0)
    // 3. Recompute contract.fip — should downgrade to Never
    // 4. Re-verify — should pass (contract now reflects reality)

    // Step 1: Contract from extract_contract() with optimistic defaults.
    let mut contract = make_contract(FipContract::Certified, EffectSummary::OPTIMISTIC);
    assert_eq!(contract.fip, FipContract::Certified);
    assert!(!contract.effects.may_deallocate);

    // Step 2: Second pass updates may_deallocate from realization evidence.
    contract.effects.may_deallocate = true;

    // Step 3: Recompute contract.fip — Certified + may_deallocate → Never.
    let downgraded = recompute_fip_for_may_deallocate(&mut contract);
    assert!(
        downgraded,
        "Certified should be downgraded when may_deallocate is true"
    );
    assert_eq!(
        contract.fip,
        FipContract::Never,
        "contract.fip must reflect the downgrade, not remain Certified"
    );

    // Step 4: Re-verify with corrected contract — should pass cleanly.
    // The evidence still shows missed_reuses, but the contract is now Never
    // (which always passes verification).
    let evidence = FipEvidence {
        fip_gates: vec![],
        missed_reuses: 1,
    };
    let errors = verify_fip_contract(name(42), &contract, &evidence);
    assert!(
        errors.is_empty(),
        "post-recompute verification should pass: contract is Never, which has no checks: {errors:?}"
    );
}

// Display

#[test]
fn display_messages_are_readable() {
    let errors = vec![
        FipVerificationError::CertifiedButHasMissedReuses {
            function: name(1),
            missed_count: 3,
        },
        FipVerificationError::CertifiedButUnboundedStack { function: name(2) },
        FipVerificationError::BoundedExceeded {
            function: name(3),
            declared: 1,
            actual: 5,
        },
    ];
    for e in &errors {
        let msg = e.to_string();
        assert!(!msg.is_empty(), "Display should produce non-empty message");
    }
}
