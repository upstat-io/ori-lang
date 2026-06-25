use super::*;

#[test]
fn flags_size() {
    assert_eq!(std::mem::size_of::<TypeFlags>(), 4);
}

#[test]
fn has_any_var_works() {
    assert!(TypeFlags::HAS_VAR.has_any_var());
    assert!(TypeFlags::HAS_BOUND_VAR.has_any_var());
    assert!(TypeFlags::HAS_RIGID_VAR.has_any_var());
    assert!(!TypeFlags::IS_PRIMITIVE.has_any_var());
    assert!(!TypeFlags::HAS_ERROR.has_any_var());
}

#[test]
fn propagate_from_works() {
    let child = TypeFlags::HAS_VAR | TypeFlags::IS_PRIMITIVE;
    let propagated = TypeFlags::propagate_from(child);

    // HAS_VAR should propagate
    assert!(propagated.contains(TypeFlags::HAS_VAR));
    // IS_PRIMITIVE should NOT propagate
    assert!(!propagated.contains(TypeFlags::IS_PRIMITIVE));
}

#[test]
fn propagate_all_works() {
    let child1 = TypeFlags::HAS_VAR;
    let child2 = TypeFlags::HAS_ERROR;
    let child3 = TypeFlags::IS_PRIMITIVE; // Won't propagate

    let combined = TypeFlags::propagate_all([child1, child2, child3]);

    assert!(combined.contains(TypeFlags::HAS_VAR));
    assert!(combined.contains(TypeFlags::HAS_ERROR));
    assert!(!combined.contains(TypeFlags::IS_PRIMITIVE));
}

#[test]
fn category_detection_works() {
    assert_eq!(TypeFlags::IS_PRIMITIVE.category(), TypeCategory::Primitive);
    assert_eq!(TypeFlags::IS_FUNCTION.category(), TypeCategory::Function);
    assert_eq!(TypeFlags::IS_CONTAINER.category(), TypeCategory::Container);
    assert_eq!(TypeFlags::IS_COMPOSITE.category(), TypeCategory::Composite);
    assert_eq!(TypeFlags::IS_SCHEME.category(), TypeCategory::Scheme);
    assert_eq!(TypeFlags::IS_NAMED.category(), TypeCategory::Named);
    assert_eq!(TypeFlags::HAS_VAR.category(), TypeCategory::Variable);
    assert_eq!(TypeFlags::HAS_BOUND_VAR.category(), TypeCategory::Variable);
    assert_eq!(TypeFlags::HAS_RIGID_VAR.category(), TypeCategory::Variable);
    assert_eq!(TypeFlags::empty().category(), TypeCategory::Unknown);
}

#[test]
fn category_priority_correct() {
    // When multiple flags are set, more specific category wins
    let fn_with_var = TypeFlags::IS_FUNCTION | TypeFlags::HAS_VAR;
    assert_eq!(fn_with_var.category(), TypeCategory::Function);

    let container_with_error = TypeFlags::IS_CONTAINER | TypeFlags::HAS_ERROR;
    assert_eq!(container_with_error.category(), TypeCategory::Container);
}

#[test]
fn flag_categories_dont_overlap() {
    // Ensure flag categories use distinct bit ranges
    let presence = TypeFlags::HAS_VAR
        | TypeFlags::HAS_BOUND_VAR
        | TypeFlags::HAS_RIGID_VAR
        | TypeFlags::HAS_ERROR
        | TypeFlags::HAS_INFER
        | TypeFlags::HAS_SELF
        | TypeFlags::HAS_PROJECTION;

    let category = TypeFlags::IS_PRIMITIVE
        | TypeFlags::IS_CONTAINER
        | TypeFlags::IS_FUNCTION
        | TypeFlags::IS_COMPOSITE
        | TypeFlags::IS_NAMED
        | TypeFlags::IS_SCHEME;

    let optimization = TypeFlags::NEEDS_SUBST
        | TypeFlags::IS_RESOLVED
        | TypeFlags::IS_MONO
        | TypeFlags::IS_COPYABLE;

    let capability =
        TypeFlags::HAS_CAPABILITY | TypeFlags::IS_PURE | TypeFlags::HAS_IO | TypeFlags::HAS_ASYNC;

    // No overlap between categories
    assert!(!presence.intersects(category));
    assert!(!presence.intersects(optimization));
    assert!(!presence.intersects(capability));
    assert!(!category.intersects(optimization));
    assert!(!category.intersects(capability));
    assert!(!optimization.intersects(capability));
}

/// Regression: the monomorphization recorder minted a phantom `MonoInstance` over
/// a poison (`HAS_ERROR`) substitution because the concreteness gate keyed only on
/// `has_any_var_or_infer` (which excludes `HAS_ERROR`), materializing `assert_eq` method
/// invokes on a poison receiver (AOT missing-mono). `is_recordable` adds the poison
/// half — reverting `&& !has_errors()` flips the `HAS_ERROR` cases to true.
#[test]
fn is_recordable_concrete_true_poison_and_var_false() {
    // Concrete, no poison, no var -> recordable.
    assert!(TypeFlags::IS_PRIMITIVE.is_recordable());
    assert!(TypeFlags::IS_RESOLVED.is_recordable());

    // Poison (HAS_ERROR) -> NOT recordable (the load-bearing distinction; the
    // var/infer-only gate would wrongly accept this).
    assert!(!TypeFlags::HAS_ERROR.is_recordable());

    // Var / infer holes -> NOT recordable (the existing var/infer half preserved).
    assert!(!TypeFlags::HAS_VAR.is_recordable());
    assert!(!TypeFlags::HAS_INFER.is_recordable());
    assert!(!TypeFlags::HAS_BOUND_VAR.is_recordable());

    // Compound poison: HAS_ERROR rides PROPAGATE_MASK, so a container carrying a
    // poison child (e.g. List<Idx::ERROR>) is also refused.
    let compound_poison = TypeFlags::IS_CONTAINER | TypeFlags::HAS_ERROR;
    assert!(!compound_poison.is_recordable());
}
