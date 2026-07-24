use super::*;

#[test]
fn analyze_triviality_validation_zero_mismatches() {
    use ori_types::{EnumVariant, Idx, Pool};

    let mut pool = Pool::new();

    let opt_int = pool.option(Idx::INT);
    let tuple_trivial = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let sn = Name::from_raw(8000);
    let f1 = Name::from_raw(8001);
    let f2 = Name::from_raw(8002);
    let struct_trivial = pool.struct_type(sn, &[(f1, Idx::INT), (f2, Idx::FLOAT)]);
    let result_nontrivial = pool.result(Idx::INT, Idx::STR);
    let enum_trivial = pool.enum_type(
        Name::from_raw(8010),
        &[
            EnumVariant {
                name: Name::from_raw(8011),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(8012),
                field_types: vec![Idx::INT],
            },
        ],
    );

    let iter_int = pool.iterator(Idx::INT);
    let deiter_int = pool.double_ended_iterator(Idx::INT);

    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    assert!(plan.is_trivial(opt_int), "Option<int> should be trivial");
    assert!(
        plan.is_trivial(tuple_trivial),
        "(int, float) should be trivial"
    );
    assert!(
        plan.is_trivial(struct_trivial),
        "struct {{int, float}} should be trivial"
    );
    assert!(
        !plan.is_trivial(result_nontrivial),
        "Result<int, str> should be non-trivial"
    );
    assert!(
        !plan.is_trivial(iter_int),
        "Iterator<int> is non-trivial — needs ori_iter_drop at scope exit"
    );
    assert!(
        !plan.is_trivial(deiter_int),
        "DoubleEndedIterator<int> is non-trivial — needs ori_iter_drop at scope exit"
    );
    assert!(
        plan.is_trivial(enum_trivial),
        "enum {{unit, int}} should be trivial"
    );
}

#[test]
fn repr_plan_error_type_is_trivial() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(
        plan.is_trivial(Idx::ERROR),
        "Idx::ERROR must be trivial — matches classify_triviality() and ArcClassifier"
    );
}

#[test]
fn repr_plan_error_type_has_canonical_repr() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(
        plan.get_repr(Idx::ERROR).is_some(),
        "Idx::ERROR must have a canonical representation"
    );
}

#[test]
fn repr_plan_error_triviality_matches_classify_triviality() {
    use ori_types::triviality::{classify_triviality, Triviality};

    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    let plan_trivial = plan.is_trivial(Idx::ERROR);
    let classify_trivial = classify_triviality(Idx::ERROR, &pool) == Triviality::Trivial;
    assert_eq!(
        plan_trivial, classify_trivial,
        "ReprPlan::is_trivial(ERROR) = {plan_trivial}, classify_triviality(ERROR) = {classify_trivial} — must agree"
    );
}
