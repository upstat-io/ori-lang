use super::*;
use ori_ir::Name;
use ori_types::EnumVariant;
use ori_types::Pool;

fn test_store() -> (&'static Pool, TypeInfoStore<'static>) {
    let pool: &'static Pool = Box::leak(Box::new(Pool::new()));
    let store = TypeInfoStore::new(pool);
    (pool, store)
}

// -- abi_size tests --

#[test]
fn primitive_abi_sizes() {
    let (_pool, store) = test_store();

    assert_eq!(abi_size(Idx::INT, &store, None), 8);
    assert_eq!(abi_size(Idx::FLOAT, &store, None), 8);
    assert_eq!(abi_size(Idx::BOOL, &store, None), 1);
    assert_eq!(abi_size(Idx::CHAR, &store, None), 4);
    assert_eq!(abi_size(Idx::BYTE, &store, None), 1);
    assert_eq!(abi_size(Idx::UNIT, &store, None), 8);
    assert_eq!(abi_size(Idx::ORDERING, &store, None), 1);
}

#[test]
fn composite_abi_sizes() {
    let (_pool, store) = test_store();

    // str: {i64, i64, ptr} = 24
    assert_eq!(abi_size(Idx::STR, &store, None), 24);
}

#[test]
fn list_abi_size_is_indirect() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    // [int]: {i64, i64, ptr} = 24
    assert_eq!(abi_size(list_int, &store, None), 24);
}

#[test]
fn map_abi_size_is_indirect() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let store = TypeInfoStore::new(&pool);

    // {str: int}: {i64, i64, ptr} = 24
    assert_eq!(abi_size(map_ty, &store, None), 24);
}

#[test]
fn option_int_is_direct() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    // option[int]: {i8, i64} = 16
    assert_eq!(abi_size(opt_int, &store, None), 16);
}

#[test]
fn tuple_abi_size_computed_recursively() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let store = TypeInfoStore::new(&pool);

    // (int, float) = 8 + 8 = 16
    assert_eq!(abi_size(tup, &store, None), 16);
}

#[test]
fn large_tuple_exceeds_threshold() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::STR]);
    let store = TypeInfoStore::new(&pool);

    // (int, float, str) = 8 + 8 + 24 = 40
    assert_eq!(abi_size(tup, &store, None), 40);
}

#[test]
fn mixed_alignment_tuple_includes_padding() {
    // abi_size matches LLVM layout for mixed-alignment aggregates:
    // (byte, int, byte) = 1 + 7 pad + 8 + 1 + 7 trailing pad = 24.
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::BYTE, Idx::INT, Idx::BYTE]);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(abi_size(tup, &store, None), 24);

    // 24 > 16 — classified Indirect, matching the LLVM layout the
    // callee actually sees. A padding-unaware Direct here misaligns
    // registers (the BUG-04-071 misclassification class).
    assert_eq!(
        compute_param_passing(tup, &store, None),
        ParamPassing::Indirect { alignment: 8 },
        "mixed-alignment tuple must classify by padded LLVM size"
    );
}

#[test]
fn mixed_alignment_struct_includes_padding() {
    // struct { byte, int } = 1 + 7 pad + 8 = 16 — boundary Direct.
    let mut pool = Pool::new();
    let s = pool.struct_type(
        Name::from_raw(950),
        &[
            (Name::from_raw(951), Idx::BYTE),
            (Name::from_raw(952), Idx::INT),
        ],
    );
    let store = TypeInfoStore::new(&pool);
    assert_eq!(abi_size(s, &store, None), 16);
    assert_eq!(compute_param_passing(s, &store, None), ParamPassing::Direct);
}

#[test]
fn uniform_small_alignment_tuple_stays_compact() {
    // (byte, byte, byte) = 3 bytes, alignment 1 — no padding inflation.
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::BYTE, Idx::BYTE, Idx::BYTE]);
    let store = TypeInfoStore::new(&pool);
    assert_eq!(abi_size(tup, &store, None), 3);
    assert_eq!(
        compute_param_passing(tup, &store, None),
        ParamPassing::Direct
    );
}

// -- Enum abi_size tests --

#[test]
fn all_unit_enum_abi_size_is_tag_size() {
    // all-unit enum with ≤256 variants has an i8 tag → 1 byte.
    let mut pool = Pool::new();
    let dir = pool.enum_type(
        Name::from_raw(100),
        &[
            EnumVariant {
                name: Name::from_raw(101),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(102),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(103),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(104),
                field_types: vec![],
            },
        ],
    );
    let store = TypeInfoStore::new(&pool);

    // All-unit enum with 4 variants: { i8 tag } = 1 byte
    assert_eq!(abi_size(dir, &store, None), 1);
}

#[test]
fn enum_with_payload_abi_size() {
    let mut pool = Pool::new();
    let color = pool.enum_type(
        Name::from_raw(200),
        &[
            EnumVariant {
                name: Name::from_raw(201),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(202),
                field_types: vec![],
            },
        ],
    );
    let store = TypeInfoStore::new(&pool);

    // { i8 tag, [1 x i64] payload } — tag padded to 8 due to
    // payload alignment. Total = 8 + 8 = 16 bytes.
    assert_eq!(abi_size(color, &store, None), 16);
}

/// Regression: `abi_size` for payload enums must account for
/// [M x i64] slot layout. A(int, bool) | B has payload [2 x i64] = 16,
/// total = 8 (padded tag) + 16 = 24.
#[test]
fn enum_with_mixed_payload_abi_size() {
    let mut pool = Pool::new();
    let mixed = pool.enum_type(
        Name::from_raw(300),
        &[
            EnumVariant {
                name: Name::from_raw(301),
                field_types: vec![Idx::INT, Idx::BOOL],
            },
            EnumVariant {
                name: Name::from_raw(302),
                field_types: vec![],
            },
        ],
    );
    let store = TypeInfoStore::new(&pool);

    // { i8, [2 x i64] } = 8 + 16 = 24 bytes
    assert_eq!(abi_size(mixed, &store, None), 24);
    // 24 > 16 → Sret, not Direct
    assert_eq!(
        compute_return_passing(mixed, &store, None),
        ReturnPassing::Sret { alignment: 8 }
    );
}

// -- Param passing tests --

#[test]
fn param_passing_direct_for_small_types() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_param_passing(Idx::INT, &store, None),
        ParamPassing::Direct
    );
    // Str is 24 bytes (SSO layout) — exceeds 16-byte Direct threshold
    assert_eq!(
        compute_param_passing(Idx::STR, &store, None),
        ParamPassing::Indirect { alignment: 8 }
    );
}

#[test]
fn param_passing_void_for_unit() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_param_passing(Idx::UNIT, &store, None),
        ParamPassing::Void
    );
    assert_eq!(
        compute_param_passing(Idx::NEVER, &store, None),
        ParamPassing::Void
    );
}

#[test]
fn param_passing_indirect_for_large_types() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(
        compute_param_passing(list_int, &store, None),
        ParamPassing::Indirect { alignment: 8 }
    );
}

// -- Return passing tests --

#[test]
fn return_passing_direct_for_small_types() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_return_passing(Idx::INT, &store, None),
        ReturnPassing::Direct
    );
    // Str is 24 bytes (SSO layout) — returned via sret pointer
    assert_eq!(
        compute_return_passing(Idx::STR, &store, None),
        ReturnPassing::Sret { alignment: 8 }
    );
}

#[test]
fn return_passing_void_for_unit() {
    let (_pool, store) = test_store();

    assert_eq!(
        compute_return_passing(Idx::UNIT, &store, None),
        ReturnPassing::Void
    );
}

#[test]
fn return_passing_sret_for_large_types() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(
        compute_return_passing(list_int, &store, None),
        ReturnPassing::Sret { alignment: 8 }
    );
}

#[test]
fn return_passing_sret_for_map() {
    let mut pool = Pool::new();
    let map_ty = pool.map(Idx::STR, Idx::INT);
    let store = TypeInfoStore::new(&pool);

    assert_eq!(
        compute_return_passing(map_ty, &store, None),
        ReturnPassing::Sret { alignment: 8 }
    );
}

// -- Calling convention tests --

#[test]
fn call_conv_fast_for_ordinary_functions() {
    assert_eq!(select_call_conv(CallConvSite::OriFunction), CallConv::Fast);
}

#[test]
fn call_conv_c_for_main() {
    assert_eq!(select_call_conv(CallConvSite::Main), CallConv::C);
}

#[test]
fn call_conv_c_for_non_capturing_lambda() {
    assert_eq!(
        select_call_conv(CallConvSite::NonCapturingLambda),
        CallConv::C
    );
}

#[test]
fn call_conv_c_for_test_wrapper() {
    assert_eq!(select_call_conv(CallConvSite::TestWrapper), CallConv::C);
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
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 2,
        param_defaults: vec![],
        param_hashes: vec![0; 2],
        return_hash: 0,
        return_projection: None,
    };

    let abi = compute_function_abi(&sig, &store, None);

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
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 0,
        param_defaults: vec![],
        param_hashes: vec![],
        return_hash: 0,
        return_projection: None,
    };

    let abi = compute_function_abi(&sig, &store, None);

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
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 0,
        param_defaults: vec![],
        param_hashes: vec![],
        return_hash: 0,
        return_projection: None,
    };

    let abi = compute_function_abi(&sig, &store, None);
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
            None,
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
            None,
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
            None,
            Ownership::Borrowed,
            ArcClass::Scalar,
        ),
        ParamPassing::Direct
    );
}

#[test]
fn owned_definiteref_uses_size_based() {
    let (_pool, store) = test_store();
    // str (24 bytes, > 16-byte threshold) + Owned → Indirect (size-based)
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::STR,
            &store,
            None,
            Ownership::Owned,
            ArcClass::DefiniteRef,
        ),
        ParamPassing::Indirect { alignment: 8 }
    );
}

#[test]
fn owned_scalar_stays_direct() {
    let (_pool, store) = test_store();
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::INT,
            &store,
            None,
            Ownership::Owned,
            ArcClass::Scalar
        ),
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
            None,
            Ownership::Borrowed,
            ArcClass::Scalar,
        ),
        ParamPassing::Void
    );
    assert_eq!(
        compute_param_passing_with_ownership(
            Idx::NEVER,
            &store,
            None,
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
            None,
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
            None,
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
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 2,
        param_defaults: vec![],
        param_hashes: vec![0; 2],
        return_hash: 0,
        return_projection: None,
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

    let abi =
        compute_function_abi_with_ownership(&sig, &store, Some(&annotated), &classifier, None);

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
        is_fbip: false,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: 1,
        param_defaults: vec![],
        param_hashes: vec![0; 1],
        return_hash: 0,
        return_projection: None,
    };

    // No borrow info → falls through to standard compute_function_abi
    // Str is 24 bytes (SSO layout), so Indirect
    let abi = compute_function_abi_with_ownership(&sig, &store, None, &classifier, None);
    assert_eq!(
        abi.params[0].passing,
        ParamPassing::Indirect { alignment: 8 }
    );
}

// -- abi_size / abi_alignment ↔ LLVM layout sync tests --
//
// The abi walkers in `size.rs` mirror `TypeLayoutResolver`'s lowered LLVM
// layouts. These tests enforce the mirror mechanically: for a representative
// type corpus, the abi-side size/alignment MUST equal the store size /
// alignment of the LLVM type the resolver actually produces. A divergence
// flips AB-1 Direct/Indirect classification (register-misalignment SIGSEGV
// class).

use inkwell::context::Context;

use crate::codegen::type_info::type_size::{type_abi_alignment, type_store_size};
use crate::codegen::type_info::TypeLayoutResolver;
use crate::context::SimpleCx;

/// Representative corpus: primitives, fat pointers, Option/Result (niche +
/// explicit), tagless/explicit/payload enums, padded structs and tuples.
fn layout_sync_corpus(pool: &mut Pool) -> Vec<(&'static str, Idx)> {
    let list_int = pool.list(Idx::INT);
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let set_int = pool.set(Idx::INT);
    let opt_int = pool.option(Idx::INT);
    let opt_str = pool.option(Idx::STR);
    let opt_opt_int = {
        let inner = pool.option(Idx::INT);
        pool.option(inner)
    };
    let res_int_str = pool.result(Idx::INT, Idx::STR);
    let res_str_str = pool.result(Idx::STR, Idx::STR);
    let tup_mixed = pool.tuple(&[Idx::BYTE, Idx::INT, Idx::BYTE]);
    let tup_small = pool.tuple(&[Idx::BYTE, Idx::BYTE, Idx::BYTE]);
    let tup_large = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::STR]);
    let struct_padded = pool.struct_type(
        Name::from_raw(960),
        &[
            (Name::from_raw(961), Idx::BYTE),
            (Name::from_raw(962), Idx::INT),
            (Name::from_raw(963), Idx::CHAR),
        ],
    );
    let struct_nested = pool.struct_type(
        Name::from_raw(964),
        &[
            (Name::from_raw(965), struct_padded),
            (Name::from_raw(966), Idx::STR),
        ],
    );
    let mut corpus = vec![
        ("int", Idx::INT),
        ("float", Idx::FLOAT),
        ("bool", Idx::BOOL),
        ("char", Idx::CHAR),
        ("byte", Idx::BYTE),
        ("unit", Idx::UNIT),
        ("ordering", Idx::ORDERING),
        ("str", Idx::STR),
        ("[int]", list_int),
        ("{str: int}", map_str_int),
        ("Set<int>", set_int),
        ("Option<int>", opt_int),
        ("Option<str>", opt_str),
        ("Option<Option<int>>", opt_opt_int),
        ("Result<int, str>", res_int_str),
        ("Result<str, str>", res_str_str),
        ("(byte, int, byte)", tup_mixed),
        ("(byte, byte, byte)", tup_small),
        ("(int, float, str)", tup_large),
        ("struct{byte,int,char}", struct_padded),
        ("struct{struct,str}", struct_nested),
    ];
    corpus.extend(layout_sync_enum_corpus(pool));
    corpus
}

/// Enum cells of the layout sync corpus (all-unit, payload, single-variant
/// tagless, void-payload-field, and an all-unit enum embedded in a struct).
fn layout_sync_enum_corpus(pool: &mut Pool) -> Vec<(&'static str, Idx)> {
    let enum_all_unit = pool.enum_type(
        Name::from_raw(970),
        &[
            EnumVariant {
                name: Name::from_raw(971),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(972),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(973),
                field_types: vec![],
            },
        ],
    );
    let enum_payload = pool.enum_type(
        Name::from_raw(974),
        &[
            EnumVariant {
                name: Name::from_raw(975),
                field_types: vec![Idx::INT, Idx::BOOL],
            },
            EnumVariant {
                name: Name::from_raw(976),
                field_types: vec![],
            },
        ],
    );
    let enum_single_variant = pool.enum_type(
        Name::from_raw(977),
        &[EnumVariant {
            name: Name::from_raw(978),
            field_types: vec![Idx::INT, Idx::FLOAT],
        }],
    );
    // Void payload fields are skipped by the resolver's payload sizing —
    // the abi walker must skip them identically.
    let enum_unit_payload = pool.enum_type(
        Name::from_raw(980),
        &[
            EnumVariant {
                name: Name::from_raw(981),
                field_types: vec![Idx::UNIT, Idx::INT],
            },
            EnumVariant {
                name: Name::from_raw(982),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::from_raw(983),
                field_types: vec![],
            },
        ],
    );
    // All-unit enum embedded in a struct — pins the tag-only alignment
    // (1, not 8) inside aggregate padding computation.
    let struct_with_unit_enum = pool.struct_type(
        Name::from_raw(984),
        &[
            (Name::from_raw(985), Idx::BYTE),
            (Name::from_raw(986), enum_all_unit),
            (Name::from_raw(987), Idx::INT),
        ],
    );

    vec![
        ("enum all-unit x3", enum_all_unit),
        ("enum A(int,bool)|B", enum_payload),
        ("enum single-variant", enum_single_variant),
        ("enum A(void,int)|B|C", enum_unit_payload),
        ("struct{byte,all-unit-enum,int}", struct_with_unit_enum),
    ]
}

/// Assert size + alignment agreement for every corpus entry under one
/// `repr_plan` configuration.
fn assert_layout_sync(
    pool: &Pool,
    corpus: &[(&'static str, Idx)],
    repr_plan: Option<&ReprPlan>,
    config_label: &str,
) {
    let store = TypeInfoStore::new(pool);
    let ctx = Context::create();
    let scx = SimpleCx::new(&ctx, "abi_sync_test");
    let resolver = TypeLayoutResolver::new(&store, &scx, None, repr_plan);

    for &(label, ty) in corpus {
        let llvm_ty = resolver.resolve(ty);
        assert_eq!(
            abi_size(ty, &store, repr_plan),
            type_store_size(llvm_ty),
            "{config_label}: abi_size diverges from LLVM store size for {label}"
        );
        assert_eq!(
            abi_alignment(ty, &store, repr_plan, 0),
            type_abi_alignment(llvm_ty),
            "{config_label}: abi_alignment diverges from LLVM alignment for {label}"
        );
    }
}

#[test]
fn abi_size_and_alignment_match_llvm_layout_without_repr_plan() {
    let mut pool = Pool::new();
    let corpus = layout_sync_corpus(&mut pool);
    assert_layout_sync(&pool, &corpus, None, "repr_plan=None");
}

#[test]
fn abi_size_and_alignment_match_llvm_layout_with_niche_fallback_plan() {
    // An empty ReprPlan still activates the canonical niche/tagless fallback
    // ladder (`enum_repr_with_fallback`) on BOTH the abi walkers and the
    // layout resolver — covering the niche-encoded Option/Result and
    // tagless-enum layouts production codegen sees.
    let mut pool = Pool::new();
    let corpus = layout_sync_corpus(&mut pool);
    let plan = ReprPlan::new(ori_repr::NarrowingPolicy::Disabled);
    assert_layout_sync(&pool, &corpus, Some(&plan), "repr_plan=empty(fallback)");
}
