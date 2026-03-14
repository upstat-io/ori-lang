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
