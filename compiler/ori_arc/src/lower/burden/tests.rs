//! Burden trait dispatch parity tests.

use core::num::NonZeroU32;

use ori_registry::burden::{
    BorrowedField, BuiltinBurdenSpec, FnSym, OwnedField, TransferKind, TransferRule, TypeId,
    VariantBurden, VariantId,
};
use ori_types::burden::{UserBurdenSpec, UserOwnedField, UserTransferRule, UserVariantBurden};
use ori_types::Idx;

use super::{Burden, BurdenRef, TypeRef};

const TID_7: TypeId = match NonZeroU32::new(7) {
    Some(n) => TypeId::new(n),
    None => unreachable!(),
};
const TID_42: TypeId = match NonZeroU32::new(42) {
    Some(n) => TypeId::new(n),
    None => unreachable!(),
};
const FN_3: FnSym = match NonZeroU32::new(3) {
    Some(n) => FnSym::new(n),
    None => unreachable!(),
};
const VID_1: VariantId = match NonZeroU32::new(1) {
    Some(n) => VariantId::new(n),
    None => unreachable!(),
};

const BUILTIN_OWNED: &[OwnedField] = &[OwnedField {
    field_path: &[0],
    field_type: TID_7,
}];
const BUILTIN_BORROWED: &[BorrowedField] = &[];
const BUILTIN_VARIANTS: &[VariantBurden] = &[];
const BUILTIN_SPEC: BuiltinBurdenSpec = BuiltinBurdenSpec {
    self_heap_alloc: true,
    owned_fields: BUILTIN_OWNED,
    borrowed_fields: BUILTIN_BORROWED,
    variant_burdens: BUILTIN_VARIANTS,
    element_burden: Some(TID_42),
    compiled_drop: Some(FN_3),
    ..BuiltinBurdenSpec::EMPTY
};

fn make_user_spec() -> UserBurdenSpec {
    UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        element_burden: Some(Idx::INT),
        compiled_drop: Some(FN_3),
        ..UserBurdenSpec::default()
    }
}

#[test]
fn test_type_ref_variants() {
    let b = TypeRef::Builtin(TID_7);
    let u = TypeRef::User(Idx::INT);
    assert_ne!(b, u);
    match b {
        TypeRef::Builtin(id) => assert_eq!(id, TID_7),
        TypeRef::User(_) => panic!("wrong variant"),
    }
    match u {
        TypeRef::User(id) => assert_eq!(id, Idx::INT),
        TypeRef::Builtin(_) => panic!("wrong variant"),
    }
}

#[test]
fn test_burden_ref_builtin_dispatches_to_static() {
    let r = BurdenRef::Builtin(&BUILTIN_SPEC);
    assert!(r.self_heap_alloc());
    let owned: Vec<_> = r.owned_fields().collect();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].field_path.as_ref(), &[0u32]);
    assert_eq!(owned[0].field_type, TypeRef::Builtin(TID_7));
}

#[test]
fn test_burden_ref_user_dispatches_to_heap() {
    let spec = make_user_spec();
    let r = BurdenRef::User(&spec);
    assert!(r.self_heap_alloc());
    let owned: Vec<_> = r.owned_fields().collect();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].field_path.as_ref(), &[0u32]);
    assert_eq!(owned[0].field_type, TypeRef::User(Idx::STR));
}

#[test]
fn test_burden_ref_self_heap_alloc_dispatch() {
    static EMPTY_BUILTIN: BuiltinBurdenSpec = BuiltinBurdenSpec::EMPTY;
    let user_spec = make_user_spec();
    let builtin_ref = BurdenRef::Builtin(&BUILTIN_SPEC);
    let user_ref = BurdenRef::User(&user_spec);
    assert_eq!(builtin_ref.self_heap_alloc(), user_ref.self_heap_alloc());

    let r = BurdenRef::Builtin(&EMPTY_BUILTIN);
    // EMPTY constant in static; trait method delegates to wrapped spec.
    assert!(!r.self_heap_alloc());
}

#[test]
fn test_burden_ref_dispatch_parity() {
    // Structurally-equivalent shape: one owned field at path [0], element burden present.
    let user_spec = make_user_spec();
    let builtin_ref = BurdenRef::Builtin(&BUILTIN_SPEC);
    let user_ref = BurdenRef::User(&user_spec);

    let b_owned: Vec<_> = builtin_ref.owned_fields().collect();
    let u_owned: Vec<_> = user_ref.owned_fields().collect();
    assert_eq!(b_owned.len(), u_owned.len());
    assert_eq!(
        b_owned[0].field_path.as_ref(),
        u_owned[0].field_path.as_ref()
    );

    // element_burden present on both sides — different TypeRef variants.
    assert!(builtin_ref.element_burden().is_some());
    assert!(user_ref.element_burden().is_some());

    // compiled_drop is shared FnSym type (re-exported from ori_registry).
    assert_eq!(builtin_ref.compiled_drop(), user_ref.compiled_drop());
    assert_eq!(builtin_ref.user_drop(), user_ref.user_drop());
    assert!(builtin_ref.user_drop().is_none());
}

#[test]
fn test_burden_ref_variant_burdens_dispatch() {
    static OWNED_STATIC: &[OwnedField] = &[OwnedField {
        field_path: &[1],
        field_type: TID_7,
    }];
    static TRANSFER_STATIC: &[TransferRule] = &[TransferRule {
        source_field_path: &[0],
        binding_index: 0,
        field_type: TID_42,
        transfer_kind: TransferKind::Move,
    }];
    static VARIANT_STATIC: &[VariantBurden] = &[VariantBurden {
        variant_id: VID_1,
        transfers_on_match: TRANSFER_STATIC,
        retained_owned: OWNED_STATIC,
    }];
    static BUILTIN_WITH_VARIANTS: BuiltinBurdenSpec = BuiltinBurdenSpec {
        variant_burdens: VARIANT_STATIC,
        ..BuiltinBurdenSpec::EMPTY
    };

    let user_spec = UserBurdenSpec {
        variant_burdens: vec![UserVariantBurden {
            variant_id: VID_1,
            transfers_on_match: vec![UserTransferRule {
                source_field_path: vec![0],
                binding_index: 0,
                field_type: Idx::INT,
                transfer_kind: TransferKind::Move,
            }],
            retained_owned: vec![UserOwnedField {
                field_path: vec![1],
                field_type: Idx::STR,
            }],
        }],
        ..UserBurdenSpec::default()
    };

    let builtin_ref = BurdenRef::Builtin(&BUILTIN_WITH_VARIANTS);
    let user_ref = BurdenRef::User(&user_spec);

    let b_variants: Vec<_> = builtin_ref.variant_burdens().collect();
    let u_variants: Vec<_> = user_ref.variant_burdens().collect();

    assert_eq!(b_variants.len(), 1);
    assert_eq!(u_variants.len(), 1);
    assert_eq!(b_variants[0].variant_id, u_variants[0].variant_id);
    assert_eq!(
        b_variants[0].transfers_on_match.len(),
        u_variants[0].transfers_on_match.len()
    );
    assert_eq!(
        b_variants[0].retained_owned.len(),
        u_variants[0].retained_owned.len()
    );
    assert_eq!(
        b_variants[0].transfers_on_match[0].transfer_kind,
        u_variants[0].transfers_on_match[0].transfer_kind
    );
}

#[test]
fn test_burden_ref_empty_owned_fields_parity() {
    // Matrix cell: both sides have zero owned_fields. Verify both yield
    // empty iterators without panicking.
    static EMPTY_BUILTIN: BuiltinBurdenSpec = BuiltinBurdenSpec::EMPTY;
    let empty_user = UserBurdenSpec::default();
    let builtin_ref = BurdenRef::Builtin(&EMPTY_BUILTIN);
    let user_ref = BurdenRef::User(&empty_user);

    let b_owned: Vec<_> = builtin_ref.owned_fields().collect();
    let u_owned: Vec<_> = user_ref.owned_fields().collect();
    assert!(
        b_owned.is_empty(),
        "builtin empty owned_fields → empty view"
    );
    assert!(u_owned.is_empty(), "user empty owned_fields → empty view");
    assert_eq!(b_owned.len(), u_owned.len(), "parity: both zero");
}

#[test]
fn test_burden_ref_multi_field_parity() {
    // Matrix cell: both sides have >1 owned_fields with structurally-
    // equivalent shape. Verify iterator order + count parity.
    static OWNED_MULTI: &[OwnedField] = &[
        OwnedField {
            field_path: &[0],
            field_type: TID_7,
        },
        OwnedField {
            field_path: &[1],
            field_type: TID_42,
        },
        OwnedField {
            field_path: &[2],
            field_type: TID_7,
        },
    ];
    static BUILTIN_MULTI: BuiltinBurdenSpec = BuiltinBurdenSpec {
        self_heap_alloc: true,
        owned_fields: OWNED_MULTI,
        ..BuiltinBurdenSpec::EMPTY
    };
    let user_multi = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![
            UserOwnedField {
                field_path: vec![0],
                field_type: Idx::STR,
            },
            UserOwnedField {
                field_path: vec![1],
                field_type: Idx::INT,
            },
            UserOwnedField {
                field_path: vec![2],
                field_type: Idx::STR,
            },
        ],
        ..UserBurdenSpec::default()
    };
    let builtin_ref = BurdenRef::Builtin(&BUILTIN_MULTI);
    let user_ref = BurdenRef::User(&user_multi);

    let b_owned: Vec<_> = builtin_ref.owned_fields().collect();
    let u_owned: Vec<_> = user_ref.owned_fields().collect();
    assert_eq!(b_owned.len(), 3);
    assert_eq!(u_owned.len(), 3);
    for i in 0..3 {
        assert_eq!(
            b_owned[i].field_path.as_ref(),
            u_owned[i].field_path.as_ref(),
            "row {i} field_path parity"
        );
    }
}

#[test]
fn test_burden_ref_zero_arity_variant_burdens_parity() {
    // Matrix cell: variant present, payload empty (`None` / unit variant).
    // Verify zero-arity variants yield zero-length transfers + retained.
    static UNIT_VARIANT_STATIC: &[VariantBurden] = &[VariantBurden {
        variant_id: VID_1,
        transfers_on_match: &[],
        retained_owned: &[],
    }];
    static BUILTIN_UNIT_VARIANT: BuiltinBurdenSpec = BuiltinBurdenSpec {
        variant_burdens: UNIT_VARIANT_STATIC,
        ..BuiltinBurdenSpec::EMPTY
    };
    let user_unit_variant = UserBurdenSpec {
        variant_burdens: vec![UserVariantBurden {
            variant_id: VID_1,
            transfers_on_match: vec![],
            retained_owned: vec![],
        }],
        ..UserBurdenSpec::default()
    };
    let builtin_ref = BurdenRef::Builtin(&BUILTIN_UNIT_VARIANT);
    let user_ref = BurdenRef::User(&user_unit_variant);

    let b_variants: Vec<_> = builtin_ref.variant_burdens().collect();
    let u_variants: Vec<_> = user_ref.variant_burdens().collect();
    assert_eq!(b_variants.len(), 1);
    assert_eq!(u_variants.len(), 1);
    assert!(b_variants[0].transfers_on_match.is_empty());
    assert!(u_variants[0].transfers_on_match.is_empty());
    assert!(b_variants[0].retained_owned.is_empty());
    assert!(u_variants[0].retained_owned.is_empty());
    assert_eq!(b_variants[0].variant_id, u_variants[0].variant_id);
}
