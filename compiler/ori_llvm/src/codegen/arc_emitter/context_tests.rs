use super::{
    intercepted_emission_invokes_unwind, intercepted_is_nounwind, MAY_UNWIND_INTERCEPTED_METHODS,
};

#[test]
fn intercepted_is_nounwind_defaults_true_for_unknown_methods() {
    assert!(intercepted_is_nounwind("map"));
    assert!(intercepted_is_nounwind("filter"));
    assert!(intercepted_is_nounwind("len"));
    assert!(intercepted_is_nounwind("is_empty"));
    assert!(intercepted_is_nounwind(""));
}

#[test]
fn intercepted_is_nounwind_rejects_may_unwind_methods() {
    for &name in MAY_UNWIND_INTERCEPTED_METHODS {
        assert!(
            !intercepted_is_nounwind(name),
            "method {name:?} must be classified may-unwind"
        );
    }
}

#[test]
fn may_unwind_list_covers_option_result_panic_methods() {
    for expected in ["unwrap", "unwrap_err", "expect", "expect_err"] {
        assert!(
            MAY_UNWIND_INTERCEPTED_METHODS.contains(&expected),
            "expected method {expected:?} missing from may-unwind list"
        );
    }
}

#[test]
fn may_unwind_list_covers_iterator_callback_boundaries() {
    for expected in [
        "__iter_next",
        "__collect_set",
        "next",
        "next_back",
        "rev",
        "collect",
        "count",
        "any",
        "all",
        "find",
        "for_each",
        "fold",
        "last",
        "rfind",
        "rfold",
        "join",
    ] {
        assert!(
            MAY_UNWIND_INTERCEPTED_METHODS.contains(&expected),
            "iterator callback boundary {expected:?} must be may-unwind"
        );
    }
}

#[test]
fn iterator_callback_emissions_keep_their_unwind_edges() {
    for method in [
        "__iter_next",
        "__collect_set",
        "next",
        "next_back",
        "rev",
        "collect",
        "count",
        "any",
        "all",
        "find",
        "for_each",
        "fold",
        "last",
        "rfind",
        "rfold",
        "join",
    ] {
        assert!(intercepted_emission_invokes_unwind(
            method,
            ori_types::Tag::Iterator
        ));
        assert!(intercepted_emission_invokes_unwind(
            method,
            ori_types::Tag::DoubleEndedIterator
        ));
        assert!(!intercepted_emission_invokes_unwind(
            method,
            ori_types::Tag::List
        ));
    }
}
