use super::{
    intercepted_emission_invokes_unwind, intercepted_is_nounwind, MAY_UNWIND_INTERCEPTED_METHODS,
};

#[test]
fn intercepted_is_nounwind_defaults_true_for_unknown_methods() {
    assert!(intercepted_is_nounwind("map", None, None));
    assert!(intercepted_is_nounwind("filter", None, None));
    assert!(intercepted_is_nounwind("len", None, None));
    assert!(intercepted_is_nounwind("is_empty", None, None));
    assert!(intercepted_is_nounwind("", None, None));
}

#[test]
fn intercepted_is_nounwind_rejects_may_unwind_methods() {
    use ori_types::Tag;

    for &name in MAY_UNWIND_INTERCEPTED_METHODS {
        let (receiver, result) = match name {
            "__cast" => (Tag::Int, Some(Tag::Byte)),
            "abs" | "byte" => (Tag::Int, Some(Tag::Int)),
            "int" | "to_int" => (Tag::Float, Some(Tag::Int)),
            "updated" | "__index" => (Tag::List, None),
            "unwrap" | "expect" => (Tag::Option, None),
            "unwrap_err" | "expect_err" => (Tag::Result, None),
            "__iter_next" | "__collect_set" | "next" | "next_back" | "rev" | "collect"
            | "count" | "any" | "all" | "find" | "for_each" | "fold" | "last" | "rfind"
            | "rfold" | "join" => (Tag::Iterator, None),
            _ => (Tag::Int, None),
        };
        assert!(
            !intercepted_is_nounwind(name, Some(receiver), result),
            "method {name:?} must be classified may-unwind"
        );
    }
}

#[test]
fn checked_intercepts_remain_type_directed() {
    use ori_types::Tag;

    assert!(intercepted_is_nounwind(
        "__cast",
        Some(Tag::Int),
        Some(Tag::Float)
    ));
    assert!(intercepted_is_nounwind(
        "to_int",
        Some(Tag::Int),
        Some(Tag::Int)
    ));
    assert!(intercepted_is_nounwind(
        "byte",
        Some(Tag::Byte),
        Some(Tag::Byte)
    ));
    assert!(intercepted_is_nounwind(
        "abs",
        Some(Tag::Float),
        Some(Tag::Float)
    ));
}

#[test]
fn may_unwind_list_covers_checked_conversion_intercepts() {
    for expected in ["__cast", "abs", "byte", "int", "to_int"] {
        assert!(
            MAY_UNWIND_INTERCEPTED_METHODS.contains(&expected),
            "checked conversion intercept {expected:?} missing from may-unwind inventory"
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
            Some(ori_types::Tag::Iterator),
            None,
        ));
        assert!(intercepted_emission_invokes_unwind(
            method,
            Some(ori_types::Tag::DoubleEndedIterator),
            None,
        ));
        assert!(!intercepted_emission_invokes_unwind(
            method,
            Some(ori_types::Tag::List),
            None,
        ));
    }
}
