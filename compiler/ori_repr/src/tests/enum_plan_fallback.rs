use super::*;

#[test]
fn canonical_single_variant_enum_is_tagless() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 400);
    let variant_name = Name::new(0, 401);
    let enum_idx = pool.enum_type(
        enum_name,
        &[EnumVariant {
            name: variant_name,
            field_types: vec![Idx::INT],
        }],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 1);
        assert_eq!(
            e.tag,
            EnumTag::None,
            "single-variant enum should be tagless"
        );
        assert!(e.tag.is_tagless());
        assert!(!e.tag.needs_tag_field());
        assert_eq!(e.tag.payload_gep_index(), 0);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

#[test]
fn canonical_single_variant_unit_enum_is_tagless() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 410);
    let variant_name = Name::new(0, 411);
    let enum_idx = pool.enum_type(
        enum_name,
        &[EnumVariant {
            name: variant_name,
            field_types: vec![],
        }],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.tag, EnumTag::None);
        assert_eq!(e.size, 1);
        assert_eq!(e.align, 1);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

#[test]
fn enum_repr_with_fallback_plan_miss_recomputes_canonical_option() {
    use ori_types::Pool;

    let mut pool = Pool::new();
    let opt_str = pool.option(ori_types::Idx::STR);

    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan.enum_repr(opt_str).is_none());

    let via_ladder = plan
        .enum_repr_with_fallback(&pool, opt_str)
        .unwrap_or_else(|| panic!("fallback must recompute canonical for Option<str>"));
    let canonical = crate::canonical_enum_for_type(&pool, opt_str)
        .unwrap_or_else(|| panic!("canonical_enum_for_type must cover Option<str>"));
    assert_eq!(*via_ladder, canonical);
}

#[test]
fn enum_repr_with_fallback_non_enum_type_returns_none() {
    use ori_types::Pool;

    let pool = Pool::new();
    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    assert!(plan
        .enum_repr_with_fallback(&pool, ori_types::Idx::INT)
        .is_none());
}
