use super::*;

#[test]
fn module_checker_basic() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();

    let checker = ModuleChecker::new(&arena, &interner);

    assert!(!checker.has_errors());
    assert!(checker.signatures.is_empty());
    assert!(checker.expr_types.is_empty());
}

#[test]
fn module_checker_with_registries() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let types = TypeRegistry::new();
    let traits = TraitRegistry::new();

    let checker = ModuleChecker::with_registries(&arena, &interner, types, traits);

    assert!(!checker.has_errors());
}

#[test]
fn module_checker_expr_types() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Store expression types
    checker.store_expr_type(0, Idx::INT);
    checker.store_expr_type(2, Idx::STR); // Skip index 1
    checker.store_expr_type(1, Idx::BOOL);

    assert_eq!(checker.get_expr_type(0), Some(Idx::INT));
    assert_eq!(checker.get_expr_type(1), Some(Idx::BOOL));
    assert_eq!(checker.get_expr_type(2), Some(Idx::STR));
    assert_eq!(checker.get_expr_type(99), None);
}

#[test]
fn module_checker_function_scope() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let fn_type = Idx::UNIT; // Placeholder
    let mut caps = FxHashSet::default();
    caps.insert(Name::from_raw(1)); // "Http"

    assert!(checker.current_function().is_none());

    checker.with_function_scope(fn_type, caps, |c| {
        assert_eq!(c.current_function(), Some(fn_type));
        assert!(c.has_capability(Name::from_raw(1)));
        assert!(!c.has_capability(Name::from_raw(2)));
    });

    assert!(checker.current_function().is_none());
}

#[test]
fn module_checker_impl_scope() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let self_ty = Idx::INT;

    assert!(checker.current_impl_self().is_none());

    checker.with_impl_scope(self_ty, |c| {
        assert_eq!(c.current_impl_self(), Some(self_ty));
    });

    assert!(checker.current_impl_self().is_none());
}

#[test]
fn module_checker_error_accumulation() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    assert!(!checker.has_errors());

    checker.error_undefined(Name::from_raw(1), Span::DUMMY);
    assert!(checker.has_errors());
    assert_eq!(checker.errors().len(), 1);

    checker.error_undefined(Name::from_raw(2), Span::DUMMY);
    assert_eq!(checker.errors().len(), 2);
}

#[test]
fn module_checker_finish() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    checker.store_expr_type(0, Idx::INT);
    checker.store_expr_type(1, Idx::STR);

    let result = checker.finish();

    assert!(!result.has_errors());
    assert_eq!(result.typed.expr_types.len(), 2);
}

#[test]
fn module_checker_finish_with_pool() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Create a custom type in the pool
    let list_int = checker.pool_mut().list(Idx::INT);
    checker.store_expr_type(0, list_int);

    let (result, pool) = checker.finish_with_pool();

    assert_eq!(result.typed.expr_types[0], list_int);
    assert_eq!(pool.tag(list_int), crate::Tag::List);
}
