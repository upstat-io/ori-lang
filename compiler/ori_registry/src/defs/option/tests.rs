//! Tests for the Option `TypeDef`.

use super::*;

#[test]
fn option_method_count() {
    assert_eq!(OPTION.methods.len(), 18);
}

#[test]
fn option_is_structural() {
    assert_eq!(OPTION.memory, MemoryStrategy::Structural);
}

#[test]
fn option_is_generic_with_one_param() {
    assert_eq!(OPTION.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn option_no_operators() {
    assert_eq!(OPTION.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn option_trait_methods() {
    let expected = [
        ("clone", "Clone"),
        ("compare", "Comparable"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("hash", "Hashable"),
    ];
    for (name, trait_name) in expected {
        let m = OPTION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Option.{name}");
    }
}

#[test]
fn option_unwrap_returns_element() {
    for name in ["unwrap", "expect", "unwrap_or"] {
        let m = OPTION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("unwrap method should exist"));
        assert_eq!(m.returns, ELEM, "Option.{name} should return ElementType");
    }
}

#[test]
fn option_predicates_return_bool() {
    for name in ["is_some", "is_none"] {
        let m = OPTION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("predicate method should exist"));
        assert_eq!(m.returns, BOOL, "Option.{name} should return bool");
    }
}

#[test]
fn option_higher_order_methods_return_fresh() {
    for name in ["map", "and_then", "flat_map", "filter", "or_else"] {
        let m = OPTION
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("higher-order method should exist"));
        assert_eq!(
            m.returns, FRESH,
            "Option.{name} should return Fresh (closure-dependent)"
        );
    }
}

#[test]
fn option_ok_or_returns_result_of_projection() {
    let m = OPTION
        .methods
        .iter()
        .find(|m| m.name == "ok_or")
        .unwrap_or_else(|| panic!("ok_or method should exist"));
    assert_eq!(
        m.returns,
        ReturnTag::ResultOfProjectionFresh(TypeProjection::Element),
    );
}

#[test]
fn option_iter_returns_iterator() {
    let m = OPTION
        .methods
        .iter()
        .find(|m| m.name == "iter")
        .unwrap_or_else(|| panic!("iter method should exist"));
    assert_eq!(m.returns, ReturnTag::IteratorOf(TypeProjection::Element),);
}

#[test]
fn option_methods_alphabetically_sorted() {
    let names: Vec<&str> = OPTION.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "Option methods must be alphabetically sorted"
    );
}
