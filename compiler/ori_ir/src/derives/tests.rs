use super::*;

#[test]
fn test_derived_trait_from_name() {
    assert_eq!(DerivedTrait::from_name("Eq"), Some(DerivedTrait::Eq));
    assert_eq!(DerivedTrait::from_name("Clone"), Some(DerivedTrait::Clone));
    assert_eq!(
        DerivedTrait::from_name("Hashable"),
        Some(DerivedTrait::Hashable)
    );
    assert_eq!(
        DerivedTrait::from_name("Printable"),
        Some(DerivedTrait::Printable)
    );
    assert_eq!(DerivedTrait::from_name("Debug"), Some(DerivedTrait::Debug));
    assert_eq!(
        DerivedTrait::from_name("Default"),
        Some(DerivedTrait::Default)
    );
    assert_eq!(
        DerivedTrait::from_name("Comparable"),
        Some(DerivedTrait::Comparable)
    );
    assert_eq!(DerivedTrait::from_name("Unknown"), None);
}

#[test]
fn test_derived_trait_method_name() {
    assert_eq!(DerivedTrait::Eq.method_name(), "eq");
    assert_eq!(DerivedTrait::Clone.method_name(), "clone");
    assert_eq!(DerivedTrait::Hashable.method_name(), "hash");
    assert_eq!(DerivedTrait::Printable.method_name(), "to_str");
    assert_eq!(DerivedTrait::Debug.method_name(), "debug");
    assert_eq!(DerivedTrait::Default.method_name(), "default");
    assert_eq!(DerivedTrait::Comparable.method_name(), "compare");
}

// --- New tests for macro-generated metadata ---

#[test]
fn all_contains_every_variant() {
    assert_eq!(DerivedTrait::ALL.len(), 7);
    assert_eq!(DerivedTrait::COUNT, 7);
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Eq));
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Clone));
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Hashable));
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Printable));
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Debug));
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Default));
    assert!(DerivedTrait::ALL.contains(&DerivedTrait::Comparable));
}

#[test]
fn count_matches_all_len() {
    assert_eq!(DerivedTrait::COUNT, DerivedTrait::ALL.len());
}

#[test]
fn trait_name_roundtrips_with_from_name() {
    for &t in DerivedTrait::ALL {
        let name = t.trait_name();
        let parsed = DerivedTrait::from_name(name);
        assert_eq!(parsed, Some(t), "round-trip failed for {name}");
    }
}

#[test]
fn trait_name_returns_expected_strings() {
    assert_eq!(DerivedTrait::Eq.trait_name(), "Eq");
    assert_eq!(DerivedTrait::Clone.trait_name(), "Clone");
    assert_eq!(DerivedTrait::Hashable.trait_name(), "Hashable");
    assert_eq!(DerivedTrait::Printable.trait_name(), "Printable");
    assert_eq!(DerivedTrait::Debug.trait_name(), "Debug");
    assert_eq!(DerivedTrait::Default.trait_name(), "Default");
    assert_eq!(DerivedTrait::Comparable.trait_name(), "Comparable");
}

#[test]
fn shape_correctness() {
    assert_eq!(
        DerivedTrait::Eq.shape(),
        DerivedMethodShape::BinaryPredicate
    );
    assert_eq!(
        DerivedTrait::Clone.shape(),
        DerivedMethodShape::UnaryIdentity
    );
    assert_eq!(
        DerivedTrait::Hashable.shape(),
        DerivedMethodShape::UnaryToInt
    );
    assert_eq!(
        DerivedTrait::Printable.shape(),
        DerivedMethodShape::UnaryToStr
    );
    assert_eq!(DerivedTrait::Debug.shape(), DerivedMethodShape::UnaryToStr);
    assert_eq!(DerivedTrait::Default.shape(), DerivedMethodShape::Nullary);
    assert_eq!(
        DerivedTrait::Comparable.shape(),
        DerivedMethodShape::BinaryToOrdering
    );
}

#[test]
fn requires_supertrait_correctness() {
    assert_eq!(DerivedTrait::Eq.requires_supertrait(), None);
    assert_eq!(DerivedTrait::Clone.requires_supertrait(), None);
    assert_eq!(
        DerivedTrait::Hashable.requires_supertrait(),
        Some(DerivedTrait::Eq)
    );
    assert_eq!(DerivedTrait::Printable.requires_supertrait(), None);
    assert_eq!(DerivedTrait::Debug.requires_supertrait(), None);
    assert_eq!(DerivedTrait::Default.requires_supertrait(), None);
    assert_eq!(
        DerivedTrait::Comparable.requires_supertrait(),
        Some(DerivedTrait::Eq)
    );
}

#[test]
fn supports_sum_types_correctness() {
    assert!(DerivedTrait::Eq.supports_sum_types());
    assert!(DerivedTrait::Clone.supports_sum_types());
    assert!(DerivedTrait::Hashable.supports_sum_types());
    assert!(DerivedTrait::Printable.supports_sum_types());
    assert!(DerivedTrait::Debug.supports_sum_types());
    assert!(!DerivedTrait::Default.supports_sum_types()); // Default cannot be derived for sum types
    assert!(DerivedTrait::Comparable.supports_sum_types());
}

// --- Cross-crate sync enforcement (Section 05.1, Test 1) ---

#[test]
fn all_derived_traits_round_trip() {
    for &trait_kind in DerivedTrait::ALL {
        let name = trait_kind.trait_name();
        let method = trait_kind.method_name();

        // from_name round-trips
        assert_eq!(
            DerivedTrait::from_name(name),
            Some(trait_kind),
            "from_name({name:?}) failed for {trait_kind:?}"
        );

        // method_name is non-empty
        assert!(!method.is_empty(), "method_name() empty for {trait_kind:?}");

        // trait_name is non-empty
        assert!(!name.is_empty(), "trait_name() empty for {trait_kind:?}");

        // shape has valid param_count
        let shape = trait_kind.shape();
        assert!(
            shape.param_count() <= 2,
            "shape param_count > 2 for {trait_kind:?}"
        );
    }
}

// --- DerivedMethodShape tests ---

#[test]
fn shape_has_self() {
    assert!(DerivedMethodShape::BinaryPredicate.has_self());
    assert!(DerivedMethodShape::UnaryIdentity.has_self());
    assert!(DerivedMethodShape::UnaryToInt.has_self());
    assert!(DerivedMethodShape::UnaryToStr.has_self());
    assert!(!DerivedMethodShape::Nullary.has_self());
    assert!(DerivedMethodShape::BinaryToOrdering.has_self());
}

#[test]
fn shape_has_other() {
    assert!(DerivedMethodShape::BinaryPredicate.has_other());
    assert!(!DerivedMethodShape::UnaryIdentity.has_other());
    assert!(!DerivedMethodShape::UnaryToInt.has_other());
    assert!(!DerivedMethodShape::UnaryToStr.has_other());
    assert!(!DerivedMethodShape::Nullary.has_other());
    assert!(DerivedMethodShape::BinaryToOrdering.has_other());
}

#[test]
fn shape_param_count() {
    assert_eq!(DerivedMethodShape::BinaryPredicate.param_count(), 2);
    assert_eq!(DerivedMethodShape::UnaryIdentity.param_count(), 1);
    assert_eq!(DerivedMethodShape::UnaryToInt.param_count(), 1);
    assert_eq!(DerivedMethodShape::UnaryToStr.param_count(), 1);
    assert_eq!(DerivedMethodShape::Nullary.param_count(), 0);
    assert_eq!(DerivedMethodShape::BinaryToOrdering.param_count(), 2);
}
