use super::*;
use ori_repr::{EnumTag, IntWidth};

// TagEncoding from Explicit { I64 } (backward compat baseline)

#[test]
fn explicit_i64_variant_to_tag_value() {
    let enc = TagEncoding::new(
        EnumTag::Explicit {
            width: IntWidth::I64,
        },
        3,
    );
    assert_eq!(enc.variant_to_tag_value(0), 0);
    assert_eq!(enc.variant_to_tag_value(1), 1);
    assert_eq!(enc.variant_to_tag_value(2), 2);
}

#[test]
fn explicit_i64_needs_tag_store() {
    let enc = TagEncoding::new(
        EnumTag::Explicit {
            width: IntWidth::I64,
        },
        3,
    );
    assert!(enc.needs_tag_store(0));
    assert!(enc.needs_tag_store(1));
    assert!(enc.needs_tag_store(2));
}

// TagEncoding from Niche

#[test]
fn niche_variant_to_tag_value_niche_variant() {
    // Option<bool>: variant 0 = Some, variant 1 = None (niche)
    let enc = TagEncoding::new(
        EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 1,
        },
        2,
    );
    // Niche variant (last) returns the niche value
    assert_eq!(enc.variant_to_tag_value(1), 2);
    // Non-niche variant: identity
    assert_eq!(enc.variant_to_tag_value(0), 0);
}

#[test]
fn niche_needs_tag_store() {
    let enc = TagEncoding::new(
        EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 1,
        },
        2,
    );
    // Only the niche variant (last) needs a tag store
    assert!(!enc.needs_tag_store(0)); // Some — payload write is enough
    assert!(enc.needs_tag_store(1)); // None — must write niche value
}

#[test]
fn niche_field_and_value() {
    let enc = TagEncoding::new(
        EnumTag::Niche {
            field_index: 2,
            niche_value: 0,
            niche_variant_idx: 1,
        },
        2,
    );
    assert_eq!(enc.niche_field_index(), Some(2));
    assert_eq!(enc.niche_value(), Some(0));
}

// Niche at index 0 (Option<bool> with type-checker ordering: None=0, Some=1)

#[test]
fn niche_at_index_zero_variant_to_tag_value() {
    let enc = TagEncoding::new(
        EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 0,
        },
        2,
    );
    assert_eq!(enc.variant_to_tag_value(0), 2);
    assert_eq!(enc.variant_to_tag_value(1), 1);
}

#[test]
fn niche_at_index_zero_needs_tag_store() {
    let enc = TagEncoding::new(
        EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 0,
        },
        2,
    );
    assert!(enc.needs_tag_store(0));
    assert!(!enc.needs_tag_store(1));
}

#[test]
fn niche_at_index_zero_accessors() {
    let enc = TagEncoding::new(
        EnumTag::Niche {
            field_index: 0,
            niche_value: 2,
            niche_variant_idx: 0,
        },
        2,
    );
    assert_eq!(enc.niche_variant_idx(), Some(0));
    assert_eq!(enc.niche_field_index(), Some(0));
    assert_eq!(enc.niche_value(), Some(2));
}

// TagEncoding from None (single-variant / newtype erasure)

#[test]
fn none_needs_tag_store_is_false() {
    let enc = TagEncoding::new(EnumTag::None, 1);
    assert!(!enc.needs_tag_store(0));
}

#[test]
fn none_variant_to_tag_value() {
    let enc = TagEncoding::new(EnumTag::None, 1);
    assert_eq!(enc.variant_to_tag_value(0), 0);
}

// from_enum_repr integration

#[test]
fn from_enum_repr_explicit() {
    use ori_ir::Name;
    use ori_repr::{EnumRepr, VariantRepr};

    let repr = EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![
            VariantRepr {
                name: Name::from_raw(1),
                fields: vec![],
                size: 0,
                alignment: 1,
            },
            VariantRepr {
                name: Name::from_raw(2),
                fields: vec![],
                size: 0,
                alignment: 1,
            },
        ],
        size: 8,
        align: 8,
    };
    let enc = TagEncoding::from_enum_repr(&repr);
    assert_eq!(enc.variant_to_tag_value(1), 1);
    assert!(enc.needs_tag_store(1));
}
