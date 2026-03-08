use super::*;
use crate::query;
use crate::MethodKind;

#[test]
fn iterator_has_24_methods() {
    assert_eq!(
        ITERATOR.methods.len(),
        24,
        "Iterator TypeDef should have exactly 24 user-callable methods"
    );
}

#[test]
fn iterator_is_arc() {
    assert_eq!(ITERATOR.memory, MemoryStrategy::Arc);
}

#[test]
fn iterator_is_generic_with_one_param() {
    assert_eq!(ITERATOR.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn iterator_has_no_operators() {
    assert_eq!(ITERATOR.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn all_methods_are_instance() {
    for m in ITERATOR.methods {
        assert_eq!(
            m.kind,
            MethodKind::Instance,
            "Iterator method `{}` should be Instance",
            m.name
        );
    }
}

#[test]
fn all_methods_borrow_receiver() {
    for m in ITERATOR.methods {
        assert_eq!(
            m.receiver,
            Ownership::Borrow,
            "Iterator method `{}` should borrow receiver",
            m.name
        );
    }
}

#[test]
fn all_methods_are_pure() {
    for m in ITERATOR.methods {
        assert!(m.pure, "Iterator method `{}` should be pure", m.name);
    }
}

#[test]
fn no_trait_methods() {
    for m in ITERATOR.methods {
        assert!(
            m.trait_name.is_none(),
            "Iterator method `{}` should have no trait_name",
            m.name
        );
    }
}

#[test]
fn dei_only_methods_are_exactly_five() {
    let dei_only: Vec<&str> = ITERATOR
        .methods
        .iter()
        .filter(|m| m.dei_only)
        .map(|m| m.name)
        .collect();

    assert_eq!(
        dei_only,
        vec!["last", "next_back", "rev", "rfind", "rfold"],
        "DEI-only methods should be exactly 5 (alphabetical order)"
    );
}

#[test]
fn dei_propagation_propagate_methods() {
    let propagate: Vec<&str> = ITERATOR
        .methods
        .iter()
        .filter(|m| m.dei_propagation == DeiPropagation::Propagate)
        .map(|m| m.name)
        .collect();

    assert_eq!(
        propagate,
        vec!["filter", "map"],
        "Only filter and map should propagate DEI"
    );
}

#[test]
fn dei_propagation_downgrade_methods() {
    let downgrade: Vec<&str> = ITERATOR
        .methods
        .iter()
        .filter(|m| m.dei_propagation == DeiPropagation::Downgrade)
        .map(|m| m.name)
        .collect();

    assert_eq!(
        downgrade,
        vec![
            "chain",
            "cycle",
            "enumerate",
            "flat_map",
            "flatten",
            "skip",
            "take",
            "zip"
        ],
        "Adapter methods that always downgrade to plain Iterator"
    );
}

#[test]
fn consumers_have_not_applicable_propagation() {
    let consumers = [
        "all", "any", "collect", "count", "find", "fold", "for_each", "join",
    ];

    for name in consumers {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.dei_propagation,
            DeiPropagation::NotApplicable,
            "Consumer `{name}` should have NotApplicable dei_propagation"
        );
    }
}

#[test]
fn protocol_methods_have_not_applicable_propagation() {
    for name in ["next", "next_back"] {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.dei_propagation,
            DeiPropagation::NotApplicable,
            "Protocol method `{name}` should have NotApplicable dei_propagation"
        );
    }
}

#[test]
fn next_and_next_back_return_next_result() {
    for name in ["next", "next_back"] {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::NextResult,
            "`{name}` should return NextResult"
        );
    }
}

#[test]
fn collect_returns_list_of_element() {
    let m = ITERATOR
        .methods
        .iter()
        .find(|m| m.name == "collect")
        .unwrap_or_else(|| panic!("collect method should exist"));
    assert_eq!(m.returns, ReturnTag::ListOf(TypeProjection::Element));
}

#[test]
fn enumerate_returns_iterator_of_tuple_int_element() {
    let m = ITERATOR
        .methods
        .iter()
        .find(|m| m.name == "enumerate")
        .unwrap_or_else(|| panic!("enumerate method should exist"));
    assert_eq!(m.returns, ReturnTag::IteratorOfTupleIntElement);
}

#[test]
fn filter_returns_self_type() {
    let m = ITERATOR
        .methods
        .iter()
        .find(|m| m.name == "filter")
        .unwrap_or_else(|| panic!("filter method should exist"));
    assert_eq!(m.returns, ReturnTag::SelfType);
}

#[test]
fn rev_returns_self_type() {
    let m = ITERATOR
        .methods
        .iter()
        .find(|m| m.name == "rev")
        .unwrap_or_else(|| panic!("rev method should exist"));
    assert_eq!(m.returns, ReturnTag::SelfType);
    assert!(m.dei_only);
}

#[test]
fn backend_required_matches_llvm_coverage() {
    // Methods with LLVM support (from declare_builtins! in iterator.rs)
    let llvm_supported = [
        "all",
        "any",
        "chain",
        "collect",
        "count",
        "enumerate",
        "filter",
        "find",
        "fold",
        "for_each",
        "map",
        "skip",
        "take",
        "zip",
    ];

    // Methods WITHOUT LLVM support
    let llvm_missing = [
        "cycle",
        "flat_map",
        "flatten",
        "join",
        "last",
        "next",
        "next_back",
        "rev",
        "rfind",
        "rfold",
    ];

    for name in llvm_supported {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert!(
            m.backend_required,
            "Method `{name}` has LLVM support — backend_required should be true"
        );
    }

    for name in llvm_missing {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert!(
            !m.backend_required,
            "Method `{name}` lacks LLVM support — backend_required should be false"
        );
    }
}

#[test]
fn methods_alphabetically_sorted() {
    let names: Vec<&str> = ITERATOR.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "Iterator methods must be sorted alphabetically"
    );
}

#[test]
fn higher_order_methods_return_fresh() {
    // Methods whose return type depends on closure argument
    let fresh_return = ["flat_map", "flatten", "fold", "map", "rfold", "zip"];

    for name in fresh_return {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::Fresh,
            "Higher-order method `{name}` should return Fresh"
        );
    }
}

#[test]
fn downgrade_adapters_return_iterator_of_element() {
    // Adapters that downgrade and preserve element type (no closure)
    let iter_of_elem = ["chain", "cycle", "skip", "take"];

    for name in iter_of_elem {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::IteratorOf(TypeProjection::Element),
            "Adapter `{name}` should return IteratorOf(Element)"
        );
    }
}

#[test]
fn find_and_last_return_option_of_element() {
    for name in ["find", "last", "rfind"] {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::OptionOf(TypeProjection::Element),
            "`{name}` should return OptionOf(Element)"
        );
    }
}

#[test]
fn for_each_returns_unit() {
    let m = ITERATOR
        .methods
        .iter()
        .find(|m| m.name == "for_each")
        .unwrap_or_else(|| panic!("for_each method should exist"));
    assert_eq!(m.returns, ReturnTag::Unit);
}

#[test]
fn fold_takes_two_params() {
    for name in ["fold", "rfold"] {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.params.len(),
            2,
            "`{name}` should take 2 params (initial + op)"
        );
        assert_eq!(m.params[0].name, "initial");
        assert_eq!(m.params[1].name, "op");
    }
}

#[test]
fn join_takes_str_separator() {
    let m = ITERATOR
        .methods
        .iter()
        .find(|m| m.name == "join")
        .unwrap_or_else(|| panic!("join method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].name, "separator");
    assert_eq!(m.params[0].ty, ReturnTag::Concrete(TypeTag::Str));
    assert_eq!(m.params[0].ownership, Ownership::Borrow);
}

#[test]
fn take_and_skip_take_int_count() {
    for name in ["take", "skip"] {
        let m = ITERATOR
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(m.params.len(), 1);
        assert_eq!(m.params[0].name, "count");
        assert_eq!(m.params[0].ty, ReturnTag::Concrete(TypeTag::Int));
        assert_eq!(m.params[0].ownership, Ownership::Copy);
    }
}

// Query helper tests

#[test]
fn query_dei_only_methods_returns_exactly_five() {
    let names: Vec<&str> = query::dei_only_methods().collect();
    assert_eq!(names, vec!["last", "next_back", "rev", "rfind", "rfold"]);
}

#[test]
fn query_iterator_method_names_returns_24() {
    let names: Vec<&str> = query::iterator_method_names().collect();
    assert_eq!(names.len(), 24);
    // Verify alphabetical order
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn query_is_dei_only_true_for_dei_methods() {
    assert!(query::is_dei_only("next_back"));
    assert!(query::is_dei_only("rev"));
    assert!(query::is_dei_only("last"));
    assert!(query::is_dei_only("rfind"));
    assert!(query::is_dei_only("rfold"));
}

#[test]
fn query_is_dei_only_false_for_non_dei_methods() {
    assert!(!query::is_dei_only("next"));
    assert!(!query::is_dei_only("map"));
    assert!(!query::is_dei_only("filter"));
    assert!(!query::is_dei_only("collect"));
    assert!(!query::is_dei_only("fold"));
}

#[test]
fn query_is_dei_only_false_for_unknown_methods() {
    assert!(!query::is_dei_only("nonexistent"));
}
