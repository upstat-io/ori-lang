//! Tests for the List `TypeDef`.

use super::*;

#[test]
fn list_method_count() {
    assert_eq!(LIST.methods.len(), 57);
}

#[test]
fn list_is_arc() {
    assert_eq!(LIST.memory, MemoryStrategy::Arc);
}

#[test]
fn list_is_generic_with_one_param() {
    assert_eq!(LIST.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn list_operators_add_only() {
    // List supports `+` for concatenation (RuntimeCall to list_concat).
    // All other operators are unsupported.
    assert!(LIST.operators.add != OpStrategy::Unsupported);
    assert_eq!(LIST.operators.sub, OpStrategy::Unsupported);
    assert_eq!(LIST.operators.mul, OpStrategy::Unsupported);
    assert_eq!(LIST.operators.eq, OpStrategy::Unsupported);
    assert_eq!(LIST.operators.neg, OpStrategy::Unsupported);
    assert_eq!(LIST.operators.bit_and, OpStrategy::Unsupported);
}

#[test]
fn list_trait_methods() {
    let expected = [
        ("clone", "Clone"),
        ("compare", "Comparable"),
        ("debug", "Debug"),
        ("equals", "Eq"),
        ("hash", "Hashable"),
    ];
    for (name, trait_name) in expected {
        let m = LIST
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("trait method should exist"));
        assert_eq!(m.trait_name, Some(trait_name), "List.{name}");
    }
}

#[test]
fn list_option_returning_methods() {
    for name in ["first", "last", "pop", "get"] {
        let m = LIST
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns, OPT_ELEM,
            "List.{name} should return OptionOf(Element)"
        );
    }
}

#[test]
fn list_self_returning_mutations() {
    let mutations = [
        "reverse",
        "sort",
        "sort_stable",
        "sorted",
        "unique",
        "flatten",
        "push",
        "append",
        "prepend",
        "concat",
        "clone",
        "set",
        "insert",
        "remove",
        "slice",
        "take",
        "skip",
        "drop",
    ];
    for name in mutations {
        let m = LIST
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("mutation method should exist"));
        assert_eq!(m.returns, SELF, "List.{name} should return SelfType");
    }
}

#[test]
fn list_iter_returns_dei() {
    let m = LIST
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
fn list_enumerate_returns_list_of_tuple_int_element() {
    let m = LIST
        .methods
        .iter()
        .find(|m| m.name == "enumerate")
        .unwrap_or_else(|| panic!("enumerate method should exist"));
    assert_eq!(m.returns, ReturnTag::ListOfTupleIntElement);
}

#[test]
fn list_higher_order_methods_return_fresh() {
    let hofs = [
        "map",
        "filter",
        "flat_map",
        "find",
        "any",
        "all",
        "fold",
        "reduce",
        "for_each",
        "take_while",
        "skip_while",
        "min",
        "max",
        "sum",
        "product",
        "min_by",
        "max_by",
        "sort_by",
        "group_by",
        "partition",
        "chunk",
        "window",
        "zip",
    ];
    for name in hofs {
        let m = LIST
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("higher-order method should exist"));
        assert_eq!(m.returns, FRESH, "List.{name} should return Fresh");
    }
}

#[test]
fn list_push_takes_owned_element() {
    let m = LIST
        .methods
        .iter()
        .find(|m| m.name == "push")
        .unwrap_or_else(|| panic!("push method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::ElementType);
    assert_eq!(m.params[0].ownership, Ownership::Owned);
}

#[test]
fn list_set_takes_index_and_element() {
    let m = LIST
        .methods
        .iter()
        .find(|m| m.name == "set")
        .unwrap_or_else(|| panic!("set method should exist"));
    assert_eq!(m.params.len(), 2);
    assert_eq!(m.params[0].ty, ReturnTag::Concrete(TypeTag::Int));
    assert_eq!(m.params[1].ty, ReturnTag::ElementType);
}

#[test]
fn list_join_takes_separator() {
    let m = LIST
        .methods
        .iter()
        .find(|m| m.name == "join")
        .unwrap_or_else(|| panic!("join method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::Concrete(TypeTag::Str));
    assert_eq!(m.returns, STR);
}

#[test]
fn list_methods_alphabetically_sorted() {
    let names: Vec<&str> = LIST.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "List methods must be alphabetically sorted");
}
