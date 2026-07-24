use super::*;

#[test]
fn canonical_tuple_abi_size() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 16, "(int, bool) must be 16 bytes with ABI padding");
        assert_eq!(t.align, 8, "(int, bool) alignment must be 8");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_tuple_abi_size_reversed() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::BOOL, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 16, "(bool, int) must be 16 bytes with ABI padding");
        assert_eq!(t.align, 8);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_tuple_no_padding_needed() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::BOOL, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.size, 2, "(bool, bool) is 2 bytes — no padding");
        assert_eq!(t.align, 1);
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_struct_abi_size() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.size, 16, "struct(int, float) must be 16 bytes");
        assert_eq!(s.align, 8);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn canonical_struct_abi_padding() {
    let mut pool = Pool::new();
    let name_a = Name::new(0, 100);
    let name_b = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_a, Idx::BOOL), (name_b, Idx::INT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(
            s.size, 16,
            "struct(bool, int) must be 16 bytes with ABI padding"
        );
        assert_eq!(s.align, 8);
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn canonical_map_retains_value_repr() {
    let mut pool = Pool::new();
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let repr = canonical(&pool, map_idx);
    if let MachineRepr::FatPointer(FatRepr::Map {
        ref key_repr,
        ref value_repr,
    }) = repr
    {
        assert_eq!(**key_repr, MachineRepr::FatPointer(FatRepr::Str));
        assert_eq!(
            **value_repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
    } else {
        panic!("expected FatPointer(Map), got {repr:?}");
    }
}

#[test]
fn canonical_recursive_enum_no_stack_overflow() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let tree_name = Name::new(0, 400);
    let leaf_name = Name::new(0, 401);
    let node_name = Name::new(0, 402);

    let tree_named = pool.named(tree_name);

    let tree_enum = pool.enum_type(
        tree_name,
        &[
            EnumVariant {
                name: leaf_name,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: node_name,
                field_types: vec![tree_named, tree_named],
            },
        ],
    );

    pool.set_resolution(tree_named, tree_enum);

    let repr = canonical(&pool, tree_enum);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].fields.len(), 1);
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(e.variants[1].fields.len(), 2);
        assert!(
            matches!(e.variants[1].fields[0], MachineRepr::RcPointer(_)),
            "recursive field must be RcPointer, got {:?}",
            e.variants[1].fields[0]
        );
        assert!(
            matches!(e.variants[1].fields[1], MachineRepr::RcPointer(_)),
            "recursive field must be RcPointer, got {:?}",
            e.variants[1].fields[1]
        );
    } else {
        panic!("expected Enum for recursive Tree, got {repr:?}");
    }
}

#[test]
fn semantic_pin_recursive_field_is_rc_pointer() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let list_name = Name::new(0, 500);
    let nil_name = Name::new(0, 501);
    let cons_name = Name::new(0, 502);
    let list_named = pool.named(list_name);

    let list_enum = pool.enum_type(
        list_name,
        &[
            EnumVariant {
                name: nil_name,
                field_types: vec![],
            },
            EnumVariant {
                name: cons_name,
                field_types: vec![Idx::INT, list_named],
            },
        ],
    );
    pool.set_resolution(list_named, list_enum);

    let repr = canonical(&pool, list_enum);
    if let MachineRepr::Enum(ref e) = repr {
        let cons = &e.variants[1];
        assert_eq!(cons.fields.len(), 2);
        if let MachineRepr::RcPointer(ref rc) = cons.fields[1] {
            assert_eq!(rc.rc_width, IntWidth::I64);
            assert!(rc.atomic);
            assert!(!rc.stack_promotable);
        } else {
            panic!(
                "recursive field must be RcPointer, got {:?}",
                cons.fields[1]
            );
        }
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

#[test]
fn canonical_mutual_recursion_consistent() {
    let mut pool = Pool::new();

    let a_name = Name::new(0, 600);
    let b_name = Name::new(0, 601);
    let a_named = pool.named(a_name);
    let b_named = pool.named(b_name);

    let b_field_name = Name::new(0, 602);
    let a_field_name = Name::new(0, 603);

    let a_struct = pool.struct_type(a_name, &[(b_field_name, b_named)]);
    let b_struct = pool.struct_type(b_name, &[(a_field_name, a_named)]);

    pool.set_resolution(a_named, a_struct);
    pool.set_resolution(b_named, b_struct);

    let mut cache = rustc_hash::FxHashMap::default();
    let Some(a_repr) = crate::canonical::canonical_cached(&pool, a_struct, &mut cache) else {
        panic!("A should canonicalize");
    };
    let Some(b_repr) = crate::canonical::canonical_cached(&pool, b_struct, &mut cache) else {
        panic!("B should canonicalize");
    };

    let MachineRepr::Struct(ref a_s) = a_repr else {
        panic!("expected Struct for A, got {a_repr:?}");
    };
    let MachineRepr::Struct(ref b_s) = b_repr else {
        panic!("expected Struct for B, got {b_repr:?}");
    };

    assert_eq!(a_s.fields.len(), 1, "A should have 1 field");
    assert_eq!(b_s.fields.len(), 1, "B should have 1 field");

    assert!(
        matches!(a_s.fields[0].repr, MachineRepr::Struct(_)),
        "A's B field should be full Struct (first visit), got {:?}",
        a_s.fields[0].repr
    );
    assert!(
        matches!(b_s.fields[0].repr, MachineRepr::RcPointer(_)),
        "B's A field should be RcPointer (back-edge), got {:?}",
        b_s.fields[0].repr
    );

    let b_inside_a = &a_s.fields[0].repr;
    assert_eq!(
        b_inside_a, &b_repr,
        "B nested inside A must equal standalone B (cache consistency)"
    );

    let Some(a_repr2) = crate::canonical::canonical_cached(&pool, a_struct, &mut cache) else {
        panic!("cached A should canonicalize");
    };
    assert_eq!(a_repr, a_repr2, "cached result must be stable");
}

#[test]
fn canonical_returns_none_for_struct_with_error_child() {
    let mut pool = Pool::new();
    let field_name = Name::new(0, 42);
    let struct_idx = pool.struct_type(Name::new(0, 100), &[(field_name, Idx::ERROR)]);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, struct_idx, &mut cache).is_none(),
        "struct with Error child must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_option_of_error() {
    let mut pool = Pool::new();
    let option_idx = pool.option(Idx::ERROR);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, option_idx, &mut cache).is_none(),
        "Option<Error> must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_list_of_error() {
    let mut pool = Pool::new();
    let list_idx = pool.list(Idx::ERROR);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, list_idx, &mut cache).is_none(),
        "[Error] must return None, not panic"
    );
}

#[test]
fn populate_canonical_no_panics_with_error_types() {
    use crate::plan::NarrowingPolicy;

    let mut pool = Pool::new();
    let _list_int = pool.list(Idx::INT);
    let _option_str = pool.option(Idx::STR);
    let _list_error = pool.list(Idx::ERROR);
    let _var = pool.fresh_var();

    let plan = crate::compute_repr_plan(&pool, &[], NarrowingPolicy::Aggressive, &[]);

    assert!(
        plan.get_repr(Idx::INT).is_some(),
        "Int should have a canonical repr"
    );
}

#[test]
fn canonical_non_recursive_repeated_type() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::INT);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.elements.len(), 2);
        assert!(
            matches!(t.elements[0].repr, MachineRepr::Int { .. }),
            "repeated non-recursive type must not be RcPointer"
        );
        assert!(
            matches!(t.elements[1].repr, MachineRepr::Int { .. }),
            "repeated non-recursive type must not be RcPointer"
        );
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}
