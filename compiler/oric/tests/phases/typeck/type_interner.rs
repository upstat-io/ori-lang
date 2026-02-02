//! Tests for type interning (`ori_types::type_interner`).
//!
//! These tests verify:
//! - Primitive type pre-interning (Int, Float, Bool, etc.)
//! - Intern/lookup roundtrips
//! - Fresh type variable generation
//! - Complex type interning (Result, Function, Tuple)
//! - Shared interner behavior
//! - Type-to-TypeId conversions

use ori_ir::Name;
use ori_types::{SharedTypeInterner, Type, TypeData, TypeInterner, TypeVar};

// TypeId is in ori_ir, not ori_types
use ori_ir::TypeId;

#[test]
fn test_primitive_preinterned() {
    let interner = TypeInterner::new();

    // Primitives should return pre-interned IDs
    assert_eq!(interner.intern(TypeData::Int), TypeId::INT);
    assert_eq!(interner.intern(TypeData::Float), TypeId::FLOAT);
    assert_eq!(interner.intern(TypeData::Bool), TypeId::BOOL);
    assert_eq!(interner.intern(TypeData::Str), TypeId::STR);
    assert_eq!(interner.intern(TypeData::Char), TypeId::CHAR);
    assert_eq!(interner.intern(TypeData::Byte), TypeId::BYTE);
    assert_eq!(interner.intern(TypeData::Unit), TypeId::VOID);
    assert_eq!(interner.intern(TypeData::Never), TypeId::NEVER);
}

#[test]
fn test_intern_and_lookup() {
    let interner = TypeInterner::new();

    let list_int = interner.list(TypeId::INT);
    let list_bool = interner.list(TypeId::BOOL);
    let list_int2 = interner.list(TypeId::INT);

    // Same type returns same ID
    assert_eq!(list_int, list_int2);
    // Different types return different IDs
    assert_ne!(list_int, list_bool);

    // Lookup returns correct data
    assert_eq!(interner.lookup(list_int), TypeData::List(TypeId::INT));
    assert_eq!(interner.lookup(list_bool), TypeData::List(TypeId::BOOL));
}

#[test]
fn test_fresh_var() {
    let interner = TypeInterner::new();

    let var1 = interner.fresh_var();
    let var2 = interner.fresh_var();

    // Each fresh_var is different
    assert_ne!(var1, var2);

    // Lookup returns Var with incrementing IDs
    let data1 = interner.lookup(var1);
    let data2 = interner.lookup(var2);
    match (&data1, &data2) {
        (TypeData::Var(v1), TypeData::Var(v2)) => {
            assert_eq!(v1.0, 0);
            assert_eq!(v2.0, 1);
        }
        _ => panic!("Expected TypeData::Var but got {data1:?} and {data2:?}"),
    }
}

#[test]
fn test_complex_types() {
    let interner = TypeInterner::new();

    // Result<int, str>
    let result_type = interner.result(TypeId::INT, TypeId::STR);

    // Function(int, bool) -> str
    let fn_type = interner.function(vec![TypeId::INT, TypeId::BOOL], TypeId::STR);

    // Tuple(int, bool)
    let tuple_type = interner.tuple(vec![TypeId::INT, TypeId::BOOL]);

    // All different
    assert_ne!(result_type, fn_type);
    assert_ne!(fn_type, tuple_type);

    // Lookup verifies structure
    let result_data = interner.lookup(result_type);
    match result_data {
        TypeData::Result { ok, err } => {
            assert_eq!(ok, TypeId::INT);
            assert_eq!(err, TypeId::STR);
        }
        _ => panic!("Expected TypeData::Result but got {result_data:?}"),
    }

    let fn_data = interner.lookup(fn_type);
    match fn_data {
        TypeData::Function { params, ret } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], TypeId::INT);
            assert_eq!(params[1], TypeId::BOOL);
            assert_eq!(ret, TypeId::STR);
        }
        _ => panic!("Expected TypeData::Function but got {fn_data:?}"),
    }
}

#[test]
fn test_shared_interner() {
    let interner = SharedTypeInterner::new();
    let interner2 = interner.clone();

    let list1 = interner.list(TypeId::INT);
    let list2 = interner2.list(TypeId::INT);

    // Same type from shared interner
    assert_eq!(list1, list2);
}

#[test]
fn test_error_type() {
    let interner = TypeInterner::new();

    let error = interner.error();
    let error2 = interner.intern(TypeData::Error);

    assert_eq!(error, error2);
    assert!(interner.lookup(error).is_error());
}

#[test]
fn test_named_types() {
    let interner = TypeInterner::new();

    // Create two named types with different Names
    let name1 = Name::new(1, 100);
    let name2 = Name::new(1, 200);

    let named1 = interner.named(name1);
    let named2 = interner.named(name2);
    let named1_again = interner.named(name1);

    assert_eq!(named1, named1_again);
    assert_ne!(named1, named2);
}

#[test]
fn test_applied_generic() {
    let interner = TypeInterner::new();

    let name = Name::new(0, 50); // e.g., "Vec"

    // Vec<int>
    let vec_int = interner.applied(name, vec![TypeId::INT]);
    // Vec<bool>
    let vec_bool = interner.applied(name, vec![TypeId::BOOL]);
    // Vec<int> again
    let vec_int2 = interner.applied(name, vec![TypeId::INT]);

    assert_eq!(vec_int, vec_int2);
    assert_ne!(vec_int, vec_bool);
}

#[test]
fn test_type_to_type_id_roundtrip() {
    let interner = TypeInterner::new();

    // Test primitives
    assert_eq!(interner.to_type(TypeId::INT), Type::Int);
    assert_eq!(interner.to_type(TypeId::FLOAT), Type::Float);
    assert_eq!(interner.to_type(TypeId::BOOL), Type::Bool);
    assert_eq!(interner.to_type(TypeId::STR), Type::Str);

    // Test Type -> TypeId -> Type roundtrip for primitives
    assert_eq!(interner.to_type(Type::Int.to_type_id(&interner)), Type::Int);
    assert_eq!(
        interner.to_type(Type::Float.to_type_id(&interner)),
        Type::Float
    );
}

#[test]
fn test_container_type_roundtrip() {
    let interner = TypeInterner::new();

    // List<int>
    let list_int = Type::List(Box::new(Type::Int));
    let list_id = list_int.to_type_id(&interner);
    let roundtrip = interner.to_type(list_id);
    assert_eq!(roundtrip, list_int);

    // Option<str>
    let opt_str = Type::Option(Box::new(Type::Str));
    let opt_id = opt_str.to_type_id(&interner);
    assert_eq!(interner.to_type(opt_id), opt_str);

    // Result<int, str>
    let result_ty = Type::Result {
        ok: Box::new(Type::Int),
        err: Box::new(Type::Str),
    };
    let result_id = result_ty.to_type_id(&interner);
    assert_eq!(interner.to_type(result_id), result_ty);

    // Map<str, int>
    let map_ty = Type::Map {
        key: Box::new(Type::Str),
        value: Box::new(Type::Int),
    };
    let map_id = map_ty.to_type_id(&interner);
    assert_eq!(interner.to_type(map_id), map_ty);
}

#[test]
fn test_function_type_roundtrip() {
    let interner = TypeInterner::new();

    // (int, bool) -> str
    let fn_ty = Type::Function {
        params: vec![Type::Int, Type::Bool],
        ret: Box::new(Type::Str),
    };
    let fn_id = fn_ty.to_type_id(&interner);
    assert_eq!(interner.to_type(fn_id), fn_ty);
}

#[test]
fn test_tuple_type_roundtrip() {
    let interner = TypeInterner::new();

    let tuple_ty = Type::Tuple(vec![Type::Int, Type::Bool, Type::Str]);
    let tuple_id = tuple_ty.to_type_id(&interner);
    assert_eq!(interner.to_type(tuple_id), tuple_ty);
}

#[test]
fn test_nested_type_roundtrip() {
    let interner = TypeInterner::new();

    // [[int]] - nested list
    let nested = Type::List(Box::new(Type::List(Box::new(Type::Int))));
    let nested_id = nested.to_type_id(&interner);
    assert_eq!(interner.to_type(nested_id), nested);

    // Option<Result<int, str>>
    let complex = Type::Option(Box::new(Type::Result {
        ok: Box::new(Type::Int),
        err: Box::new(Type::Str),
    }));
    let complex_id = complex.to_type_id(&interner);
    assert_eq!(interner.to_type(complex_id), complex);
}

#[test]
fn test_var_type_roundtrip() {
    let interner = TypeInterner::new();

    let var = Type::Var(TypeVar::new(42));
    let var_id = var.to_type_id(&interner);
    assert_eq!(interner.to_type(var_id), var);
}

#[test]
fn test_named_type_roundtrip() {
    let interner = TypeInterner::new();

    let name = Name::new(0, 100);
    let named = Type::Named(name);
    let named_id = named.to_type_id(&interner);
    assert_eq!(interner.to_type(named_id), named);
}

#[test]
fn test_applied_type_roundtrip() {
    let interner = TypeInterner::new();

    let name = Name::new(0, 50);
    let applied = Type::Applied {
        name,
        args: vec![Type::Int, Type::Bool],
    };
    let applied_id = applied.to_type_id(&interner);
    assert_eq!(interner.to_type(applied_id), applied);
}

#[test]
fn test_projection_type_roundtrip() {
    let interner = TypeInterner::new();

    let trait_name = Name::new(0, 10);
    let assoc_name = Name::new(0, 20);
    let projection = Type::Projection {
        base: Box::new(Type::Var(TypeVar::new(5))),
        trait_name,
        assoc_name,
    };
    let proj_id = projection.to_type_id(&interner);
    assert_eq!(interner.to_type(proj_id), projection);
}

#[test]
fn test_same_type_same_id() {
    let interner = TypeInterner::new();

    // Two structurally equal types should get the same TypeId
    let list1 = Type::List(Box::new(Type::Int));
    let list2 = Type::List(Box::new(Type::Int));

    let id1 = list1.to_type_id(&interner);
    let id2 = list2.to_type_id(&interner);

    assert_eq!(id1, id2);
}

#[test]
fn test_module_namespace() {
    let interner = TypeInterner::new();

    let name1 = Name::new(0, 100); // e.g., "add"
    let name2 = Name::new(0, 200); // e.g., "subtract"

    // Create a module namespace with two function items
    let fn_type1 = interner.function(vec![TypeId::INT, TypeId::INT], TypeId::INT);
    let fn_type2 = interner.function(vec![TypeId::INT, TypeId::INT], TypeId::INT);

    let ns_id = interner.module_namespace(vec![(name1, fn_type1), (name2, fn_type2)]);

    // Verify lookup returns correct data
    match interner.lookup(ns_id) {
        TypeData::ModuleNamespace { items } => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].0, name1);
            assert_eq!(items[0].1, fn_type1);
            assert_eq!(items[1].0, name2);
            assert_eq!(items[1].1, fn_type2);
        }
        _ => panic!("Expected ModuleNamespace"),
    }
}

#[test]
fn test_module_namespace_roundtrip() {
    let interner = TypeInterner::new();

    let name1 = Name::new(0, 100);
    let name2 = Name::new(0, 200);

    let ns_ty = Type::ModuleNamespace {
        items: vec![
            (
                name1,
                Type::Function {
                    params: vec![Type::Int],
                    ret: Box::new(Type::Int),
                },
            ),
            (
                name2,
                Type::Function {
                    params: vec![Type::Str],
                    ret: Box::new(Type::Bool),
                },
            ),
        ],
    };
    let ns_id = ns_ty.to_type_id(&interner);
    assert_eq!(interner.to_type(ns_id), ns_ty);
}
