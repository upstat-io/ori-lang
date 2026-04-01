//! Tests for the Channel `TypeDef`.

use super::*;

#[test]
fn channel_method_count() {
    assert_eq!(CHANNEL.methods.len(), 9);
}

#[test]
fn channel_is_arc() {
    assert_eq!(CHANNEL.memory, MemoryStrategy::Arc);
}

#[test]
fn channel_is_generic_with_one_param() {
    assert_eq!(CHANNEL.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn channel_no_operators() {
    assert_eq!(CHANNEL.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn channel_all_methods_impure() {
    for m in CHANNEL.methods {
        assert!(
            !m.pure,
            "Channel.{} should be impure (shared mutable state)",
            m.name
        );
    }
}

#[test]
fn channel_send_takes_element_owned() {
    let m = CHANNEL
        .methods
        .iter()
        .find(|m| m.name == "send")
        .unwrap_or_else(|| panic!("send method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::ElementType);
    assert_eq!(m.params[0].ownership, Ownership::Owned);
}

#[test]
fn channel_recv_returns_option_of_element() {
    for name in ["recv", "receive", "try_recv", "try_receive"] {
        let m = CHANNEL
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("recv method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::OptionOf(TypeProjection::Element),
            "Channel.{name} should return OptionOf(Element)"
        );
    }
}

#[test]
fn channel_close_and_send_return_unit() {
    for name in ["close", "send"] {
        let m = CHANNEL
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::Unit,
            "Channel.{name} should return Unit"
        );
    }
}

#[test]
fn channel_trait_methods() {
    let expected_traits: &[(&str, Option<&str>)] =
        &[("is_empty", Some("IsEmpty")), ("len", Some("Len"))];
    for m in CHANNEL.methods {
        let expected = expected_traits
            .iter()
            .find(|(name, _)| *name == m.name)
            .and_then(|(_, t)| *t);
        assert_eq!(
            m.trait_name, expected,
            "Channel.{} trait_name should be {:?}",
            m.name, expected
        );
    }
}

#[test]
fn channel_methods_alphabetically_sorted() {
    let names: Vec<&str> = CHANNEL.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "Channel methods must be alphabetically sorted"
    );
}
