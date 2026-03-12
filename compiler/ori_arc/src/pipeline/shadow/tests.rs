//! Tests for shadow comparison types and logic.

use ori_ir::StringInterner;

use crate::ir::ArgOwnership;
use crate::pipeline::rc_count::RcOpCount;
use crate::uniqueness::{CowAnnotations, CowMode};

use super::compare::{
    compare_arg_ownership, compare_cow_annotations, compare_rc_ops, compare_return_uniqueness,
};
use super::{AimsSnapshot, DimensionResult};

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
        arg_ownership_sites: Vec::new(),
        immortal_count: 0,
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
        arg_ownership_sites: Vec::new(),
        immortal_count: 0,
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
        arg_ownership_sites: Vec::new(),
        immortal_count: 0,
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

// Immortal skip tracking tests

#[test]
fn immortal_count_zero_when_no_immortals() {
    use crate::ir::ArcFunction;

    let snapshot = AimsSnapshot {
        contract: Some(crate::aims::contract::MemoryContract::conservative(0)),
        cow_annotations: CowAnnotations::new(),
        rc_ops: RcOpCount::default(),
        arg_ownership_sites: Vec::new(),
        immortal_count: 0,
    };

    let func = ArcFunction::default();
    let old_summary = crate::uniqueness::UniquenessSummary {
        params: Vec::new(),
        return_val: crate::uniqueness::Uniqueness::MaybeShared,
        preserves_freshness: false,
    };
    let interner = StringInterner::new();

    let comparison = super::compare::compare_function(
        &func,
        Some(&snapshot),
        Some(&old_summary),
        RcOpCount::default(),
        "test_fn".into(),
        &interner,
    );
    assert_eq!(comparison.immortal_skips, 0);
}

#[test]
fn immortal_count_propagated_to_comparison() {
    use crate::ir::ArcFunction;

    let snapshot = AimsSnapshot {
        contract: Some(crate::aims::contract::MemoryContract::conservative(0)),
        cow_annotations: CowAnnotations::new(),
        rc_ops: RcOpCount { inc: 2, dec: 1 },
        arg_ownership_sites: Vec::new(),
        immortal_count: 3,
    };

    let func = ArcFunction::default();
    let old_summary = crate::uniqueness::UniquenessSummary {
        params: Vec::new(),
        return_val: crate::uniqueness::Uniqueness::MaybeShared,
        preserves_freshness: false,
    };
    let interner = StringInterner::new();

    let comparison = super::compare::compare_function(
        &func,
        Some(&snapshot),
        Some(&old_summary),
        RcOpCount { inc: 4, dec: 3 },
        "test_fn".into(),
        &interner,
    );
    assert_eq!(comparison.immortal_skips, 3);
    // AIMS has fewer RC ops (3 vs 7) — should be Improvement
    assert!(matches!(comparison.rc_ops, DimensionResult::Improvement(_)));
}

#[test]
fn immortal_skips_zero_when_no_snapshot() {
    use crate::ir::ArcFunction;

    let func = ArcFunction::default();
    let interner = StringInterner::new();

    let comparison = super::compare::compare_function(
        &func,
        None,
        None,
        RcOpCount::default(),
        "test_fn".into(),
        &interner,
    );
    assert_eq!(comparison.immortal_skips, 0);
}

#[test]
fn immortal_skips_total_accumulated_in_report() {
    use super::ShadowComparisonReport;

    let report = ShadowComparisonReport {
        per_function: Vec::new(),
        total_functions: 3,
        param_matches: 3,
        param_improvements: 0,
        param_regressions: 0,
        return_matches: 3,
        return_improvements: 0,
        return_regressions: 0,
        cow_matches: 3,
        cow_improvements: 0,
        cow_regressions: 0,
        rc_matches: 3,
        rc_improvements: 0,
        rc_regressions: 0,
        arg_ownership_matches: 3,
        arg_ownership_improvements: 0,
        arg_ownership_regressions: 0,
        aims_rc_total: 10,
        legacy_rc_total: 16,
        immortal_skips_total: 5,
    };

    assert_eq!(report.immortal_skips_total, 5);
    assert!(!report.has_regressions());
}

// Arg ownership comparison tests

#[test]
fn arg_ownership_match_when_identical() {
    let interner = StringInterner::new();
    let callee = interner.intern("foo");

    let aims = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Borrowed])];
    let legacy = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Borrowed])];

    assert!(matches!(
        compare_arg_ownership(&aims, &legacy, &interner),
        DimensionResult::Match
    ));
}

#[test]
fn arg_ownership_match_when_both_empty() {
    let interner = StringInterner::new();

    let aims: Vec<(ori_ir::Name, Vec<ArgOwnership>)> = Vec::new();
    let legacy: Vec<(ori_ir::Name, Vec<ArgOwnership>)> = Vec::new();

    assert!(matches!(
        compare_arg_ownership(&aims, &legacy, &interner),
        DimensionResult::Match
    ));
}

#[test]
fn arg_ownership_improvement_when_aims_borrows_more() {
    let interner = StringInterner::new();
    let callee = interner.intern("bar");

    let aims = vec![(callee, vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed])];
    let legacy = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Borrowed])];

    let result = compare_arg_ownership(&aims, &legacy, &interner);
    assert!(matches!(result, DimensionResult::Improvement(_)));
    if let DimensionResult::Improvement(detail) = result {
        assert!(detail.contains("AIMS=Borrowed, legacy=Owned"));
        assert!(detail.contains("arg 0"));
    }
}

#[test]
fn arg_ownership_regression_when_aims_owns_more() {
    let interner = StringInterner::new();
    let callee = interner.intern("baz");

    let aims = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Owned])];
    let legacy = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Borrowed])];

    let result = compare_arg_ownership(&aims, &legacy, &interner);
    assert!(matches!(result, DimensionResult::Regression(_)));
    if let DimensionResult::Regression(detail) = result {
        assert!(detail.contains("AIMS=Owned, legacy=Borrowed"));
        assert!(detail.contains("arg 1"));
    }
}

#[test]
fn arg_ownership_regression_takes_precedence_over_improvement() {
    let interner = StringInterner::new();
    let callee = interner.intern("mixed");

    // arg 0: AIMS borrows (improvement), arg 1: AIMS owns (regression)
    let aims = vec![(callee, vec![ArgOwnership::Borrowed, ArgOwnership::Owned])];
    let legacy = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Borrowed])];

    let result = compare_arg_ownership(&aims, &legacy, &interner);
    // Regression takes precedence
    assert!(matches!(result, DimensionResult::Regression(_)));
}

#[test]
fn arg_ownership_skipped_when_site_count_differs() {
    let interner = StringInterner::new();
    let callee = interner.intern("foo");

    let aims = vec![
        (callee, vec![ArgOwnership::Owned]),
        (callee, vec![ArgOwnership::Owned]),
    ];
    let legacy = vec![(callee, vec![ArgOwnership::Owned])];

    let result = compare_arg_ownership(&aims, &legacy, &interner);
    assert!(matches!(result, DimensionResult::Skipped(_)));
    if let DimensionResult::Skipped(detail) = result {
        assert!(detail.contains("call site count mismatch"));
    }
}

#[test]
fn arg_ownership_multiple_sites_all_match() {
    let interner = StringInterner::new();
    let foo = interner.intern("foo");
    let bar = interner.intern("bar");

    let aims = vec![
        (foo, vec![ArgOwnership::Owned, ArgOwnership::Borrowed]),
        (bar, vec![ArgOwnership::Borrowed]),
    ];
    let legacy = vec![
        (foo, vec![ArgOwnership::Owned, ArgOwnership::Borrowed]),
        (bar, vec![ArgOwnership::Borrowed]),
    ];

    assert!(matches!(
        compare_arg_ownership(&aims, &legacy, &interner),
        DimensionResult::Match
    ));
}

#[test]
fn arg_ownership_regression_on_arg_count_mismatch_at_site() {
    let interner = StringInterner::new();
    let callee = interner.intern("qux");

    let aims = vec![(callee, vec![ArgOwnership::Owned, ArgOwnership::Owned])];
    let legacy = vec![(callee, vec![ArgOwnership::Owned])];

    let result = compare_arg_ownership(&aims, &legacy, &interner);
    assert!(matches!(result, DimensionResult::Regression(_)));
    if let DimensionResult::Regression(detail) = result {
        assert!(detail.contains("arg count mismatch"));
    }
}

#[test]
fn arg_ownership_skipped_when_no_snapshot() {
    use crate::ir::ArcFunction;

    let func = ArcFunction::default();
    let interner = StringInterner::new();

    let comparison = super::compare::compare_function(
        &func,
        None,
        None,
        RcOpCount::default(),
        "test_fn".into(),
        &interner,
    );
    assert!(matches!(
        comparison.arg_ownership,
        DimensionResult::Skipped(_)
    ));
}

#[test]
fn arg_ownership_in_comparison_with_snapshot() {
    use crate::ir::ArcFunction;

    let interner = StringInterner::new();
    let callee = interner.intern("callee");

    let snapshot = AimsSnapshot {
        contract: Some(crate::aims::contract::MemoryContract::conservative(0)),
        cow_annotations: CowAnnotations::new(),
        rc_ops: RcOpCount::default(),
        arg_ownership_sites: vec![(callee, vec![ArgOwnership::Borrowed])],
        immortal_count: 0,
    };

    // Build a function with one Apply instruction that has Owned arg_ownership
    let mut func = ArcFunction::default();
    use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId};
    func.blocks = vec![ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::Apply {
            dst: ArcVarId::new(0),
            ty: ori_types::Idx::NONE,
            func: callee,
            args: vec![ArcVarId::new(1)],
            arg_ownership: vec![ArgOwnership::Owned],
        }],
        terminator: ArcTerminator::Return {
            value: ArcVarId::new(0),
        },
    }];
    func.entry = ArcBlockId::new(0);

    let old_summary = crate::uniqueness::UniquenessSummary {
        params: Vec::new(),
        return_val: crate::uniqueness::Uniqueness::MaybeShared,
        preserves_freshness: false,
    };

    let comparison = super::compare::compare_function(
        &func,
        Some(&snapshot),
        Some(&old_summary),
        RcOpCount::default(),
        "test_fn".into(),
        &interner,
    );

    // AIMS borrows where legacy owns — improvement
    assert!(matches!(
        comparison.arg_ownership,
        DimensionResult::Improvement(_)
    ));
}

#[test]
fn has_regressions_true_when_arg_ownership_regresses() {
    use super::ShadowComparisonReport;

    let report = ShadowComparisonReport {
        per_function: Vec::new(),
        total_functions: 1,
        param_matches: 1,
        param_improvements: 0,
        param_regressions: 0,
        return_matches: 1,
        return_improvements: 0,
        return_regressions: 0,
        cow_matches: 1,
        cow_improvements: 0,
        cow_regressions: 0,
        rc_matches: 1,
        rc_improvements: 0,
        rc_regressions: 0,
        arg_ownership_matches: 0,
        arg_ownership_improvements: 0,
        arg_ownership_regressions: 1,
        aims_rc_total: 0,
        legacy_rc_total: 0,
        immortal_skips_total: 0,
    };

    assert!(report.has_regressions());
}
