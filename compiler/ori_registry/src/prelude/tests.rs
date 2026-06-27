//! Tests for prelude function definitions.

use super::*;

/// Semantic pin: `PRELUDE_FUNCTIONS` contains exactly the expected entries.
#[test]
fn prelude_functions_complete() {
    let names: Vec<&str> = PRELUDE_FUNCTIONS.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        [
            "byte",
            "drop_early",
            "float",
            "hash_combine",
            "int",
            "repeat",
            "str"
        ],
        "PRELUDE_FUNCTIONS must contain exactly these entries in alphabetical order"
    );
}

/// Semantic pin: `drop_early` has signature `<T>(value: T) -> void` and CONSUMES
/// its argument (`Ownership::Owned`), so the caller does not also drop at scope exit.
///
/// Regression: `drop_early` was documented in the prelude + recommended by E2048 but
/// implemented in zero compiler layers — `drop_early(value: x)` failed typecheck with
/// E2003. Its arg MUST be `Owned` (consume), never the shared `Ownership::Borrow`
/// `GENERIC_PARAM` (which would leave the caller's scope-exit drop in place → double-drop).
#[test]
fn drop_early_signature() {
    let f =
        find_prelude_function("drop_early").unwrap_or_else(|| panic!("drop_early should exist"));
    assert_eq!(f.params.len(), 1, "drop_early should have 1 param");
    assert_eq!(
        f.params[0].ty,
        ReturnTag::Fresh,
        "drop_early param should be Fresh (generic T)"
    );
    assert_eq!(
        f.params[0].ownership,
        crate::Ownership::Owned,
        "drop_early MUST consume its argument (Owned), never Borrow — else double-drop"
    );
    assert_eq!(
        f.returns,
        ReturnTag::Unit,
        "drop_early returns void (ReturnTag::Unit)"
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
