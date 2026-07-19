use super::*;
use inkwell::context::Context;
use inkwell::types::BasicTypeEnum;
use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::context::SimpleCx;

/// Helper to create a Pool with just the pre-interned primitives.
fn test_pool() -> Pool {
    Pool::new()
}

// TypeInfo classification tests

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

/// Iterator types require `ori_iter_drop` at scope exit to free their
/// box-allocated state. `TypeInfo` must agree with the canonical triviality
/// classifiers that these types are non-trivial.
#[test]
fn iterator_types_are_non_trivial() {
    assert!(
        !TypeInfo::Iterator { element: Idx::INT }.is_trivial(),
        "Iterator needs ori_iter_drop at scope exit — not trivial"
    );
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

// Size tests

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
    assert_eq!(TypeInfo::Str.size(), Some(24));
    assert_eq!(TypeInfo::List { element: Idx::INT }.size(), Some(24));
    assert_eq!(
        TypeInfo::Map {
            key: Idx::STR,
            value: Idx::INT
        }
        .size(),
        Some(24)
    );
    assert_eq!(TypeInfo::Range.size(), Some(32));
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
    assert_eq!(TypeInfo::Option { inner: Idx::INT }.size(), None);
    assert_eq!(
        TypeInfo::Result {
            ok: Idx::INT,
            err: Idx::STR
        }
        .size(),
        None
    );
    assert_eq!(TypeInfo::Struct { fields: vec![] }.size(), None);
    assert_eq!(TypeInfo::Enum { variants: vec![] }.size(), None);
}

// Alignment tests

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

// Loadability tests

#[test]
fn loadable_types() {
    assert!(TypeInfo::Int.is_loadable());
}

#[test]
fn non_loadable_types() {
    assert!(!TypeInfo::Str.is_loadable()); // 24 bytes (SSO layout)
    assert!(!TypeInfo::List { element: Idx::INT }.is_loadable()); // 24 bytes
    assert!(!TypeInfo::Map {
        key: Idx::STR,
        value: Idx::INT
    }
    .is_loadable()); // 24 bytes
}

// Storage type tests

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

// TypeInfoStore tests

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

// Transitive triviality tests

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

// TypeLayoutResolver tests

fn assert_result_payload_is_i8(ok: Idx, err: Idx) {
    let mut pool = Pool::new();
    let result = pool.result(ok, err);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

    let BasicTypeEnum::StructType(result_type) = resolver.resolve(result) else {
        panic!("expected Result to resolve to an LLVM struct");
    };
    let Some(payload) = result_type.get_field_type_at_index(1) else {
        panic!("expected Result struct to contain a payload field");
    };

    assert_eq!(payload, scx.type_i8().into());
}

/// Regression: Equal-byte payload arms retain the widest integer value range.
#[test]
fn resolver_result_bool_ordering_equal_store_size_uses_i8_payload() {
    assert_result_payload_is_i8(Idx::BOOL, Idx::ORDERING);
}

/// Regression: Payload selection is independent of Result arm order.
#[test]
fn resolver_result_ordering_bool_equal_store_size_uses_i8_payload() {
    assert_result_payload_is_i8(Idx::ORDERING, Idx::BOOL);
}

#[test]
fn resolver_primitive_types() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

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
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

    let ty1 = resolver.resolve(Idx::INT);
    let ty2 = resolver.resolve(Idx::INT);
    assert_eq!(ty1, ty2);
}

// Benchmark: TypeInfoStore lookup performance

/// Benchmark TypeInfoStore lookup on a representative type workload.
///
/// Constructs a Pool with primitives, collections, composites, and
/// user-defined types, then measures lookup latency across all of them.
/// Reports per-lookup timing for cached (hot) and first-access (cold) paths.
fn benchmark_type_workload() -> (Pool, Vec<Idx>) {
    let mut pool = Pool::new();
    let mut indices = vec![
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

    let list_int = pool.list(Idx::INT);
    let list_str = pool.list(Idx::STR);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_int = pool.set(Idx::INT);
    let range_int = pool.range(Idx::INT);
    let opt_int = pool.option(Idx::INT);
    let opt_str = pool.option(Idx::STR);
    let result_int_str = pool.result(Idx::INT, Idx::STR);
    let channel_int = pool.channel(Idx::INT);
    indices.extend([
        list_int,
        list_str,
        map_str_int,
        set_int,
        range_int,
        opt_int,
        opt_str,
        result_int_str,
        channel_int,
    ]);

    let tuple_pair = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let tuple_triple = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
    let function_simple = pool.function(&[Idx::INT], Idx::INT);
    let function_multi = pool.function(&[Idx::INT, Idx::STR, Idx::BOOL], Idx::FLOAT);
    indices.extend([tuple_pair, tuple_triple, function_simple, function_multi]);

    let (point, person) = add_benchmark_user_types(&mut pool, &mut indices);
    let list_of_tuple = pool.list(tuple_pair);
    let option_point = pool.option(point);
    let result_person_str = pool.result(person, Idx::STR);
    indices.extend([list_of_tuple, option_point, result_person_str]);
    (pool, indices)
}

fn add_benchmark_user_types(pool: &mut Pool, indices: &mut Vec<Idx>) -> (Idx, Idx) {
    let point = pool.struct_type(
        Name::from_raw(100),
        &[
            (Name::from_raw(101), Idx::INT),
            (Name::from_raw(102), Idx::INT),
        ],
    );
    let person = pool.struct_type(
        Name::from_raw(110),
        &[
            (Name::from_raw(111), Idx::STR),
            (Name::from_raw(112), Idx::INT),
        ],
    );
    let color = pool.enum_type(
        Name::from_raw(120),
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(121),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(122),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(123),
                field_types: vec![],
            },
        ],
    );
    let shape = pool.enum_type(
        Name::from_raw(130),
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(131),
                field_types: vec![Idx::FLOAT],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(132),
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    indices.extend([point, person, color, shape]);
    (point, person)
}

#[test]
fn benchmark_type_info_store_lookup() {
    use std::hint::black_box;
    use std::time::Instant;

    let (pool, all_indices) = benchmark_type_workload();
    let type_count = all_indices.len();

    // Cold lookups: first access (compute + cache)
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

    // Hot lookups: cached access
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

    // Triviality classification
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

    // Report
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

// Integration test: compile through new type system

/// End-to-end integration test: constructs a Pool with a variety of
/// types (primitives, collections, structs, enums, recursive types),
/// creates a `TypeInfoStore`, resolves all types through the
/// `TypeLayoutResolver`, and verifies the resulting LLVM types.
///
/// This validates the full TypeInfo pipeline:
/// Pool → TypeInfoStore → TypeLayoutResolver → LLVM BasicTypeEnum
struct IntegrationTypeFixture {
    all_types: Vec<Idx>,
    point: Idx,
    color: Idx,
    shape: Idx,
    tree: Idx,
    my_point: Idx,
    option_int: Idx,
    option_str: Idx,
}

fn integration_type_fixture() -> (Pool, IntegrationTypeFixture) {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_float = pool.set(Idx::FLOAT);
    let range_int = pool.range(Idx::INT);
    let option_int = pool.option(Idx::INT);
    let option_str = pool.option(Idx::STR);
    let result_int_str = pool.result(Idx::INT, Idx::STR);
    let channel_byte = pool.channel(Idx::BYTE);
    let tuple = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let function = pool.function(&[Idx::INT], Idx::INT);

    let (point, color, shape, tree, tree_named, my_point) = add_integration_user_types(&mut pool);
    let option_point = pool.option(point);
    let list_shape = pool.list(shape);
    let result_tree_str = pool.result(tree_named, Idx::STR);
    let all_types = vec![
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
        list_int,
        map_str_int,
        set_float,
        range_int,
        option_int,
        option_str,
        result_int_str,
        channel_byte,
        tuple,
        function,
        point,
        color,
        shape,
        tree,
        my_point,
        tree_named,
        option_point,
        list_shape,
        result_tree_str,
    ];
    (
        pool,
        IntegrationTypeFixture {
            all_types,
            point,
            color,
            shape,
            tree,
            my_point,
            option_int,
            option_str,
        },
    )
}

fn add_integration_user_types(pool: &mut Pool) -> (Idx, Idx, Idx, Idx, Idx, Idx) {
    let point = pool.struct_type(
        Name::from_raw(500),
        &[
            (Name::from_raw(501), Idx::INT),
            (Name::from_raw(502), Idx::INT),
        ],
    );
    let color = pool.enum_type(
        Name::from_raw(510),
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(511),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(512),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(513),
                field_types: vec![],
            },
        ],
    );
    let shape = pool.enum_type(
        Name::from_raw(520),
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(521),
                field_types: vec![Idx::FLOAT],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(522),
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    let tree_named = pool.named(Name::from_raw(530));
    let tree = pool.enum_type(
        Name::from_raw(530),
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(531),
                field_types: vec![Idx::INT],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(532),
                field_types: vec![tree_named, tree_named],
            },
        ],
    );
    pool.set_resolution(tree_named, tree);
    let my_point = pool.named(Name::from_raw(540));
    pool.set_resolution(my_point, point);
    (point, color, shape, tree, tree_named, my_point)
}

fn assert_integration_types_resolve(
    pool: &Pool,
    fixture: &IntegrationTypeFixture,
    store: &TypeInfoStore<'_>,
    resolver: &TypeLayoutResolver<'_, '_, '_>,
) {
    for &idx in &fixture.all_types {
        let info = store.get(idx);
        assert!(
            !matches!(info, TypeInfo::Error),
            "TypeInfo::Error for idx {} (tag {:?})",
            idx.raw(),
            pool.tag(idx)
        );
        std::hint::black_box(resolver.resolve(idx));
    }
}

fn assert_integration_layouts(
    fixture: &IntegrationTypeFixture,
    resolver: &TypeLayoutResolver<'_, '_, '_>,
    scx: &SimpleCx<'_>,
) {
    assert_eq!(resolver.resolve(Idx::INT), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::FLOAT), scx.type_f64().into());
    assert_eq!(resolver.resolve(Idx::BOOL), scx.type_i1().into());
    assert_eq!(resolver.resolve(Idx::CHAR), scx.type_i32().into());

    match resolver.resolve(fixture.point) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2, "Point should have 2 fields");
            assert!(st.get_name().is_some(), "Point should be a named struct");
        }
        other => panic!("Point should be StructType, got {other:?}"),
    }

    match resolver.resolve(fixture.color) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(
                st.count_fields(),
                1,
                "All-unit Color enum should have 1 field (tag)"
            );
        }
        other => panic!("Color should be StructType, got {other:?}"),
    }

    match resolver.resolve(fixture.shape) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(st.count_fields(), 2, "Shape enum should have tag + payload");
        }
        other => panic!("Shape should be StructType, got {other:?}"),
    }

    match resolver.resolve(fixture.tree) {
        BasicTypeEnum::StructType(st) => {
            assert!(st.get_name().is_some(), "Tree should be a named struct");
        }
        other => panic!("Tree should be StructType, got {other:?}"),
    }

    match resolver.resolve(fixture.my_point) {
        BasicTypeEnum::StructType(st) => {
            assert_eq!(
                st.count_fields(),
                2,
                "MyPoint alias should resolve to Point's 2 fields"
            );
        }
        other => panic!("MyPoint alias should resolve to StructType, got {other:?}"),
    }
}

fn assert_integration_triviality(fixture: &IntegrationTypeFixture, store: &TypeInfoStore<'_>) {
    assert!(store.is_trivial(Idx::INT), "int should be trivial");
    assert!(!store.is_trivial(Idx::STR), "str should NOT be trivial");
    assert!(
        store.is_trivial(fixture.point),
        "Point{{int,int}} should be trivial"
    );
    assert!(
        !store.is_trivial(fixture.tree),
        "Recursive Tree should NOT be trivial"
    );
    assert!(
        store.is_trivial(fixture.color),
        "All-unit Color enum should be trivial"
    );
    assert!(
        store.is_trivial(fixture.option_int),
        "option[int] should be trivial"
    );
    assert!(
        !store.is_trivial(fixture.option_str),
        "option[str] should NOT be trivial"
    );
}

#[test]
fn integration_compile_through_type_system() {
    let (pool, fixture) = integration_type_fixture();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "integration_test");
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

    assert_integration_types_resolve(&pool, &fixture, &store, &resolver);
    assert_integration_layouts(&fixture, &resolver, &scx);
    assert_integration_triviality(&fixture, &store);
    assert!(matches!(store.get(Idx::NONE), TypeInfo::Error));
    assert_eq!(resolver.resolve(Idx::NONE), scx.type_i64().into());
}

// Phase A: ReprPlan integration tests

/// Phase A fallback for 12 primitives: empty ReprPlan produces the same
/// LLVM types as TypeInfoStore alone.
#[test]
fn store_fallback_resolves_primitives() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_phase_a");

    // Baseline: no ReprPlan (None).
    let no_plan = TypeLayoutResolver::new(&store, &scx, None, None);

    // Phase A: empty ReprPlan (no decisions recorded).
    let empty_plan = ori_repr::ReprPlan::new(ori_repr::NarrowingPolicy::Disabled);
    let with_plan = TypeLayoutResolver::new(&store, &scx, None, Some(&empty_plan));

    // All 12 primitives must produce identical LLVM types.
    let primitives = [
        (Idx::INT, "Int"),
        (Idx::FLOAT, "Float"),
        (Idx::BOOL, "Bool"),
        (Idx::STR, "Str"),
        (Idx::CHAR, "Char"),
        (Idx::BYTE, "Byte"),
        (Idx::UNIT, "Unit"),
        (Idx::NEVER, "Never"),
        (Idx::DURATION, "Duration"),
        (Idx::SIZE, "Size"),
        (Idx::ORDERING, "Ordering"),
        (Idx::ERROR, "Error"),
    ];

    for (idx, name) in primitives {
        assert_eq!(
            with_plan.resolve(idx),
            no_plan.resolve(idx),
            "Phase A fallback mismatch for {name}: empty plan must match no plan"
        );
    }
}

/// Phase A fallback for composite types: empty ReprPlan produces the same
/// LLVM type structure as TypeInfoStore alone for Option, Result, Tuple,
/// Struct, Enum.
///
/// Named structs in the same LLVM context get uniquified names (`%ori.400`
/// vs `%ori.400.0`), so we compare field counts and field types instead
/// of pointer identity.
#[test]
fn store_fallback_resolves_composites() {
    use inkwell::types::BasicTypeEnum::StructType as ST;

    let mut pool = Pool::new();
    let name = Name::from_raw(400);
    let x_name = Name::from_raw(401);
    let y_name = Name::from_raw(402);

    let opt_int = pool.option(Idx::INT);
    let res_int_str = pool.result(Idx::INT, Idx::STR);
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let struct_idx = pool.struct_type(name, &[(x_name, Idx::INT), (y_name, Idx::FLOAT)]);
    let enum_idx = pool.enum_type(
        name,
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(403),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(404),
                field_types: vec![Idx::INT],
            },
        ],
    );

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_phase_a_composites");

    let no_plan = TypeLayoutResolver::new(&store, &scx, None, None);
    let empty_plan = ori_repr::ReprPlan::new(ori_repr::NarrowingPolicy::Disabled);
    let with_plan = TypeLayoutResolver::new(&store, &scx, None, Some(&empty_plan));

    // For anonymous structs (Option, Result, Tuple), pointer equality works.
    assert_eq!(
        with_plan.resolve(opt_int),
        no_plan.resolve(opt_int),
        "Phase A fallback mismatch for Option<int>"
    );
    assert_eq!(
        with_plan.resolve(res_int_str),
        no_plan.resolve(res_int_str),
        "Phase A fallback mismatch for Result<int, str>"
    );
    assert_eq!(
        with_plan.resolve(tup),
        no_plan.resolve(tup),
        "Phase A fallback mismatch for (int, float)"
    );

    // Named structs get uniquified names — compare field counts.
    let (no_plan_struct, with_plan_struct) =
        (no_plan.resolve(struct_idx), with_plan.resolve(struct_idx));
    if let (ST(a), ST(b)) = (no_plan_struct, with_plan_struct) {
        assert_eq!(
            a.count_fields(),
            b.count_fields(),
            "struct field count mismatch"
        );
    } else {
        panic!("struct should resolve to StructType");
    }

    let (no_plan_enum, with_plan_enum) = (no_plan.resolve(enum_idx), with_plan.resolve(enum_idx));
    if let (ST(a), ST(b)) = (no_plan_enum, with_plan_enum) {
        assert_eq!(
            a.count_fields(),
            b.count_fields(),
            "enum field count mismatch"
        );
    } else {
        panic!("enum should resolve to StructType");
    }
}

/// Phase A override: populate ReprPlan with a narrowed decision for Int,
/// verify the ReprPlan path is used (produces i32 instead of i64).
#[test]
fn repr_plan_override_takes_precedence_over_store() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_phase_a_override");

    // Create a plan with a narrowed Int representation (i32 instead of i64).
    let mut plan = ori_repr::ReprPlan::new(ori_repr::NarrowingPolicy::Conservative);
    plan.set_repr(
        Idx::INT,
        ori_repr::ReprDecision {
            source: ori_repr::DecisionSource::Canonical,
            type_idx: Idx::INT,
            repr: ori_repr::MachineRepr::Int {
                width: ori_repr::IntWidth::I32,
                signed: true,
            },
            reason: ori_repr::DecisionReason::RangeFits {
                range: ori_repr::range::ValueRange::Bounded { lo: 0, hi: 1000 },
                min_width: ori_repr::IntWidth::I32,
            },
        },
    );

    let resolver = TypeLayoutResolver::new(&store, &scx, None, Some(&plan));

    // ReprPlan path: Int should produce i32 (narrowed), not i64 (canonical).
    assert_eq!(
        resolver.resolve(Idx::INT),
        scx.type_i32().into(),
        "Phase A override: ReprPlan says I32, but resolver produced something else"
    );

    // Other primitives without decisions should still use TypeInfoStore.
    let no_plan_resolver = TypeLayoutResolver::new(&store, &scx, None, None);
    assert_eq!(
        resolver.resolve(Idx::FLOAT),
        no_plan_resolver.resolve(Idx::FLOAT),
        "Float has no ReprPlan decision — should fall back to TypeInfoStore"
    );
}

/// Phase A with None ReprPlan: backward-compatibility test — all lookups
/// go through TypeInfoStore exclusively.
#[test]
fn none_repr_plan_resolves_via_store() {
    let pool = test_pool();
    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_phase_a_none");

    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

    // All primitives should resolve correctly through TypeInfoStore.
    assert_eq!(resolver.resolve(Idx::INT), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::FLOAT), scx.type_f64().into());
    assert_eq!(resolver.resolve(Idx::BOOL), scx.type_i1().into());
    assert_eq!(resolver.resolve(Idx::CHAR), scx.type_i32().into());
    assert_eq!(resolver.resolve(Idx::BYTE), scx.type_i8().into());
    assert_eq!(resolver.resolve(Idx::UNIT), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::NEVER), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::DURATION), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::SIZE), scx.type_i64().into());
    assert_eq!(resolver.resolve(Idx::ORDERING), scx.type_i8().into());
}

/// Semantic pin: empty ReprPlan must produce IDENTICAL output to no
/// ReprPlan for all resolvable primitive types. This test guards against
/// Phase A introducing any behavioral change.
#[test]
fn semantic_pin_empty_plan_equals_no_plan() {
    let mut pool = Pool::new();

    // Add some dynamic types to test beyond primitives.
    let opt_int = pool.option(Idx::INT);
    let list_str = pool.list(Idx::STR);
    let range_idx = pool.range(Idx::INT);
    let fn_idx = pool.function(&[Idx::INT], Idx::BOOL);
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_semantic_pin");

    let no_plan = TypeLayoutResolver::new(&store, &scx, None, None);
    let empty_plan = ori_repr::ReprPlan::new(ori_repr::NarrowingPolicy::Disabled);
    let with_plan = TypeLayoutResolver::new(&store, &scx, None, Some(&empty_plan));

    // Every resolvable type must produce bit-identical LLVM types.
    let all_types = [
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
        opt_int,
        list_str,
        range_idx,
        fn_idx,
        tup,
    ];

    for idx in all_types {
        assert_eq!(
            with_plan.resolve(idx),
            no_plan.resolve(idx),
            "Semantic pin violation at idx {idx:?}: empty plan != no plan"
        );
    }
}

/// Cross-crate parity: `compute_repr_plan` canonical representations
/// must produce the same LLVM types as the legacy TypeInfoStore path
/// for all codegen-reachable types in the 29-type matrix.
///
/// This is the live verification for the `ori_repr` ↔ `ori_llvm` contract.
/// Unlike the empty-plan semantic pins above, this test exercises the
/// *populated* ReprPlan (canonical decisions from `populate_canonical`)
/// and verifies parity against TypeInfoStore for every type that reaches
/// LLVM codegen.
///
/// Covers: 12 primitives, 7 simple containers (Option, List, Set, Channel,
/// Range, Iterator, DoubleEndedIterator), 2 two-child (Map, Result),
/// 3 complex (Function, Tuple, Struct, Enum).
struct ReprParityFixture {
    containers: [(Idx, &'static str); 7],
    option_int: Idx,
    map_str_int: Idx,
    result_int_str: Idx,
    function: Idx,
    tuple: Idx,
    structure: Idx,
    enumeration: Idx,
}

fn repr_parity_fixture() -> (Pool, ReprParityFixture) {
    let mut pool = Pool::new();
    let option_int = pool.option(Idx::INT);
    let list_str = pool.list(Idx::STR);
    let set_int = pool.set(Idx::INT);
    let channel_int = pool.channel(Idx::INT);
    let range_int = pool.range(Idx::INT);
    let iterator_int = pool.iterator(Idx::INT);
    let double_ended_iterator_int = pool.double_ended_iterator(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let result_int_str = pool.result(Idx::INT, Idx::STR);
    let function = pool.function(&[Idx::INT, Idx::FLOAT], Idx::BOOL);
    let tuple = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
    let type_name = Name::from_raw(500);
    let structure = pool.struct_type(
        type_name,
        &[
            (Name::from_raw(501), Idx::INT),
            (Name::from_raw(502), Idx::FLOAT),
        ],
    );
    let enumeration = pool.enum_type(
        type_name,
        &[
            ori_types::EnumVariant {
                name: Name::from_raw(503),
                field_types: vec![],
            },
            ori_types::EnumVariant {
                name: Name::from_raw(504),
                field_types: vec![Idx::INT],
            },
        ],
    );
    let containers = [
        (option_int, "Option<int>"),
        (list_str, "List<str>"),
        (set_int, "Set<int>"),
        (channel_int, "Channel<int>"),
        (range_int, "Range<int>"),
        (iterator_int, "Iterator<int>"),
        (double_ended_iterator_int, "DoubleEndedIterator<int>"),
    ];
    (
        pool,
        ReprParityFixture {
            containers,
            option_int,
            map_str_int,
            result_int_str,
            function,
            tuple,
            structure,
            enumeration,
        },
    )
}

fn assert_direct_repr_parity(
    fixture: &ReprParityFixture,
    no_plan: &TypeLayoutResolver<'_, '_, '_>,
    with_plan: &TypeLayoutResolver<'_, '_, '_>,
) {
    let primitives = [
        (Idx::INT, "Int"),
        (Idx::FLOAT, "Float"),
        (Idx::BOOL, "Bool"),
        (Idx::STR, "Str"),
        (Idx::CHAR, "Char"),
        (Idx::BYTE, "Byte"),
        (Idx::UNIT, "Unit"),
        (Idx::NEVER, "Never"),
        (Idx::DURATION, "Duration"),
        (Idx::SIZE, "Size"),
        (Idx::ORDERING, "Ordering"),
        (Idx::ERROR, "Error"),
    ];
    for (idx, name) in primitives {
        assert_eq!(
            with_plan.resolve(idx),
            no_plan.resolve(idx),
            "Canonical parity failed for primitive {name} (idx {idx:?})"
        );
    }
    for &(idx, name) in &fixture.containers {
        assert_eq!(
            with_plan.resolve(idx),
            no_plan.resolve(idx),
            "Canonical parity failed for container {name} (idx {idx:?})"
        );
    }
    for (idx, name) in [
        (fixture.map_str_int, "Map<str, int>"),
        (fixture.result_int_str, "Result<int, str>"),
        (fixture.function, "Function"),
        (fixture.tuple, "Tuple"),
    ] {
        assert_eq!(
            with_plan.resolve(idx),
            no_plan.resolve(idx),
            "Canonical parity failed for {name}"
        );
    }
}

fn assert_named_repr_parity(
    label: &str,
    idx: Idx,
    no_plan: &TypeLayoutResolver<'_, '_, '_>,
    with_plan: &TypeLayoutResolver<'_, '_, '_>,
) {
    use inkwell::types::BasicTypeEnum::StructType;

    let (baseline, planned) = (no_plan.resolve(idx), with_plan.resolve(idx));
    let (StructType(baseline), StructType(planned)) = (baseline, planned) else {
        panic!("{label} should resolve to StructType, got {baseline:?} / {planned:?}");
    };
    assert_eq!(
        baseline.count_fields(),
        planned.count_fields(),
        "Canonical parity: {label} field count mismatch"
    );
    for index in 0..baseline.count_fields() {
        assert_eq!(
            baseline.get_field_type_at_index(index),
            planned.get_field_type_at_index(index),
            "Canonical parity: {label} field {index} type mismatch"
        );
    }
}

fn assert_repr_plan_populated(plan: &ori_repr::ReprPlan, fixture: &ReprParityFixture) {
    for (idx, label) in [
        (Idx::INT, "Int"),
        (Idx::STR, "Str"),
        (fixture.option_int, "Option<int>"),
        (fixture.structure, "struct"),
        (fixture.enumeration, "enum"),
    ] {
        assert!(
            plan.get_repr(idx).is_some(),
            "ReprPlan should have a canonical decision for {label}"
        );
    }
}

#[test]
fn repr_plan_canonical_parity_full_matrix() {
    let (pool, fixture) = repr_parity_fixture();

    // Populate a ReprPlan via the real compute_repr_plan pipeline.
    let plan = ori_repr::compute_repr_plan(
        &pool,
        &[], // no arc_functions for canonical-only
        ori_repr::NarrowingPolicy::Disabled,
        &[], // no repr_attrs
    );

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test_parity_matrix");

    // Baseline: TypeInfoStore only (no ReprPlan).
    let no_plan = TypeLayoutResolver::new(&store, &scx, None, None);

    // Under test: populated ReprPlan from compute_repr_plan.
    let with_plan = TypeLayoutResolver::new(&store, &scx, None, Some(&plan));
    assert_direct_repr_parity(&fixture, &no_plan, &with_plan);

    assert_named_repr_parity("struct", fixture.structure, &no_plan, &with_plan);
    assert_named_repr_parity("enum", fixture.enumeration, &no_plan, &with_plan);
    assert_repr_plan_populated(&plan, &fixture);
}

// Iterator triviality convergence tests

/// classify_trivial fallback (via TypeInfoStore::new)
/// must classify Iterator/DoubleEndedIterator as NON-trivial — matching
/// the production path through ReprPlan.
///
/// Box allocation has no RC header, but iterator values still require
/// `ori_iter_drop` at scope exit and therefore remain non-trivial to ARC.
#[test]
fn iterator_non_trivial_via_fallback_path() {
    let mut pool = Pool::new();
    let iter_int = pool.iterator(Idx::INT);
    let de_iter_int = pool.double_ended_iterator(Idx::INT);

    let store = TypeInfoStore::new(&pool);
    assert!(
        !store.is_trivial(iter_int),
        "Iterator<int> should be non-trivial via fallback — needs ori_iter_drop"
    );
    assert!(
        !store.is_trivial(de_iter_int),
        "DoubleEndedIterator<int> should be non-trivial via fallback — needs ori_iter_drop"
    );
}

/// Production path (via TypeInfoStore::new_with_plan) must
/// classify Iterator/DoubleEndedIterator as NON-trivial through ReprPlan.
/// See the fallback-path test above for rationale.
#[test]
fn iterator_non_trivial_via_production_path() {
    let mut pool = Pool::new();
    let iter_int = pool.iterator(Idx::INT);
    let de_iter_int = pool.double_ended_iterator(Idx::INT);

    let plan = ori_repr::compute_repr_plan(&pool, &[], ori_repr::NarrowingPolicy::Disabled, &[]);
    let store = TypeInfoStore::new_with_plan(&pool, &plan);
    assert!(
        !store.is_trivial(iter_int),
        "Iterator<int> should be non-trivial via ReprPlan production path"
    );
    assert!(
        !store.is_trivial(de_iter_int),
        "DoubleEndedIterator<int> should be non-trivial via ReprPlan production path"
    );
}

/// Both paths must agree on Iterator triviality.
/// This test creates both a fallback and production store and asserts they
/// return the same result for Iterator and DoubleEndedIterator.
#[test]
fn iterator_triviality_paths_agree() {
    let mut pool = Pool::new();
    let iter_int = pool.iterator(Idx::INT);
    let de_iter_int = pool.double_ended_iterator(Idx::INT);
    let iter_str = pool.iterator(Idx::STR);

    let plan = ori_repr::compute_repr_plan(&pool, &[], ori_repr::NarrowingPolicy::Disabled, &[]);

    let fallback_store = TypeInfoStore::new(&pool);
    let production_store = TypeInfoStore::new_with_plan(&pool, &plan);

    // Iterator<int> — both paths agree
    assert_eq!(
        fallback_store.is_trivial(iter_int),
        production_store.is_trivial(iter_int),
        "fallback and production must agree on Iterator<int>"
    );
    // DoubleEndedIterator<int> — both paths agree
    assert_eq!(
        fallback_store.is_trivial(de_iter_int),
        production_store.is_trivial(de_iter_int),
        "fallback and production must agree on DoubleEndedIterator<int>"
    );
    // Iterator<str> — both paths agree (str element doesn't affect iterator triviality)
    assert_eq!(
        fallback_store.is_trivial(iter_str),
        production_store.is_trivial(iter_str),
        "fallback and production must agree on Iterator<str>"
    );
}

// pool_type_store_size <-> type_store_size cross-crate sync pin

/// Tag-matrix cross-check: `ori_arc::lower::pool_type_store_size` (Pool
/// level) and `type_size::type_store_size` (LLVM level) compute the same
/// store size for every representable shape. The two are a manual sync
/// contract (lowering bakes elem sizes before the ReprPlan exists); this
/// pin makes drift loud instead of silently miscompiling buffer strides.
#[test]
fn pool_store_size_matches_llvm_store_size_across_tag_matrix() {
    use super::type_size::type_store_size;
    use ori_arc::lower::pool_type_store_size;
    use ori_types::EnumVariant;

    let mut pool = Pool::new();

    let struct_mixed = pool.struct_type(
        Name::from_raw(900),
        &[
            (Name::from_raw(901), Idx::BYTE),
            (Name::from_raw(902), Idx::INT),
            (Name::from_raw(903), Idx::BYTE),
        ],
    );
    let struct_heap = pool.struct_type(
        Name::from_raw(904),
        &[
            (Name::from_raw(905), Idx::STR),
            (Name::from_raw(906), Idx::INT),
        ],
    );
    let tuple_mixed = pool.tuple(&[Idx::CHAR, Idx::CHAR, Idx::INT]);
    let list_int = pool.list(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_int = pool.set(Idx::INT);
    let opt_int = pool.option(Idx::INT);
    let opt_str = pool.option(Idx::STR);
    let res_int_str = pool.result(Idx::INT, Idx::STR);
    let enum_all_unit = pool.enum_type(
        Name::from_raw(910),
        &[
            EnumVariant {
                name: Name::from_raw(911),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(912),
                field_types: vec![],
            },
        ],
    );
    let enum_payload = pool.enum_type(
        Name::from_raw(913),
        &[
            EnumVariant {
                name: Name::from_raw(914),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(915),
                field_types: vec![Idx::STR],
            },
        ],
    );

    let store = TypeInfoStore::new(&pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "test");
    let resolver = TypeLayoutResolver::new(&store, &scx, None, None);

    let matrix: &[(&str, Idx)] = &[
        ("int", Idx::INT),
        ("float", Idx::FLOAT),
        ("bool", Idx::BOOL),
        ("byte", Idx::BYTE),
        ("char", Idx::CHAR),
        ("ordering", Idx::ORDERING),
        ("duration", Idx::DURATION),
        ("size", Idx::SIZE),
        ("str", Idx::STR),
        ("list_int", list_int),
        ("map_str_int", map_str_int),
        ("set_int", set_int),
        ("struct_mixed", struct_mixed),
        ("struct_heap", struct_heap),
        ("tuple_mixed", tuple_mixed),
        ("opt_int", opt_int),
        ("opt_str", opt_str),
        ("res_int_str", res_int_str),
        ("enum_all_unit", enum_all_unit),
        ("enum_payload", enum_payload),
    ];

    let mut visited = 0usize;
    for &(label, idx) in matrix {
        let pool_size = pool_type_store_size(idx, &pool, 0);
        let llvm_size = type_store_size(resolver.resolve(idx));
        assert_eq!(
            u64::try_from(pool_size).unwrap_or(u64::MAX),
            llvm_size,
            "store-size sync drift for `{label}`: pool={pool_size}, llvm={llvm_size}"
        );
        visited += 1;
    }
    assert_eq!(visited, matrix.len(), "matrix cell skipped");
}
