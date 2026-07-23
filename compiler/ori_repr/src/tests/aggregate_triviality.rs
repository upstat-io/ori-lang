use super::*;

#[test]
fn canonical_tuple_unit_zero_sized() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::UNIT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 1,
            "(unit, bool) must be 1 byte — Unit is zero-sized"
        );
        assert_eq!(t.align, 1);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_tuple_unit_middle() {
    let mut pool = Pool::new();
    let tuple_idx = pool.triple(Idx::BOOL, Idx::UNIT, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 16,
            "(bool, unit, int) must be 16 — Unit contributes 0"
        );
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_struct_unit_field() {
    let mut pool = Pool::new();
    let name_a = Name::new(0, 100);
    let name_b = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_a, Idx::BOOL), (name_b, Idx::UNIT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.size, 1, "struct(bool, unit) must be 1 byte");
        assert_eq!(s.align, 1);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn canonical_option_unit_zero_payload() {
    let mut pool = Pool::new();
    let opt_idx = pool.option(Idx::UNIT);
    let repr = canonical(&pool, opt_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(
            e.size, 8,
            "Option<()> must be 8 bytes — i64 tag (not narrowed for runtime compat)"
        );
    } else {
        panic!("expected Enum for Option<()>, got {repr:?}");
    }
}

#[test]
fn canonical_tuple_never_zero_sized() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::NEVER);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(
            t.size, 8,
            "(int, Never) must be 8 bytes — Never is zero-sized"
        );
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn trivial_struct_containing_trivial_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let name_t = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_t, tuple_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing (int, bool) must be trivial — all scalars"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn nontrivial_struct_containing_nontrivial_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::STR);
    let name_t = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_t, tuple_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            !s.trivial,
            "struct containing (int, str) must NOT be trivial — str has RC"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn trivial_all_unit_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![],
            },
        ],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        let all_trivial = e.variants.iter().all(|v| {
            v.fields.iter().all(|f| {
                !matches!(
                    f,
                    MachineRepr::FatPointer(_)
                        | MachineRepr::RcPointer(_)
                        | MachineRepr::Closure(_)
                        | MachineRepr::OpaquePtr
                )
            })
        });
        assert!(all_trivial, "all-unit enum must be trivial");
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

#[test]
fn trivial_scalar_payload_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: Name::new(0, 301),
                field_types: vec![Idx::FLOAT],
            },
            EnumVariant {
                name: Name::new(0, 302),
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    let name_s = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_s, enum_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(
            s.trivial,
            "struct containing all-scalar enum must be trivial"
        );
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}
