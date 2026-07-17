use super::*;
use crate::{Expected, ExpectedOrigin, Pool, TypeErrorKind};

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

/// Observable snapshot of an `InferEngine`'s default state — covers every
/// field that `new` / `with_env` initialize. Adding a new `InferEngine`
/// field requires extending this snapshot so the build-SSOT regression
/// test continues to pin "both constructors produce identical state".
#[derive(Debug, PartialEq, Eq)]
struct EngineSnapshot {
    error_count: usize,
    has_context: bool,
    self_type: Option<Idx>,
    impl_self_type: Option<Idx>,
    has_current_loop_break_type: bool,
    env_is_empty: bool,
    pool_int_idx: Idx,
    pool_unit_idx: Idx,
    pool_never_idx: Idx,
}

fn snapshot(engine: &InferEngine<'_>) -> EngineSnapshot {
    EngineSnapshot {
        error_count: engine.error_count(),
        has_context: engine.current_context().is_some(),
        self_type: engine.self_type(),
        impl_self_type: engine.impl_self_type(),
        has_current_loop_break_type: engine.current_loop_break_type().is_some(),
        env_is_empty: engine.env().names().next().is_none(),
        pool_int_idx: Idx::INT,
        pool_unit_idx: Idx::UNIT,
        pool_never_idx: Idx::NEVER,
    }
}

/// Regression pin: both `InferEngine::new` and `InferEngine::with_env`
/// must route through the `build` SSOT so that adding a new field to
/// `InferEngine` requires exactly one edit, not two.
///
/// If a future field is added to only one constructor, the snapshots will
/// diverge on whatever observable that field affects — catching the
/// duplication before it drifts further.
#[test]
fn infer_engine_new_and_with_env_produce_identical_default_state() {
    let snap_new = {
        let mut pool = Pool::new();
        let engine = InferEngine::new(&mut pool);
        snapshot(&engine)
    };

    let snap_with_env = {
        let mut pool = Pool::new();
        let engine = InferEngine::with_env(&mut pool, TypeEnv::new());
        snapshot(&engine)
    };

    assert_eq!(
        snap_new, snap_with_env,
        "InferEngine::new and InferEngine::with_env(TypeEnv::new()) must \
         produce structurally identical engines — both MUST delegate to \
         Self::build. A divergence here means one constructor was edited \
         without updating the other."
    );
}
