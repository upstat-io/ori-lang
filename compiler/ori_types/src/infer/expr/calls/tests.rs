use super::suggest_iterator_fix;
use crate::Tag;

// Regression:
// `suggest_iterator_fix` is the tag-specialized hint helper used by the
// flat_map closure-return diagnostic. Types whose values can be turned into
// an iterator with a method call (`.iter()`) get a concrete suggestion;
// other types get a generic message because there is no canonical wrapping.

#[test]
fn suggest_iterator_fix_for_list_recommends_iter() {
    let s = suggest_iterator_fix(Tag::List);
    assert!(
        s.message.contains(".iter()"),
        "List return should suggest `.iter()`, got: {}",
        s.message
    );
}

#[test]
fn suggest_iterator_fix_for_int_uses_generic_message() {
    let s = suggest_iterator_fix(Tag::Int);
    assert!(
        !s.message.contains(".iter()"),
        "int return should NOT suggest `.iter()` (no canonical wrap), got: {}",
        s.message
    );
    assert!(
        s.message.contains("flat_map"),
        "generic suggestion should name flat_map, got: {}",
        s.message
    );
}

#[test]
fn suggest_iterator_fix_for_struct_uses_generic_message() {
    // Tag::Struct represents user-defined struct types. Without knowing the
    // struct's API, no `.iter()` suggestion is appropriate.
    let s = suggest_iterator_fix(Tag::Struct);
    assert!(
        !s.message.contains(".iter()"),
        "struct return should NOT suggest `.iter()`, got: {}",
        s.message
    );
}

#[test]
fn suggest_iterator_fix_for_option_recommends_iter() {
    // Option is in the suggestion-friendly set per Plan TPR finding A12 —
    // `Option<T>::iter()` exists and yields a 0-or-1-element iterator.
    let s = suggest_iterator_fix(Tag::Option);
    assert!(
        s.message.contains(".iter()"),
        "Option return should suggest `.iter()`, got: {}",
        s.message
    );
}

// Sanity coverage for the remaining suggestion-friendly tags so the matrix
// is fully clamped (any future re-categorization breaks one of these).
#[test]
fn suggest_iterator_fix_full_friendly_set() {
    for friendly in [Tag::Set, Tag::Map, Tag::Str, Tag::Range] {
        let s = suggest_iterator_fix(friendly);
        assert!(
            s.message.contains(".iter()"),
            "tag {:?} should be in the suggestion-friendly set, got: {}",
            friendly,
            s.message
        );
    }
}

#[test]
fn suggest_iterator_fix_tuple_uses_generic_message() {
    // Tuple is intentionally NOT in the suggestion-friendly set per Plan
    // TPR finding A13 — tuples have no `.iter()` method.
    let s = suggest_iterator_fix(Tag::Tuple);
    assert!(
        !s.message.contains(".iter()"),
        "Tuple return should NOT suggest `.iter()`, got: {}",
        s.message
    );
}
