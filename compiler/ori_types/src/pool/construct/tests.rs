use super::*;
use crate::TypeFlags;

#[test]
fn list_construction() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    assert_eq!(pool.tag(list_int), Tag::List);
    assert_eq!(Idx::from_raw(pool.data(list_int)), Idx::INT);
}

#[test]
fn option_construction() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);

    assert_eq!(pool.tag(opt_str), Tag::Option);
    assert_eq!(Idx::from_raw(pool.data(opt_str)), Idx::STR);
}

#[test]
fn map_construction() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);

    assert_eq!(pool.tag(map_ty), Tag::Map);
    assert_eq!(pool.map_key(map_ty), Idx::STR);
    assert_eq!(pool.map_value(map_ty), Idx::INT);
}

#[test]
fn result_construction() {
    let mut pool = Pool::new();
    let res_ty = pool.result(Idx::INT, Idx::STR);

    assert_eq!(pool.tag(res_ty), Tag::Result);
    assert_eq!(pool.result_ok(res_ty), Idx::INT);
    assert_eq!(pool.result_err(res_ty), Idx::STR);
}

#[test]
fn borrowed_construction() {
    let mut pool = Pool::new();
    let borrowed_ty = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);

    assert_eq!(pool.tag(borrowed_ty), Tag::Borrowed);
    assert_eq!(pool.borrowed_inner(borrowed_ty), Idx::INT);
    assert_eq!(
        pool.borrowed_lifetime(borrowed_ty),
        crate::LifetimeId::STATIC
    );
}

#[test]
fn borrowed_dedup() {
    let mut pool = Pool::new();
    let b1 = pool.borrowed(Idx::STR, crate::LifetimeId::STATIC);
    let b2 = pool.borrowed(Idx::STR, crate::LifetimeId::STATIC);

    assert_eq!(b1, b2);
}

#[test]
fn borrowed_different_lifetime() {
    let mut pool = Pool::new();
    let b_static = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);
    let b_scoped = pool.borrowed(Idx::INT, crate::LifetimeId::SCOPED);

    assert_ne!(b_static, b_scoped);
}

#[test]
fn function_construction() {
    let mut pool = Pool::new();
    let fn_ty = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);

    assert_eq!(pool.tag(fn_ty), Tag::Function);

    let params = pool.function_params(fn_ty);
    assert_eq!(params.len(), 2);
    assert_eq!(params[0], Idx::INT);
    assert_eq!(params[1], Idx::STR);

    assert_eq!(pool.function_return(fn_ty), Idx::BOOL);
}

#[test]
fn tuple_construction() {
    let mut pool = Pool::new();

    // Empty tuple is unit
    let empty = pool.tuple(&[]);
    assert_eq!(empty, Idx::UNIT);

    // Non-empty tuple
    let tuple = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
    assert_eq!(pool.tag(tuple), Tag::Tuple);

    let elems = pool.tuple_elems(tuple);
    assert_eq!(elems.len(), 3);
    assert_eq!(elems[0], Idx::INT);
    assert_eq!(elems[1], Idx::STR);
    assert_eq!(elems[2], Idx::BOOL);
}

#[test]
fn fresh_var_construction() {
    let mut pool = Pool::new();

    let var1 = pool.fresh_var();
    let var2 = pool.fresh_var();

    // Should be different variables
    assert_ne!(pool.data(var1), pool.data(var2));

    // Both should be Var type
    assert_eq!(pool.tag(var1), Tag::Var);
    assert_eq!(pool.tag(var2), Tag::Var);

    // Check var states
    match pool.var_state(pool.data(var1)) {
        VarState::Unbound { id, .. } => assert_eq!(*id, pool.data(var1)),
        _ => panic!("Expected Unbound"),
    }
}

#[test]
fn scheme_construction() {
    let mut pool = Pool::new();

    // Monomorphic body returns body unchanged
    let mono = pool.scheme(&[], Idx::INT);
    assert_eq!(mono, Idx::INT);

    // Polymorphic scheme
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    let fn_ty = pool.function(&[var], var);
    let scheme = pool.scheme(&[var_id], fn_ty);

    assert_eq!(pool.tag(scheme), Tag::Scheme);

    let vars = pool.scheme_vars(scheme);
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0], var_id);

    assert_eq!(pool.scheme_body(scheme), fn_ty);
}

#[test]
fn type_deduplication() {
    let mut pool = Pool::new();

    let list1 = pool.list(Idx::INT);
    let list2 = pool.list(Idx::INT);

    // Same type should return same index
    assert_eq!(list1, list2);

    // Pool size shouldn't increase
    let size_before = pool.len();
    let list3 = pool.list(Idx::INT);
    assert_eq!(pool.len(), size_before);
    assert_eq!(list1, list3);
}

#[test]
fn nested_type_construction() {
    let mut pool = Pool::new();

    // [[int]]
    let inner = pool.list(Idx::INT);
    let outer = pool.list(inner);

    assert_eq!(pool.tag(outer), Tag::List);
    assert_eq!(Idx::from_raw(pool.data(outer)), inner);
}

#[test]
fn applied_type_accessors() {
    let mut pool = Pool::new();

    // Create a name for testing (using raw value)
    let name = ori_ir::Name::from_raw(42);

    // Create Applied<int, str>
    let applied = pool.applied(name, &[Idx::INT, Idx::STR]);

    assert_eq!(pool.tag(applied), Tag::Applied);
    assert_eq!(pool.applied_name(applied), name);
    assert_eq!(pool.applied_arg_count(applied), 2);
    assert_eq!(pool.applied_arg(applied, 0), Idx::INT);
    assert_eq!(pool.applied_arg(applied, 1), Idx::STR);

    let args = pool.applied_args(applied);
    assert_eq!(args, vec![Idx::INT, Idx::STR]);
}

#[test]
fn applied_type_no_args() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(99);
    let applied = pool.applied(name, &[]);

    assert_eq!(pool.applied_name(applied), name);
    assert_eq!(pool.applied_arg_count(applied), 0);
    assert!(pool.applied_args(applied).is_empty());
}

#[test]
fn named_type_accessor() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(123);
    let named = pool.named(name);

    assert_eq!(pool.tag(named), Tag::Named);
    assert_eq!(pool.named_name(named), name);
}

// === Struct construction tests ===

#[test]
fn struct_construction() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(10);
    let x_name = ori_ir::Name::from_raw(20);
    let y_name = ori_ir::Name::from_raw(21);

    let struct_ty = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

    assert_eq!(pool.tag(struct_ty), Tag::Struct);
    assert_eq!(pool.struct_name(struct_ty), name);
    assert_eq!(pool.struct_field_count(struct_ty), 2);

    let (f0_name, f0_ty) = pool.struct_field(struct_ty, 0);
    assert_eq!(f0_name, x_name);
    assert_eq!(f0_ty, Idx::INT);

    let (f1_name, f1_ty) = pool.struct_field(struct_ty, 1);
    assert_eq!(f1_name, y_name);
    assert_eq!(f1_ty, Idx::FLOAT);

    let fields = pool.struct_fields(struct_ty);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], (x_name, Idx::INT));
    assert_eq!(fields[1], (y_name, Idx::FLOAT));
}

#[test]
fn struct_empty() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(30);
    let struct_ty = pool.struct_type(name, &[]);

    assert_eq!(pool.tag(struct_ty), Tag::Struct);
    assert_eq!(pool.struct_name(struct_ty), name);
    assert_eq!(pool.struct_field_count(struct_ty), 0);
    assert!(pool.struct_fields(struct_ty).is_empty());
}

#[test]
fn struct_dedup() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(40);
    let x_name = ori_ir::Name::from_raw(41);

    let s1 = pool.struct_type(name, &[(x_name, Idx::INT)]);
    let s2 = pool.struct_type(name, &[(x_name, Idx::INT)]);

    // Same name + same fields = same Idx
    assert_eq!(s1, s2);
}

#[test]
fn struct_nominal_typing() {
    let mut pool = Pool::new();

    let name_a = ori_ir::Name::from_raw(50);
    let name_b = ori_ir::Name::from_raw(51);
    let x_name = ori_ir::Name::from_raw(52);

    // Same field layout, different names = different Idx
    let s_a = pool.struct_type(name_a, &[(x_name, Idx::INT)]);
    let s_b = pool.struct_type(name_b, &[(x_name, Idx::INT)]);

    assert_ne!(s_a, s_b);
}

// === Enum construction tests ===

#[test]
fn enum_construction() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(60);
    let none_name = ori_ir::Name::from_raw(61);
    let some_name = ori_ir::Name::from_raw(62);

    let variants = vec![
        EnumVariant {
            name: none_name,
            field_types: vec![],
        },
        EnumVariant {
            name: some_name,
            field_types: vec![Idx::INT],
        },
    ];

    let enum_ty = pool.enum_type(name, &variants);

    assert_eq!(pool.tag(enum_ty), Tag::Enum);
    assert_eq!(pool.enum_name(enum_ty), name);
    assert_eq!(pool.enum_variant_count(enum_ty), 2);

    let (v0_name, v0_fields) = pool.enum_variant(enum_ty, 0);
    assert_eq!(v0_name, none_name);
    assert!(v0_fields.is_empty());

    let (v1_name, v1_fields) = pool.enum_variant(enum_ty, 1);
    assert_eq!(v1_name, some_name);
    assert_eq!(v1_fields, vec![Idx::INT]);

    let all_variants = pool.enum_variants(enum_ty);
    assert_eq!(all_variants.len(), 2);
    assert_eq!(all_variants[0].0, none_name);
    assert_eq!(all_variants[1].1, vec![Idx::INT]);
}

#[test]
fn enum_unit_only() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(70);
    let less = ori_ir::Name::from_raw(71);
    let equal = ori_ir::Name::from_raw(72);
    let greater = ori_ir::Name::from_raw(73);

    let variants = vec![
        EnumVariant {
            name: less,
            field_types: vec![],
        },
        EnumVariant {
            name: equal,
            field_types: vec![],
        },
        EnumVariant {
            name: greater,
            field_types: vec![],
        },
    ];

    let enum_ty = pool.enum_type(name, &variants);

    assert_eq!(pool.enum_variant_count(enum_ty), 3);
    for i in 0..3 {
        let (_, fields) = pool.enum_variant(enum_ty, i);
        assert!(fields.is_empty());
    }
}

#[test]
fn enum_empty() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(80);
    let enum_ty = pool.enum_type(name, &[]);

    assert_eq!(pool.tag(enum_ty), Tag::Enum);
    assert_eq!(pool.enum_name(enum_ty), name);
    assert_eq!(pool.enum_variant_count(enum_ty), 0);
    assert!(pool.enum_variants(enum_ty).is_empty());
}

#[test]
fn enum_dedup() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(90);
    let v_name = ori_ir::Name::from_raw(91);

    let variants = vec![EnumVariant {
        name: v_name,
        field_types: vec![Idx::INT],
    }];

    let e1 = pool.enum_type(name, &variants);
    let e2 = pool.enum_type(name, &variants);

    assert_eq!(e1, e2);
}

#[test]
fn enum_nominal_typing() {
    let mut pool = Pool::new();

    let name_a = ori_ir::Name::from_raw(100);
    let name_b = ori_ir::Name::from_raw(101);
    let v_name = ori_ir::Name::from_raw(102);

    let variants = vec![EnumVariant {
        name: v_name,
        field_types: vec![],
    }];

    let e_a = pool.enum_type(name_a, &variants);
    let e_b = pool.enum_type(name_b, &variants);

    assert_ne!(e_a, e_b);
}

// === Resolution tests ===

#[test]
fn resolution_basic() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(110);
    let x_name = ori_ir::Name::from_raw(111);

    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    assert_eq!(pool.resolve(named_idx), Some(struct_idx));
}

#[test]
fn resolution_chain() {
    let mut pool = Pool::new();

    let name_a = ori_ir::Name::from_raw(120);
    let name_b = ori_ir::Name::from_raw(121);
    let x_name = ori_ir::Name::from_raw(122);

    let a = pool.named(name_a);
    let b = pool.named(name_b);
    let concrete = pool.struct_type(name_b, &[(x_name, Idx::INT)]);

    pool.set_resolution(a, b);
    pool.set_resolution(b, concrete);

    // Should follow chain: a -> b -> concrete
    assert_eq!(pool.resolve(a), Some(concrete));
}

#[test]
fn resolution_none() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(130);
    let named_idx = pool.named(name);

    // No resolution registered
    assert_eq!(pool.resolve(named_idx), None);
}

// === Flags propagation tests ===

#[test]
fn struct_flags_propagate() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(140);
    let x_name = ori_ir::Name::from_raw(141);

    // Struct with only primitives should have IS_COMPOSITE
    let struct_ty = pool.struct_type(name, &[(x_name, Idx::INT)]);
    let flags = pool.flags(struct_ty);
    assert!(flags.contains(TypeFlags::IS_COMPOSITE));
    assert!(!flags.has_errors());
}

#[test]
fn struct_flags_propagate_error() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(150);
    let x_name = ori_ir::Name::from_raw(151);

    // Struct containing an error type should propagate HAS_ERROR
    let struct_ty = pool.struct_type(name, &[(x_name, Idx::ERROR)]);
    let flags = pool.flags(struct_ty);
    assert!(flags.contains(TypeFlags::IS_COMPOSITE));
    assert!(flags.has_errors());
}

#[test]
fn enum_flags_propagate() {
    let mut pool = Pool::new();

    let name = ori_ir::Name::from_raw(160);
    let v_name = ori_ir::Name::from_raw(161);

    // Enum with unit variants should just be IS_COMPOSITE
    let variants = vec![EnumVariant {
        name: v_name,
        field_types: vec![],
    }];
    let enum_ty = pool.enum_type(name, &variants);
    let flags = pool.flags(enum_ty);
    assert!(flags.contains(TypeFlags::IS_COMPOSITE));
    assert!(!flags.has_errors());
}
