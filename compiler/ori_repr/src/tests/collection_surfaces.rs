use super::*;

#[test]
fn imported_collection_surface_does_not_suppress_narrowing() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();

    let type_name = interner.intern("Wrapper");
    let field_xs = interner.intern("xs");
    let list_int = pool.list(Idx::INT);
    let _struct_idx = pool.struct_type(type_name, &[(field_xs, list_int)]);

    let list_int_hash = pool.hash(list_int);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[list_int_hash],
        &[],
        false,
    );

    assert!(
        !plan.is_public_type(list_int),
        "Imported collection surface should NOT suppress narrowing"
    );
}

#[test]
fn imported_collection_surface_unknown_hash_no_panic() {
    let mut pool = Pool::new();
    let bogus_hash = 0xDEAD_BEEF_CAFE_BABE;

    let list_int = pool.list(Idx::INT);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[bogus_hash],
        &[],
        false,
    );

    assert!(
        !plan.is_public_type(list_int),
        "Unknown collection-surface hash must not mark any local type public"
    );
}

#[test]
fn imported_collection_surface_empty_is_noop() {
    let mut pool = Pool::new();
    let interner = ori_ir::StringInterner::new();
    let type_name = interner.intern("Pixel");
    let field_r = interner.intern("r");
    let named_idx = pool.named(type_name);
    let struct_idx = pool.struct_type(type_name, &[(field_r, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);

    let plan_without = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    let plan_with_empty = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[named_idx],
        &[],
        &[],
        &[],
        false,
    );

    assert_eq!(
        plan_without.get_repr(struct_idx),
        plan_with_empty.get_repr(struct_idx),
        "Empty collection surfaces should not change narrowing behavior"
    );
}

#[test]
fn imported_collection_surfaces_multiple_hashes_no_panic() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);
    let set_int = pool.set(Idx::INT);

    let list_hash = pool.hash(list_int);
    let set_hash = pool.hash(set_int);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[list_hash, set_hash],
        &[],
        false,
    );

    assert!(
        !plan.is_public_type(list_int),
        "Imported surface should NOT mark List<int> as public"
    );
    assert!(
        !plan.is_public_type(set_int),
        "Imported surface should NOT mark Set<int> as public"
    );
}

#[test]
fn imported_collection_surface_allows_private_narrowing() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);
    let list_hash = pool.hash(list_int);

    let plan_with_import = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[list_hash],
        &[],
        false,
    );

    let plan_without_import = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[],
        &[],
        &[],
        &[],
        false,
    );

    assert!(
        !plan_with_import.is_public_type(list_int),
        "Imported surface should NOT mark [int] as public"
    );

    assert!(
        !plan_without_import.is_public_type(list_int),
        "Without imports, private [int] is not marked public"
    );
}

#[test]
fn local_public_function_still_suppresses_narrowing() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);

    let plan = crate::compute_repr_plan_with_interner(
        &pool,
        &[],
        NarrowingPolicy::Aggressive,
        &[],
        None,
        &[list_int],
        &[],
        &[],
        &[],
        false,
    );

    assert!(
        plan.is_public_type(list_int),
        "Local public function should suppress [int] narrowing"
    );
}
