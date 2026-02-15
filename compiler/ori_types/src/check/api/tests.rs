use super::*;

#[test]
fn check_empty_module() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let module = Module::default();

    let result = check_module(&module, &arena, &interner);

    assert!(!result.has_errors());
    assert!(result.typed.functions.is_empty());
}

#[test]
fn check_module_with_pool_returns_pool() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let module = Module::default();

    let (result, pool) = check_module_with_pool(&module, &arena, &interner);

    assert!(!result.has_errors());
    // Pool should have pre-interned primitives
    assert_eq!(pool.tag(crate::Idx::INT), crate::Tag::Int);
}
