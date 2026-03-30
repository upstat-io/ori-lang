use crate::pool::{EnumVariant, Pool};
use crate::Idx;

use super::walk_collection_types;

/// Direct List type is discovered.
#[test]
fn direct_list_discovered() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    let mut found = Vec::new();
    walk_collection_types(&pool, list_int, &mut |idx| found.push(idx));

    assert!(found.contains(&list_int), "List<int> should be found");
}

/// Direct Set type is discovered.
#[test]
fn direct_set_discovered() {
    let mut pool = Pool::new();
    let set_int = pool.set(Idx::INT);

    let mut found = Vec::new();
    walk_collection_types(&pool, set_int, &mut |idx| found.push(idx));

    assert!(found.contains(&set_int), "Set<int> should be found");
}

/// List nested inside Option is discovered.
#[test]
fn option_wrapped_list_discovered() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let opt_list = pool.option(list_int);

    let mut found = Vec::new();
    walk_collection_types(&pool, opt_list, &mut |idx| found.push(idx));

    assert!(
        found.contains(&list_int),
        "List<int> inside Option should be found"
    );
}

/// List inside Result's ok type is discovered.
#[test]
fn result_ok_list_discovered() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let result_ty = pool.result(list_int, Idx::STR);

    let mut found = Vec::new();
    walk_collection_types(&pool, result_ty, &mut |idx| found.push(idx));

    assert!(
        found.contains(&list_int),
        "List<int> in Result ok should be found"
    );
}

/// List inside a tuple is discovered.
#[test]
fn tuple_nested_list_discovered() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let tuple_ty = pool.tuple(&[Idx::INT, list_int, Idx::BOOL]);

    let mut found = Vec::new();
    walk_collection_types(&pool, tuple_ty, &mut |idx| found.push(idx));

    assert!(
        found.contains(&list_int),
        "List<int> in tuple should be found"
    );
}

/// Map values containing a list are discovered.
#[test]
fn map_value_list_discovered() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let map_ty = pool.map(Idx::STR, list_int);

    let mut found = Vec::new();
    walk_collection_types(&pool, map_ty, &mut |idx| found.push(idx));

    assert!(
        found.contains(&list_int),
        "List<int> in map value should be found"
    );
}

/// Struct field containing a list is discovered.
#[test]
fn struct_field_list_discovered() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let list_int = pool.list(Idx::INT);
    let name_wrapper = interner.intern("Wrapper");
    let struct_ty = pool.struct_type(
        name_wrapper,
        &[
            (interner.intern("xs"), list_int),
            (interner.intern("n"), Idx::INT),
        ],
    );

    let mut found = Vec::new();
    walk_collection_types(&pool, struct_ty, &mut |idx| found.push(idx));

    assert!(
        found.contains(&list_int),
        "List<int> in struct field should be found"
    );
}

/// Enum variant payload containing a set is discovered.
#[test]
fn enum_variant_set_discovered() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let set_int = pool.set(Idx::INT);
    let name_container = interner.intern("Container");
    let enum_ty = pool.enum_type(
        name_container,
        &[
            EnumVariant {
                name: interner.intern("A"),
                field_types: vec![],
            },
            EnumVariant {
                name: interner.intern("B"),
                field_types: vec![set_int],
            },
        ],
    );

    let mut found = Vec::new();
    walk_collection_types(&pool, enum_ty, &mut |idx| found.push(idx));

    assert!(
        found.contains(&set_int),
        "Set<int> in enum variant should be found"
    );
}

/// Non-collection type yields no results.
#[test]
fn primitive_yields_nothing() {
    let pool = Pool::new();

    let mut found = Vec::new();
    walk_collection_types(&pool, Idx::INT, &mut |idx| found.push(idx));

    assert!(
        found.is_empty(),
        "Primitive int should yield no collections"
    );
}

/// Deeply nested structure (Option<Result<[int], str>>) works.
#[test]
fn deeply_nested_discovered() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let result_ty = pool.result(list_int, Idx::STR);
    let opt_result = pool.option(result_ty);

    let mut found = Vec::new();
    walk_collection_types(&pool, opt_result, &mut |idx| found.push(idx));

    assert!(
        found.contains(&list_int),
        "Deeply nested List<int> should be found"
    );
}
