use pretty_assertions::assert_eq;

use ori_types::{EnumVariant, Idx, Pool};

use super::*;

// ── Primitive types ─────────────────────────────────────────────

#[test]
fn primitives_are_scalar() {
    let pool = Pool::new();
    let cls = ArcClassifier::new(&pool);

    let scalars = [
        Idx::INT,
        Idx::FLOAT,
        Idx::BOOL,
        Idx::CHAR,
        Idx::BYTE,
        Idx::UNIT,
        Idx::NEVER,
        Idx::ERROR,
        Idx::DURATION,
        Idx::SIZE,
        Idx::ORDERING,
    ];

    for idx in scalars {
        assert_eq!(
            cls.arc_class(idx),
            ArcClass::Scalar,
            "expected Scalar for primitive {}",
            idx.display_name(),
        );
        assert!(cls.is_scalar(idx));
        assert!(!cls.needs_rc(idx));
    }
}

#[test]
fn str_is_definite_ref() {
    let pool = Pool::new();
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(Idx::STR), ArcClass::DefiniteRef);
    assert!(!cls.is_scalar(Idx::STR));
    assert!(cls.needs_rc(Idx::STR));
}

#[test]
fn none_sentinel_is_scalar() {
    let pool = Pool::new();
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(Idx::NONE), ArcClass::Scalar);
}

// ── Heap-allocated containers ───────────────────────────────────

#[test]
fn list_is_definite_ref() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(list_int), ArcClass::DefiniteRef);
}

#[test]
fn map_is_definite_ref() {
    let mut pool = Pool::new();
    let map = pool.map(Idx::STR, Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(map), ArcClass::DefiniteRef);
}

#[test]
fn set_is_definite_ref() {
    let mut pool = Pool::new();
    let set = pool.set(Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(set), ArcClass::DefiniteRef);
}

#[test]
fn channel_is_definite_ref() {
    let mut pool = Pool::new();
    let chan = pool.channel(Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(chan), ArcClass::DefiniteRef);
}

#[test]
fn function_is_definite_ref() {
    let mut pool = Pool::new();
    let func = pool.function(&[Idx::INT, Idx::FLOAT], Idx::BOOL);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(func), ArcClass::DefiniteRef);
}

// ── Option (transitive) ─────────────────────────────────────────

#[test]
fn option_of_scalar_is_scalar() {
    let mut pool = Pool::new();
    let opt_int = pool.option(Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(opt_int), ArcClass::Scalar);
}

#[test]
fn option_of_ref_is_definite_ref() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(opt_str), ArcClass::DefiniteRef);
}

#[test]
fn option_of_list_is_definite_ref() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    let opt_list = pool.option(list);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(opt_list), ArcClass::DefiniteRef);
}

// ── Result (transitive) ─────────────────────────────────────────

#[test]
fn result_of_scalars_is_scalar() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(res), ArcClass::Scalar);
}

#[test]
fn result_with_ref_ok_is_definite_ref() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::STR, Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(res), ArcClass::DefiniteRef);
}

#[test]
fn result_with_ref_err_is_definite_ref() {
    let mut pool = Pool::new();
    let res = pool.result(Idx::INT, Idx::STR);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(res), ArcClass::DefiniteRef);
}

// ── Range (transitive) ──────────────────────────────────────────

#[test]
fn range_of_scalar_is_scalar() {
    let mut pool = Pool::new();
    let range = pool.range(Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(range), ArcClass::Scalar);
}

// ── Tuple (transitive) ──────────────────────────────────────────

#[test]
fn tuple_of_scalars_is_scalar() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(tup), ArcClass::Scalar);
}

#[test]
fn tuple_with_ref_is_definite_ref() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::STR]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(tup), ArcClass::DefiniteRef);
}

#[test]
fn empty_tuple_is_unit_and_scalar() {
    let mut pool = Pool::new();
    // Pool::tuple(&[]) returns Idx::UNIT.
    let tup = pool.tuple(&[]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(tup, Idx::UNIT);
    assert_eq!(cls.arc_class(tup), ArcClass::Scalar);
}

// ── Struct (transitive) ─────────────────────────────────────────

#[test]
fn struct_all_scalar_fields_is_scalar() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(10);
    let x = ori_ir::Name::from_raw(11);
    let y = ori_ir::Name::from_raw(12);
    let struct_idx = pool.struct_type(name, &[(x, Idx::INT), (y, Idx::INT)]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(struct_idx), ArcClass::Scalar);
}

#[test]
fn struct_with_ref_field_is_definite_ref() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(20);
    let label = ori_ir::Name::from_raw(21);
    let id = ori_ir::Name::from_raw(22);
    let struct_idx = pool.struct_type(name, &[(label, Idx::STR), (id, Idx::INT)]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(struct_idx), ArcClass::DefiniteRef);
}

// ── Enum (transitive) ───────────────────────────────────────────

#[test]
fn enum_all_unit_variants_is_scalar() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(30);
    let enum_idx = pool.enum_type(
        name,
        &[
            EnumVariant {
                name: ori_ir::Name::from_raw(31),
                field_types: vec![],
            },
            EnumVariant {
                name: ori_ir::Name::from_raw(32),
                field_types: vec![],
            },
            EnumVariant {
                name: ori_ir::Name::from_raw(33),
                field_types: vec![],
            },
        ],
    );
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(enum_idx), ArcClass::Scalar);
}

#[test]
fn enum_with_ref_variant_is_definite_ref() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(40);
    let enum_idx = pool.enum_type(
        name,
        &[
            EnumVariant {
                name: ori_ir::Name::from_raw(41),
                field_types: vec![Idx::FLOAT],
            },
            EnumVariant {
                name: ori_ir::Name::from_raw(42),
                field_types: vec![Idx::STR],
            },
        ],
    );
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(enum_idx), ArcClass::DefiniteRef);
}

#[test]
fn enum_with_scalar_payloads_is_scalar() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(50);
    let enum_idx = pool.enum_type(
        name,
        &[
            EnumVariant {
                name: ori_ir::Name::from_raw(51),
                field_types: vec![Idx::INT],
            },
            EnumVariant {
                name: ori_ir::Name::from_raw(52),
                field_types: vec![Idx::FLOAT],
            },
        ],
    );
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(enum_idx), ArcClass::Scalar);
}

// ── Type variables ──────────────────────────────────────────────

#[test]
fn type_variable_is_possible_ref() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(var), ArcClass::PossibleRef);
    assert!(!cls.is_scalar(var));
    assert!(cls.needs_rc(var));
}

// ── Named type resolution ───────────────────────────────────────

#[test]
fn named_type_resolved_to_scalar_struct() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(60);
    let x = ori_ir::Name::from_raw(61);
    let y = ori_ir::Name::from_raw(62);
    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[(x, Idx::INT), (y, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(named_idx), ArcClass::Scalar);
}

#[test]
fn named_type_resolved_to_ref_struct() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(70);
    let n = ori_ir::Name::from_raw(71);
    let age = ori_ir::Name::from_raw(72);
    let named_idx = pool.named(name);
    let struct_idx = pool.struct_type(name, &[(n, Idx::STR), (age, Idx::INT)]);
    pool.set_resolution(named_idx, struct_idx);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(named_idx), ArcClass::DefiniteRef);
}

#[test]
fn unresolved_named_type_is_possible_ref() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(80);
    let named_idx = pool.named(name);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(named_idx), ArcClass::PossibleRef);
}

// ── Nested compound types ───────────────────────────────────────

#[test]
fn nested_option_of_scalar_tuple_is_scalar() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let opt = pool.option(tup);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(opt), ArcClass::Scalar);
}

#[test]
fn nested_result_of_option_str_is_definite_ref() {
    let mut pool = Pool::new();
    let opt_str = pool.option(Idx::STR);
    let res = pool.result(opt_str, Idx::INT);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(res), ArcClass::DefiniteRef);
}

// ── Compound with type variable ─────────────────────────────────

#[test]
fn option_of_type_variable_is_possible_ref() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let opt = pool.option(var);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(opt), ArcClass::PossibleRef);
}

#[test]
fn tuple_with_type_variable_is_possible_ref() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let tup = pool.tuple(&[Idx::INT, var]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(tup), ArcClass::PossibleRef);
}

#[test]
fn tuple_with_ref_and_variable_is_definite_ref() {
    // DefiniteRef dominates PossibleRef.
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let tup = pool.tuple(&[Idx::STR, var]);
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(tup), ArcClass::DefiniteRef);
}

// ── Caching ─────────────────────────────────────────────────────

#[test]
fn classification_is_cached() {
    let mut pool = Pool::new();
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT, Idx::BOOL]);
    let cls = ArcClassifier::new(&pool);

    // First call computes.
    assert_eq!(cls.arc_class(tup), ArcClass::Scalar);
    // Second call hits cache (same result).
    assert_eq!(cls.arc_class(tup), ArcClass::Scalar);
    // Verify cache has the entry.
    assert!(cls.cache.borrow().contains_key(&tup));
}

// ── Rigid and bound variables ───────────────────────────────────

#[test]
fn rigid_var_is_possible_ref() {
    let mut pool = Pool::new();
    let var = pool.rigid_var(ori_ir::Name::from_raw(90));
    let cls = ArcClassifier::new(&pool);

    assert_eq!(cls.arc_class(var), ArcClass::PossibleRef);
}

// ── Cache export/import ─────────────────────────────────────────

#[test]
fn export_cache_captures_computed_classifications() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);
    let cls = ArcClassifier::new(&pool);

    // Classify some types to populate cache.
    cls.arc_class(list);
    cls.arc_class(tup);

    let exported = cls.export_cache();
    assert!(exported.contains_key(&list));
    assert!(exported.contains_key(&tup));
    assert_eq!(exported[&list], ArcClass::DefiniteRef);
    assert_eq!(exported[&tup], ArcClass::Scalar);
}

#[test]
fn with_cache_uses_precomputed_classifications() {
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    let tup = pool.tuple(&[Idx::INT, Idx::FLOAT]);

    // First classifier: compute classifications.
    let cls1 = ArcClassifier::new(&pool);
    cls1.arc_class(list);
    cls1.arc_class(tup);
    let exported = cls1.export_cache();

    // Second classifier: pre-seeded with exported cache.
    let cls2 = ArcClassifier::with_cache(&pool, exported);

    // Should return correct results from cache (no re-computation).
    assert_eq!(cls2.arc_class(list), ArcClass::DefiniteRef);
    assert_eq!(cls2.arc_class(tup), ArcClass::Scalar);

    // Cache should have entries from import.
    assert!(cls2.cache.borrow().contains_key(&list));
    assert!(cls2.cache.borrow().contains_key(&tup));
}

#[test]
fn with_cache_empty_is_equivalent_to_new() {
    let pool = Pool::new();
    let cls = ArcClassifier::with_cache(&pool, FxHashMap::default());

    // Should work identically to new().
    assert_eq!(cls.arc_class(Idx::INT), ArcClass::Scalar);
    assert_eq!(cls.arc_class(Idx::STR), ArcClass::DefiniteRef);
}

#[test]
fn export_empty_cache_returns_empty_map() {
    let pool = Pool::new();
    let cls = ArcClassifier::new(&pool);

    // No classifications performed yet.
    let exported = cls.export_cache();
    assert!(exported.is_empty());
}
