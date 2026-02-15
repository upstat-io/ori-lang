use super::*;
use ori_types::Pool;

fn test_store() -> (Pool, TypeInfoStore<'static>) {
    let pool = Pool::new();
    // SAFETY: We're creating the store in the same scope as the pool.
    // The store borrows the pool, and we return both together.
    // This is safe because both live for the duration of the test.
    let store = unsafe {
        let pool_ptr = &raw const pool;
        TypeInfoStore::new(&*pool_ptr)
    };
    (pool, store)
}

// -- abi_size tests --

#[test]
fn primitive_abi_sizes() {
    let (_pool, store) = test_store();

    assert_eq!(abi_size(Idx::INT, &store), 8);
    assert_eq!(abi_size(Idx::FLOAT, &store), 8);
    assert_eq!(abi_size(Idx::BOOL, &store), 1);
    assert_eq!(abi_size(Idx::CHAR, &store), 4);
    assert_eq!(abi_size(Idx::BYTE, &store), 1);
    assert_eq!(abi_size(Idx::UNIT, &store), 8);
    assert_eq!(abi_size(Idx::ORDERING, &store), 1);
}

#[test]
fn composite_abi_sizes() {
    let (_pool, store) = test_store();

    // str: {i64, ptr} = 16
    assert_eq!(abi_size(Idx::STR, &store), 16);
}

#[test]
fn list_abi_size_is_indirect() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    // [int]: {i64, i64, ptr} = 24
    assert_eq!(abi_size(list_int, &store), 24);
}

#[test]
fn map_abi_size_is_indirect() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let store = TypeInfoStore::new(&pool);

    // {str: int}: {i64, i64, ptr, ptr} = 32
    assert_eq!(abi_size(map_ty, &store), 32);
}

#[test]
fn option_int_is_direct() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    // option[int]: {i8, i64} = 16
    assert_eq!(abi_size(opt_int, &store), 16);
}

#[test]
fn tuple_abi_size_computed_recursively() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let store = TypeInfoStore::new(&pool);

    // (int, float) = 8 + 8 = 16
    assert_eq!(abi_size(tup, &store), 16);
}

#[test]
fn large_tuple_exceeds_threshold() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::STR]);
    let store = TypeInfoStore::new(&pool);

    // (int, float, str) = 8 + 8 + 16 = 32
    assert_eq!(abi_size(tup, &store), 32);
}

// -- Param passing tests --

#[test]
fn param_passing_direct_for_small_types() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_param_passing(Idx::INT, &store),
        ParamPassing::Direct
    );
    assert_eq!(
        compute_param_passing(Idx::STR, &store),
        ParamPassing::Direct
    );
}

#[test]
fn param_passing_void_for_unit() {
    let (_pool, store) = test_store();

    assert_eq!(compute_param_passing(Idx::UNIT, &store), ParamPassing::Void);
    assert_eq!(
        compute_param_passing(Idx::NEVER, &store),
        ParamPassing::Void
    );
}

#[test]
fn param_passing_indirect_for_large_types() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(
        compute_param_passing(list_int, &store),
        ParamPassing::Indirect { alignment: 8 }
    );
}

// -- Return passing tests --

#[test]
fn return_passing_direct_for_small_types() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_return_passing(Idx::INT, &store),
        ReturnPassing::Direct
    );
    assert_eq!(
        compute_return_passing(Idx::STR, &store),
        ReturnPassing::Direct
    );
}

#[test]
fn return_passing_void_for_unit() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_return_passing(Idx::UNIT, &store),
        ReturnPassing::Void
    );
}

#[test]
fn return_passing_sret_for_large_types() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(
        compute_return_passing(list_int, &store),
        ReturnPassing::Sret { alignment: 8 }
    );
}

#[test]
fn return_passing_sret_for_map() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(
        compute_return_passing(map_ty, &store),
        ReturnPassing::Sret { alignment: 8 }
    );
}

// -- Calling convention tests --

#[test]
fn call_conv_fast_for_normal_functions() {
    assert_eq!(select_call_conv("my_func", false, false), CallConv::Fast);
    assert_eq!(select_call_conv("add", false, false), CallConv::Fast);
}

#[test]
fn call_conv_c_for_main() {
    assert_eq!(select_call_conv("main", true, false), CallConv::C);
}

#[test]
fn call_conv_c_for_extern() {
    assert_eq!(select_call_conv("ffi_func", false, true), CallConv::C);
}

#[test]
fn call_conv_c_for_runtime() {
    assert_eq!(select_call_conv("ori_print", false, false), CallConv::C);
}

// -- compute_function_abi e2e --

#[test]
fn compute_abi_simple_function() {
    let pool = Pool::new();
    let store = TypeInfoStore::new(&pool);

    let sig = FunctionSig {
        name: Name::from_raw(1),
        type_params: vec![],
        const_params: vec![],
        param_names: vec![Name::from_raw(2), Name::from_raw(3)],
        param_types: vec![Idx::INT, Idx::INT],
        return_type: Idx::INT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 2,
        param_defaults: vec![],
    };

    let abi = compute_function_abi(&sig, &store);

    assert_eq!(abi.params.len(), 2);
    assert_eq!(abi.params[0].passing, ParamPassing::Direct);
    assert_eq!(abi.params[1].passing, ParamPassing::Direct);
    assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
    assert_eq!(abi.call_conv, CallConv::Fast);
}

#[test]
fn compute_abi_void_return() {
    let pool = Pool::new();
    let store = TypeInfoStore::new(&pool);

    let sig = FunctionSig {
        name: Name::from_raw(1),
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::UNIT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 0,
        param_defaults: vec![],
    };

    let abi = compute_function_abi(&sig, &store);

    assert!(abi.params.is_empty());
    assert_eq!(abi.return_abi.passing, ReturnPassing::Void);
}

#[test]
fn compute_abi_main_uses_c_convention() {
    let pool = Pool::new();
    let store = TypeInfoStore::new(&pool);

    let sig = FunctionSig {
        name: Name::from_raw(1),
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::UNIT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: true,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 0,
        param_defaults: vec![],
    };

    let abi = compute_function_abi(&sig, &store);
    assert_eq!(abi.call_conv, CallConv::C);
}

// -- Borrow-aware param passing tests --

#[test]
fn borrowed_definiteref_becomes_reference() {
    let (_pool, store) = test_store();
    // str is DefiniteRef (heap-allocated), Borrowed → Reference
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::STR,
            &store,
            Ownership::Borrowed,
            ArcClass::DefiniteRef,
        ),
        ParamPassing::Reference
    );
}

#[test]
fn borrowed_possibleref_becomes_reference() {
    let (_pool, store) = test_store();
    // PossibleRef + Borrowed → Reference (conservative: might need RC)
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::STR,
            &store,
            Ownership::Borrowed,
            ArcClass::PossibleRef,
        ),
        ParamPassing::Reference
    );
}

#[test]
fn borrowed_scalar_stays_direct() {
    let (_pool, store) = test_store();
    // int is Scalar — Borrowed doesn't change passing (no RC regardless)
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::INT,
            &store,
            Ownership::Borrowed,
            ArcClass::Scalar,
        ),
        ParamPassing::Direct
    );
}

#[test]
fn owned_definiteref_uses_size_based() {
    let (_pool, store) = test_store();
    // str (16 bytes, ≤ threshold) + Owned → Direct (size-based)
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::STR,
            &store,
            Ownership::Owned,
            ArcClass::DefiniteRef,
        ),
        ParamPassing::Direct
    );
}

#[test]
fn owned_scalar_stays_direct() {
    let (_pool, store) = test_store();
    assert_eq!(
        compute_param_passing_with_ownership(Idx::INT, &store, Ownership::Owned, ArcClass::Scalar,),
        ParamPassing::Direct
    );
}

#[test]
fn unit_always_void_regardless_of_ownership() {
    let (_pool, store) = test_store();
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::UNIT,
            &store,
            Ownership::Borrowed,
            ArcClass::Scalar,
        ),
        ParamPassing::Void
    );
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::NEVER,
            &store,
            Ownership::Owned,
            ArcClass::Scalar,
        ),
        ParamPassing::Void
    );
}

#[test]
fn owned_large_type_stays_indirect() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);
    // [int] (24 bytes, > threshold) + Owned → Indirect
    assert_eq!(
        compute_param_passing_with_ownership(
            list_int,
            &store,
            Ownership::Owned,
            ArcClass::DefiniteRef,
        ),
        ParamPassing::Indirect { alignment: 8 }
    );
}

#[test]
fn borrowed_large_type_becomes_reference() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);
    // [int] + Borrowed + DefiniteRef → Reference (not Indirect)
    assert_eq!(
        compute_param_passing_with_ownership(
            list_int,
            &store,
            Ownership::Borrowed,
            ArcClass::DefiniteRef,
        ),
        ParamPassing::Reference
    );
}

// -- compute_function_abi_with_ownership e2e --

#[test]
fn abi_with_ownership_uses_reference_for_borrowed_params() {
    let pool = Pool::new();
    let store = TypeInfoStore::new(&pool);
    let classifier = ArcClassifier::new(&pool);

    let sig = FunctionSig {
        name: Name::from_raw(1),
        type_params: vec![],
        const_params: vec![],
        param_names: vec![Name::from_raw(2), Name::from_raw(3)],
        param_types: vec![Idx::STR, Idx::INT],
        return_type: Idx::INT,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 2,
        param_defaults: vec![],
    };

    let annotated = AnnotatedSig {
        params: vec![
            ori_arc::AnnotatedParam {
                name: Name::from_raw(2),
                ty: Idx::STR,
                ownership: Ownership::Borrowed,
            },
            ori_arc::AnnotatedParam {
                name: Name::from_raw(3),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            },
        ],
        return_type: Idx::INT,
    };

    let abi = compute_function_abi_with_ownership(&sig, &store, Some(&annotated), &classifier);

    // str param is Borrowed + DefiniteRef → Reference
    assert_eq!(abi.params[0].passing, ParamPassing::Reference);
    // int param is Owned + Scalar → Direct
    assert_eq!(abi.params[1].passing, ParamPassing::Direct);
    assert_eq!(abi.return_abi.passing, ReturnPassing::Direct);
}

#[test]
fn abi_with_ownership_none_falls_through() {
    let pool = Pool::new();
    let store = TypeInfoStore::new(&pool);
    let classifier = ArcClassifier::new(&pool);

    let sig = FunctionSig {
        name: Name::from_raw(1),
        type_params: vec![],
        const_params: vec![],
        param_names: vec![Name::from_raw(2)],
        param_types: vec![Idx::STR],
        return_type: Idx::STR,
        capabilities: vec![],
        is_public: false,
        is_test: false,
        is_main: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        required_params: 1,
        param_defaults: vec![],
    };

    // No borrow info → falls through to standard compute_function_abi
    let abi = compute_function_abi_with_ownership(&sig, &store, None, &classifier);
    assert_eq!(abi.params[0].passing, ParamPassing::Direct);
}
