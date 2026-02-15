use super::*;
use inkwell::context::Context;

/// Helper to create a Pool with just the pre-interned primitives.
fn test_pool() -> Pool {
    Pool::new()
}

// -- TypeInfo classification tests --

#[test]
fn primitive_triviality() {
    assert!(TypeInfo::Int.is_trivial());
    assert!(TypeInfo::Float.is_trivial());
    assert!(TypeInfo::Bool.is_trivial());
    assert!(TypeInfo::Char.is_trivial());
    assert!(TypeInfo::Byte.is_trivial());
    assert!(TypeInfo::Unit.is_trivial());
    assert!(TypeInfo::Never.is_trivial());
    assert!(TypeInfo::Duration.is_trivial());
    assert!(TypeInfo::Size.is_trivial());
    assert!(TypeInfo::Ordering.is_trivial());
    assert!(TypeInfo::Range.is_trivial());
    assert!(TypeInfo::Error.is_trivial());
}

#[test]
fn heap_types_not_trivial() {
    assert!(!TypeInfo::Str.is_trivial());
    assert!(!TypeInfo::List { element: Idx::INT }.is_trivial());
    assert!(!TypeInfo::Map {
        key: Idx::STR,
        value: Idx::INT
    }
    .is_trivial());
    assert!(!TypeInfo::Set { element: Idx::INT }.is_trivial());
    assert!(!TypeInfo::Channel { element: Idx::INT }.is_trivial());
    assert!(!TypeInfo::Function {
        params: vec![Idx::INT],
        ret: Idx::INT
    }
    .is_trivial());
}

#[test]
fn tagged_unions_not_trivial() {
    assert!(!TypeInfo::Option { inner: Idx::INT }.is_trivial());
    assert!(!TypeInfo::Result {
        ok: Idx::INT,
        err: Idx::STR
    }
    .is_trivial());
}

// -- Size tests --

#[test]
fn primitive_sizes() {
    assert_eq!(TypeInfo::Int.size(), Some(8));
    assert_eq!(TypeInfo::Float.size(), Some(8));
    assert_eq!(TypeInfo::Bool.size(), Some(1));
    assert_eq!(TypeInfo::Char.size(), Some(4));
    assert_eq!(TypeInfo::Byte.size(), Some(1));
    assert_eq!(TypeInfo::Unit.size(), Some(8));
    assert_eq!(TypeInfo::Never.size(), Some(8));
    assert_eq!(TypeInfo::Duration.size(), Some(8));
    assert_eq!(TypeInfo::Size.size(), Some(8));
    assert_eq!(TypeInfo::Ordering.size(), Some(1));
}

#[test]
fn composite_sizes() {
    assert_eq!(TypeInfo::Str.size(), Some(16));
    assert_eq!(TypeInfo::List { element: Idx::INT }.size(), Some(24));
    assert_eq!(
        TypeInfo::Map {
            key: Idx::STR,
            value: Idx::INT
        }
        .size(),
        Some(32)
    );
    assert_eq!(TypeInfo::Range.size(), Some(24));
    assert_eq!(TypeInfo::Option { inner: Idx::INT }.size(), Some(16));
    assert_eq!(TypeInfo::Channel { element: Idx::INT }.size(), Some(8));
    assert_eq!(
        TypeInfo::Function {
            params: vec![],
            ret: Idx::UNIT
        }
        .size(),
        Some(16)
    );
}

#[test]
fn dynamic_sizes_are_none() {
    assert_eq!(
        TypeInfo::Tuple {
            elements: vec![Idx::INT, Idx::STR]
        }
        .size(),
        None
    );
    assert_eq!(TypeInfo::Struct { fields: vec![] }.size(), None);
    assert_eq!(TypeInfo::Enum { variants: vec![] }.size(), None);
}

// -- Alignment tests --

#[test]
fn alignment_values() {
    assert_eq!(TypeInfo::Bool.alignment(), 1);
    assert_eq!(TypeInfo::Byte.alignment(), 1);
    assert_eq!(TypeInfo::Ordering.alignment(), 1);
    assert_eq!(TypeInfo::Char.alignment(), 4);
    assert_eq!(TypeInfo::Int.alignment(), 8);
    assert_eq!(TypeInfo::Float.alignment(), 8);
    assert_eq!(TypeInfo::Str.alignment(), 8);
}

// -- Loadability tests --

#[test]
fn loadable_types() {
    assert!(TypeInfo::Int.is_loadable());
    assert!(TypeInfo::Str.is_loadable()); // 16 bytes fits in 2 registers
    assert!(TypeInfo::Option { inner: Idx::INT }.is_loadable()); // 16 bytes
}

#[test]
fn non_loadable_types() {
    assert!(!TypeInfo::List { element: Idx::INT }.is_loadable()); // 24 bytes
    assert!(!TypeInfo::Map {
        key: Idx::STR,
        value: Idx::INT
    }
    .is_loadable()); // 32 bytes
}

// -- Storage type tests --

#[test]
fn primitive_storage_types() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");

    // i64 types
    let i64_ty: BasicTypeEnum = scx.type_i64().into();
    assert_eq!(TypeInfo::Int.storage_type(&scx), i64_ty);
    assert_eq!(TypeInfo::Duration.storage_type(&scx), i64_ty);
    assert_eq!(TypeInfo::Size.storage_type(&scx), i64_ty);
    assert_eq!(TypeInfo::Unit.storage_type(&scx), i64_ty);
    assert_eq!(TypeInfo::Never.storage_type(&scx), i64_ty);

    // Other primitives
    assert_eq!(TypeInfo::Float.storage_type(&scx), scx.type_f64().into());
    assert_eq!(TypeInfo::Bool.storage_type(&scx), scx.type_i1().into());
    assert_eq!(TypeInfo::Char.storage_type(&scx), scx.type_i32().into());
    assert_eq!(TypeInfo::Byte.storage_type(&scx), scx.type_i8().into());
    assert_eq!(TypeInfo::Ordering.storage_type(&scx), scx.type_i8().into());
}

#[test]
fn channel_type_is_pointer() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");

    let ptr_ty: BasicTypeEnum = scx.type_ptr().into();
    assert_eq!(
        TypeInfo::Channel { element: Idx::INT }.storage_type(&scx),
        ptr_ty
    );
}

#[test]
fn function_type_is_fat_pointer() {
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");

    let func_ty = TypeInfo::Function {
        params: vec![],
        ret: Idx::UNIT,
    }
    .storage_type(&scx);
    // Should be a struct { ptr, ptr }
    match func_ty {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2, "fat pointer should have 2 fields");
            assert!(
                st.get_field_type_at_index(0).unwrap().is_pointer_type(),
                "first field should be ptr"
            );
            assert!(
                st.get_field_type_at_index(1).unwrap().is_pointer_type(),
                "second field should be ptr"
            );
        }
        other => panic!("Expected StructType for Function, got {other:?}"),
    }
}

// -- TypeInfoStore tests --

#[test]
fn store_primitive_lookup() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);

    // Primitives should be pre-populated
    assert!(matches!(store.get(Idx::INT), TypeInfo::Int));
    assert!(matches!(store.get(Idx::FLOAT), TypeInfo::Float));
    assert!(matches!(store.get(Idx::BOOL), TypeInfo::Bool));
    assert!(matches!(store.get(Idx::STR), TypeInfo::Str));
    assert!(matches!(store.get(Idx::CHAR), TypeInfo::Char));
    assert!(matches!(store.get(Idx::BYTE), TypeInfo::Byte));
    assert!(matches!(store.get(Idx::UNIT), TypeInfo::Unit));
    assert!(matches!(store.get(Idx::NEVER), TypeInfo::Never));
    assert!(matches!(store.get(Idx::DURATION), TypeInfo::Duration));
    assert!(matches!(store.get(Idx::SIZE), TypeInfo::Size));
    assert!(matches!(store.get(Idx::ORDERING), TypeInfo::Ordering));
}

#[test]
fn store_none_returns_error() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    assert!(matches!(store.get(Idx::NONE), TypeInfo::Error));
}

#[test]
fn store_reserved_slots_are_error() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);

    // Indices 12-63 are reserved padding
    assert!(matches!(store.get(Idx::from_raw(12)), TypeInfo::Error));
    assert!(matches!(store.get(Idx::from_raw(32)), TypeInfo::Error));
    assert!(matches!(store.get(Idx::from_raw(63)), TypeInfo::Error));
}

#[test]
fn store_dynamic_list_type() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(list_int);
    match info {
        TypeInfo::List { element } => assert_eq!(element, Idx::INT),
        other => panic!("Expected TypeInfo::List, got {other:?}"),
    }
}

#[test]
fn store_dynamic_map_type() {
    let mut pool = Pool::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(map_str_int);
    match info {
        TypeInfo::Map { key, value } => {
            assert_eq!(key, Idx::STR);
            assert_eq!(value, Idx::INT);
        }
        other => panic!("Expected TypeInfo::Map, got {other:?}"),
    }
}

#[test]
fn store_dynamic_option_type() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(opt_int);
    match info {
        TypeInfo::Option { inner } => assert_eq!(inner, Idx::INT),
        other => panic!("Expected TypeInfo::Option, got {other:?}"),
    }
}

#[test]
fn store_dynamic_result_type() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::STR);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(res);
    match info {
        TypeInfo::Result { ok, err } => {
            assert_eq!(ok, Idx::INT);
            assert_eq!(err, Idx::STR);
        }
        other => panic!("Expected TypeInfo::Result, got {other:?}"),
    }
}

#[test]
fn store_dynamic_tuple_type() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(tup);
    match info {
        TypeInfo::Tuple { elements } => {
            assert_eq!(elements, vec![Idx::INT, Idx::STR, Idx::BOOL]);
        }
        other => panic!("Expected TypeInfo::Tuple, got {other:?}"),
    }
}

#[test]
fn store_dynamic_function_type() {
    let mut pool = Pool::new();
    let func = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(func);
    match info {
        TypeInfo::Function { params, ret } => {
            assert_eq!(params, vec![Idx::INT, Idx::STR]);
            assert_eq!(ret, Idx::BOOL);
        }
        other => panic!("Expected TypeInfo::Function, got {other:?}"),
    }
}

#[test]
fn store_dynamic_set_type() {
    let mut pool = Pool::new();
    let set_int = pool.set(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(set_int);
    match info {
        TypeInfo::Set { element } => assert_eq!(element, Idx::INT),
        other => panic!("Expected TypeInfo::Set, got {other:?}"),
    }
}

#[test]
fn store_dynamic_range_type() {
    let mut pool = Pool::new();
    let range = pool.range(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(range);
    assert!(matches!(info, TypeInfo::Range));
}

#[test]
fn store_caches_on_second_access() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    let store = TypeInfoStore::new(&pool);

    // First access: computes and caches
    let info1 = store.get(list_int);
    // Second access: returns cached
    let info2 = store.get(list_int);

    // Both should be List with same element
    match (&info1, &info2) {
        (TypeInfo::List { element: e1 }, TypeInfo::List { element: e2 }) => {
            assert_eq!(e1, e2);
        }
        _ => panic!("Expected matching List types"),
    }
}

#[test]
fn store_dynamic_channel_type() {
    let mut pool = Pool::new();
    let chan_int = pool.channel(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(chan_int);
    match info {
        TypeInfo::Channel { element } => assert_eq!(element, Idx::INT),
        other => panic!("Expected TypeInfo::Channel, got {other:?}"),
    }
}

#[test]
fn store_struct_from_pool() {
    let mut pool = Pool::new();
    let name = Name::from_raw(10);
    let x_name = Name::from_raw(20);
    let y_name = Name::from_raw(21);

    let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(struct_idx);
    match info {
        TypeInfo::Struct { fields } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0], (x_name, Idx::INT));
            assert_eq!(fields[1], (y_name, Idx::FLOAT));
        }
        other => panic!("Expected TypeInfo::Struct, got {other:?}"),
    }
}

#[test]
fn store_enum_from_pool() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let name = Name::from_raw(30);
    let none_name = Name::from_raw(31);
    let some_name = Name::from_raw(32);

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
    let enum_idx = pool.enum_type(name, &variants);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(enum_idx);
    match info {
        TypeInfo::Enum { variants } => {
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, none_name);
            assert!(variants[0].fields.is_empty());
            assert_eq!(variants[1].name, some_name);
            assert_eq!(variants[1].fields, vec![Idx::INT]);
        }
        other => panic!("Expected TypeInfo::Enum, got {other:?}"),
    }
}

#[test]
fn store_named_resolves_to_struct() {
    let mut pool = Pool::new();
    let name = Name::from_raw(40);
    let x_name = Name::from_raw(41);

    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let store = TypeInfoStore::new(&pool);
    let info = store.get(named_idx);
    match info {
        TypeInfo::Struct { fields } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0], (x_name, Idx::INT));
        }
        other => panic!("Expected TypeInfo::Struct via resolution, got {other:?}"),
    }
}

#[test]
fn store_named_unresolved_is_error() {
    let mut pool = Pool::new();
    let name = Name::from_raw(50);
    let named_idx = pool.named(name);
    // No resolution registered

    let store = TypeInfoStore::new(&pool);
    let info = store.get(named_idx);
    assert!(matches!(info, TypeInfo::Error));
}

// -- Transitive triviality tests --

#[test]
fn trivial_primitives() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);

    assert!(store.is_trivial(Idx::INT));
    assert!(store.is_trivial(Idx::FLOAT));
    assert!(store.is_trivial(Idx::BOOL));
    assert!(store.is_trivial(Idx::CHAR));
    assert!(store.is_trivial(Idx::BYTE));
    assert!(store.is_trivial(Idx::UNIT));
    assert!(store.is_trivial(Idx::NEVER));
    assert!(store.is_trivial(Idx::DURATION));
    assert!(store.is_trivial(Idx::SIZE));
    assert!(store.is_trivial(Idx::ORDERING));
}

#[test]
fn trivial_option_int() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(opt_int));
}

#[test]
fn nontrivial_option_str() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);

    let store = TypeInfoStore::new(&pool);
    assert!(!store.is_trivial(opt_str));
}

#[test]
fn trivial_tuple_scalars() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(tup));
}

#[test]
fn nontrivial_tuple_with_str() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);

    let store = TypeInfoStore::new(&pool);
    assert!(!store.is_trivial(tup));
}

#[test]
fn trivial_result_scalars() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::BOOL);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(res));
}

#[test]
fn nontrivial_result_with_str() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::STR);

    let store = TypeInfoStore::new(&pool);
    assert!(!store.is_trivial(res));
}

#[test]
fn trivial_struct_all_scalars() {
    let mut pool = Pool::new();
    let name = Name::from_raw(200);
    let x_name = Name::from_raw(201);
    let y_name = Name::from_raw(202);

    let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(struct_idx));
}

#[test]
fn nontrivial_struct_with_str_field() {
    let mut pool = Pool::new();
    let name = Name::from_raw(210);
    let x_name = Name::from_raw(211);

    let struct_idx = pool.struct_type(name, &[(x_name, Idx::STR)]);

    let store = TypeInfoStore::new(&pool);
    assert!(!store.is_trivial(struct_idx));
}

#[test]
fn trivial_nested_option_in_struct() {
    // struct Foo { x: option[int] } — trivial because option[int] is trivial
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);
    let name = Name::from_raw(220);
    let x_name = Name::from_raw(221);

    let struct_idx = pool.struct_type(name, &[(x_name, opt_int)]);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(struct_idx));
}

#[test]
fn trivial_enum_all_unit_variants() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let name = Name::from_raw(230);
    let a = Name::from_raw(231);
    let b = Name::from_raw(232);

    let variants = vec![
        EnumVariant {
            name: a,
            field_types: vec![],
        },
        EnumVariant {
            name: b,
            field_types: vec![],
        },
    ];
    let enum_idx = pool.enum_type(name, &variants);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(enum_idx));
}

#[test]
fn trivial_enum_with_scalar_fields() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let name = Name::from_raw(240);
    let a = Name::from_raw(241);
    let b = Name::from_raw(242);

    let variants = vec![
        EnumVariant {
            name: a,
            field_types: vec![Idx::INT],
        },
        EnumVariant {
            name: b,
            field_types: vec![Idx::FLOAT, Idx::BOOL],
        },
    ];
    let enum_idx = pool.enum_type(name, &variants);

    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(enum_idx));
}

#[test]
fn nontrivial_enum_with_str_field() {
    use ori_types::EnumVariant;

    let mut pool = Pool::new();
    let name = Name::from_raw(250);
    let a = Name::from_raw(251);
    let b = Name::from_raw(252);

    let variants = vec![
        EnumVariant {
            name: a,
            field_types: vec![Idx::INT],
        },
        EnumVariant {
            name: b,
            field_types: vec![Idx::STR],
        },
    ];
    let enum_idx = pool.enum_type(name, &variants);

    let store = TypeInfoStore::new(&pool);
    assert!(!store.is_trivial(enum_idx));
}

#[test]
fn nontrivial_heap_types() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let set_int = pool.set(Idx::INT);
    let chan_int = pool.channel(Idx::INT);
    let func_ty = pool.function(&[Idx::INT], Idx::INT);

    let store = TypeInfoStore::new(&pool);
    assert!(!store.is_trivial(Idx::STR));
    assert!(!store.is_trivial(list_int));
    assert!(!store.is_trivial(map_ty));
    assert!(!store.is_trivial(set_int));
    assert!(!store.is_trivial(chan_int));
    assert!(!store.is_trivial(func_ty));
}

#[test]
fn trivial_none_sentinel() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    assert!(store.is_trivial(Idx::NONE));
}

#[test]
fn triviality_caching() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    // First call computes
    assert!(store.is_trivial(opt_int));
    // Second call hits cache — verify same result
    assert!(store.is_trivial(opt_int));
}

// -- TypeLayoutResolver tests --

#[test]
fn resolver_primitive_types() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    assert_eq!(resolver.resolve(Idx::INT), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::FLOAT), scx.type_f64().into());
    assert_eq!(resolver.resolve(Idx::BOOL), scx.type_i1().into());
    assert_eq!(resolver.resolve(Idx::CHAR), scx.type_i32().into());
    assert_eq!(resolver.resolve(Idx::BYTE), scx.type_i8().into());
}

#[test]
fn resolver_simple_struct() {
    let mut pool = Pool::new();
    let name = Name::from_raw(300);
    let x_name = Name::from_raw(301);
    let y_name = Name::from_raw(302);

    let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let ty = resolver.resolve(struct_idx);
    // Should be a named struct with 2 fields
    match ty {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2);
            assert!(st.get_name().is_some());
        }
        other => panic!("Expected StructType, got {other:?}"),
    }
}

#[test]
fn resolver_nested_struct() {
    // struct Inner { x: int }
    // struct Outer { a: Inner, b: float }
    let mut pool = Pool::new();
    let inner_name = Name::from_raw(310);
    let outer_name = Name::from_raw(311);
    let x_name = Name::from_raw(312);
    let a_name = Name::from_raw(313);
    let b_name = Name::from_raw(314);

    let inner_idx = pool.struct_type(inner_name, &[(x_name, Idx::INT)]);
    let outer_idx = pool.struct_type(outer_name, &[(a_name, inner_idx), (b_name, Idx::FLOAT)]);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let ty = resolver.resolve(outer_idx);
    match ty {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2);
            // First field should be a named struct (Inner)
            let field0 = st.get_field_type_at_index(0).unwrap();
            assert!(matches!(field0, BasicTypeEnum::StructType(_)));
        }
        other => panic!("Expected StructType, got {other:?}"),
    }
}

#[test]
fn resolver_recursive_enum() {
    use ori_types::EnumVariant;

    // type Tree = Leaf(int) | Node(Tree, Tree)
    let mut pool = Pool::new();
    let tree_name = Name::from_raw(320);
    let leaf_name = Name::from_raw(321);
    let node_name = Name::from_raw(322);

    // Create a Named ref for Tree to use in Node's fields
    let tree_named = pool.named(tree_name);

    // Create the enum with Tree references in Node variant
    let variants = vec![
        EnumVariant {
            name: leaf_name,
            field_types: vec![Idx::INT],
        },
        EnumVariant {
            name: node_name,
            field_types: vec![tree_named, tree_named],
        },
    ];
    let tree_enum = pool.enum_type(tree_name, &variants);

    // Link Named -> Enum
    pool.set_resolution(tree_named, tree_enum);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    // Should not infinite loop!
    let ty = resolver.resolve(tree_enum);
    match ty {
        BasicTypeEnum::StructType(st) => {
            // Should be a named struct (tagged union)
            assert!(st.get_name().is_some());
            // Should have at least a tag field
            assert!(st.count_fields() >= 1);
        }
        other => panic!("Expected StructType for Tree enum, got {other:?}"),
    }

    // Recursive type should be non-trivial
    assert!(!store.is_trivial(tree_enum));
}

#[test]
fn resolver_enum_all_unit() {
    use ori_types::EnumVariant;

    // type Color = Red | Green | Blue
    let mut pool = Pool::new();
    let name = Name::from_raw(330);
    let r = Name::from_raw(331);
    let g = Name::from_raw(332);
    let b = Name::from_raw(333);

    let variants = vec![
        EnumVariant {
            name: r,
            field_types: vec![],
        },
        EnumVariant {
            name: g,
            field_types: vec![],
        },
        EnumVariant {
            name: b,
            field_types: vec![],
        },
    ];
    let enum_idx = pool.enum_type(name, &variants);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let ty = resolver.resolve(enum_idx);
    match ty {
        BasicTypeEnum::StructType(st) => {
            // All-unit enum: just { i8 tag }
            assert_eq!(st.count_fields(), 1);
        }
        other => panic!("Expected StructType, got {other:?}"),
    }
}

#[test]
fn resolver_option_with_recursive_resolve() {
    // option[int] should resolve correctly through the resolver
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let ty = resolver.resolve(opt_int);
    match ty {
        BasicTypeEnum::StructType(st) => {
            // { i8 tag, i64 payload }
            assert_eq!(st.count_fields(), 2);
        }
        other => panic!("Expected StructType for option, got {other:?}"),
    }
}

#[test]
fn resolver_tuple() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::BOOL, Idx::FLOAT]);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let ty = resolver.resolve(tup);
    match ty {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 3);
        }
        other => panic!("Expected StructType for tuple, got {other:?}"),
    }
}

#[test]
fn resolver_caches_results() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    let ty1 = resolver.resolve(Idx::INT);
    let ty2 = resolver.resolve(Idx::INT);
    assert_eq!(ty1, ty2);
}

// -- Benchmark: TypeInfoStore lookup performance --

/// Benchmark TypeInfoStore lookup on a representative type workload.
///
/// Constructs a Pool with primitives, collections, composites, and
/// user-defined types, then measures lookup latency across all of them.
/// Reports per-lookup timing for cached (hot) and first-access (cold) paths.
#[test]
fn benchmark_type_info_store_lookup() {
    use ori_types::EnumVariant;
    use std::hint::black_box;
    use std::time::Instant;

    // --- Build a representative type workload ---
    let mut pool = Pool::new();
    let mut all_indices: Vec<Idx> = Vec::new();

    // 1. Primitives (pre-interned, indices 0-11)
    let primitives = [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
        Idx::NEVER,
        Idx::DURATION,
        Idx::SIZE,
        Idx::ORDERING,
    ];
    all_indices.extend_from_slice(&primitives);

    // 2. Simple collections
    let list_int = pool.list(Idx::INT);
    let list_str = pool.list(Idx::STR);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_int = pool.set(Idx::INT);
    let range_int = pool.range(Idx::INT);
    let opt_int = pool.option(Idx::INT);
    let opt_str = pool.option(Idx::STR);
    let res_int_str = pool.result(Idx::INT, Idx::STR);
    let chan_int = pool.channel(Idx::INT);
    all_indices.extend_from_slice(&[
        list_int,
        list_str,
        map_str_int,
        set_int,
        range_int,
        opt_int,
        opt_str,
        res_int_str,
        chan_int,
    ]);

    // 3. Tuples and functions
    let tup2 = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let tup3 = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
    let func_simple = pool.function(&[Idx::INT], Idx::INT);
    let func_multi = pool.function(&[Idx::INT, Idx::STR, Idx::BOOL], Idx::FLOAT);
    all_indices.extend_from_slice(&[tup2, tup3, func_simple, func_multi]);

    // 4. User-defined structs
    let point_name = Name::from_raw(100);
    let x_name = Name::from_raw(101);
    let y_name = Name::from_raw(102);
    let point = pool.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::INT)]);

    let person_name = Name::from_raw(110);
    let name_field = Name::from_raw(111);
    let age_field = Name::from_raw(112);
    let person = pool.struct_type(
        person_name,
        &[(name_field, Idx::STR), (age_field, Idx::INT)],
    );
    all_indices.extend_from_slice(&[point, person]);

    // 5. User-defined enums
    let color_name = Name::from_raw(120);
    let red = Name::from_raw(121);
    let green = Name::from_raw(122);
    let blue = Name::from_raw(123);
    let color = pool.enum_type(
        color_name,
        &[
            EnumVariant {
                name: red,
                field_types: vec![],
            },
            EnumVariant {
                name: green,
                field_types: vec![],
            },
            EnumVariant {
                name: blue,
                field_types: vec![],
            },
        ],
    );

    let shape_name = Name::from_raw(130);
    let circle = Name::from_raw(131);
    let rect = Name::from_raw(132);
    let shape = pool.enum_type(
        shape_name,
        &[
            EnumVariant {
                name: circle,
                field_types: vec![Idx::FLOAT],
            },
            EnumVariant {
                name: rect,
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    all_indices.extend_from_slice(&[color, shape]);

    // 6. Nested collections (list of tuples, option of struct, etc.)
    let list_of_tup = pool.list(tup2);
    let opt_point = pool.option(point);
    let res_person_str = pool.result(person, Idx::STR);
    all_indices.extend_from_slice(&[list_of_tup, opt_point, res_person_str]);

    let type_count = all_indices.len();

    // --- Cold lookups: first access (compute + cache) ---
    let store = TypeInfoStore::new(&pool);
    let iterations = 1000;

    let cold_start = Instant::now();
    for _ in 0..iterations {
        // Create a fresh store each iteration to measure cold path
        let fresh_store = TypeInfoStore::new(&pool);
        for &idx in &all_indices {
            black_box(fresh_store.get(idx));
        }
    }
    let cold_elapsed = cold_start.elapsed();
    let cold_per_lookup_ns =
        cold_elapsed.as_nanos() as f64 / (iterations as f64 * type_count as f64);

    // --- Hot lookups: cached access ---
    // Warm up the cache
    for &idx in &all_indices {
        store.get(idx);
    }

    let hot_iterations = 10_000;
    let hot_start = Instant::now();
    for _ in 0..hot_iterations {
        for &idx in &all_indices {
            black_box(store.get(idx));
        }
    }
    let hot_elapsed = hot_start.elapsed();
    let hot_per_lookup_ns =
        hot_elapsed.as_nanos() as f64 / (hot_iterations as f64 * type_count as f64);

    // --- Triviality classification ---
    let triv_iterations = 10_000;
    let triv_start = Instant::now();
    for _ in 0..triv_iterations {
        for &idx in &all_indices {
            black_box(store.is_trivial(idx));
        }
    }
    let triv_elapsed = triv_start.elapsed();
    let triv_per_lookup_ns =
        triv_elapsed.as_nanos() as f64 / (triv_iterations as f64 * type_count as f64);

    // --- Report ---
    eprintln!("\n=== TypeInfoStore Benchmark ===");
    eprintln!("Types: {type_count}");
    eprintln!("Cold lookup (compute+cache): {cold_per_lookup_ns:.1} ns/lookup");
    eprintln!("Hot lookup (cached):         {hot_per_lookup_ns:.1} ns/lookup");
    eprintln!("Triviality (cached):         {triv_per_lookup_ns:.1} ns/lookup");
    eprintln!("================================\n");

    // Sanity: hot lookups must be faster than cold
    assert!(
        hot_per_lookup_ns < cold_per_lookup_ns,
        "Hot lookups ({hot_per_lookup_ns:.1}ns) should be faster than cold ({cold_per_lookup_ns:.1}ns)"
    );
}

// -- Integration test: compile through new type system --

/// End-to-end integration test: constructs a Pool with a variety of
/// types (primitives, collections, structs, enums, recursive types),
/// creates a `TypeInfoStore`, resolves all types through the
/// `TypeLayoutResolver`, and verifies the resulting LLVM types.
///
/// This validates the full TypeInfo pipeline:
/// Pool → TypeInfoStore → TypeLayoutResolver → LLVM BasicTypeEnum
#[test]
fn integration_compile_through_type_system() {
    use ori_types::EnumVariant;
    use std::hint::black_box;

    let mut pool = Pool::new();

    // --- Primitives ---
    // Already interned; just verify they resolve.

    // --- Collections ---
    let list_int = pool.list(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_float = pool.set(Idx::FLOAT);
    let range_int = pool.range(Idx::INT);
    let opt_int = pool.option(Idx::INT);
    let opt_str = pool.option(Idx::STR);
    let res_int_str = pool.result(Idx::INT, Idx::STR);
    let chan_byte = pool.channel(Idx::BYTE);

    // --- Composites ---
    let tup_if = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let func_ii = pool.function(&[Idx::INT], Idx::INT);

    // --- User-defined struct: Point { x: int, y: int } ---
    let point_name = Name::from_raw(500);
    let x_name = Name::from_raw(501);
    let y_name = Name::from_raw(502);
    let point = pool.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::INT)]);

    // --- User-defined enum: Color = Red | Green | Blue ---
    let color_name = Name::from_raw(510);
    let red = Name::from_raw(511);
    let green = Name::from_raw(512);
    let blue = Name::from_raw(513);
    let color = pool.enum_type(
        color_name,
        &[
            EnumVariant {
                name: red,
                field_types: vec![],
            },
            EnumVariant {
                name: green,
                field_types: vec![],
            },
            EnumVariant {
                name: blue,
                field_types: vec![],
            },
        ],
    );

    // --- Enum with payloads: Shape = Circle(float) | Rect(float, float) ---
    let shape_name = Name::from_raw(520);
    let circle = Name::from_raw(521);
    let rect = Name::from_raw(522);
    let shape = pool.enum_type(
        shape_name,
        &[
            EnumVariant {
                name: circle,
                field_types: vec![Idx::FLOAT],
            },
            EnumVariant {
                name: rect,
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );

    // --- Recursive enum: Tree = Leaf(int) | Node(Tree, Tree) ---
    let tree_name = Name::from_raw(530);
    let leaf = Name::from_raw(531);
    let node = Name::from_raw(532);
    let tree_named = pool.named(tree_name);
    let tree_enum = pool.enum_type(
        tree_name,
        &[
            EnumVariant {
                name: leaf,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: node,
                field_types: vec![tree_named, tree_named],
            },
        ],
    );
    pool.set_resolution(tree_named, tree_enum);

    // --- Named type alias: MyPoint -> Point ---
    let my_point_name = Name::from_raw(540);
    let my_point = pool.named(my_point_name);
    pool.set_resolution(my_point, point);

    // --- Nested: option[Point], [Shape], result[Tree, str] ---
    let opt_point = pool.option(point);
    let list_shape = pool.list(shape);
    let res_tree_str = pool.result(tree_named, Idx::STR);

    // === Build TypeInfoStore and TypeLayoutResolver ===
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "integration_test");
    let resolver = TypeLayoutResolver::new(&store, &scx);

    // === Verify all types resolve without panic ===
    let all_types = [
        // Primitives
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
        Idx::NEVER,
        Idx::DURATION,
        Idx::SIZE,
        Idx::ORDERING,
        // Collections
        list_int,
        map_str_int,
        set_float,
        range_int,
        opt_int,
        opt_str,
        res_int_str,
        chan_byte,
        // Composites
        tup_if,
        func_ii,
        // User-defined
        point,
        color,
        shape,
        tree_enum,
        // Named/alias
        my_point,
        tree_named,
        // Nested
        opt_point,
        list_shape,
        res_tree_str,
    ];

    for &idx in &all_types {
        // TypeInfoStore: get() must succeed
        let info = store.get(idx);
        assert!(
            !matches!(info, TypeInfo::Error),
            "TypeInfo::Error for idx {} (tag {:?})",
            idx.raw(),
            pool.tag(idx)
        );

        // TypeLayoutResolver: resolve() must produce a valid LLVM type
        let llvm_ty = resolver.resolve(idx);
        // All types should produce a non-void BasicTypeEnum
        let _ = black_box(llvm_ty);
    }

    // === Verify specific type properties ===

    // Primitives: correct storage types
    assert_eq!(resolver.resolve(Idx::INT), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::FLOAT), scx.type_f64().into());
    assert_eq!(resolver.resolve(Idx::BOOL), scx.type_i1().into());
    assert_eq!(resolver.resolve(Idx::CHAR), scx.type_i32().into());

    // Point struct: 2 fields (both i64)
    match resolver.resolve(point) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2, "Point should have 2 fields");
            assert!(st.get_name().is_some(), "Point should be a named struct");
        }
        other => panic!("Point should be StructType, got {other:?}"),
    }

    // Color enum: all-unit → just {i8 tag}
    match resolver.resolve(color) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(
                st.count_fields(),
                1,
                "All-unit Color enum should have 1 field (tag)"
            );
        }
        other => panic!("Color should be StructType, got {other:?}"),
    }

    // Shape enum: {i8 tag, payload}
    match resolver.resolve(shape) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2, "Shape enum should have tag + payload");
        }
        other => panic!("Shape should be StructType, got {other:?}"),
    }

    // Tree (recursive): should resolve without infinite loop, be a named struct
    match resolver.resolve(tree_enum) {
        BasicTypeEnum::StructType(st) => {
            assert!(st.get_name().is_some(), "Tree should be a named struct");
        }
        other => panic!("Tree should be StructType, got {other:?}"),
    }

    // Named alias: MyPoint should resolve to same shape as Point
    let my_point_ty = resolver.resolve(my_point);
    match my_point_ty {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(
                st.count_fields(),
                2,
                "MyPoint alias should resolve to Point's 2 fields"
            );
        }
        other => panic!("MyPoint alias should resolve to StructType, got {other:?}"),
    }

    // === Verify triviality classification ===
    assert!(store.is_trivial(Idx::INT), "int should be trivial");
    assert!(!store.is_trivial(Idx::STR), "str should NOT be trivial");
    assert!(
        store.is_trivial(point),
        "Point{{int,int}} should be trivial"
    );
    assert!(
        !store.is_trivial(tree_enum),
        "Recursive Tree should NOT be trivial"
    );
    assert!(
        store.is_trivial(color),
        "All-unit Color enum should be trivial"
    );
    assert!(store.is_trivial(opt_int), "option[int] should be trivial");
    assert!(
        !store.is_trivial(opt_str),
        "option[str] should NOT be trivial"
    );

    // === Sentinel handling ===
    assert!(matches!(store.get(Idx::NONE), TypeInfo::Error));
    assert_eq!(resolver.resolve(Idx::NONE), scx.type_i64().into());
}
