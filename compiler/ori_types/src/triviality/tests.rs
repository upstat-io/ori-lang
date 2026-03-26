//! Tests for transitive triviality classification.
//!
//! Matrix dimensions: Tag (37 variants) × Classification (Trivial/NonTrivial/Unknown)
//!                    × Resolution path (primitive/compound/Named/cycle)

use ori_ir::Name;

use crate::lifetime::LifetimeId;
use crate::triviality::{classify_triviality, Triviality};
use crate::{EnumVariant, Idx, Pool, Tag};

// Primitive tags — all scalar, all trivial

#[test]
fn int_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::INT, &pool), Triviality::Trivial);
}

#[test]
fn float_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::FLOAT, &pool), Triviality::Trivial);
}

#[test]
fn bool_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::BOOL, &pool), Triviality::Trivial);
}

#[test]
fn char_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::CHAR, &pool), Triviality::Trivial);
}

#[test]
fn byte_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::BYTE, &pool), Triviality::Trivial);
}

#[test]
fn unit_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::UNIT, &pool), Triviality::Trivial);
}

#[test]
fn never_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::NEVER, &pool), Triviality::Trivial);
}

#[test]
fn duration_is_trivial() {
    let pool = Pool::new();
    assert_eq!(
        classify_triviality(Idx::DURATION, &pool),
        Triviality::Trivial
    );
}

#[test]
fn size_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::SIZE, &pool), Triviality::Trivial);
}

#[test]
fn ordering_is_trivial() {
    let pool = Pool::new();
    assert_eq!(
        classify_triviality(Idx::ORDERING, &pool),
        Triviality::Trivial
    );
}

#[test]
fn error_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::ERROR, &pool), Triviality::Trivial);
}

// Heap-allocated types — always non-trivial

#[test]
fn str_is_non_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::STR, &pool), Triviality::NonTrivial);
}

#[test]
fn list_int_is_non_trivial() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    assert_eq!(classify_triviality(list_int, &pool), Triviality::NonTrivial);
}

#[test]
fn set_int_is_non_trivial() {
    let mut pool = Pool::new();
    let set_int = pool.set(Idx::INT);
    assert_eq!(classify_triviality(set_int, &pool), Triviality::NonTrivial);
}

#[test]
fn map_str_int_is_non_trivial() {
    let mut pool = Pool::new();
    let map = pool.map(Idx::STR, Idx::INT);
    assert_eq!(classify_triviality(map, &pool), Triviality::NonTrivial);
}

#[test]
fn channel_int_is_non_trivial() {
    let mut pool = Pool::new();
    let channel = pool.channel(Idx::INT);
    assert_eq!(classify_triviality(channel, &pool), Triviality::NonTrivial);
}

// Iterator types — Box-allocated, no RC header → Trivial
// This is a semantic pin: matches ArcClassifier, resolves TypeInfoStore drift

#[test]
fn iterator_int_is_trivial() {
    let mut pool = Pool::new();
    let iter = pool.iterator(Idx::INT);
    assert_eq!(classify_triviality(iter, &pool), Triviality::Trivial);
}

#[test]
fn double_ended_iterator_int_is_trivial() {
    let mut pool = Pool::new();
    let iter = pool.double_ended_iterator(Idx::INT);
    assert_eq!(classify_triviality(iter, &pool), Triviality::Trivial);
}

// Simple containers — triviality depends on inner type

#[test]
fn option_int_is_trivial() {
    let mut pool = Pool::new();
    let opt = pool.option(Idx::INT);
    assert_eq!(classify_triviality(opt, &pool), Triviality::Trivial);
}

#[test]
fn option_str_is_non_trivial() {
    let mut pool = Pool::new();
    let opt = pool.option(Idx::STR);
    assert_eq!(classify_triviality(opt, &pool), Triviality::NonTrivial);
}

#[test]
fn option_option_int_is_trivial() {
    let mut pool = Pool::new();
    let inner = pool.option(Idx::INT);
    let outer = pool.option(inner);
    assert_eq!(classify_triviality(outer, &pool), Triviality::Trivial);
}

#[test]
fn range_int_is_trivial() {
    let mut pool = Pool::new();
    let range = pool.range(Idx::INT);
    assert_eq!(classify_triviality(range, &pool), Triviality::Trivial);
}

// Two-child containers

#[test]
fn result_int_ordering_is_trivial() {
    let mut pool = Pool::new();
    let result = pool.result(Idx::INT, Idx::ORDERING);
    assert_eq!(classify_triviality(result, &pool), Triviality::Trivial);
}

#[test]
fn result_int_str_is_non_trivial() {
    let mut pool = Pool::new();
    let result = pool.result(Idx::INT, Idx::STR);
    assert_eq!(classify_triviality(result, &pool), Triviality::NonTrivial);
}

// Tuples

#[test]
fn tuple_int_float_bool_is_trivial() {
    let mut pool = Pool::new();
    let tuple = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
    assert_eq!(classify_triviality(tuple, &pool), Triviality::Trivial);
}

#[test]
fn tuple_int_str_is_non_trivial() {
    let mut pool = Pool::new();
    let tuple = pool.pair(Idx::INT, Idx::STR);
    assert_eq!(classify_triviality(tuple, &pool), Triviality::NonTrivial);
}

// Structs

#[test]
fn struct_all_scalar_fields_is_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(100);
    let x = Name::from_raw(101);
    let y = Name::from_raw(102);
    let point = pool.struct_type(name, &[(x, Idx::INT), (y, Idx::INT)]);
    assert_eq!(classify_triviality(point, &pool), Triviality::Trivial);
}

#[test]
fn struct_with_str_field_is_non_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(200);
    let n = Name::from_raw(201);
    let age = Name::from_raw(202);
    let person = pool.struct_type(name, &[(n, Idx::STR), (age, Idx::INT)]);
    assert_eq!(classify_triviality(person, &pool), Triviality::NonTrivial);
}

// Enums

#[test]
fn enum_all_scalar_variants_is_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(300);
    let variants = vec![
        EnumVariant {
            name: Name::from_raw(301),
            field_types: vec![],
        },
        EnumVariant {
            name: Name::from_raw(302),
            field_types: vec![Idx::INT],
        },
    ];
    let e = pool.enum_type(name, &variants);
    assert_eq!(classify_triviality(e, &pool), Triviality::Trivial);
}

#[test]
fn enum_one_non_trivial_variant_is_non_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(400);
    let variants = vec![
        EnumVariant {
            name: Name::from_raw(401),
            field_types: vec![Idx::INT],
        },
        EnumVariant {
            name: Name::from_raw(402),
            field_types: vec![Idx::STR],
        },
    ];
    let e = pool.enum_type(name, &variants);
    assert_eq!(classify_triviality(e, &pool), Triviality::NonTrivial);
}

// Function type — closures capture heap refs

#[test]
fn function_is_non_trivial() {
    let mut pool = Pool::new();
    let func = pool.function1(Idx::INT, Idx::INT);
    assert_eq!(classify_triviality(func, &pool), Triviality::NonTrivial);
}

// Named type resolution (newtypes)

#[test]
fn newtype_wrapping_int_is_trivial() {
    let mut pool = Pool::new();
    let user_id = pool.named(Name::from_raw(500));
    // Set the resolution: UserId resolves to int
    pool.set_resolution(user_id, Idx::INT);
    assert_eq!(classify_triviality(user_id, &pool), Triviality::Trivial);
}

#[test]
fn newtype_wrapping_str_is_non_trivial() {
    let mut pool = Pool::new();
    let name_type = pool.named(Name::from_raw(600));
    pool.set_resolution(name_type, Idx::STR);
    assert_eq!(
        classify_triviality(name_type, &pool),
        Triviality::NonTrivial
    );
}

#[test]
fn unresolvable_named_is_unknown() {
    let mut pool = Pool::new();
    let unresolved = pool.named(Name::from_raw(700));
    // No resolution set — stays as Named
    assert_eq!(classify_triviality(unresolved, &pool), Triviality::Unknown);
}

// Type variables — always Unknown

#[test]
fn var_is_unknown() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    assert_eq!(classify_triviality(var, &pool), Triviality::Unknown);
}

// Sentinels

#[test]
fn idx_none_is_trivial() {
    let pool = Pool::new();
    assert_eq!(classify_triviality(Idx::NONE, &pool), Triviality::Trivial);
}

// Compound types with trivial children

#[test]
fn option_trivial_struct_is_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(800);
    let x = Name::from_raw(801);
    let y = Name::from_raw(802);
    let point = pool.struct_type(name, &[(x, Idx::INT), (y, Idx::FLOAT)]);
    let opt = pool.option(point);
    assert_eq!(classify_triviality(opt, &pool), Triviality::Trivial);
}

#[test]
fn result_trivial_pair_is_trivial() {
    let mut pool = Pool::new();
    let a_name = Name::from_raw(900);
    let a_field = Name::from_raw(901);
    let struct_a = pool.struct_type(a_name, &[(a_field, Idx::INT)]);
    let b_name = Name::from_raw(910);
    let b_field = Name::from_raw(911);
    let struct_b = pool.struct_type(b_name, &[(b_field, Idx::FLOAT)]);
    let result = pool.result(struct_a, struct_b);
    assert_eq!(classify_triviality(result, &pool), Triviality::Trivial);
}

// merge_triviality lattice behavior (tested indirectly)

#[test]
fn tuple_with_one_unknown_child_is_unknown() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let tuple = pool.pair(Idx::INT, var);
    assert_eq!(classify_triviality(tuple, &pool), Triviality::Unknown);
}

#[test]
fn result_trivial_ok_unknown_err_is_unknown() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let result = pool.result(Idx::INT, var);
    assert_eq!(classify_triviality(result, &pool), Triviality::Unknown);
}

// Nested newtype resolution

#[test]
fn nested_newtype_resolves_to_trivial() {
    let mut pool = Pool::new();
    let user_id = pool.named(Name::from_raw(1000));
    pool.set_resolution(user_id, Idx::INT);
    let admin_id = pool.named(Name::from_raw(1001));
    pool.set_resolution(admin_id, user_id);
    assert_eq!(classify_triviality(admin_id, &pool), Triviality::Trivial);
}

// Generic instantiation — different instantiations, different results

#[test]
fn generic_struct_with_int_fields_is_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(1100);
    let a = Name::from_raw(1101);
    let b = Name::from_raw(1102);
    // Pair<int> = { a: int, b: int }
    let pair_int = pool.struct_type(name, &[(a, Idx::INT), (b, Idx::INT)]);
    assert_eq!(classify_triviality(pair_int, &pool), Triviality::Trivial);
}

#[test]
fn generic_struct_with_str_fields_is_non_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(1200);
    let a = Name::from_raw(1201);
    let b = Name::from_raw(1202);
    // Pair<str> = { a: str, b: str }
    let pair_str = pool.struct_type(name, &[(a, Idx::STR), (b, Idx::STR)]);
    assert_eq!(classify_triviality(pair_str, &pool), Triviality::NonTrivial);
}

#[test]
fn option_of_generic_trivial_struct_is_trivial() {
    let mut pool = Pool::new();
    let name = Name::from_raw(1300);
    let a = Name::from_raw(1301);
    let b = Name::from_raw(1302);
    let pair_int = pool.struct_type(name, &[(a, Idx::INT), (b, Idx::INT)]);
    let opt = pool.option(pair_int);
    assert_eq!(classify_triviality(opt, &pool), Triviality::Trivial);
}

// §02.6: Result<Pair<int>, Pair<float>> → both arms trivial → Trivial

#[test]
fn result_of_two_trivial_structs_is_trivial() {
    let mut pool = Pool::new();
    let a = Name::from_raw(2300);
    let b = Name::from_raw(2301);
    let pair_int = pool.struct_type(Name::from_raw(2302), &[(a, Idx::INT), (b, Idx::INT)]);
    let c = Name::from_raw(2303);
    let d = Name::from_raw(2304);
    let pair_float = pool.struct_type(Name::from_raw(2305), &[(c, Idx::FLOAT), (d, Idx::FLOAT)]);
    let res = pool.result(pair_int, pair_float);
    assert_eq!(classify_triviality(res, &pool), Triviality::Trivial);
}

// Out-of-bounds index

#[test]
fn out_of_bounds_idx_is_unknown() {
    let pool = Pool::new();
    let oob = Idx::from_raw(999_999);
    assert_eq!(classify_triviality(oob, &pool), Triviality::Unknown);
}

// Recursive type — cycle detection (semantic pin for §02.2)

#[test]
fn recursive_struct_via_option_is_non_trivial() {
    // Simulate: type Node = { value: int, next: Option<Node> }
    // Build: Named("Node") → Struct { value: int, next: Option<Named("Node")> }
    // When classify_recursive walks into Option<Named("Node")>, it resolves
    // Named("Node") → Struct, which is already in the visiting set → NonTrivial.
    let mut pool = Pool::new();
    let node_name = Name::from_raw(1500);
    let value_field = Name::from_raw(1501);
    let next_field = Name::from_raw(1502);

    // Create Named("Node") first
    let node_named = pool.named(node_name);
    // Create Option<Named("Node")>
    let opt_node = pool.option(node_named);
    // Create the struct with fields { value: int, next: Option<Named("Node")> }
    let node_struct = pool.struct_type(
        node_name,
        &[(value_field, Idx::INT), (next_field, opt_node)],
    );
    // Resolve Named("Node") → the struct
    pool.set_resolution(node_named, node_struct);

    assert_eq!(
        classify_triviality(node_named, &pool),
        Triviality::NonTrivial,
        "recursive struct must be NonTrivial (cycle detection)"
    );
}

// Newtype wrapping trivial struct

#[test]
fn newtype_wrapping_trivial_struct_is_trivial() {
    let mut pool = Pool::new();
    let point_name = Name::from_raw(1600);
    let x = Name::from_raw(1601);
    let y = Name::from_raw(1602);
    let point = pool.struct_type(point_name, &[(x, Idx::INT), (y, Idx::FLOAT)]);
    let coord = pool.named(Name::from_raw(1603));
    pool.set_resolution(coord, point);
    assert_eq!(classify_triviality(coord, &pool), Triviality::Trivial);
}

// BoundVar, RigidVar — unresolved type variables → Unknown

#[test]
fn bound_var_is_unknown() {
    let mut pool = Pool::new();
    let bv = pool.intern(Tag::BoundVar, 0);
    assert_eq!(classify_triviality(bv, &pool), Triviality::Unknown);
}

#[test]
fn rigid_var_is_unknown() {
    let mut pool = Pool::new();
    let rv = pool.rigid_var(Name::from_raw(1700));
    assert_eq!(classify_triviality(rv, &pool), Triviality::Unknown);
}

// Borrowed — reserved for future &T, conservative fallback

#[test]
fn borrowed_is_unknown() {
    let mut pool = Pool::new();
    let b = pool.borrowed(Idx::INT, LifetimeId::from_raw(0));
    assert_eq!(classify_triviality(b, &pool), Triviality::Unknown);
}

// Scheme — quantified type, should not reach codegen

#[test]
fn scheme_is_unknown() {
    let mut pool = Pool::new();
    let s = pool.scheme(&[0], Idx::INT);
    assert_eq!(classify_triviality(s, &pool), Triviality::Unknown);
}

// Projection, ModuleNs, Infer, SelfType — internal compiler types

#[test]
fn projection_is_unknown() {
    let mut pool = Pool::new();
    let p = pool.intern(Tag::Projection, 0);
    assert_eq!(classify_triviality(p, &pool), Triviality::Unknown);
}

#[test]
fn module_ns_is_unknown() {
    let mut pool = Pool::new();
    let m = pool.intern(Tag::ModuleNs, 0);
    assert_eq!(classify_triviality(m, &pool), Triviality::Unknown);
}

#[test]
fn infer_is_unknown() {
    let mut pool = Pool::new();
    let i = pool.intern(Tag::Infer, 0);
    assert_eq!(classify_triviality(i, &pool), Triviality::Unknown);
}

#[test]
fn self_type_is_unknown() {
    let mut pool = Pool::new();
    let s = pool.intern(Tag::SelfType, 0);
    assert_eq!(classify_triviality(s, &pool), Triviality::Unknown);
}

// Applied type — resolves through to inner type

#[test]
fn applied_with_trivial_args_resolves_to_trivial() {
    // Applied("Pair", [int, float]) — if Applied resolves to a concrete struct,
    // it's trivial. If unresolvable → Unknown.
    let mut pool = Pool::new();
    let pair_name = Name::from_raw(1800);
    let applied = pool.applied(pair_name, &[Idx::INT, Idx::FLOAT]);
    // Without resolution, Applied cannot be resolved further → Unknown
    assert_eq!(classify_triviality(applied, &pool), Triviality::Unknown);
}

#[test]
fn applied_with_resolution_is_trivial() {
    let mut pool = Pool::new();
    let wrapper_name = Name::from_raw(1900);
    let applied = pool.applied(wrapper_name, &[Idx::INT]);
    // Resolve the Applied type to the underlying int
    pool.set_resolution(applied, Idx::INT);
    assert_eq!(classify_triviality(applied, &pool), Triviality::Trivial);
}

// Alias — resolves through like Named

#[test]
fn alias_unresolvable_is_unknown() {
    let mut pool = Pool::new();
    let a = pool.intern(Tag::Alias, 0);
    assert_eq!(classify_triviality(a, &pool), Triviality::Unknown);
}

// §02.5: Newtype wrapping list → NonTrivial

#[test]
fn newtype_wrapping_list_is_non_trivial() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let wrapper = pool.named(Name::from_raw(2000));
    pool.set_resolution(wrapper, list_int);
    assert_eq!(classify_triviality(wrapper, &pool), Triviality::NonTrivial);
}

// §02.5: Simulated FFI types — Named types resolved to primitives/opaques

#[test]
fn simulated_cptr_resolved_to_int_is_trivial() {
    // CPtr in the real compiler resolves to an opaque pointer at the Pool level.
    // In this test, we simulate it as resolving to Int (both are trivial).
    let mut pool = Pool::new();
    let cptr = pool.named(Name::from_raw(2100));
    pool.set_resolution(cptr, Idx::INT);
    assert_eq!(classify_triviality(cptr, &pool), Triviality::Trivial);
}

#[test]
fn simulated_c_int_resolved_to_int_is_trivial() {
    let mut pool = Pool::new();
    let c_int = pool.named(Name::from_raw(2101));
    pool.set_resolution(c_int, Idx::INT);
    assert_eq!(classify_triviality(c_int, &pool), Triviality::Trivial);
}

#[test]
fn option_of_simulated_cptr_is_trivial() {
    let mut pool = Pool::new();
    let cptr = pool.named(Name::from_raw(2102));
    pool.set_resolution(cptr, Idx::INT);
    let opt_cptr = pool.option(cptr);
    assert_eq!(classify_triviality(opt_cptr, &pool), Triviality::Trivial);
}

#[test]
fn simulated_ffi_struct_all_c_types_is_trivial() {
    let mut pool = Pool::new();
    // Simulate c_int and c_float as Named types resolving to primitives.
    let c_int_ty = pool.named(Name::from_raw(2103));
    pool.set_resolution(c_int_ty, Idx::INT);
    let c_float_ty = pool.named(Name::from_raw(2104));
    pool.set_resolution(c_float_ty, Idx::FLOAT);
    // FFI struct containing only C types.
    let sn = Name::from_raw(2105);
    let f1 = Name::from_raw(2106);
    let f2 = Name::from_raw(2107);
    let ffi_struct = pool.struct_type(sn, &[(f1, c_int_ty), (f2, c_float_ty)]);
    assert_eq!(classify_triviality(ffi_struct, &pool), Triviality::Trivial);
}

// §02.5: Unresolved generic newtype → Unknown

#[test]
fn unresolved_generic_newtype_is_unknown() {
    // Simulate: `type Wrapper<T> = T` where T hasn't been monomorphized yet.
    // The Named type doesn't resolve (no set_resolution) → Unknown.
    let mut pool = Pool::new();
    let wrapper = pool.named(Name::from_raw(2200));
    // No resolution set — simulates unresolved generic parameter.
    assert_eq!(classify_triviality(wrapper, &pool), Triviality::Unknown);
}
