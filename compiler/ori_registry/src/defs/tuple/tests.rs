//! Tests for the Tuple `TypeDef`.

use super::*;

#[test]
fn tuple_method_count() {
    assert_eq!(TUPLE.methods.len(), 6);
}

#[test]
fn tuple_is_structural() {
    assert_eq!(TUPLE.memory, MemoryStrategy::Structural);
}

#[test]
fn tuple_is_variadic() {
    assert_eq!(TUPLE.type_params, TypeParamArity::Variadic);
}

#[test]
fn tuple_no_operators() {
    assert_eq!(TUPLE.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn tuple_trait_methods() {
    let expected = [
        ("clone", "Clone"),
        ("compare", "Comparable"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("hash", "Hashable"),
    ];
    for (name, trait_name) in expected {
        let m = TUPLE
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Tuple.{name}");
    }
}

#[test]
fn tuple_len_is_inherent() {
    let m = TUPLE
        .methods
        .iter()
        .find(|m| m.name == "len")
        .unwrap_or_else(|| panic!("len method should exist"));
    assert!(m.trait_name.is_none(), "len is an inherent method");
}

#[test]
fn tuple_equals_takes_self_param() {
    let m = TUPLE
        .methods
        .iter()
        .find(|m| m.name == "equals")
        .unwrap_or_else(|| panic!("equals method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::SelfType);
    assert_eq!(m.params[0].ownership, Ownership::Owned);
}

#[test]
fn tuple_methods_alphabetically_sorted() {
    let names: Vec<&str> = TUPLE.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "Tuple methods must be alphabetically sorted");
}
