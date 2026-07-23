use super::*;

#[test]
fn canonical_named_resolves_to_struct() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);

    let named_idx = pool.named(Name::new(0, 42));
    pool.set_resolution(named_idx, struct_idx);

    let repr = canonical(&pool, named_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.fields.len(), 2, "Named→Struct must resolve to 2 fields");
        assert_eq!(s.fields[0].name, name_x);
        assert_eq!(s.fields[1].name, name_y);
        assert!(s.trivial, "struct of (int, float) must be trivial");
        assert_eq!(s.size, 16);
    } else {
        panic!("expected Struct for Named→Struct, got {repr:?}");
    }
}

#[test]
fn trivial_struct_containing_all_unit_enum() {
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
            EnumVariant {
                name: Name::new(0, 303),
                field_types: vec![],
            },
        ],
    );
    let name_e = Name::new(0, 100);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_e, enum_idx)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert!(s.trivial, "struct containing all-unit enum must be trivial");
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn canonical_returns_none_for_self_type() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let self_idx = pool.intern(Tag::SelfType, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, self_idx, &mut cache).is_none(),
        "SelfType must return None, not panic"
    );
}

#[test]
fn canonical_fat_pointer_variants_cover_str_list_and_map() {
    let mut pool = Pool::new();

    let str_repr = canonical(&pool, Idx::STR);
    assert!(
        matches!(str_repr, MachineRepr::FatPointer(FatRepr::Str)),
        "str must be FatPointer(Str)"
    );

    let list_idx = pool.list(Idx::INT);
    let list_repr = canonical(&pool, list_idx);
    assert!(
        matches!(
            list_repr,
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "list must be FatPointer(Collection)"
    );

    let map_idx = pool.map(Idx::STR, Idx::INT);
    let map_repr = canonical(&pool, map_idx);
    assert!(
        matches!(map_repr, MachineRepr::FatPointer(FatRepr::Map { .. })),
        "map must be FatPointer(Map)"
    );
}

#[test]
fn canonical_container_representations() {
    let mut pool = Pool::new();

    let list_idx = pool.list(Idx::INT);
    assert!(
        matches!(
            canonical(&pool, list_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "List canonical"
    );
    let opt_idx = pool.option(Idx::INT);
    assert!(
        matches!(canonical(&pool, opt_idx), MachineRepr::Enum(_)),
        "Option canonical"
    );
    let set_idx = pool.set(Idx::STR);
    assert!(
        matches!(
            canonical(&pool, set_idx),
            MachineRepr::FatPointer(FatRepr::Collection { .. })
        ),
        "Set canonical"
    );
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(
        canonical(&pool, chan_idx),
        MachineRepr::OpaquePtr,
        "Channel canonical"
    );
    let range_idx = pool.range(Idx::INT);
    assert_eq!(
        canonical(&pool, range_idx),
        MachineRepr::Range,
        "Range canonical"
    );
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, iter_idx),
        MachineRepr::UnmanagedPtr,
        "Iterator canonical"
    );
    let deiter_idx = pool.double_ended_iterator(Idx::INT);
    assert_eq!(
        canonical(&pool, deiter_idx),
        MachineRepr::UnmanagedPtr,
        "DoubleEndedIterator"
    );

    let map_idx = pool.map(Idx::STR, Idx::INT);
    assert!(
        matches!(
            canonical(&pool, map_idx),
            MachineRepr::FatPointer(FatRepr::Map { .. })
        ),
        "Map canonical"
    );
    let result_idx = pool.result(Idx::INT, Idx::STR);
    assert!(
        matches!(canonical(&pool, result_idx), MachineRepr::Enum(_)),
        "Result canonical"
    );
}

#[test]
fn canonical_complex_and_resolved_representations() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();

    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, fn_idx), MachineRepr::Closure(_)),
        "Function canonical"
    );
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    assert!(
        matches!(canonical(&pool, tuple_idx), MachineRepr::Tuple(_)),
        "Tuple canonical"
    );
    let struct_name = Name::new(0, 500);
    let struct_idx = pool.struct_type(struct_name, &[(Name::new(0, 501), Idx::INT)]);
    assert!(
        matches!(canonical(&pool, struct_idx), MachineRepr::Struct(_)),
        "Struct canonical"
    );
    let enum_idx = pool.enum_type(
        Name::new(0, 600),
        &[
            EnumVariant {
                name: Name::new(0, 601),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 602),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert!(
        matches!(canonical(&pool, enum_idx), MachineRepr::Enum(_)),
        "Enum canonical"
    );

    let named_idx = pool.named(Name::new(0, 700));
    pool.set_resolution(named_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, named_idx), MachineRepr::Struct(_)),
        "Named→Struct"
    );
    let applied_idx = pool.applied(Name::new(0, 800), &[Idx::INT]);
    pool.set_resolution(applied_idx, struct_idx);
    assert!(
        matches!(canonical(&pool, applied_idx), MachineRepr::Struct(_)),
        "Applied→Struct"
    );
    let alias_named = pool.named(Name::new(0, 900));
    pool.set_resolution(alias_named, Idx::INT);
    assert_eq!(
        canonical(&pool, alias_named),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Alias→Int canonical"
    );
}

#[test]
fn canonical_zst_aggregate_layouts() {
    let mut pool = Pool::new();

    let opt_unit = pool.option(Idx::UNIT);
    if let MachineRepr::Enum(ref e) = canonical(&pool, opt_unit) {
        assert_eq!(
            e.size, 8,
            "Option<()> = 8 bytes (i64 tag, not narrowed for runtime compat)"
        );
    } else {
        panic!("Option<()> must be Enum");
    }

    let tup_unit_bool = pool.pair(Idx::UNIT, Idx::BOOL);
    if let MachineRepr::Tuple(ref t) = canonical(&pool, tup_unit_bool) {
        assert_eq!(
            t.size, 1,
            "((), bool) = 1 byte — Unit zero-sized in aggregates"
        );
    } else {
        panic!("((), bool) must be Tuple");
    }

    let result_unit_int = pool.result(Idx::UNIT, Idx::INT);
    if let MachineRepr::Enum(ref e) = canonical(&pool, result_unit_int) {
        assert_eq!(e.size, 16, "Result<(), int> = 16 bytes");
    } else {
        panic!("Result<(), int> must be Enum");
    }

    let struct_unit_idx = pool.struct_type(
        Name::new(0, 1000),
        &[
            (Name::new(0, 1001), Idx::UNIT),
            (Name::new(0, 1002), Idx::INT),
        ],
    );
    if let MachineRepr::Struct(ref s) = canonical(&pool, struct_unit_idx) {
        assert_eq!(s.size, 8, "Struct(unit, int) = 8 bytes — Unit zero-sized");
    } else {
        panic!("Struct with Unit field must be Struct");
    }
}
