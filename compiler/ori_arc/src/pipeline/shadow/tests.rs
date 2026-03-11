//! Tests for shadow comparison types and logic.

use crate::pipeline::rc_count::RcOpCount;
use crate::uniqueness::{CowAnnotations, CowMode};

use super::{
    compare_cow_annotations, compare_rc_ops, compare_return_uniqueness, AimsSnapshot,
    DimensionResult,
};

#[test]
fn cow_annotations_match_when_equal() {
    let mut aims = CowAnnotations::new();
    aims.set(0, 1, CowMode::StaticUnique);
    aims.set(1, 0, CowMode::Dynamic);

    let mut legacy = CowAnnotations::new();
    legacy.set(0, 1, CowMode::StaticUnique);
    legacy.set(1, 0, CowMode::Dynamic);

    assert!(matches!(
        compare_cow_annotations(&aims, &legacy),
        DimensionResult::Match
    ));
}

#[test]
fn cow_annotations_improvement_when_aims_has_more_static_unique() {
    let mut aims = CowAnnotations::new();
    aims.set(0, 0, CowMode::StaticUnique);
    aims.set(0, 1, CowMode::StaticUnique);

    let mut legacy = CowAnnotations::new();
    legacy.set(0, 0, CowMode::StaticUnique);
    legacy.set(0, 1, CowMode::Dynamic);

    assert!(matches!(
        compare_cow_annotations(&aims, &legacy),
        DimensionResult::Improvement(_)
    ));
}

#[test]
fn cow_annotations_regression_when_aims_has_fewer_static_unique() {
    let mut aims = CowAnnotations::new();
    aims.set(0, 0, CowMode::Dynamic);

    let mut legacy = CowAnnotations::new();
    legacy.set(0, 0, CowMode::StaticUnique);

    assert!(matches!(
        compare_cow_annotations(&aims, &legacy),
        DimensionResult::Regression(_)
    ));
}

#[test]
fn return_uniqueness_match_when_both_unique() {
    let snapshot = AimsSnapshot {
        contract: Some(crate::aims::contract::MemoryContract::conservative(0)),
        cow_annotations: CowAnnotations::new(),
        rc_ops: RcOpCount::default(),
    };

    let old_summary = crate::uniqueness::UniquenessSummary {
        params: Vec::new(),
        return_val: crate::uniqueness::Uniqueness::MaybeShared,
        preserves_freshness: false,
    };

    // Conservative contract has MaybeShared return — should match MaybeShared legacy
    let result = compare_return_uniqueness(&snapshot, Some(&old_summary));
    assert!(matches!(result, DimensionResult::Match));
}

#[test]
fn return_uniqueness_improvement_when_aims_unique_legacy_maybe_shared() {
    use crate::aims::contract::{MemoryContract, ReturnContract};
    use crate::aims::lattice::Uniqueness as AimsUniqueness;

    let mut contract = MemoryContract::conservative(0);
    contract.return_info = ReturnContract {
        uniqueness: AimsUniqueness::Unique,
        ..ReturnContract::CONSERVATIVE
    };

    let snapshot = AimsSnapshot {
        contract: Some(contract),
        cow_annotations: CowAnnotations::new(),
        rc_ops: RcOpCount::default(),
    };

    let old_summary = crate::uniqueness::UniquenessSummary {
        params: Vec::new(),
        return_val: crate::uniqueness::Uniqueness::MaybeShared,
        preserves_freshness: false,
    };

    let result = compare_return_uniqueness(&snapshot, Some(&old_summary));
    assert!(matches!(result, DimensionResult::Improvement(_)));
}

#[test]
fn return_uniqueness_regression_when_aims_shared_legacy_unique() {
    use crate::aims::contract::{MemoryContract, ReturnContract};
    use crate::aims::lattice::Uniqueness as AimsUniqueness;

    let mut contract = MemoryContract::conservative(0);
    contract.return_info = ReturnContract {
        uniqueness: AimsUniqueness::Shared,
        ..ReturnContract::CONSERVATIVE
    };

    let snapshot = AimsSnapshot {
        contract: Some(contract),
        cow_annotations: CowAnnotations::new(),
        rc_ops: RcOpCount::default(),
    };

    let old_summary = crate::uniqueness::UniquenessSummary {
        params: Vec::new(),
        return_val: crate::uniqueness::Uniqueness::Unique,
        preserves_freshness: false,
    };

    let result = compare_return_uniqueness(&snapshot, Some(&old_summary));
    assert!(matches!(result, DimensionResult::Regression(_)));
}

// RC operation comparison tests

#[test]
fn rc_ops_match_when_equal() {
    let aims = RcOpCount { inc: 5, dec: 3 };
    let legacy = RcOpCount { inc: 5, dec: 3 };
    assert!(matches!(
        compare_rc_ops(aims, legacy),
        DimensionResult::Match
    ));
}

#[test]
fn rc_ops_improvement_when_aims_fewer() {
    let aims = RcOpCount { inc: 3, dec: 2 };
    let legacy = RcOpCount { inc: 5, dec: 3 };
    let result = compare_rc_ops(aims, legacy);
    assert!(matches!(result, DimensionResult::Improvement(_)));
    if let DimensionResult::Improvement(detail) = result {
        assert!(detail.contains("saved 3"));
    }
}

#[test]
fn rc_ops_regression_when_aims_more() {
    let aims = RcOpCount { inc: 6, dec: 4 };
    let legacy = RcOpCount { inc: 3, dec: 2 };
    let result = compare_rc_ops(aims, legacy);
    assert!(matches!(result, DimensionResult::Regression(_)));
    if let DimensionResult::Regression(detail) = result {
        assert!(detail.contains("excess 5"));
    }
}

#[test]
fn rc_ops_match_when_both_zero() {
    let aims = RcOpCount::default();
    let legacy = RcOpCount::default();
    assert!(matches!(
        compare_rc_ops(aims, legacy),
        DimensionResult::Match
    ));
}

#[test]
fn rc_ops_improvement_different_distribution_same_total() {
    // Same total (8) but different inc/dec distribution — should be Match
    let aims = RcOpCount { inc: 6, dec: 2 };
    let legacy = RcOpCount { inc: 3, dec: 5 };
    assert!(matches!(
        compare_rc_ops(aims, legacy),
        DimensionResult::Match
    ));
}
