//! Tests for the Result `TypeDef`.

use super::*;

#[test]
fn result_method_count() {
    assert_eq!(RESULT.methods.len(), 21);
}

#[test]
fn result_is_structural() {
    assert_eq!(RESULT.memory, MemoryStrategy::Structural);
}

#[test]
fn result_has_two_type_params() {
    assert_eq!(RESULT.type_params, TypeParamArity::Fixed(2));
}

#[test]
fn result_no_operators() {
    assert_eq!(RESULT.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn result_trait_methods() {
    let expected = [
        ("clone", "Clone"),
        ("compare", "Comparable"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("has_trace", "Traceable"),
        ("hash", "Hashable"),
        ("trace", "Traceable"),
        ("trace_entries", "Traceable"),
    ];
    for (name, trait_name) in expected {
        let m = RESULT
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "Result.{name}");
    }
}

#[test]
fn result_unwrap_returns_ok_type() {
    for name in ["unwrap", "expect", "unwrap_or"] {
        let m = RESULT
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("unwrap method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::OkType,
            "Result.{name} should return OkType"
        );
    }
}

#[test]
fn result_unwrap_err_returns_err_type() {
    for name in ["unwrap_err", "expect_err"] {
        let m = RESULT
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("unwrap_err method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::ErrType,
            "Result.{name} should return ErrType"
        );
    }
}

#[test]
fn result_ok_returns_option_of_ok() {
    let m = RESULT
        .methods
        .iter()
        .find(|m| m.name == "ok")
        .unwrap_or_else(|| panic!("ok method should exist"));
    assert_eq!(m.returns, ReturnTag::OptionOf(TypeProjection::Ok));
}

#[test]
fn result_err_returns_option_of_err() {
    let m = RESULT
        .methods
        .iter()
        .find(|m| m.name == "err")
        .unwrap_or_else(|| panic!("err method should exist"));
    assert_eq!(m.returns, ReturnTag::OptionOf(TypeProjection::Err));
}

#[test]
fn result_higher_order_methods_return_fresh() {
    for name in ["map", "map_err", "and_then", "or_else"] {
        let m = RESULT
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("higher-order method should exist"));
        assert_eq!(m.returns, FRESH, "Result.{name} should return Fresh");
    }
}

#[test]
fn result_methods_alphabetically_sorted() {
    let names: Vec<&str> = RESULT.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "Result methods must be alphabetically sorted"
    );
}
