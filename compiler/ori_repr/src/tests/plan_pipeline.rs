use super::*;

#[test]
fn compute_repr_plan_populates_primitives() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert!(plan.get_repr(Idx::INT).is_some(), "Int must be populated");
    assert!(
        plan.get_repr(Idx::FLOAT).is_some(),
        "Float must be populated"
    );
    assert!(plan.get_repr(Idx::BOOL).is_some(), "Bool must be populated");
    assert!(plan.get_repr(Idx::STR).is_some(), "Str must be populated");
    assert!(plan.get_repr(Idx::CHAR).is_some(), "Char must be populated");
    assert!(plan.get_repr(Idx::BYTE).is_some(), "Byte must be populated");
    assert!(plan.get_repr(Idx::UNIT).is_some(), "Unit must be populated");
    assert!(
        plan.get_repr(Idx::NEVER).is_some(),
        "Never must be populated"
    );
    assert!(
        plan.get_repr(Idx::DURATION).is_some(),
        "Duration must be populated"
    );
    assert!(plan.get_repr(Idx::SIZE).is_some(), "Size must be populated");
    assert!(
        plan.get_repr(Idx::ORDERING).is_some(),
        "Ordering must be populated"
    );
    assert!(
        plan.get_repr(Idx::ERROR).is_some(),
        "Error must be populated as Unit"
    );
}

#[test]
fn compute_repr_plan_disabled_policy_preserves_canonical_repr() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled, &[]);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Disabled);
}

#[test]
fn compute_repr_plan_aggressive_policy_preserves_canonical_repr() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(plan.narrowing_policy(), NarrowingPolicy::Aggressive);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        })
    );
}

#[test]
fn compute_repr_plan_canonical_int_semantic_pin() {
    let pool = ori_types::Pool::new();
    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    assert_eq!(
        plan.get_repr(Idx::INT),
        Some(&MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }),
        "canonical int must be i64 signed — semantic pin"
    );
}

#[test]
fn compute_repr_plan_zero_behavioral_change_with_disabled() {
    let pool = ori_types::Pool::new();
    let plan_aggressive = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);
    let plan_disabled = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Disabled, &[]);
    for raw in 0..Idx::PRIMITIVE_COUNT {
        let idx = Idx::from_raw(raw);
        assert_eq!(
            plan_aggressive.get_repr(idx),
            plan_disabled.get_repr(idx),
            "canonical repr for primitive {raw} must match regardless of policy"
        );
    }
}

#[test]
fn is_env_truthy_accepts_1() {
    assert!(crate::plan::query::is_env_truthy("1"));
}

#[test]
fn is_env_truthy_accepts_true_lowercase() {
    assert!(crate::plan::query::is_env_truthy("true"));
}

#[test]
fn is_env_truthy_accepts_true_uppercase() {
    assert!(crate::plan::query::is_env_truthy("TRUE"));
}

#[test]
fn is_env_truthy_accepts_true_mixed_case() {
    assert!(crate::plan::query::is_env_truthy("True"));
}

#[test]
fn is_env_truthy_accepts_yes_lowercase() {
    assert!(crate::plan::query::is_env_truthy("yes"));
}

#[test]
fn is_env_truthy_accepts_yes_uppercase() {
    assert!(crate::plan::query::is_env_truthy("YES"));
}

#[test]
fn is_env_truthy_rejects_0() {
    assert!(!crate::plan::query::is_env_truthy("0"));
}

#[test]
fn is_env_truthy_rejects_false() {
    assert!(!crate::plan::query::is_env_truthy("false"));
}

#[test]
fn is_env_truthy_rejects_no() {
    assert!(!crate::plan::query::is_env_truthy("no"));
}

#[test]
fn is_env_truthy_rejects_empty() {
    assert!(!crate::plan::query::is_env_truthy(""));
}

#[test]
fn is_env_truthy_rejects_arbitrary() {
    assert!(!crate::plan::query::is_env_truthy("banana"));
}

#[test]
fn env_disabled_rejects_falsey_values() {
    assert!(
        !crate::plan::query::is_env_truthy("0"),
        "0 must not enable --no-repr-opt"
    );
    assert!(
        !crate::plan::query::is_env_truthy("false"),
        "false must not enable --no-repr-opt"
    );
    assert!(
        !crate::plan::query::is_env_truthy(""),
        "empty must not enable --no-repr-opt"
    );
    assert!(
        crate::plan::query::is_env_truthy("1"),
        "1 must enable --no-repr-opt"
    );
    assert!(
        crate::plan::query::is_env_truthy("true"),
        "true must enable --no-repr-opt"
    );
}
