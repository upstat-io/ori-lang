use super::*;
use crate::{Pool, Tag};
use ori_ir::{ExprArena, ParsedTypeId, StringInterner};

#[test]
fn collect_signatures_empty_module() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);
    let module = Module::default();

    collect_signatures(&mut checker, &module);

    // Base env should be frozen even with empty module
    assert!(checker.base_env().is_some());
}

#[test]
fn resolve_const_param_type_primitive_int() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);

    let param = ori_ir::GenericParam {
        name: Name::from_raw(1),
        bounds: vec![],
        default_type: None,
        is_const: true,
        const_type: Some(ParsedType::Primitive(ori_ir::TypeId::INT)),
        default_value: None,
        span: ori_ir::Span::DUMMY,
    };

    assert_eq!(resolve_const_param_type(&checker, &param), Idx::INT);
}

#[test]
fn resolve_const_param_type_primitive_bool() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);

    let param = ori_ir::GenericParam {
        name: Name::from_raw(1),
        bounds: vec![],
        default_type: None,
        is_const: true,
        const_type: Some(ParsedType::Primitive(ori_ir::TypeId::BOOL)),
        default_value: None,
        span: ori_ir::Span::DUMMY,
    };

    assert_eq!(resolve_const_param_type(&checker, &param), Idx::BOOL);
}

#[test]
fn resolve_const_param_type_named_int() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);

    let int_name = interner.intern("int");
    let param = ori_ir::GenericParam {
        name: Name::from_raw(1),
        bounds: vec![],
        default_type: None,
        is_const: true,
        const_type: Some(ParsedType::Named {
            name: int_name,
            type_args: ori_ir::ParsedTypeRange::EMPTY,
        }),
        default_value: None,
        span: ori_ir::Span::DUMMY,
    };

    assert_eq!(resolve_const_param_type(&checker, &param), Idx::INT);
}

#[test]
fn resolve_const_param_type_none_returns_error() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);

    let param = ori_ir::GenericParam {
        name: Name::from_raw(1),
        bounds: vec![],
        default_type: None,
        is_const: true,
        const_type: None,
        default_value: None,
        span: ori_ir::Span::DUMMY,
    };

    assert_eq!(resolve_const_param_type(&checker, &param), Idx::ERROR);
}

// ============================================================================
// Well-Known Type Resolution Sync Test
// ============================================================================

/// Verify that all three type resolution paths produce the same Tag for
/// well-known generic types.
///
/// When adding a new well-known type (with a dedicated Pool constructor),
/// it must be handled in ALL THREE resolution paths:
/// 1. `resolve_parsed_type_simple` — registration phase
/// 2. `resolve_type_with_vars` — signature collection phase
/// 3. `resolve_parsed_type` — inference phase
///
/// This test catches drift: if a new type is added to one path but not the
/// others, unification between annotations and inferred types will fail
/// silently at runtime.
#[test]
fn well_known_type_resolution_sync() {
    use super::super::registration::resolve_parsed_type_simple;
    use crate::infer::{resolve_parsed_type, InferEngine};

    // (name, arity, expected_tag)
    let well_known_types: &[(&str, usize, Tag)] = &[
        ("Option", 1, Tag::Option),
        ("Result", 2, Tag::Result),
        ("Set", 1, Tag::Set),
        ("Channel", 1, Tag::Channel),
        ("Chan", 1, Tag::Channel),
        ("Range", 1, Tag::Range),
        ("Iterator", 1, Tag::Iterator),
        ("DoubleEndedIterator", 1, Tag::DoubleEndedIterator),
    ];

    for &(name_str, arity, expected_tag) in well_known_types {
        // Fresh arena + interner for each type to avoid cross-contamination
        let mut arena = ExprArena::new();
        let interner = StringInterner::new();
        let name = interner.intern(name_str);

        // Build dummy type arguments (all int)
        let type_arg_ids: Vec<ParsedTypeId> = (0..arity)
            .map(|_| arena.alloc_parsed_type(ParsedType::Primitive(ori_ir::TypeId::INT)))
            .collect();
        let type_args = arena.alloc_parsed_type_list(type_arg_ids);

        let parsed = ParsedType::Named { name, type_args };

        // Path 1: resolve_parsed_type_simple (registration)
        let mut checker = ModuleChecker::new(&arena, &interner);
        let arena_ref = checker.arena();
        let reg_idx = resolve_parsed_type_simple(&mut checker, &parsed, arena_ref);
        let reg_tag = checker.pool().tag(reg_idx);

        // Path 2: resolve_type_with_vars (signatures)
        let mut checker2 = ModuleChecker::new(&arena, &interner);
        let empty_vars = FxHashMap::default();
        let sig_idx = resolve_type_with_vars(&mut checker2, &parsed, &empty_vars, &arena);
        let sig_tag = checker2.pool().tag(sig_idx);

        // Path 3: resolve_parsed_type (inference)
        let mut pool = Pool::new();
        let mut engine = InferEngine::new(&mut pool);
        engine.set_interner(&interner);
        let infer_idx = resolve_parsed_type(&mut engine, &arena, &parsed);
        let infer_tag = engine.pool().tag(infer_idx);

        // All three must produce the same tag
        assert_eq!(
            reg_tag, expected_tag,
            "resolve_parsed_type_simple: {name_str} expected {expected_tag:?}, got {reg_tag:?}",
        );
        assert_eq!(
            sig_tag, expected_tag,
            "resolve_type_with_vars: {name_str} expected {expected_tag:?}, got {sig_tag:?}",
        );
        assert_eq!(
            infer_tag, expected_tag,
            "resolve_parsed_type: {name_str} expected {expected_tag:?}, got {infer_tag:?}",
        );
    }
}
