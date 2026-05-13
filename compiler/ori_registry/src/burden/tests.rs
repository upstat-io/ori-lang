//! Pure-const compliance tests for the burden module.

use super::*;

#[test]
fn builtin_burden_spec_size_fits_in_64_bytes() {
    // Compile-time assertion in mod.rs guarantees <=64; this re-asserts at
    // runtime for visibility in test output.
    assert!(core::mem::size_of::<BuiltinBurdenSpec>() <= 64);
}

#[test]
fn burden_field_types_implement_copy_trait() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<OwnedField>();
    assert_copy::<BorrowedField>();
    assert_copy::<TransferRule>();
    assert_copy::<VariantBurden>();
    assert_copy::<TransferKind>();
    assert_copy::<TypeId>();
    assert_copy::<VariantId>();
    assert_copy::<FnSym>();
}

#[test]
fn transfer_kind_has_two_distinct_variants() {
    let kinds = [TransferKind::Move, TransferKind::BorrowOnly];
    assert_eq!(kinds.len(), 2);
    assert_ne!(kinds[0], kinds[1]);
}

#[test]
fn empty_builtin_burden_spec_is_const_constructible() {
    const EMPTY: BuiltinBurdenSpec = BuiltinBurdenSpec::EMPTY;
    const { assert!(!EMPTY.self_heap_alloc) };
    const { assert!(EMPTY.owned_fields.is_empty()) };
    const { assert!(EMPTY.borrowed_fields.is_empty()) };
    const { assert!(EMPTY.variant_burdens.is_empty()) };
    const { assert!(EMPTY.element_burden.is_none()) };
    const { assert!(EMPTY.compiled_drop.is_none()) };
    const { assert!(EMPTY.user_drop.is_none()) };
}

#[test]
fn builtin_burden_spec_const_construction_with_non_empty_data() {
    use core::num::NonZeroU32;

    // Verify non-empty const data is constructible from &'static slices.
    const FORTY_TWO: NonZeroU32 = match NonZeroU32::new(42) {
        Some(n) => n,
        None => unreachable!(),
    };
    const SEVEN: NonZeroU32 = match NonZeroU32::new(7) {
        Some(n) => n,
        None => unreachable!(),
    };
    const NINETY_NINE: NonZeroU32 = match NonZeroU32::new(99) {
        Some(n) => n,
        None => unreachable!(),
    };
    const OWNED: &[OwnedField] = &[OwnedField {
        field_path: &[0, 1],
        field_type: TypeId::new(FORTY_TWO),
    }];
    const SPEC: BuiltinBurdenSpec = BuiltinBurdenSpec {
        self_heap_alloc: true,
        owned_fields: OWNED,
        element_burden: Some(TypeId::new(SEVEN)),
        compiled_drop: Some(FnSym::new(NINETY_NINE)),
        ..BuiltinBurdenSpec::EMPTY
    };
    const { assert!(SPEC.self_heap_alloc) };
    assert_eq!(SPEC.owned_fields.len(), 1);
    assert_eq!(SPEC.owned_fields[0].field_path, &[0, 1]);
    assert_eq!(SPEC.owned_fields[0].field_type, TypeId::new(FORTY_TWO));
    assert_eq!(SPEC.element_burden, Some(TypeId::new(SEVEN)));
    assert_eq!(SPEC.compiled_drop, Some(FnSym::new(NINETY_NINE)));
}
