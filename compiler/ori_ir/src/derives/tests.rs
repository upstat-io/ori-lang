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
    assert_eq!(
        DerivedTrait::from_name("Default"),
        Some(DerivedTrait::Default)
    );
    assert_eq!(DerivedTrait::from_name("Unknown"), None);
}

#[test]
fn test_derived_trait_method_name() {
    assert_eq!(DerivedTrait::Eq.method_name(), "eq");
    assert_eq!(DerivedTrait::Clone.method_name(), "clone");
    assert_eq!(DerivedTrait::Hashable.method_name(), "hash");
    assert_eq!(DerivedTrait::Printable.method_name(), "to_string");
    assert_eq!(DerivedTrait::Default.method_name(), "default");
}
