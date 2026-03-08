use crate::{Ownership, ReturnTag, TypeTag};

use super::*;

// find_type tests

#[test]
fn find_type_returns_known_types() {
    for td in BUILTIN_TYPES {
        let found = find_type(td.tag);
        assert!(
            found.is_some(),
            "find_type({:?}) should find a TypeDef",
            td.tag
        );
        assert_eq!(
            found.unwrap_or_else(|| panic!("type should be found")).name,
            td.name
        );
    }
}

#[test]
fn find_type_returns_none_for_non_registry_tags() {
    assert!(find_type(TypeTag::Unit).is_none());
    assert!(find_type(TypeTag::Never).is_none());
    assert!(find_type(TypeTag::Function).is_none());
    // DEI has no separate TypeDef — must use base_type() aliasing
    assert!(find_type(TypeTag::DoubleEndedIterator).is_none());
}

#[test]
fn find_type_deterministic() {
    let first = find_type(TypeTag::Int);
    let second = find_type(TypeTag::Int);
    assert!(std::ptr::eq(
        first.unwrap_or_else(|| panic!("first lookup should succeed")),
        second.unwrap_or_else(|| panic!("second lookup should succeed")),
    ));
}

// find_type_by_name tests

#[test]
fn find_type_by_name_returns_known_types() {
    for td in BUILTIN_TYPES {
        let found = find_type_by_name(td.name);
        assert!(
            found.is_some(),
            "find_type_by_name({:?}) should find a TypeDef",
            td.name
        );
        assert_eq!(
            found
                .unwrap_or_else(|| panic!("type should be found by name"))
                .tag,
            td.tag
        );
    }
}

#[test]
fn find_type_by_name_returns_none_for_unknown() {
    assert!(find_type_by_name("FooBar").is_none());
    assert!(find_type_by_name("").is_none());
}

#[test]
fn find_type_by_name_is_case_sensitive() {
    // "int" exists, "Int" does not
    assert!(find_type_by_name("int").is_some());
    assert!(find_type_by_name("Int").is_none());
    // "Iterator" exists, "iterator" does not
    assert!(find_type_by_name("Iterator").is_some());
    assert!(find_type_by_name("iterator").is_none());
}

// find_method tests

#[test]
fn find_method_returns_known_methods() {
    let abs = find_method(TypeTag::Int, "abs");
    assert!(abs.is_some());
    let abs_def = abs.unwrap_or_else(|| panic!("abs method should exist"));
    assert_eq!(abs_def.name, "abs");
    assert_eq!(abs_def.returns, ReturnTag::Concrete(TypeTag::Int));
}

#[test]
fn find_method_returns_none_for_unknown_method() {
    assert!(find_method(TypeTag::Int, "nonexistent").is_none());
}

#[test]
fn find_method_returns_none_for_non_registry_type() {
    assert!(find_method(TypeTag::Unit, "foo").is_none());
}

#[test]
fn find_method_dei_aware_plain_iterator() {
    // Plain Iterator should NOT see DEI-only methods
    assert!(find_method(TypeTag::Iterator, "next_back").is_none());
    assert!(find_method(TypeTag::Iterator, "rev").is_none());
    assert!(find_method(TypeTag::Iterator, "last").is_none());
    assert!(find_method(TypeTag::Iterator, "rfind").is_none());
    assert!(find_method(TypeTag::Iterator, "rfold").is_none());

    // But should see non-DEI methods
    assert!(find_method(TypeTag::Iterator, "next").is_some());
    assert!(find_method(TypeTag::Iterator, "map").is_some());
    assert!(find_method(TypeTag::Iterator, "collect").is_some());
}

#[test]
fn find_method_dei_aware_double_ended_iterator() {
    // DEI should see ALL methods including DEI-only
    assert!(find_method(TypeTag::DoubleEndedIterator, "next_back").is_some());
    assert!(find_method(TypeTag::DoubleEndedIterator, "rev").is_some());
    assert!(find_method(TypeTag::DoubleEndedIterator, "last").is_some());
    assert!(find_method(TypeTag::DoubleEndedIterator, "rfind").is_some());
    assert!(find_method(TypeTag::DoubleEndedIterator, "rfold").is_some());

    // And non-DEI methods too
    assert!(find_method(TypeTag::DoubleEndedIterator, "next").is_some());
    assert!(find_method(TypeTag::DoubleEndedIterator, "map").is_some());
    assert!(find_method(TypeTag::DoubleEndedIterator, "collect").is_some());
}

// has_method tests

#[test]
fn has_method_agrees_with_find_method() {
    let cases = [
        (TypeTag::Int, "abs", true),
        (TypeTag::Int, "nonexistent", false),
        (TypeTag::Str, "length", true),
        (TypeTag::Unit, "foo", false),
        (TypeTag::Iterator, "map", true),
        (TypeTag::Iterator, "next_back", false), // DEI-only
        (TypeTag::DoubleEndedIterator, "next_back", true),
    ];

    for (tag, name, expected) in cases {
        assert_eq!(
            has_method(tag, name),
            expected,
            "has_method({tag:?}, {name:?}) should be {expected}"
        );
    }
}

// methods_for tests

#[test]
fn methods_for_returns_nonempty_for_known_types() {
    for td in BUILTIN_TYPES {
        let methods = methods_for(td.tag);
        assert!(
            !methods.is_empty(),
            "methods_for({:?}) should be non-empty",
            td.tag
        );
    }
}

#[test]
fn methods_for_returns_empty_for_unknown_types() {
    assert!(methods_for(TypeTag::Unit).is_empty());
    assert!(methods_for(TypeTag::Never).is_empty());
}

// method_names_for tests

#[test]
fn method_names_for_returns_expected_names() {
    let int_names: Vec<&str> = method_names_for(TypeTag::Int).collect();
    assert!(int_names.contains(&"abs"));
    assert!(int_names.contains(&"to_str"));
}

#[test]
fn method_names_for_dei_filters_dei_only() {
    let iter_names: Vec<&str> = method_names_for(TypeTag::Iterator).collect();
    // Plain Iterator: 19 non-DEI methods
    assert_eq!(iter_names.len(), 19);
    assert!(!iter_names.contains(&"next_back"));
    assert!(!iter_names.contains(&"rev"));

    let dei_names: Vec<&str> = method_names_for(TypeTag::DoubleEndedIterator).collect();
    // DEI: all 24 methods
    assert_eq!(dei_names.len(), 24);
    assert!(dei_names.contains(&"next_back"));
    assert!(dei_names.contains(&"rev"));
}

// borrowing_methods tests

#[test]
fn borrowing_methods_returns_only_borrow_receiver() {
    for m in borrowing_methods(TypeTag::Str) {
        assert_eq!(
            m.receiver,
            Ownership::Borrow,
            "borrowing_methods should only return Borrow receiver methods"
        );
    }
}

// BUILTIN_TYPES enforcement tests

#[test]
fn builtin_types_no_duplicate_tags() {
    let mut seen = std::collections::HashSet::new();
    for td in BUILTIN_TYPES {
        assert!(
            seen.insert(td.tag),
            "Duplicate TypeTag {:?} in BUILTIN_TYPES",
            td.tag
        );
    }
}

#[test]
fn builtin_types_no_duplicate_names() {
    let mut seen = std::collections::HashSet::new();
    for td in BUILTIN_TYPES {
        assert!(
            seen.insert(td.name),
            "Duplicate name {:?} in BUILTIN_TYPES",
            td.name
        );
    }
}

#[test]
fn builtin_types_is_nonempty() {
    assert!(!BUILTIN_TYPES.is_empty());
}

#[test]
fn builtin_types_sorted_by_tag_discriminant() {
    let tags: Vec<u8> = BUILTIN_TYPES.iter().map(|td| td.tag as u8).collect();
    let mut sorted = tags.clone();
    sorted.sort_unstable();
    assert_eq!(
        tags, sorted,
        "BUILTIN_TYPES must be sorted by TypeTag discriminant value"
    );
}

// find_method alias tests

#[test]
fn find_method_alias_methods_work() {
    // str has both "length" and "len" registered
    assert!(find_method(TypeTag::Str, "length").is_some());
    assert!(find_method(TypeTag::Str, "len").is_some());
}

// borrowing_methods count tests

#[test]
fn borrowing_methods_subset_of_all_methods() {
    // For types that have both borrow and owned methods, borrowing count
    // should be less than total.
    for td in BUILTIN_TYPES {
        let borrow_count = borrowing_methods(td.tag).count();
        let total_count = methods_for(td.tag).len();
        assert!(
            borrow_count <= total_count,
            "borrowing_methods({:?}) count {} should not exceed methods_for count {}",
            td.tag,
            borrow_count,
            total_count
        );
    }
}

// legacy_type_name tests

#[test]
fn legacy_type_name_maps_pascal_to_lowercase() {
    use super::legacy_type_name;

    assert_eq!(legacy_type_name("Error"), "error");
    assert_eq!(legacy_type_name("List"), "list");
    assert_eq!(legacy_type_name("Map"), "map");
    assert_eq!(legacy_type_name("Range"), "range");
    assert_eq!(legacy_type_name("Tuple"), "tuple");
    assert_eq!(legacy_type_name("int"), "int");
    assert_eq!(legacy_type_name("str"), "str");
}

// const fn find_type test

#[test]
fn find_type_is_const_fn() {
    // Verify find_type can be used in const context
    const INT_DEF: Option<&crate::TypeDef> = find_type(TypeTag::Int);
    let Some(int_def) = INT_DEF else {
        panic!("const lookup should succeed");
    };
    assert_eq!(int_def.name, "int");
}
