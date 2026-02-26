use super::*;
use crate::Pool;

#[test]
fn typed_module_basic() {
    let mut module = TypedModule::new();

    // Store expression types
    module.expr_types.push(Idx::INT);
    module.expr_types.push(Idx::STR);
    module.expr_types.push(Idx::BOOL);

    assert_eq!(module.expr_type(0), Some(Idx::INT));
    assert_eq!(module.expr_type(1), Some(Idx::STR));
    assert_eq!(module.expr_type(2), Some(Idx::BOOL));
    assert_eq!(module.expr_type(99), None);
    assert!(!module.has_errors());
}

#[test]
fn function_sig_simple() {
    let mut pool = Pool::new();
    let name = Name::from_raw(1);

    let sig = FunctionSig::simple(name, vec![Idx::INT, Idx::STR], Idx::BOOL);

    assert_eq!(sig.name, name);
    assert_eq!(sig.arity(), 2);
    assert!(!sig.is_generic());
    assert!(!sig.has_capabilities());

    let func_ty = sig.to_function_type(&mut pool);
    assert_eq!(pool.tag(func_ty), crate::Tag::Function);
}

#[test]
fn function_sig_generic() {
    let name = Name::from_raw(1);
    let t_param = Name::from_raw(2);

    let sig = FunctionSig {
        name,
        type_params: vec![t_param],
        const_params: vec![],
        param_names: vec![Name::from_raw(3)],
        param_types: vec![Idx::INT],
        return_type: Idx::INT,
        capabilities: vec![],
        is_public: true,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds: vec![vec![]],
        where_clauses: vec![],
        generic_param_mapping: vec![None],
        scheme_var_ids: vec![],
        required_params: 1,
        param_defaults: vec![],
        param_hashes: vec![0; 1],
        return_hash: 0,
    };

    assert!(sig.is_generic());
    assert!(sig.is_public);
}

#[test]
fn type_check_result_ok() {
    let module = TypedModule::new();
    let result = TypeCheckResult::ok(module);

    assert!(!result.has_errors());
    assert!(result.error_guarantee.is_none());
}

#[test]
fn type_check_result_from_typed() {
    // No errors
    let module = TypedModule::new();
    let result = TypeCheckResult::from_typed(module);
    assert!(!result.has_errors());

    // With errors
    let mut module_with_errors = TypedModule::new();
    module_with_errors
        .errors
        .push(TypeCheckError::undefined_identifier(
            Name::from_raw(1),
            ori_ir::Span::DUMMY,
        ));
    let result = TypeCheckResult::from_typed(module_with_errors);
    assert!(result.has_errors());
}

#[test]
fn typed_module_with_capacity() {
    let module = TypedModule::with_capacity(100, 10);
    assert_eq!(module.expr_types.capacity(), 100);
    assert_eq!(module.functions.capacity(), 10);
}

#[test]
fn effect_class_pure() {
    let interner = ori_ir::StringInterner::new();
    let name = Name::from_raw(1);
    let sig = FunctionSig::simple(name, vec![Idx::INT], Idx::BOOL);

    assert_eq!(sig.effect_class(&interner), EffectClass::Pure);
}

#[test]
fn effect_class_reads_only() {
    let interner = ori_ir::StringInterner::new();
    let name = Name::from_raw(1);
    let clock = interner.intern("Clock");
    let env = interner.intern("Env");

    let sig = FunctionSig {
        name,
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::STR,
        capabilities: vec![clock, env],
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
    };

    assert_eq!(sig.effect_class(&interner), EffectClass::ReadsOnly);
}

#[test]
fn effect_class_has_effects() {
    let interner = ori_ir::StringInterner::new();
    let name = Name::from_raw(1);
    let http = interner.intern("Http");

    let sig = FunctionSig {
        name,
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::STR,
        capabilities: vec![http],
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
    };

    assert_eq!(sig.effect_class(&interner), EffectClass::HasEffects);
}

#[test]
fn effect_class_mixed_caps_is_has_effects() {
    let interner = ori_ir::StringInterner::new();
    let name = Name::from_raw(1);
    let clock = interner.intern("Clock");
    let http = interner.intern("Http");

    // One read-only + one effectful → HasEffects
    let sig = FunctionSig {
        name,
        type_params: vec![],
        const_params: vec![],
        param_names: vec![],
        param_types: vec![],
        return_type: Idx::UNIT,
        capabilities: vec![clock, http],
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
    };

    assert_eq!(sig.effect_class(&interner), EffectClass::HasEffects);
}

#[test]
fn effect_class_ordering() {
    assert!(EffectClass::Pure < EffectClass::ReadsOnly);
    assert!(EffectClass::ReadsOnly < EffectClass::HasEffects);
}

#[test]
fn function_lookup() {
    let mut module = TypedModule::new();
    let foo = Name::from_raw(1);
    let bar = Name::from_raw(2);

    module
        .functions
        .push(FunctionSig::simple(foo, vec![], Idx::UNIT));
    module
        .functions
        .push(FunctionSig::simple(bar, vec![Idx::INT], Idx::STR));

    assert!(module.function(foo).is_some());
    assert!(module.function(bar).is_some());
    assert!(module.function(Name::from_raw(99)).is_none());

    assert_eq!(module.function(foo).map(FunctionSig::arity), Some(0));
    assert_eq!(module.function(bar).map(FunctionSig::arity), Some(1));
}

#[test]
fn type_def_export() {
    use crate::registry::{FieldDef, StructDef, TypeKind, Visibility};
    use crate::ValueCategory;

    let mut module = TypedModule::new();
    let point_name = Name::from_raw(10);
    let x_name = Name::from_raw(11);
    let y_name = Name::from_raw(12);

    module.types.push(TypeEntry {
        name: point_name,
        idx: Idx::from_raw(100),
        kind: TypeKind::Struct(StructDef {
            fields: vec![
                FieldDef {
                    name: x_name,
                    ty: Idx::INT,
                    span: Span::DUMMY,
                    visibility: Visibility::Public,
                },
                FieldDef {
                    name: y_name,
                    ty: Idx::INT,
                    span: Span::DUMMY,
                    visibility: Visibility::Public,
                },
            ],
            category: ValueCategory::default(),
        }),
        span: Span::DUMMY,
        type_params: vec![],
        visibility: Visibility::Public,
        merkle_hash: 0,
    });

    assert_eq!(module.type_count(), 1);
    assert!(module.type_def(point_name).is_some());
    assert!(module.type_def(Name::from_raw(99)).is_none());

    let entry = module.type_def(point_name).unwrap();
    assert!(matches!(entry.kind, TypeKind::Struct(_)));

    if let TypeKind::Struct(ref s) = entry.kind {
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].ty, Idx::INT);
    }
}

#[test]
fn populate_hashes_matches_pool() {
    let mut pool = Pool::new();
    let list_str = pool.list(Idx::STR);
    let opt_bool = pool.option(Idx::BOOL);

    let mut sig = FunctionSig::simple(Name::from_raw(1), vec![Idx::INT, list_str], opt_bool);

    // Before populate: hashes are zeros
    assert_eq!(sig.param_hashes, vec![0, 0]);
    assert_eq!(sig.return_hash, 0);

    sig.populate_hashes(&pool);

    // After populate: hashes match pool
    assert_eq!(sig.param_hashes.len(), sig.param_types.len());
    for (i, (&idx, &hash)) in sig.param_types.iter().zip(&sig.param_hashes).enumerate() {
        assert_eq!(
            pool.hash(idx),
            hash,
            "param[{i}] hash mismatch: pool says {}, sig says {}",
            pool.hash(idx),
            hash,
        );
    }
    assert_eq!(pool.hash(sig.return_type), sig.return_hash);
}

#[test]
fn populate_hashes_computed_from_pool() {
    let pool = Pool::new();
    let mut sig = FunctionSig::simple(
        Name::from_raw(1),
        vec![Idx::INT, Idx::STR, Idx::BOOL],
        Idx::UNIT,
    );

    sig.populate_hashes(&pool);

    // Hashes must match the pool's Merkle hash for each type
    // (Note: FxHasher can produce 0 for Tag::Int — that's valid, not "uninitialized")
    for (i, (&idx, &hash)) in sig.param_types.iter().zip(&sig.param_hashes).enumerate() {
        assert_eq!(
            pool.hash(idx),
            hash,
            "param[{i}] hash inconsistent with pool"
        );
    }
    assert_eq!(pool.hash(sig.return_type), sig.return_hash);

    // At least some primitive hashes should be distinct from each other
    // (verifies computation actually runs, not just zeros everywhere)
    let distinct: std::collections::HashSet<u64> = sig.param_hashes.iter().copied().collect();
    assert!(
        distinct.len() >= 2,
        "primitive hashes should produce distinct values"
    );
}

#[test]
fn populate_hashes_cross_pool_stable() {
    // Same types interned in different pools must produce same hashes
    let mut pool_a = Pool::new();
    let mut pool_b = Pool::new();

    let list_int_a = pool_a.list(Idx::INT);
    let list_int_b = pool_b.list(Idx::INT);

    let mut sig_a = FunctionSig::simple(Name::from_raw(1), vec![list_int_a], Idx::BOOL);
    let mut sig_b = FunctionSig::simple(Name::from_raw(1), vec![list_int_b], Idx::BOOL);

    sig_a.populate_hashes(&pool_a);
    sig_b.populate_hashes(&pool_b);

    assert_eq!(
        sig_a.param_hashes, sig_b.param_hashes,
        "same types in different pools must produce identical param hashes"
    );
    assert_eq!(
        sig_a.return_hash, sig_b.return_hash,
        "same return type in different pools must produce identical return hash"
    );
}
