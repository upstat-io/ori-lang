//! Tests for the Range `TypeDef`.

use super::*;

#[test]
fn range_method_count() {
    assert_eq!(RANGE.methods.len(), 8);
}

#[test]
fn range_is_copy() {
    assert_eq!(RANGE.memory, MemoryStrategy::Copy);
}

#[test]
fn range_is_generic_with_one_param() {
    assert_eq!(RANGE.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn range_all_methods_are_instance() {
    for m in RANGE.methods {
        assert_eq!(
            m.kind,
            crate::MethodKind::Instance,
            "Range.{} should be Instance",
            m.name
        );
    }
}

#[test]
fn range_no_operators() {
    assert_eq!(RANGE.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn range_iter_returns_dei() {
    let m = RANGE
        .methods
        .iter()
        .find(|m| m.name == "iter")
        .unwrap_or_else(|| panic!("iter method should exist"));
    assert_eq!(
        m.returns,
        ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element),
    );
}

#[test]
fn range_to_list_and_collect_return_list_of_element() {
    for name in ["to_list", "collect"] {
        let m = RANGE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::ListOf(TypeProjection::Element),
            "Range.{name} should return ListOf(Element)"
        );
    }
}

#[test]
fn range_contains_takes_element_param() {
    let m = RANGE
        .methods
        .iter()
        .find(|m| m.name == "contains")
        .unwrap_or_else(|| panic!("contains method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::ElementType);
}

#[test]
fn range_step_by_takes_int_returns_self() {
    let m = RANGE
        .methods
        .iter()
        .find(|m| m.name == "step_by")
        .unwrap_or_else(|| panic!("step_by method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::Concrete(TypeTag::Int));
    assert_eq!(m.returns, ReturnTag::SelfType);
}

#[test]
fn range_no_trait_methods() {
    for m in RANGE.methods {
        assert!(
            m.trait_name.is_none(),
            "Range.{} should have no trait_name (generic trait dispatch handles these)",
            m.name
        );
    }
}

#[test]
fn range_methods_alphabetically_sorted() {
    let names: Vec<&str> = RANGE.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "Range methods must be alphabetically sorted");
}
