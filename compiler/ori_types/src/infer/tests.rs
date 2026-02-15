use super::*;
use crate::ExpectedOrigin;

#[test]
fn test_literal_inference() {
    let mut pool = Pool::new();
    let engine = InferEngine::new(&mut pool);

    assert_eq!(engine.infer_int(), Idx::INT);
    assert_eq!(engine.infer_float(), Idx::FLOAT);
    assert_eq!(engine.infer_bool(), Idx::BOOL);
    assert_eq!(engine.infer_str(), Idx::STR);
    assert_eq!(engine.infer_char(), Idx::CHAR);
    assert_eq!(engine.infer_byte(), Idx::BYTE);
    assert_eq!(engine.infer_unit(), Idx::UNIT);
}

#[test]
fn test_scope_management() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    // Initial state
    let initial_rank = engine.unify.current_rank();

    // Enter scope
    engine.enter_scope();
    assert!(engine.unify.current_rank() > initial_rank);

    // Exit scope
    engine.exit_scope();
    assert_eq!(engine.unify.current_rank(), initial_rank);
}

#[test]
fn test_context_management() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    assert!(engine.current_context().is_none());

    engine.push_context(ContextKind::IfCondition);
    assert!(matches!(
        engine.current_context(),
        Some(ContextKind::IfCondition)
    ));

    engine.push_context(ContextKind::FunctionReturn { func_name: None });
    assert!(matches!(
        engine.current_context(),
        Some(ContextKind::FunctionReturn { .. })
    ));

    engine.pop_context();
    assert!(matches!(
        engine.current_context(),
        Some(ContextKind::IfCondition)
    ));
}

#[test]
fn test_with_context() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    let result = engine.with_context(ContextKind::ListElement { index: 0 }, |eng| {
        assert!(matches!(
            eng.current_context(),
            Some(ContextKind::ListElement { index: 0 })
        ));
        42
    });

    assert_eq!(result, 42);
    assert!(engine.current_context().is_none());
}

#[test]
fn test_expression_type_storage() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    engine.store_type(0, Idx::INT);
    engine.store_type(1, Idx::STR);
    engine.store_type(2, Idx::BOOL);

    assert_eq!(engine.get_type(0), Some(Idx::INT));
    assert_eq!(engine.get_type(1), Some(Idx::STR));
    assert_eq!(engine.get_type(2), Some(Idx::BOOL));
    assert_eq!(engine.get_type(99), None);
}

#[test]
fn test_collection_inference() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    // Empty list has fresh variable element type
    let empty_list = engine.infer_empty_list();
    assert_eq!(engine.pool().tag(empty_list), crate::Tag::List);

    // List with known element type
    let int_list = engine.infer_list(Idx::INT);
    assert_eq!(engine.pool().tag(int_list), crate::Tag::List);

    // Tuple
    let tuple = engine.infer_tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
    assert_eq!(engine.pool().tag(tuple), crate::Tag::Tuple);
    assert_eq!(engine.pool().tuple_elems(tuple).len(), 3);
}

#[test]
fn test_check_type_success() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    let expected = Expected {
        ty: Idx::INT,
        origin: ExpectedOrigin::NoExpectation,
    };

    // Should succeed: INT matches INT
    let result = engine.check_type(Idx::INT, &expected, ori_ir::Span::DUMMY);
    assert!(result.is_ok());
    assert!(!engine.has_errors());
}

#[test]
fn test_check_type_with_variable() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    let var = engine.fresh_var();
    let expected = Expected {
        ty: Idx::INT,
        origin: ExpectedOrigin::NoExpectation,
    };

    // Should succeed: variable unifies with INT
    let result = engine.check_type(var, &expected, ori_ir::Span::DUMMY);
    assert!(result.is_ok());

    // Variable should now resolve to INT
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn test_check_type_failure() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    let expected = Expected {
        ty: Idx::INT,
        origin: ExpectedOrigin::NoExpectation,
    };

    // Should fail: STR doesn't match INT
    let result = engine.check_type(Idx::STR, &expected, ori_ir::Span::DUMMY);
    assert!(result.is_err());
    assert!(engine.has_errors());

    let errors = engine.errors();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0].kind, TypeErrorKind::Mismatch { .. }));
}

#[test]
#[expect(clippy::expect_used, reason = "Test code uses expect for clarity")]
fn test_let_polymorphism() {
    let mut pool = Pool::new();
    let mut engine = InferEngine::new(&mut pool);

    // Simulate: let id = |x| x
    engine.enter_scope();

    // Create id: a -> a with fresh variable
    let a = engine.fresh_var();
    let id_ty = engine.infer_function(&[a], a);

    // Generalize at scope exit
    let id_scheme = engine.generalize(id_ty);

    engine.exit_scope();

    // Bind in environment
    engine.env_mut().bind_scheme(
        ori_ir::Name::from_raw(1), // "id"
        id_scheme,
    );

    // Use id with int
    let id_int = engine.instantiate(
        engine
            .env()
            .lookup_scheme(ori_ir::Name::from_raw(1))
            .expect("id should be bound"),
    );
    let params_int = engine.pool().function_params(id_int);
    assert!(engine.unify_types(params_int[0], Idx::INT).is_ok());

    // Use id with str (should work due to polymorphism)
    let id_str = engine.instantiate(
        engine
            .env()
            .lookup_scheme(ori_ir::Name::from_raw(1))
            .expect("id should be bound"),
    );
    let params_str = engine.pool().function_params(id_str);
    assert!(engine.unify_types(params_str[0], Idx::STR).is_ok());

    // Verify independence
    assert_eq!(engine.resolve(params_int[0]), Idx::INT);
    assert_eq!(engine.resolve(params_str[0]), Idx::STR);
}
