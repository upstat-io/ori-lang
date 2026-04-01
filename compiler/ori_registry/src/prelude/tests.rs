//! Tests for prelude function definitions.

use super::*;

/// Semantic pin: `PRELUDE_FUNCTIONS` contains exactly the expected entries.
#[test]
fn prelude_functions_complete() {
    let names: Vec<&str> = PRELUDE_FUNCTIONS.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        ["byte", "float", "hash_combine", "int", "repeat", "str"],
        "PRELUDE_FUNCTIONS must contain exactly these entries in alphabetical order"
    );
}

/// Prelude functions are sorted alphabetically.
#[test]
fn prelude_functions_sorted() {
    for pair in PRELUDE_FUNCTIONS.windows(2) {
        assert!(
            pair[0].name <= pair[1].name,
            "PRELUDE_FUNCTIONS not sorted: `{}` should come after `{}`",
            pair[0].name,
            pair[1].name
        );
    }
}

/// `hash_combine` has the correct signature: `(int, int) -> int`.
#[test]
fn hash_combine_signature() {
    let f = find_prelude_function("hash_combine")
        .unwrap_or_else(|| panic!("hash_combine should exist"));
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].ty, ReturnTag::Concrete(TypeTag::Int));
    assert_eq!(f.params[1].ty, ReturnTag::Concrete(TypeTag::Int));
    assert_eq!(f.returns, ReturnTag::Concrete(TypeTag::Int));
}

/// `repeat` has the correct signature: `(T) -> Iterator<T>`.
#[test]
fn repeat_signature() {
    let f = find_prelude_function("repeat").unwrap_or_else(|| panic!("repeat should exist"));
    assert_eq!(f.params.len(), 1);
    assert_eq!(f.params[0].ty, ReturnTag::Fresh);
    assert_eq!(
        f.returns,
        ReturnTag::IteratorOf(crate::TypeProjection::Element)
    );
}

/// Conversion functions have generic param and concrete return.
#[test]
fn conversion_function_signatures() {
    let conversions = [
        ("int", TypeTag::Int),
        ("float", TypeTag::Float),
        ("str", TypeTag::Str),
        ("byte", TypeTag::Byte),
    ];
    for (name, expected_tag) in conversions {
        let f = find_prelude_function(name).unwrap_or_else(|| panic!("{name} should exist"));
        assert_eq!(f.params.len(), 1, "{name} should have 1 param");
        assert_eq!(
            f.params[0].ty,
            ReturnTag::Fresh,
            "{name} param should be Fresh (generic)"
        );
        assert_eq!(
            f.returns,
            ReturnTag::Concrete(expected_tag),
            "{name} should return {expected_tag:?}"
        );
    }
}

/// `find_prelude_function` returns `None` for unknown names.
#[test]
fn unknown_prelude_function_returns_none() {
    assert!(find_prelude_function("nonexistent").is_none());
    assert!(find_prelude_function("print").is_none());
    assert!(find_prelude_function("len").is_none());
}
