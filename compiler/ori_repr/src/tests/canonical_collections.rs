use super::*;

#[test]
fn canonical_list_int() {
    let mut pool = Pool::new();
    let list_idx = pool.list(Idx::INT);
    let repr = canonical(&pool, list_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            })
        })
    );
}

#[test]
fn canonical_set_str() {
    let mut pool = Pool::new();
    let set_idx = pool.set(Idx::STR);
    let repr = canonical(&pool, set_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Collection {
            element_repr: Box::new(MachineRepr::FatPointer(FatRepr::Str))
        })
    );
}

#[test]
fn canonical_map() {
    let mut pool = Pool::new();
    let map_idx = pool.map(Idx::STR, Idx::INT);
    let repr = canonical(&pool, map_idx);
    assert_eq!(
        repr,
        MachineRepr::FatPointer(FatRepr::Map {
            key_repr: Box::new(MachineRepr::FatPointer(FatRepr::Str)),
            value_repr: Box::new(MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }),
        })
    );
}

#[test]
fn canonical_range() {
    let mut pool = Pool::new();
    let range_idx = pool.range(Idx::INT);
    assert_eq!(canonical(&pool, range_idx), MachineRepr::Range);
}

#[test]
fn canonical_iterator() {
    let mut pool = Pool::new();
    let iter_idx = pool.iterator(Idx::INT);
    assert_eq!(canonical(&pool, iter_idx), MachineRepr::UnmanagedPtr);
}

#[test]
fn canonical_channel() {
    let mut pool = Pool::new();
    let chan_idx = pool.channel(Idx::INT);
    assert_eq!(canonical(&pool, chan_idx), MachineRepr::OpaquePtr);
}

#[test]
fn canonical_option_int() {
    let mut pool = Pool::new();
    let opt_idx = pool.option(Idx::INT);
    let repr = canonical(&pool, opt_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2, "Option should have 2 variants");
        assert_eq!(
            e.tag,
            EnumTag::Explicit {
                width: IntWidth::I64
            }
        );
        assert_eq!(e.variants[0].fields.len(), 1);
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert!(e.variants[1].fields.is_empty());
    } else {
        panic!("expected Enum for Option<int>, got {repr:?}");
    }
}

#[test]
fn canonical_result() {
    let mut pool = Pool::new();
    let result_idx = pool.result(Idx::INT, Idx::STR);
    let repr = canonical(&pool, result_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2, "Result should have 2 variants");
        assert_eq!(
            e.variants[0].fields[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(
            e.variants[1].fields[0],
            MachineRepr::FatPointer(FatRepr::Str)
        );
    } else {
        panic!("expected Enum for Result<int, str>, got {repr:?}");
    }
}

#[test]
fn canonical_function() {
    let mut pool = Pool::new();
    let fn_idx = pool.function1(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, fn_idx);
    if let MachineRepr::Closure(ref c) = repr {
        assert_eq!(c.params.len(), 1);
        assert_eq!(
            c.params[0],
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(*c.ret, MachineRepr::Bool);
    } else {
        panic!("expected Closure for function, got {repr:?}");
    }
}

#[test]
fn canonical_tuple() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::BOOL);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert_eq!(t.elements.len(), 2);
        assert_eq!(
            t.elements[0].repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true
            }
        );
        assert_eq!(t.elements[1].repr, MachineRepr::Bool);
        assert!(t.trivial, "tuple of int and bool should be trivial");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_tuple_nontrivial() {
    let mut pool = Pool::new();
    let tuple_idx = pool.pair(Idx::INT, Idx::STR);
    let repr = canonical(&pool, tuple_idx);
    if let MachineRepr::Tuple(ref t) = repr {
        assert!(!t.trivial, "(int, str) should NOT be trivial");
    } else {
        panic!("expected Tuple, got {repr:?}");
    }
}

#[test]
fn canonical_struct() {
    let mut pool = Pool::new();
    let name_x = Name::new(0, 100);
    let name_y = Name::new(0, 101);
    let struct_name = Name::new(0, 200);
    let struct_idx = pool.struct_type(struct_name, &[(name_x, Idx::INT), (name_y, Idx::FLOAT)]);
    let repr = canonical(&pool, struct_idx);
    if let MachineRepr::Struct(ref s) = repr {
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, name_x);
        assert_eq!(s.fields[1].name, name_y);
        assert_eq!(s.fields[0].original_index, 0);
        assert_eq!(s.fields[1].original_index, 1);
        assert!(s.trivial, "struct of int and float should be trivial");
    } else {
        panic!("expected Struct, got {repr:?}");
    }
}

#[test]
fn canonical_enum() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let enum_name = Name::new(0, 300);
    let a_name = Name::new(0, 301);
    let b_name = Name::new(0, 302);
    let enum_idx = pool.enum_type(
        enum_name,
        &[
            EnumVariant {
                name: a_name,
                field_types: vec![],
            },
            EnumVariant {
                name: b_name,
                field_types: vec![Idx::INT],
            },
        ],
    );
    let repr = canonical(&pool, enum_idx);
    if let MachineRepr::Enum(ref e) = repr {
        assert_eq!(e.variants.len(), 2);
        assert_eq!(
            e.tag,
            EnumTag::Explicit {
                width: IntWidth::I8
            }
        );
        assert!(e.variants[0].fields.is_empty());
        assert_eq!(e.variants[1].fields.len(), 1);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

#[test]
fn canonical_returns_none_for_var() {
    let mut pool = Pool::new();
    let var_idx = pool.fresh_var();
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, var_idx, &mut cache).is_none(),
        "Var must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_bound_var() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let bound_var_idx = pool.intern(Tag::BoundVar, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, bound_var_idx, &mut cache).is_none(),
        "BoundVar must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_rigid_var() {
    let mut pool = Pool::new();
    let rigid = pool.rigid_var(Name::new(0, 999));
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, rigid, &mut cache).is_none(),
        "RigidVar must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_error() {
    let pool = Pool::new();
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, Idx::ERROR, &mut cache).is_none(),
        "Error must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_scheme() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let scheme_idx = pool.scheme(&[0], Idx::INT);
    assert_eq!(pool.tag(scheme_idx), Tag::Scheme);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, scheme_idx, &mut cache).is_none(),
        "Scheme must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_infer() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let infer_idx = pool.intern(Tag::Infer, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, infer_idx, &mut cache).is_none(),
        "Infer must return None, not panic"
    );
}

#[test]
fn canonical_named_resolves_to_int() {
    let mut pool = Pool::new();
    let named_idx = pool.named(Name::new(0, 42));
    pool.set_resolution(named_idx, Idx::INT);

    let repr = canonical(&pool, named_idx);
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Named→Int must resolve to same repr as Int"
    );
}

#[test]
fn canonical_alias_chain_resolves() {
    let mut pool = Pool::new();
    let a_idx = pool.named(Name::new(0, 100));
    let b_idx = pool.named(Name::new(0, 200));
    pool.set_resolution(a_idx, b_idx);
    pool.set_resolution(b_idx, Idx::INT);

    let repr = canonical(&pool, a_idx);
    assert_eq!(
        repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true
        },
        "Named chain A→B→Int must resolve to Int"
    );
}
