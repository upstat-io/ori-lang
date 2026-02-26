//! Tests for cross-pool type re-interning.

use rustc_hash::FxHashMap;

use crate::pool::re_intern::{re_intern_sig, re_intern_type};
use crate::{FunctionSig, Idx, Pool};

// === Primitive Types ===

#[test]
fn primitives_are_identity() {
    let source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // All primitives should map to themselves (fixed indices)
    for idx in [
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
    ] {
        let result = re_intern_type(&source, idx, &mut target, &mut cache);
        assert_eq!(result, idx, "Primitive {idx:?} should map to itself");
    }
}

// === Simple Container Types ===

#[test]
fn list_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let list_int = source.list(Idx::INT);
    let result = re_intern_type(&source, list_int, &mut target, &mut cache);

    // Verify structural equality via Merkle hash
    assert_eq!(target.hash(result), source.hash(list_int));
    assert_eq!(target.tag(result), crate::Tag::List);
}

#[test]
fn option_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let opt_str = source.option(Idx::STR);
    let result = re_intern_type(&source, opt_str, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(opt_str));
}

// === Two-Child Container Types ===

#[test]
fn map_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let map_str_int = source.map(Idx::STR, Idx::INT);
    let result = re_intern_type(&source, map_str_int, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(map_str_int));
    assert_eq!(target.map_key(result), Idx::STR);
    assert_eq!(target.map_value(result), Idx::INT);
}

#[test]
fn result_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let res = source.result(Idx::INT, Idx::STR);
    let result = re_intern_type(&source, res, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(res));
    assert_eq!(target.result_ok(result), Idx::INT);
    assert_eq!(target.result_err(result), Idx::STR);
}

// === Complex Types ===

#[test]
fn function_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let func = source.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let result = re_intern_type(&source, func, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(func));
    assert_eq!(target.function_params(result), vec![Idx::INT, Idx::STR]);
    assert_eq!(target.function_return(result), Idx::BOOL);
}

#[test]
fn tuple_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let tup = source.tuple(&[Idx::INT, Idx::BOOL, Idx::STR]);
    let result = re_intern_type(&source, tup, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(tup));
    assert_eq!(
        target.tuple_elems(result),
        vec![Idx::INT, Idx::BOOL, Idx::STR]
    );
}

// === Nested Types ===

#[test]
fn nested_list_of_tuples_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // List<(int, str)>
    let tup = source.tuple(&[Idx::INT, Idx::STR]);
    let list_tup = source.list(tup);
    let result = re_intern_type(&source, list_tup, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(list_tup));
}

#[test]
fn deeply_nested_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // Option<Map<str, List<int>>>
    let list_int = source.list(Idx::INT);
    let map_type = source.map(Idx::STR, list_int);
    let opt = source.option(map_type);
    let result = re_intern_type(&source, opt, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(opt));
}

// === Cache Behavior ===

#[test]
fn cache_prevents_redundant_interning() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let list_int = source.list(Idx::INT);

    // Re-intern twice with the same cache
    let result1 = re_intern_type(&source, list_int, &mut target, &mut cache);
    let result2 = re_intern_type(&source, list_int, &mut target, &mut cache);

    // Should return the same Idx (cache hit)
    assert_eq!(result1, result2);

    // Cache should contain the mapping
    assert_eq!(cache.get(&list_int), Some(&result1));
}

// === Cross-Pool Stability ===

#[test]
fn re_interned_types_have_stable_hashes() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // Build a complex type in source
    let func = source.function(&[Idx::INT], Idx::STR);
    let list_func = source.list(func);

    // Also build the same type directly in target
    let target_func = target.function(&[Idx::INT], Idx::STR);
    let target_list_func = target.list(target_func);

    // Re-intern from source into target
    let result = re_intern_type(&source, list_func, &mut target, &mut cache);

    // Should resolve to the same Idx (deduplication via Merkle hash)
    assert_eq!(result, target_list_func);
    assert_eq!(target.hash(result), target.hash(target_list_func));
}

#[test]
fn fast_path_hits_for_existing_types() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    // Pre-populate target with some types to shift its next Idx
    let _ = target.list(Idx::FLOAT);
    let _ = target.option(Idx::BOOL);
    let target_list = target.list(Idx::INT);

    // Source has the same type at a different Idx (no padding types)
    let source_list = source.list(Idx::INT);
    assert_ne!(
        source_list, target_list,
        "Source and target should have different Idx for List<int>"
    );

    // Re-interning should find the existing type via hash
    let result = re_intern_type(&source, source_list, &mut target, &mut cache);
    assert_eq!(result, target_list, "Should reuse existing Idx via hash");
}

// === FunctionSig Re-interning ===

#[test]
fn sig_primitive_params_re_interned() {
    let source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let mut sig = FunctionSig::simple(
        ori_ir::Name::from_raw(1),
        vec![Idx::INT, Idx::STR],
        Idx::BOOL,
    );
    sig.populate_hashes(&source);

    let result = re_intern_sig(&sig, &source, &mut target, &mut cache);

    assert_eq!(result.param_types, vec![Idx::INT, Idx::STR]);
    assert_eq!(result.return_type, Idx::BOOL);
    // Hashes should be valid in target pool
    assert_eq!(result.param_hashes[0], target.hash(Idx::INT));
    assert_eq!(result.param_hashes[1], target.hash(Idx::STR));
    assert_eq!(result.return_hash, target.hash(Idx::BOOL));
}

#[test]
fn sig_compound_params_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let list_int = source.list(Idx::INT);
    let mut sig = FunctionSig::simple(ori_ir::Name::from_raw(1), vec![list_int], Idx::UNIT);
    sig.populate_hashes(&source);

    let result = re_intern_sig(&sig, &source, &mut target, &mut cache);

    // Param type should be re-interned (valid in target)
    let target_list_int = target.list(Idx::INT);
    assert_eq!(result.param_types[0], target_list_int);
    assert_eq!(result.param_hashes[0], target.hash(target_list_int));
}

// === Struct/Enum Types ===

#[test]
fn struct_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let interner = ori_ir::StringInterner::new();
    let point_name = interner.intern("Point");
    let x_name = interner.intern("x");
    let y_name = interner.intern("y");

    let point = source.struct_type(point_name, &[(x_name, Idx::INT), (y_name, Idx::INT)]);
    let result = re_intern_type(&source, point, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(point));
    assert_eq!(target.struct_name(result), point_name);

    let fields = target.struct_fields(result);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], (x_name, Idx::INT));
    assert_eq!(fields[1], (y_name, Idx::INT));
}

#[test]
fn enum_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let interner = ori_ir::StringInterner::new();
    let shape_name = interner.intern("Shape");
    let circle_name = interner.intern("Circle");
    let rect_name = interner.intern("Rect");

    let shape = source.enum_type(
        shape_name,
        &[
            crate::pool::construct::EnumVariant {
                name: circle_name,
                field_types: vec![Idx::FLOAT],
            },
            crate::pool::construct::EnumVariant {
                name: rect_name,
                field_types: vec![Idx::FLOAT, Idx::FLOAT],
            },
        ],
    );
    let result = re_intern_type(&source, shape, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(shape));
    assert_eq!(target.enum_name(result), shape_name);

    let variants = target.enum_variants(result);
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].0, circle_name);
    assert_eq!(variants[0].1, vec![Idx::FLOAT]);
    assert_eq!(variants[1].0, rect_name);
    assert_eq!(variants[1].1, vec![Idx::FLOAT, Idx::FLOAT]);
}

// === Named/Applied Types ===

#[test]
fn applied_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let interner = ori_ir::StringInterner::new();
    let name = interner.intern("Container");

    let applied = source.applied(name, &[Idx::INT, Idx::STR]);
    let result = re_intern_type(&source, applied, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(applied));
    assert_eq!(target.applied_name(result), name);
    assert_eq!(target.applied_args(result), vec![Idx::INT, Idx::STR]);
}

// === Scheme Types ===

#[test]
fn scheme_type_re_interned() {
    let mut source = Pool::new();
    let mut target = Pool::new();
    let mut cache = FxHashMap::default();

    let body = source.function(&[Idx::INT], Idx::STR);
    let scheme = source.scheme(&[0, 1], body);
    let result = re_intern_type(&source, scheme, &mut target, &mut cache);

    assert_eq!(target.hash(result), source.hash(scheme));
}
