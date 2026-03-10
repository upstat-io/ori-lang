//! Tests for `can_dispatch_builtin` dispatch capability check.

use ori_ir::StringInterner;
use ori_registry::TypeTag;

use super::{can_dispatch_builtin, is_collection_dispatched, minimal_value_for_tag};

fn test_interner() -> StringInterner {
    StringInterner::default()
}

// Sanity: known-working methods return true

#[test]
fn int_clone_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(TypeTag::Int, "clone", &interner));
}

#[test]
fn str_len_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(TypeTag::Str, "len", &interner));
}

#[test]
fn list_push_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(TypeTag::List, "push", &interner));
}

// Sanity: collection methods return true (these go through CollectionMethodResolver)

#[test]
fn list_map_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(TypeTag::List, "map", &interner));
}

#[test]
fn iterator_next_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(TypeTag::Iterator, "next", &interner));
}

#[test]
fn range_collect_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(TypeTag::Range, "collect", &interner));
}

#[test]
fn ordering_then_with_is_dispatchable() {
    let interner = test_interner();
    assert!(can_dispatch_builtin(
        TypeTag::Ordering,
        "then_with",
        &interner
    ));
}

// Sanity: nonexistent methods return false

#[test]
fn nonexistent_method_is_not_dispatchable() {
    let interner = test_interner();
    assert!(!can_dispatch_builtin(
        TypeTag::Int,
        "nonexistent_method_xyz",
        &interner
    ));
}

// Sanity: types without Value return false for non-collection methods

#[test]
fn unit_has_no_dispatch() {
    let interner = test_interner();
    assert!(!can_dispatch_builtin(TypeTag::Unit, "clone", &interner));
}

// is_collection_dispatched covers both resolver paths

#[test]
fn collection_dispatch_covers_iterator_methods() {
    // All iterator methods should be recognized
    let iterator_methods = [
        "next",
        "map",
        "filter",
        "take",
        "skip",
        "fold",
        "count",
        "find",
        "any",
        "all",
        "for_each",
        "collect",
        "enumerate",
        "zip",
        "chain",
        "flatten",
        "flat_map",
        "cycle",
        "next_back",
        "rev",
        "last",
        "rfind",
        "rfold",
        "join",
        "collect_set",
    ];
    for method in iterator_methods {
        assert!(
            is_collection_dispatched(TypeTag::Iterator, method),
            "Iterator.{method} should be collection-dispatched"
        );
    }
}

#[test]
fn collection_dispatch_does_not_cover_builtin_methods() {
    // clone is not a collection method
    assert!(!is_collection_dispatched(TypeTag::Int, "clone"));
    assert!(!is_collection_dispatched(TypeTag::Str, "len"));
}

// minimal_value_for_tag coverage

#[test]
fn minimal_value_covers_all_dispatchable_types() {
    let dispatchable = [
        TypeTag::Int,
        TypeTag::Float,
        TypeTag::Bool,
        TypeTag::Str,
        TypeTag::Char,
        TypeTag::Byte,
        TypeTag::Duration,
        TypeTag::Size,
        TypeTag::Ordering,
        TypeTag::Option,
        TypeTag::Result,
        TypeTag::List,
        TypeTag::Map,
        TypeTag::Set,
        TypeTag::Range,
        TypeTag::Tuple,
        TypeTag::Error,
    ];
    for tag in dispatchable {
        assert!(
            minimal_value_for_tag(tag).is_some(),
            "{tag:?} should have a minimal Value"
        );
    }
}

#[test]
fn minimal_value_returns_none_for_non_value_types() {
    let non_value = [
        TypeTag::Unit,
        TypeTag::Never,
        TypeTag::Iterator,
        TypeTag::DoubleEndedIterator,
        TypeTag::Channel,
        TypeTag::Function,
    ];
    for tag in non_value {
        assert!(
            minimal_value_for_tag(tag).is_none(),
            "{tag:?} should not have a minimal Value"
        );
    }
}
