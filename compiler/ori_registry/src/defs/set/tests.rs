//! Tests for the Set `TypeDef`.

use super::*;

#[test]
fn set_method_count() {
    assert_eq!(SET.methods.len(), 16);
}

#[test]
fn set_is_arc() {
    assert_eq!(SET.memory, MemoryStrategy::Arc);
}

#[test]
fn set_is_generic_with_one_param() {
    assert_eq!(SET.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn set_no_operators() {
    assert_eq!(SET.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn set_trait_methods() {
    let expected = [
        ("clone", "Clone"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("hash", "Hashable"),
        ("into", "Into"),
    ];
    for (name, trait_name) in expected {
        let m = SET
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Set.{name}");
    }
}

#[test]
fn set_iter_returns_iterator_not_dei() {
    let m = SET
        .methods
        .iter()
        .find(|m| m.name == "iter")
        .unwrap_or_else(|| panic!("iter method should exist"));
    assert_eq!(
        m.returns,
        ReturnTag::IteratorOf(TypeProjection::Element),
        "Set.iter() returns Iterator (not DEI — no inherent ordering)"
    );
}

#[test]
fn set_insert_takes_owned_element() {
    let m = SET
        .methods
        .iter()
        .find(|m| m.name == "insert")
        .unwrap_or_else(|| panic!("insert method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::ElementType);
    assert_eq!(m.params[0].ownership, Ownership::Owned);
}

#[test]
fn set_set_operations_take_self_param() {
    for name in ["union", "intersection", "difference"] {
        let m = SET
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("set operation method should exist"));
        assert_eq!(m.params.len(), 1, "Set.{name} takes one param");
        assert_eq!(
            m.params[0].ty,
            ReturnTag::SelfType,
            "Set.{name} param is Self"
        );
        assert_eq!(m.returns, SELF, "Set.{name} returns Self");
    }
}

#[test]
fn set_length_is_alias_for_len() {
    let len = SET
        .methods
        .iter()
        .find(|m| m.name == "len")
        .unwrap_or_else(|| panic!("len method should exist"));
    let length = SET
        .methods
        .iter()
        .find(|m| m.name == "length")
        .unwrap_or_else(|| panic!("length method should exist"));
    assert_eq!(len.returns, length.returns);
    assert_eq!(len.params.len(), length.params.len());
}

#[test]
fn set_methods_alphabetically_sorted() {
    let names: Vec<&str> = SET.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "Set methods must be alphabetically sorted");
}
