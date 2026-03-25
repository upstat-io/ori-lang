//! Tests for transitive triviality classification.
//!
//! Matrix dimensions: Tag (37 variants) × Classification (Trivial/NonTrivial/Unknown)
//!                    × Resolution path (primitive/compound/Named/cycle)

use ori_ir::Name;

use crate::triviality::{classify_triviality, Triviality};
use crate::{EnumVariant, Idx, Pool};

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

// Out-of-bounds index

#[test]
fn out_of_bounds_idx_is_unknown() {
    let pool = Pool::new();
    let oob = Idx::from_raw(999_999);
    assert_eq!(classify_triviality(oob, &pool), Triviality::Unknown);
}
