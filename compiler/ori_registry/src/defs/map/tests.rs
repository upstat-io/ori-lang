//! Tests for the Map `TypeDef`.

use super::*;

#[test]
fn map_method_count() {
    assert_eq!(MAP.methods.len(), 18);
}

#[test]
fn map_is_arc() {
    assert_eq!(MAP.memory, MemoryStrategy::Arc);
}

#[test]
fn map_has_two_type_params() {
    assert_eq!(MAP.type_params, TypeParamArity::Fixed(2));
}

#[test]
fn map_no_operators() {
    assert_eq!(MAP.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn map_trait_methods() {
    let expected = [
        ("clone", "Clone"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("hash", "Hashable"),
    ];
    for (name, trait_name) in expected {
        let m = MAP
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Map.{name}");
    }
}

#[test]
fn map_get_returns_option_of_value() {
    let m = MAP
        .methods
        .iter()
        .find(|m| m.name == "get")
        .unwrap_or_else(|| panic!("get method should exist"));
    assert_eq!(m.returns, ReturnTag::OptionOf(TypeProjection::Value));
}

#[test]
fn map_keys_returns_list_of_key() {
    let m = MAP
        .methods
        .iter()
        .find(|m| m.name == "keys")
        .unwrap_or_else(|| panic!("keys method should exist"));
    assert_eq!(m.returns, ReturnTag::ListOf(TypeProjection::Key));
}

#[test]
fn map_values_returns_list_of_value() {
    let m = MAP
        .methods
        .iter()
        .find(|m| m.name == "values")
        .unwrap_or_else(|| panic!("values method should exist"));
    assert_eq!(m.returns, ReturnTag::ListOf(TypeProjection::Value));
}

#[test]
fn map_entries_returns_list_key_value() {
    let m = MAP
        .methods
        .iter()
        .find(|m| m.name == "entries")
        .unwrap_or_else(|| panic!("entries method should exist"));
    assert_eq!(m.returns, ReturnTag::ListKeyValue);
}

#[test]
fn map_iter_returns_map_iterator() {
    let m = MAP
        .methods
        .iter()
        .find(|m| m.name == "iter")
        .unwrap_or_else(|| panic!("iter method should exist"));
    assert_eq!(m.returns, ReturnTag::MapIterator);
}

#[test]
fn map_insert_takes_key_and_value() {
    let m = MAP
        .methods
        .iter()
        .find(|m| m.name == "insert")
        .unwrap_or_else(|| panic!("insert method should exist"));
    assert_eq!(m.params.len(), 2);
    assert_eq!(m.params[0].ty, ReturnTag::KeyType);
    assert_eq!(m.params[0].ownership, Ownership::Owned);
    assert_eq!(m.params[1].ty, ReturnTag::ValueType);
    assert_eq!(m.params[1].ownership, Ownership::Owned);
}

#[test]
fn map_length_is_alias_for_len() {
    let len = MAP
        .methods
        .iter()
        .find(|m| m.name == "len")
        .unwrap_or_else(|| panic!("len method should exist"));
    let length = MAP
        .methods
        .iter()
        .find(|m| m.name == "length")
        .unwrap_or_else(|| panic!("length method should exist"));
    assert_eq!(len.returns, length.returns);
}

#[test]
fn map_methods_alphabetically_sorted() {
    let names: Vec<&str> = MAP.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "Map methods must be alphabetically sorted");
}
