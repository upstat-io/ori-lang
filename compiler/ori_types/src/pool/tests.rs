use std::collections::HashSet;

use super::*;

use crate::pool::construct::EnumVariant;
use crate::tag::Tag;

// === Structural Equality Reference Implementation (02.3) ===
//
// Recursively compares types by structure (tag + children), ignoring Idx
// values. Used to cross-check Merkle hash correctness: hash equality must
// match structural equality (no false positives or negatives).

/// Reference structural equality across two independent pools.
///
/// Compares types by their recursive structure, not by pool-local Idx.
/// `O(tree_depth)` per comparison — test-only, not for production.
fn structural_eq(p1: &Pool, idx1: Idx, p2: &Pool, idx2: Idx) -> bool {
    let tag1 = p1.tag(idx1);
    let tag2 = p2.tag(idx2);
    if tag1 != tag2 {
        return false;
    }

    match tag1 {
        // Primitives: tag equality is sufficient
        t if t.is_primitive() => true,

        // Simple containers: compare child
        t if t.has_child_in_data() => {
            let child1 = Idx::from_raw(p1.data(idx1));
            let child2 = Idx::from_raw(p2.data(idx2));
            structural_eq(p1, child1, p2, child2)
        }

        // Two-child / borrowed
        Tag::Map => structural_eq_pair(
            p1,
            p1.map_key(idx1),
            p1.map_value(idx1),
            p2,
            p2.map_key(idx2),
            p2.map_value(idx2),
        ),
        Tag::Result => structural_eq_pair(
            p1,
            p1.result_ok(idx1),
            p1.result_err(idx1),
            p2,
            p2.result_ok(idx2),
            p2.result_err(idx2),
        ),
        Tag::Borrowed => {
            p1.borrowed_lifetime(idx1) == p2.borrowed_lifetime(idx2)
                && structural_eq(p1, p1.borrowed_inner(idx1), p2, p2.borrowed_inner(idx2))
        }

        // Function: compare param count + each param + return
        Tag::Function => structural_eq_function(p1, idx1, p2, idx2),
        // Tuple: compare element count + each element
        Tag::Tuple => structural_eq_slices(p1, &p1.tuple_elems(idx1), p2, &p2.tuple_elems(idx2)),
        // Struct/Enum: compare via dedicated helpers
        Tag::Struct => structural_eq_struct(p1, idx1, p2, idx2),
        Tag::Enum => structural_eq_enum(p1, idx1, p2, idx2),

        // Named: compare name
        Tag::Named => p1.named_name(idx1) == p2.named_name(idx2),
        // Applied: compare name + type args
        Tag::Applied => {
            p1.applied_name(idx1) == p2.applied_name(idx2)
                && structural_eq_slices(p1, &p1.applied_args(idx1), p2, &p2.applied_args(idx2))
        }

        // Scheme: compare var IDs + body
        Tag::Scheme => {
            p1.scheme_vars(idx1) == p2.scheme_vars(idx2)
                && structural_eq(p1, p1.scheme_body(idx1), p2, p2.scheme_body(idx2))
        }

        // Type variables / special types: compare data directly
        _ => p1.data(idx1) == p2.data(idx2),
    }
}

/// Compare a pair of child types across pools.
fn structural_eq_pair(p1: &Pool, a1: Idx, b1: Idx, p2: &Pool, a2: Idx, b2: Idx) -> bool {
    structural_eq(p1, a1, p2, a2) && structural_eq(p1, b1, p2, b2)
}

/// Compare parallel slices of child types across pools.
fn structural_eq_slices(p1: &Pool, s1: &[Idx], p2: &Pool, s2: &[Idx]) -> bool {
    s1.len() == s2.len()
        && s1
            .iter()
            .zip(s2)
            .all(|(a, b)| structural_eq(p1, *a, p2, *b))
}

/// Compare function types across pools (params + return).
fn structural_eq_function(p1: &Pool, idx1: Idx, p2: &Pool, idx2: Idx) -> bool {
    structural_eq_slices(p1, &p1.function_params(idx1), p2, &p2.function_params(idx2))
        && structural_eq(p1, p1.function_return(idx1), p2, p2.function_return(idx2))
}

/// Compare struct types across pools (name + fields).
fn structural_eq_struct(p1: &Pool, idx1: Idx, p2: &Pool, idx2: Idx) -> bool {
    if p1.struct_name(idx1) != p2.struct_name(idx2) {
        return false;
    }
    let fields1 = p1.struct_fields(idx1);
    let fields2 = p2.struct_fields(idx2);
    fields1.len() == fields2.len()
        && fields1
            .iter()
            .zip(&fields2)
            .all(|((n1, t1), (n2, t2))| n1 == n2 && structural_eq(p1, *t1, p2, *t2))
}

/// Compare enum types across pools (name + variants).
fn structural_eq_enum(p1: &Pool, idx1: Idx, p2: &Pool, idx2: Idx) -> bool {
    if p1.enum_name(idx1) != p2.enum_name(idx2) {
        return false;
    }
    let vars1 = p1.enum_variants(idx1);
    let vars2 = p2.enum_variants(idx2);
    vars1.len() == vars2.len()
        && vars1
            .iter()
            .zip(&vars2)
            .all(|((n1, fs1), (n2, fs2))| n1 == n2 && structural_eq_slices(p1, fs1, p2, fs2))
}

#[test]
fn primitives_at_correct_indices() {
    let pool = Pool::new();

    assert_eq!(pool.tag(Idx::INT), Tag::Int);
    assert_eq!(pool.tag(Idx::FLOAT), Tag::Float);
    assert_eq!(pool.tag(Idx::BOOL), Tag::Bool);
    assert_eq!(pool.tag(Idx::STR), Tag::Str);
    assert_eq!(pool.tag(Idx::CHAR), Tag::Char);
    assert_eq!(pool.tag(Idx::BYTE), Tag::Byte);
    assert_eq!(pool.tag(Idx::UNIT), Tag::Unit);
    assert_eq!(pool.tag(Idx::NEVER), Tag::Never);
    assert_eq!(pool.tag(Idx::ERROR), Tag::Error);
    assert_eq!(pool.tag(Idx::DURATION), Tag::Duration);
    assert_eq!(pool.tag(Idx::SIZE), Tag::Size);
    assert_eq!(pool.tag(Idx::ORDERING), Tag::Ordering);
}

#[test]
fn primitive_flags_correct() {
    let pool = Pool::new();

    let int_flags = pool.flags(Idx::INT);
    assert!(int_flags.contains(TypeFlags::IS_PRIMITIVE));
    assert!(int_flags.contains(TypeFlags::IS_RESOLVED));
    assert!(int_flags.contains(TypeFlags::IS_MONO));
    assert!(!int_flags.has_errors());

    let error_flags = pool.flags(Idx::ERROR);
    assert!(error_flags.contains(TypeFlags::IS_PRIMITIVE));
    assert!(error_flags.has_errors());
}

#[test]
fn pool_starts_with_primitives() {
    let pool = Pool::new();
    assert_eq!(pool.len(), Idx::FIRST_DYNAMIC as usize);
}

// === Cross-Pool Merkle Hash Stability Tests ===
//
// These tests verify the core Merkle hashing invariant:
//   For any type T interned in Pool P1 as idx1 and in Pool P2 as idx2,
//   P1.hash(idx1) == P2.hash(idx2)
//
// Each test creates two pools and deliberately shifts indices in one pool
// by interning unrelated types first. This ensures hashes depend on
// structure (Merkle), not on pool position (old compute_hash).

#[test]
fn merkle_primitive_hashes_stable_across_pools() {
    let p1 = Pool::new();
    let p2 = Pool::new();

    let primitives = [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
        Idx::NEVER,
        Idx::ERROR,
        Idx::DURATION,
        Idx::SIZE,
        Idx::ORDERING,
    ];

    for idx in primitives {
        assert_eq!(
            p1.hash(idx),
            p2.hash(idx),
            "Primitive {:?} hash differs across pools",
            p1.tag(idx)
        );
    }
}

#[test]
fn merkle_list_stable_despite_different_interning_order() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2's indices by interning unrelated types first
    let _ = p2.list(Idx::FLOAT);
    let _ = p2.option(Idx::BOOL);
    let _ = p2.set(Idx::STR);

    let list_int_1 = p1.list(Idx::INT);
    let list_int_2 = p2.list(Idx::INT);

    // Indices differ (p2 has more items) but hashes must match
    assert_ne!(list_int_1, list_int_2, "Indices should differ");
    assert_eq!(
        p1.hash(list_int_1),
        p2.hash(list_int_2),
        "List<int> hash must be stable across pools"
    );
}

#[test]
fn merkle_simple_containers_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.map(Idx::INT, Idx::STR);
    let _ = p2.function(&[Idx::BOOL], Idx::INT);

    // All simple container types
    let pairs = [
        (p1.list(Idx::INT), p2.list(Idx::INT), "List<int>"),
        (p1.option(Idx::BOOL), p2.option(Idx::BOOL), "Option<bool>"),
        (p1.set(Idx::STR), p2.set(Idx::STR), "Set<str>"),
        (
            p1.channel(Idx::FLOAT),
            p2.channel(Idx::FLOAT),
            "Channel<float>",
        ),
        (p1.range(Idx::INT), p2.range(Idx::INT), "Range<int>"),
        (
            p1.iterator(Idx::CHAR),
            p2.iterator(Idx::CHAR),
            "Iterator<char>",
        ),
        (
            p1.double_ended_iterator(Idx::BYTE),
            p2.double_ended_iterator(Idx::BYTE),
            "DoubleEndedIterator<byte>",
        ),
    ];

    for (idx1, idx2, desc) in pairs {
        assert_eq!(p1.hash(idx1), p2.hash(idx2), "{desc} hash mismatch");
    }
}

#[test]
fn merkle_two_child_containers_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.list(Idx::BYTE);
    let _ = p2.option(Idx::DURATION);

    let map1 = p1.map(Idx::STR, Idx::INT);
    let map2 = p2.map(Idx::STR, Idx::INT);
    assert_eq!(p1.hash(map1), p2.hash(map2), "Map<str, int> hash mismatch");

    let res1 = p1.result(Idx::INT, Idx::STR);
    let res2 = p2.result(Idx::INT, Idx::STR);
    assert_eq!(
        p1.hash(res1),
        p2.hash(res2),
        "Result<int, str> hash mismatch"
    );
}

#[test]
fn merkle_function_type_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.list(Idx::INT);
    let _ = p2.map(Idx::STR, Idx::BOOL);

    // (int, str) -> bool
    let f1 = p1.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let f2 = p2.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    assert_eq!(p1.hash(f1), p2.hash(f2), "(int, str) -> bool hash mismatch");

    // () -> void
    let g1 = p1.function0(Idx::UNIT);
    let g2 = p2.function0(Idx::UNIT);
    assert_eq!(p1.hash(g1), p2.hash(g2), "() -> void hash mismatch");
}

#[test]
fn merkle_tuple_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.function(&[Idx::INT], Idx::BOOL);

    let t1 = p1.pair(Idx::INT, Idx::STR);
    let t2 = p2.pair(Idx::INT, Idx::STR);
    assert_eq!(p1.hash(t1), p2.hash(t2), "(int, str) tuple hash mismatch");

    let t3 = p1.triple(Idx::BOOL, Idx::FLOAT, Idx::CHAR);
    let t4 = p2.triple(Idx::BOOL, Idx::FLOAT, Idx::CHAR);
    assert_eq!(
        p1.hash(t3),
        p2.hash(t4),
        "(bool, float, char) tuple hash mismatch"
    );
}

#[test]
fn merkle_struct_nominal_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.list(Idx::INT);
    let _ = p2.pair(Idx::STR, Idx::BOOL);

    let name = ori_ir::Name::from_raw(100);
    let field_x = ori_ir::Name::from_raw(101);
    let field_y = ori_ir::Name::from_raw(102);

    let s1 = p1.struct_type(name, &[(field_x, Idx::INT), (field_y, Idx::FLOAT)]);
    let s2 = p2.struct_type(name, &[(field_x, Idx::INT), (field_y, Idx::FLOAT)]);
    assert_eq!(
        p1.hash(s1),
        p2.hash(s2),
        "Struct Point {{ x: int, y: float }} hash mismatch"
    );
}

#[test]
fn merkle_enum_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.map(Idx::STR, Idx::INT);

    let name = ori_ir::Name::from_raw(200);
    let some_name = ori_ir::Name::from_raw(201);
    let none_name = ori_ir::Name::from_raw(202);

    let variants = vec![
        EnumVariant {
            name: some_name,
            field_types: vec![Idx::INT],
        },
        EnumVariant {
            name: none_name,
            field_types: vec![],
        },
    ];

    let e1 = p1.enum_type(name, &variants);
    let e2 = p2.enum_type(name, &variants);
    assert_eq!(
        p1.hash(e1),
        p2.hash(e2),
        "Enum MyOption {{ Some(int), None }} hash mismatch"
    );
}

#[test]
fn merkle_nested_containers_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2 by interning in a completely different order
    let _ = p2.option(Idx::FLOAT);
    let _ = p2.set(Idx::CHAR);
    let _ = p2.map(Idx::BOOL, Idx::BYTE);

    // List<List<int>> — depth 2
    let inner1 = p1.list(Idx::INT);
    let outer1 = p1.list(inner1);

    let inner2 = p2.list(Idx::INT);
    let outer2 = p2.list(inner2);

    assert_eq!(
        p1.hash(outer1),
        p2.hash(outer2),
        "List<List<int>> hash mismatch"
    );

    // Map<str, List<int>> — mixed nesting
    let map1 = p1.map(Idx::STR, inner1);
    let map2 = p2.map(Idx::STR, inner2);
    assert_eq!(
        p1.hash(map1),
        p2.hash(map2),
        "Map<str, List<int>> hash mismatch"
    );
}

#[test]
fn merkle_function_with_compound_params_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.triple(Idx::INT, Idx::INT, Idx::INT);

    // (List<int>, Map<str, bool>) -> Option<float>
    let list_int_1 = p1.list(Idx::INT);
    let map_sb_1 = p1.map(Idx::STR, Idx::BOOL);
    let opt_f_1 = p1.option(Idx::FLOAT);
    let f1 = p1.function(&[list_int_1, map_sb_1], opt_f_1);

    let list_int_2 = p2.list(Idx::INT);
    let map_sb_2 = p2.map(Idx::STR, Idx::BOOL);
    let opt_f_2 = p2.option(Idx::FLOAT);
    let f2 = p2.function(&[list_int_2, map_sb_2], opt_f_2);

    assert_eq!(
        p1.hash(f1),
        p2.hash(f2),
        "(List<int>, Map<str, bool>) -> Option<float> hash mismatch"
    );
}

#[test]
fn merkle_named_type_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.list(Idx::INT);

    let name = ori_ir::Name::from_raw(42);
    let n1 = p1.named(name);
    let n2 = p2.named(name);

    assert_eq!(p1.hash(n1), p2.hash(n2), "Named type hash mismatch");
}

#[test]
fn merkle_applied_type_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.option(Idx::STR);

    let name = ori_ir::Name::from_raw(50);
    let a1 = p1.applied(name, &[Idx::INT, Idx::BOOL]);
    let a2 = p2.applied(name, &[Idx::INT, Idx::BOOL]);

    assert_eq!(
        p1.hash(a1),
        p2.hash(a2),
        "Applied T<int, bool> hash mismatch"
    );
}

#[test]
fn merkle_scheme_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.map(Idx::INT, Idx::INT);
    let _ = p2.list(Idx::BOOL);

    // forall [0]. List<Var(0)> — a simple generic scheme
    let var1 = p1.fresh_var();
    let body1 = p1.list(var1);
    let scheme1 = p1.scheme(&[0], body1);

    let var2 = p2.fresh_var();
    let body2 = p2.list(var2);
    let scheme2 = p2.scheme(&[0], body2);

    assert_eq!(
        p1.hash(scheme1),
        p2.hash(scheme2),
        "Scheme forall [0]. List<T> hash mismatch"
    );
}

#[test]
fn merkle_different_types_have_different_hashes() {
    let mut pool = Pool::new();

    let list_int = pool.list(Idx::INT);
    let list_str = pool.list(Idx::STR);
    let opt_int = pool.option(Idx::INT);

    // Different element types
    assert_ne!(
        pool.hash(list_int),
        pool.hash(list_str),
        "List<int> and List<str> should have different hashes"
    );

    // Different containers, same element
    assert_ne!(
        pool.hash(list_int),
        pool.hash(opt_int),
        "List<int> and Option<int> should have different hashes"
    );
}

#[test]
fn merkle_find_tuple_uses_merkle_hash() {
    let mut pool = Pool::new();

    let t = pool.pair(Idx::INT, Idx::STR);
    let found = pool.find_tuple(&[Idx::INT, Idx::STR]);

    assert_eq!(found, Some(t), "find_tuple should find the interned tuple");
}

#[test]
fn merkle_struct_different_names_different_hashes() {
    let mut pool = Pool::new();

    let name_a = ori_ir::Name::from_raw(300);
    let name_b = ori_ir::Name::from_raw(301);
    let field = ori_ir::Name::from_raw(302);

    let s_a = pool.struct_type(name_a, &[(field, Idx::INT)]);
    let s_b = pool.struct_type(name_b, &[(field, Idx::INT)]);

    // Nominal typing: different names → different hashes
    assert_ne!(
        pool.hash(s_a),
        pool.hash(s_b),
        "Structs with different names must have different hashes"
    );
}

#[test]
fn merkle_struct_with_compound_fields_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.function(&[Idx::INT], Idx::BOOL);
    let _ = p2.set(Idx::FLOAT);

    let name = ori_ir::Name::from_raw(400);
    let f1_name = ori_ir::Name::from_raw(401);
    let f2_name = ori_ir::Name::from_raw(402);

    // struct S { items: List<int>, lookup: Map<str, bool> }
    let list1 = p1.list(Idx::INT);
    let map1 = p1.map(Idx::STR, Idx::BOOL);
    let s1 = p1.struct_type(name, &[(f1_name, list1), (f2_name, map1)]);

    let list2 = p2.list(Idx::INT);
    let map2 = p2.map(Idx::STR, Idx::BOOL);
    let s2 = p2.struct_type(name, &[(f1_name, list2), (f2_name, map2)]);

    assert_eq!(
        p1.hash(s1),
        p2.hash(s2),
        "Struct with compound fields hash mismatch"
    );
}

/// The critical Merkle correctness litmus test: a container of a non-primitive
/// type at different Idx positions across pools. This is the case the old
/// `compute_hash` would fail — `List` would hash raw `Idx(50)` vs `Idx(30)`.
#[test]
fn merkle_container_of_struct_shifted() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2 heavily — 20 noise entries
    for i in 0..20 {
        let _ = p2.list(Idx::from_raw(i % 12));
    }

    // Create struct in both pools — will get DIFFERENT Idx values
    let name = ori_ir::Name::from_raw(500);
    let field_x = ori_ir::Name::from_raw(501);
    let field_y = ori_ir::Name::from_raw(502);
    let fields = [(field_x, Idx::INT), (field_y, Idx::INT)];

    let struct_1 = p1.struct_type(name, &fields);
    let struct_2 = p2.struct_type(name, &fields);

    // Struct Idx values must differ (p2 shifted)
    assert_ne!(struct_1, struct_2);
    // Struct hashes must be identical (same name, same fields)
    assert_eq!(p1.hash(struct_1), p2.hash(struct_2));

    // List<Point> — the critical test
    let list_1 = p1.list(struct_1);
    let list_2 = p2.list(struct_2);

    assert_ne!(list_1, list_2);
    assert_eq!(
        p1.hash(list_1),
        p2.hash(list_2),
        "List<Point> hash must be stable: child struct at different Idx values\n\
         p1: {}\np2: {}",
        p1.format_hash(list_1),
        p2.format_hash(list_2)
    );

    // Map<str, Point> — another compound case
    let map_1 = p1.map(Idx::STR, struct_1);
    let map_2 = p2.map(Idx::STR, struct_2);
    assert_eq!(
        p1.hash(map_1),
        p2.hash(map_2),
        "Map<str, Point> hash must be stable\n\
         p1: {}\np2: {}",
        p1.format_hash(map_1),
        p2.format_hash(map_2)
    );
}

#[test]
fn merkle_enum_unit_variants_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.list(Idx::CHAR);
    let _ = p2.option(Idx::FLOAT);

    let name = ori_ir::Name::from_raw(600);
    let north = ori_ir::Name::from_raw(601);
    let south = ori_ir::Name::from_raw(602);
    let east = ori_ir::Name::from_raw(603);
    let west = ori_ir::Name::from_raw(604);

    let variants = vec![
        EnumVariant {
            name: north,
            field_types: vec![],
        },
        EnumVariant {
            name: south,
            field_types: vec![],
        },
        EnumVariant {
            name: east,
            field_types: vec![],
        },
        EnumVariant {
            name: west,
            field_types: vec![],
        },
    ];

    let e1 = p1.enum_type(name, &variants);
    let e2 = p2.enum_type(name, &variants);

    assert_eq!(
        p1.hash(e1),
        p2.hash(e2),
        "Enum Direction {{ N, S, E, W }} hash mismatch"
    );
}

#[test]
fn merkle_nested_map_depth_3_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.function(&[Idx::INT, Idx::INT], Idx::BOOL);
    let _ = p2.set(Idx::BYTE);

    // Map<str, List<Option<int>>> — depth 3
    let opt1 = p1.option(Idx::INT);
    let list1 = p1.list(opt1);
    let map1 = p1.map(Idx::STR, list1);

    let opt2 = p2.option(Idx::INT);
    let list2 = p2.list(opt2);
    let map2 = p2.map(Idx::STR, list2);

    assert_eq!(
        p1.hash(map1),
        p2.hash(map2),
        "Map<str, List<Option<int>>> depth-3 hash mismatch"
    );
}

#[test]
fn merkle_depth_4_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2
    let _ = p2.triple(Idx::STR, Idx::STR, Idx::STR);
    let _ = p2.map(Idx::BOOL, Idx::BYTE);
    let _ = p2.function(&[Idx::CHAR], Idx::INT);

    // List<Map<str, Option<List<int>>>> — depth 4
    let inner_list_1 = p1.list(Idx::INT);
    let opt_1 = p1.option(inner_list_1);
    let map_1 = p1.map(Idx::STR, opt_1);
    let outer_1 = p1.list(map_1);

    let inner_list_2 = p2.list(Idx::INT);
    let opt_2 = p2.option(inner_list_2);
    let map_2 = p2.map(Idx::STR, opt_2);
    let outer_2 = p2.list(map_2);

    assert_eq!(
        p1.hash(outer_1),
        p2.hash(outer_2),
        "List<Map<str, Option<List<int>>>> depth-4 hash mismatch"
    );
}

#[test]
fn merkle_depth_5_function_stable() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();

    // Shift p2 significantly
    for i in 0..10 {
        let _ = p2.list(Idx::from_raw(i % 12));
    }

    // (Map<str, Option<List<int>>>) -> List<Map<str, bool>> — depth 5
    // Param: depth 3
    let param_inner_1 = p1.list(Idx::INT);
    let param_opt_1 = p1.option(param_inner_1);
    let param_1 = p1.map(Idx::STR, param_opt_1);

    // Return: depth 2
    let ret_inner_1 = p1.map(Idx::STR, Idx::BOOL);
    let ret_1 = p1.list(ret_inner_1);

    let func_1 = p1.function(&[param_1], ret_1);

    // Same in p2
    let param_inner_2 = p2.list(Idx::INT);
    let param_opt_2 = p2.option(param_inner_2);
    let param_2 = p2.map(Idx::STR, param_opt_2);

    let ret_inner_2 = p2.map(Idx::STR, Idx::BOOL);
    let ret_2 = p2.list(ret_inner_2);

    let func_2 = p2.function(&[param_2], ret_2);

    assert_eq!(
        p1.hash(func_1),
        p2.hash(func_2),
        "Depth-5 function type hash mismatch"
    );
}

// === Collision Detection & Distribution Tests (02.2) ===

/// Generate a large set of distinct types in a pool and verify that no two
/// distinct types share a Merkle hash. Since the pool deduplicates by hash,
/// a collision would cause two structurally different types to silently merge
/// into one Idx. We detect this by tracking expected unique type count and
/// comparing against actual pool growth.
#[test]
fn merkle_no_collisions_500_plus_types() {
    let mut pool = Pool::new();
    let base = pool.len();
    let mut unique_indices: HashSet<Idx> = HashSet::new();

    let primitives = [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
    ];

    // Build up layers of types with increasing structural complexity
    let level1 = generate_collision_test_level1(&mut pool, &primitives);
    unique_indices.extend(&level1);

    let level2 = generate_collision_test_level2(&mut pool, &primitives, &level1);
    unique_indices.extend(&level2);

    generate_collision_test_level3(&mut pool, &primitives, &level2, &mut unique_indices);

    // Verify sufficient distinct types generated
    let new_types = pool.len() - base;
    assert!(
        new_types >= 500,
        "Expected 500+ distinct types, got {new_types}"
    );

    // Verify all unique indices have distinct hashes (no silent collision)
    let mut hashes: HashSet<u64> = HashSet::new();
    for &idx in &unique_indices {
        let h = pool.hash(idx);
        assert!(
            hashes.insert(h),
            "Hash collision: idx {idx:?} has hash 0x{h:016x} which duplicates another type",
        );
    }

    assert!(
        unique_indices.len() >= 500,
        "Expected 500+ unique indices, got {}",
        unique_indices.len()
    );
}

/// Level 1: containers, maps, functions, tuples of primitives.
fn generate_collision_test_level1(pool: &mut Pool, primitives: &[Idx]) -> Vec<Idx> {
    let mut out = Vec::new();

    // Simple containers (5 kinds × 7 = 35)
    for &p in primitives {
        out.push(pool.list(p));
        out.push(pool.option(p));
        out.push(pool.set(p));
        out.push(pool.iterator(p));
        out.push(pool.range(p));
    }

    // Two-child (Map + Result: 7×7×2 = 98)
    for &k in primitives {
        for &v in primitives {
            out.push(pool.map(k, v));
            out.push(pool.result(k, v));
        }
    }

    // Functions (7 nullary + 7×7 unary = 56)
    for &ret in primitives {
        out.push(pool.function0(ret));
    }
    for &p in primitives {
        for &ret in primitives {
            out.push(pool.function1(p, ret));
        }
    }

    // Tuples: pairs (7×7 = 49), triples (capped at 50), 2-param functions (capped at 50)
    for &a in primitives {
        for &b in primitives {
            out.push(pool.pair(a, b));
        }
    }
    let start = out.len();
    'triples: for &a in primitives {
        for &b in primitives {
            for &c in primitives {
                out.push(pool.triple(a, b, c));
                if out.len() - start >= 50 {
                    break 'triples;
                }
            }
        }
    }
    let start = out.len();
    'funcs: for &p1 in primitives {
        for &p2 in primitives {
            for &ret in primitives {
                out.push(pool.function2(p1, p2, ret));
                if out.len() - start >= 50 {
                    break 'funcs;
                }
            }
        }
    }

    out
}

/// Level 2: containers/maps of level1 types.
fn generate_collision_test_level2(
    pool: &mut Pool,
    _primitives: &[Idx],
    level1: &[Idx],
) -> Vec<Idx> {
    let mut out = Vec::new();
    let sample: Vec<Idx> = level1.iter().copied().take(20).collect();

    for &inner in &sample {
        out.push(pool.list(inner));
        out.push(pool.option(inner));
    }
    for (i, &k) in sample.iter().take(10).enumerate() {
        for &v in sample.iter().take(10) {
            let _ = i; // suppress unused warning in release
            out.push(pool.map(k, v));
        }
    }

    out
}

/// Level 3: containers/functions of level2 types.
fn generate_collision_test_level3(
    pool: &mut Pool,
    primitives: &[Idx],
    level2: &[Idx],
    unique: &mut HashSet<Idx>,
) {
    let sample: Vec<Idx> = level2.iter().copied().take(15).collect();
    for &inner in &sample {
        unique.insert(pool.list(inner));
        unique.insert(pool.option(inner));
    }
    for &param in sample.iter().take(10) {
        for &ret in primitives {
            unique.insert(pool.function1(param, ret));
        }
    }
}

/// Verify that Merkle hash distribution has reasonable entropy across
/// the top byte — no extreme clustering that would degrade `FxHashMap`.
#[test]
fn merkle_hash_distribution_uniform() {
    let mut pool = Pool::new();

    let primitives = [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
    ];

    // Generate ~300 types for distribution check
    for &p in &primitives {
        let _ = pool.list(p);
        let _ = pool.option(p);
        let _ = pool.set(p);
        let _ = pool.iterator(p);
        let _ = pool.range(p);
    }
    for &k in &primitives {
        for &v in &primitives {
            let _ = pool.map(k, v);
        }
    }
    for &p in &primitives {
        for &ret in &primitives {
            let _ = pool.function1(p, ret);
        }
    }

    // Collect hashes from all dynamically-created types
    let dynamic_start = Idx::FIRST_DYNAMIC as usize;
    let dynamic_count = pool.len() - dynamic_start;
    assert!(
        dynamic_count >= 100,
        "Need 100+ types for distribution test"
    );

    // Check top byte distribution (256 buckets)
    let mut top_byte_counts = [0u32; 256];
    for i in dynamic_start..pool.len() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "test iterates pool indices that fit u32"
        )]
        let h = pool.hash(Idx::from_raw(i as u32));
        top_byte_counts[(h >> 56) as usize] += 1;
    }

    // No bucket should have >10x the average (very permissive — mainly
    // catches catastrophic clustering, not subtle bias)
    #[expect(
        clippy::cast_precision_loss,
        reason = "approximate bucket comparison — exact precision not needed"
    )]
    let avg = dynamic_count as f64 / 256.0;
    let max_allowed = (avg * 10.0).max(5.0); // At least 5 to avoid false positives
    for (bucket, &count) in top_byte_counts.iter().enumerate() {
        assert!(
            f64::from(count) < max_allowed,
            "Hash distribution skewed: top byte 0x{bucket:02x} has {count} entries \
             (avg {avg:.1}, max allowed {max_allowed:.0})"
        );
    }
}

// === Structural Equality Verification Tests (02.3) ===

/// Helper: generate a matching set of types in two shifted pools.
/// Returns parallel vectors: `types_1[i]` and `types_2[i]` are the same structure.
fn generate_matched_types(p1: &mut Pool, p2: &mut Pool) -> (Vec<Idx>, Vec<Idx>) {
    // Shift p2 heavily to ensure different Idx assignments
    for i in 0..15 {
        let _ = p2.list(Idx::from_raw(i % 12));
    }
    let _ = p2.map(Idx::INT, Idx::STR);
    let _ = p2.function(&[Idx::BOOL], Idx::FLOAT);

    let primitives = [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
    ];

    let mut t1 = Vec::new();
    let mut t2 = Vec::new();

    // Primitives (12)
    for &p in &[
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::STR,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
        Idx::NEVER,
        Idx::ERROR,
        Idx::DURATION,
        Idx::SIZE,
        Idx::ORDERING,
    ] {
        t1.push(p);
        t2.push(p);
    }

    // Simple containers of primitives (5 kinds × 7 = 35)
    for &p in &primitives {
        t1.push(p1.list(p));
        t2.push(p2.list(p));
        t1.push(p1.option(p));
        t2.push(p2.option(p));
        t1.push(p1.set(p));
        t2.push(p2.set(p));
        t1.push(p1.iterator(p));
        t2.push(p2.iterator(p));
        t1.push(p1.range(p));
        t2.push(p2.range(p));
    }

    // Two-child containers (Map: 7×7 = 49)
    for &k in &primitives {
        for &v in &primitives {
            t1.push(p1.map(k, v));
            t2.push(p2.map(k, v));
        }
    }

    // Functions (nullary: 7 + unary: 7×3 = 28)
    for &ret in &primitives {
        t1.push(p1.function0(ret));
        t2.push(p2.function0(ret));
    }
    for &p in &primitives[..3] {
        for &ret in &primitives {
            t1.push(p1.function1(p, ret));
            t2.push(p2.function1(p, ret));
        }
    }

    // Tuples (pairs: 3×7 = 21)
    for &a in &primitives[..3] {
        for &b in &primitives {
            t1.push(p1.pair(a, b));
            t2.push(p2.pair(a, b));
        }
    }

    // Nested (List<List<P>>, Option<List<P>>: 2 kinds × 3 = 6)
    for &p in &primitives[..3] {
        let inner1 = p1.list(p);
        let inner2 = p2.list(p);
        t1.push(p1.list(inner1));
        t2.push(p2.list(inner2));
        t1.push(p1.option(inner1));
        t2.push(p2.option(inner2));
    }

    generate_matched_named_types(p1, p2, &mut t1, &mut t2);

    (t1, t2)
}

/// Generate matched named/complex types (structs, enums, applied, named, schemes).
fn generate_matched_named_types(
    p1: &mut Pool,
    p2: &mut Pool,
    t1: &mut Vec<Idx>,
    t2: &mut Vec<Idx>,
) {
    // Structs (3 different names)
    for name_raw in 1000..1003 {
        let name = ori_ir::Name::from_raw(name_raw);
        let f1 = ori_ir::Name::from_raw(name_raw + 100);
        let f2 = ori_ir::Name::from_raw(name_raw + 200);
        t1.push(p1.struct_type(name, &[(f1, Idx::INT), (f2, Idx::STR)]));
        t2.push(p2.struct_type(name, &[(f1, Idx::INT), (f2, Idx::STR)]));
    }

    // Enums (2 different)
    for name_raw in 2000..2002 {
        let name = ori_ir::Name::from_raw(name_raw);
        let v1 = ori_ir::Name::from_raw(name_raw + 100);
        let v2 = ori_ir::Name::from_raw(name_raw + 200);
        let variants = vec![
            EnumVariant {
                name: v1,
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: v2,
                field_types: vec![],
            },
        ];
        t1.push(p1.enum_type(name, &variants));
        t2.push(p2.enum_type(name, &variants));
    }

    // Applied types (3)
    for name_raw in 3000..3003 {
        let name = ori_ir::Name::from_raw(name_raw);
        t1.push(p1.applied(name, &[Idx::INT, Idx::BOOL]));
        t2.push(p2.applied(name, &[Idx::INT, Idx::BOOL]));
    }

    // Named types (3)
    for name_raw in 4000..4003 {
        let name = ori_ir::Name::from_raw(name_raw);
        t1.push(p1.named(name));
        t2.push(p2.named(name));
    }

    // Schemes (2 — forall [0]. List<Var(0)>)
    for _ in 0..2 {
        let var1 = p1.fresh_var();
        let body1 = p1.list(var1);
        let s1 = p1.scheme(&[0], body1);

        let var2 = p2.fresh_var();
        let body2 = p2.list(var2);
        let s2 = p2.scheme(&[0], body2);

        t1.push(s1);
        t2.push(s2);
    }
}

/// Cross-check: for 100+ matched type pairs across two shifted pools,
/// verify hash equality ↔ structural equality (both directions).
#[test]
fn merkle_hash_matches_structural_equality() {
    let mut p1 = Pool::new();
    let mut p2 = Pool::new();
    let (types_1, types_2) = generate_matched_types(&mut p1, &mut p2);

    assert!(
        types_1.len() >= 100,
        "Need 100+ type pairs, got {}",
        types_1.len()
    );

    // For each matched pair, verify hash equality ↔ structural equality
    for (i, (&idx1, &idx2)) in types_1.iter().zip(&types_2).enumerate() {
        let hash_eq = p1.hash(idx1) == p2.hash(idx2);
        let struct_eq = structural_eq(&p1, idx1, &p2, idx2);
        assert_eq!(
            hash_eq,
            struct_eq,
            "Mismatch at pair {i}: hash_eq={hash_eq} but structural_eq={struct_eq} \
             (tag1={:?}, tag2={:?})",
            p1.tag(idx1),
            p2.tag(idx2)
        );
    }

    // Also verify that matched pairs ARE equal (no false negative)
    let mut eq_count = 0;
    for (&idx1, &idx2) in types_1.iter().zip(&types_2) {
        if structural_eq(&p1, idx1, &p2, idx2) {
            eq_count += 1;
        }
    }
    assert_eq!(
        eq_count,
        types_1.len(),
        "Some matched pairs reported not structurally equal"
    );
}

/// Cross-check that different types from the SAME pool have different
/// hashes AND report as structurally non-equal.
#[test]
fn merkle_structural_neq_implies_hash_neq() {
    let mut pool = Pool::new();

    // Create a handful of structurally distinct types
    let types = [
        pool.list(Idx::INT),
        pool.list(Idx::STR),
        pool.option(Idx::INT),
        pool.map(Idx::STR, Idx::INT),
        pool.function1(Idx::INT, Idx::BOOL),
        pool.pair(Idx::INT, Idx::STR),
    ];

    // Every pair should be structurally different AND hash-different
    for i in 0..types.len() {
        for j in (i + 1)..types.len() {
            let struct_eq = structural_eq(&pool, types[i], &pool, types[j]);
            let hash_eq = pool.hash(types[i]) == pool.hash(types[j]);
            assert!(
                !struct_eq,
                "Types {i} and {j} should be structurally different"
            );
            assert!(!hash_eq, "Types {i} and {j} should have different hashes");
        }
    }
}
