use super::*;
use ori_ir::{ExprArena, StringInterner};

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
