use super::*;

#[test]
fn repr_plan_set_get_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let decision = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    plan.set_repr(Idx::INT, decision);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
}

#[test]
fn repr_plan_override_returns_second_decision() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let d1 = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    let d2 = ReprDecision {
        source: DecisionSource::IntegerNarrowing,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        },
        reason: DecisionReason::RangeFits {
            range: ValueRange::Bounded { lo: 0, hi: 1000 },
            min_width: IntWidth::I32,
        },
    };
    plan.set_repr(Idx::INT, d1);
    plan.set_repr(Idx::INT, d2);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        })
    );
}

#[test]
fn repr_plan_audit_trail_preserves_both_decisions() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let d1 = ReprDecision {
        source: DecisionSource::Canonical,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        reason: DecisionReason::Canonical,
    };
    let d2 = ReprDecision {
        source: DecisionSource::IntegerNarrowing,
        type_idx: Idx::INT,
        repr: MachineRepr::Int {
            width: IntWidth::I32,
            signed: true,
        },
        reason: DecisionReason::RangeFits {
            range: ValueRange::Bounded { lo: 0, hi: 1000 },
            min_width: IntWidth::I32,
        },
    };
    plan.set_repr(Idx::INT, d1);
    plan.set_repr(Idx::INT, d2);
    let audit = plan.dump_audit(&pool);
    assert!(!audit.is_empty());
    assert!(audit.contains("Canonical"), "audit must contain Canonical");
    assert!(
        audit.contains("IntegerNarrowing"),
        "audit must contain IntegerNarrowing"
    );
    assert!(
        audit.find("Canonical") < audit.find("IntegerNarrowing"),
        "Canonical must appear before IntegerNarrowing in audit trail"
    );
}

#[test]
fn repr_plan_get_unknown_idx_returns_none() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan.get_repr(Idx::INT).is_none());
}

#[test]
fn repr_plan_var_range_no_recorded_ranges_returns_default() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let func = Name::new(0, 1);
    let var = ori_arc::ArcVarId::new(0);
    let range = plan.var_range(func, var);
    assert_eq!(range, ValueRange::default());
}

#[test]
fn repr_plan_set_var_ranges_round_trip_isolated() {
    use rustc_hash::FxHashMap;

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let func_a = Name::new(0, 1);
    let func_b = Name::new(0, 2);
    let var_0 = ori_arc::ArcVarId::new(0);
    let var_1 = ori_arc::ArcVarId::new(1);

    let range_0_100 = ValueRange::Bounded { lo: 0, hi: 100 };
    let range_neg = ValueRange::Bounded { lo: -50, hi: 50 };

    let mut ranges_a = FxHashMap::default();
    ranges_a.insert(var_0, range_0_100);
    plan.set_var_ranges(func_a, ranges_a);

    let mut ranges_b = FxHashMap::default();
    ranges_b.insert(var_1, range_neg);
    plan.set_var_ranges(func_b, ranges_b);

    assert_eq!(plan.var_range(func_a, var_0), range_0_100);
    assert_eq!(plan.var_range(func_a, var_1), ValueRange::Top);

    assert_eq!(plan.var_range(func_b, var_1), range_neg);
    assert_eq!(plan.var_range(func_b, var_0), ValueRange::Top);
}

#[test]
fn repr_plan_dump_audit_contains_tag_and_source() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_repr(
        Idx::INT,
        ReprDecision {
            source: DecisionSource::Triviality,
            type_idx: Idx::INT,
            repr: MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            reason: DecisionReason::TransitivelyTrivial,
        },
    );
    let audit = plan.dump_audit(&pool);
    assert!(!audit.is_empty());
    assert!(audit.contains("int"), "audit must contain type tag 'int'");
    assert!(
        audit.contains("Triviality"),
        "audit must contain source 'Triviality'"
    );
}

#[test]
fn int_width_default_returns_i64() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(plan.int_width(Idx::INT), IntWidth::I64);
}

#[test]
fn float_width_default_returns_f64() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(plan.float_width(Idx::FLOAT), FloatWidth::F64);
}

#[test]
fn is_trivial_default_returns_false() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(
        !plan.is_trivial(Idx::INT),
        "safe default must be non-trivial"
    );
}

#[test]
fn escapes_default_returns_true() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(
        plan.escapes(Name::new(0, 0), ori_arc::ArcVarId::new(0)),
        "safe default must assume escapes"
    );
}

#[test]
fn rc_strategy_default_is_atomic_i64() {
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert_eq!(
        plan.rc_strategy(Idx::INT),
        RcStrategy::Atomic {
            width: IntWidth::I64,
        },
    );
}

#[test]
fn rc_strategy_default_for_canonical_opaque_ptr() {
    let mut pool = Pool::new();
    let iter_idx = pool.iterator(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_repr(
        iter_idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: iter_idx,
            repr: MachineRepr::UnmanagedPtr,
            reason: DecisionReason::Canonical,
        },
    );
    assert_eq!(
        plan.rc_strategy(iter_idx),
        RcStrategy::Atomic {
            width: IntWidth::I64,
        },
    );
}

#[test]
fn set_rc_strategy_preserves_original_repr() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let original_repr = MachineRepr::Struct(StructRepr {
        fields: vec![FieldRepr {
            repr: MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            original_index: 0,
            offset: 0,
            name: Name::new(0, 1),
        }],
        size: 8,
        align: 8,
        trivial: false,
    });
    plan.set_repr(
        Idx::INT,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: Idx::INT,
            repr: original_repr.clone(),
            reason: DecisionReason::Canonical,
        },
    );
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    assert_eq!(plan.get_repr(Idx::INT), Some(&original_repr));
}

#[test]
fn set_rc_strategy_write_read_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    assert_eq!(plan.rc_strategy(Idx::INT), RcStrategy::None);
}

#[test]
fn set_rc_strategy_non_atomic_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let strategy = RcStrategy::NonAtomic {
        width: IntWidth::I16,
    };
    plan.set_rc_strategy(Idx::INT, strategy, DecisionSource::ThreadLocal);
    assert_eq!(plan.rc_strategy(Idx::INT), strategy);
}

#[test]
fn set_rc_strategy_atomic_narrow_round_trip() {
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let strategy = RcStrategy::Atomic {
        width: IntWidth::I8,
    };
    plan.set_rc_strategy(Idx::INT, strategy, DecisionSource::ArcHeader);
    assert_eq!(plan.rc_strategy(Idx::INT), strategy);
}

#[test]
fn set_rc_strategy_records_audit_entry() {
    use ori_types::Pool;

    let pool = Pool::new();
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_rc_strategy(Idx::INT, RcStrategy::None, DecisionSource::Triviality);
    let audit = plan.dump_audit(&pool);
    assert!(
        audit.contains("Triviality"),
        "audit must contain the RC strategy decision source"
    );
}
