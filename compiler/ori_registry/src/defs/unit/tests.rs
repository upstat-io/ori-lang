use super::*;

#[test]
fn unit_registered_traits_match_its_single_value_semantics() {
    let expected = [
        ("clone", "Clone"),
        ("compare", "Comparable"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("hash", "Hashable"),
    ];

    for (method_name, trait_name) in expected {
        let method = UNIT
            .methods
            .iter()
            .find(|method| method.name == method_name)
            .unwrap_or_else(|| panic!("void.{method_name} should be registered"));
        assert_eq!(method.trait_name, Some(trait_name));
    }
    assert!(UNIT.traits.contains(&"Default"));
}

#[test]
fn unit_operators_use_structural_single_value_strategies() {
    assert_eq!(UNIT.operators.eq, OpStrategy::StructuralEquality);
    assert_eq!(UNIT.operators.neq, OpStrategy::StructuralEquality);
    assert_eq!(UNIT.operators.lt, OpStrategy::StructuralOrdering);
    assert_eq!(UNIT.operators.gt, OpStrategy::StructuralOrdering);
    assert_eq!(UNIT.operators.lt_eq, OpStrategy::StructuralOrdering);
    assert_eq!(UNIT.operators.gt_eq, OpStrategy::StructuralOrdering);
    assert_eq!(UNIT.operators.add, OpStrategy::Unsupported);
}

#[test]
fn unit_default_is_an_associated_zero_value_constructor() {
    let method = UNIT
        .methods
        .iter()
        .find(|method| method.name == "default")
        .unwrap_or_else(|| panic!("void.default should be registered"));

    assert_eq!(method.kind, crate::MethodKind::Associated);
    assert_eq!(method.returns, ReturnTag::SelfType);
    assert!(method.params.is_empty());
}
