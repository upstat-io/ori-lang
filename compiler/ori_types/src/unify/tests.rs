use super::*;

#[test]
fn unify_identical_primitives() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    assert!(engine.unify(Idx::INT, Idx::INT).is_ok());
    assert!(engine.unify(Idx::STR, Idx::STR).is_ok());
}

#[test]
fn unify_different_primitives_fails() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    let result = engine.unify(Idx::INT, Idx::STR);
    assert!(matches!(result, Err(UnifyError::Mismatch { .. })));
}

#[test]
fn unify_variable_with_primitive() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    let var = engine.fresh_var();
    assert!(engine.unify(var, Idx::INT).is_ok());
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn unify_two_variables() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    let var1 = engine.fresh_var();
    let var2 = engine.fresh_var();

    assert!(engine.unify(var1, var2).is_ok());

    // Now unify one with a concrete type
    assert!(engine.unify(var1, Idx::BOOL).is_ok());

    // Both should resolve to BOOL
    assert_eq!(engine.resolve(var1), Idx::BOOL);
    assert_eq!(engine.resolve(var2), Idx::BOOL);
}

#[test]
fn path_compression() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    // Create chain: var1 -> var2 -> var3 -> INT
    let var1 = engine.fresh_var();
    let var2 = engine.fresh_var();
    let var3 = engine.fresh_var();

    assert!(engine.unify(var1, var2).is_ok());
    assert!(engine.unify(var2, var3).is_ok());
    assert!(engine.unify(var3, Idx::INT).is_ok());

    // Resolving var1 should compress the path
    let resolved = engine.resolve(var1);
    assert_eq!(resolved, Idx::INT);

    // After compression, var1 should point directly to INT
    let var1_id = pool.data(var1);
    match pool.var_state(var1_id) {
        VarState::Link { target } => assert_eq!(*target, Idx::INT),
        _ => panic!("Expected Link"),
    }
}

#[test]
fn occurs_check_detects_infinite_type() {
    let mut pool = Pool::new();

    // Create the types first, before creating the engine
    let var = pool.fresh_var();
    let list_var = pool.list(var);

    let mut engine = UnifyEngine::new(&mut pool);

    // Trying to unify var with List<var> should fail
    let result = engine.unify(var, list_var);
    assert!(matches!(result, Err(UnifyError::InfiniteType { .. })));
}

#[test]
fn unify_lists() {
    let mut pool = Pool::new();
    let list1 = pool.list(Idx::INT);
    let list2 = pool.list(Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(list1, list2).is_ok());
}

#[test]
fn unify_lists_with_variable() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let list_var = pool.list(var);
    let list_int = pool.list(Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(list_var, list_int).is_ok());
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn unify_functions() {
    let mut pool = Pool::new();
    let fn1 = pool.function(&[Idx::INT], Idx::BOOL);
    let fn2 = pool.function(&[Idx::INT], Idx::BOOL);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(fn1, fn2).is_ok());
}

#[test]
fn unify_functions_arity_mismatch() {
    let mut pool = Pool::new();
    let fn1 = pool.function(&[Idx::INT], Idx::BOOL);
    let fn2 = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);

    let mut engine = UnifyEngine::new(&mut pool);
    let result = engine.unify(fn1, fn2);
    assert!(matches!(
        result,
        Err(UnifyError::ArityMismatch {
            kind: ArityKind::Function,
            ..
        })
    ));
}

#[test]
fn unify_functions_with_variables() {
    let mut pool = Pool::new();
    let var1 = pool.fresh_var();
    let var2 = pool.fresh_var();
    let fn_vars = pool.function(&[var1], var2);
    let fn_concrete = pool.function(&[Idx::STR], Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(fn_vars, fn_concrete).is_ok());
    assert_eq!(engine.resolve(var1), Idx::STR);
    assert_eq!(engine.resolve(var2), Idx::INT);
}

#[test]
fn unify_tuples() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let tuple1 = pool.tuple(&[var, Idx::BOOL]);
    let tuple2 = pool.tuple(&[Idx::INT, Idx::BOOL]);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(tuple1, tuple2).is_ok());
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn unify_maps() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let map1 = pool.map(Idx::STR, var);
    let map2 = pool.map(Idx::STR, Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(map1, map2).is_ok());
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn never_unifies_with_anything() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    assert!(engine.unify(Idx::NEVER, Idx::INT).is_ok());
    assert!(engine.unify(Idx::STR, Idx::NEVER).is_ok());
}

#[test]
fn error_propagates() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    // Error type unifies with anything (prevents cascading errors)
    assert!(engine.unify(Idx::ERROR, Idx::INT).is_ok());
    assert!(engine.unify(Idx::STR, Idx::ERROR).is_ok());
}

#[test]
fn rigid_cannot_unify_with_concrete() {
    let mut pool = Pool::new();
    let name = ori_ir::Name::from_raw(1);
    let rigid = pool.rigid_var(name);

    let mut engine = UnifyEngine::new(&mut pool);
    let result = engine.unify(rigid, Idx::INT);
    assert!(matches!(result, Err(UnifyError::RigidMismatch { .. })));
}

#[test]
fn rank_management() {
    let mut pool = Pool::new();
    let mut engine = UnifyEngine::new(&mut pool);

    assert_eq!(engine.current_rank(), Rank::FIRST);

    engine.enter_scope();
    assert_eq!(engine.current_rank(), Rank::FIRST.next());

    engine.enter_scope();
    assert_eq!(engine.current_rank(), Rank::FIRST.next().next());

    engine.exit_scope();
    assert_eq!(engine.current_rank(), Rank::FIRST.next());

    engine.exit_scope();
    assert_eq!(engine.current_rank(), Rank::FIRST);

    // Can't go below FIRST rank
    engine.exit_scope();
    assert_eq!(engine.current_rank(), Rank::FIRST);
}

// ========================================
// Generalization Tests
// ========================================

#[test]
fn generalize_monomorphic() {
    let mut pool = Pool::new();

    // Create types before engine
    let fn_ty = pool.function(&[Idx::INT], Idx::BOOL);

    let mut engine = UnifyEngine::new(&mut pool);

    // Monomorphic types return unchanged
    let result = engine.generalize(Idx::INT);
    assert_eq!(result, Idx::INT);

    // Function with no variables
    let result = engine.generalize(fn_ty);
    assert_eq!(result, fn_ty);
}

#[test]
fn generalize_identity_function() {
    let mut pool = Pool::new();

    // Create the types first
    let var = pool.fresh_var_with_rank(Rank::FIRST.next()); // Inner scope rank
    let fn_ty = pool.function(&[var], var); // a -> a

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();

    // Generalize at this rank
    let scheme = engine.generalize(fn_ty);

    // Should be a scheme
    assert_eq!(engine.pool().tag(scheme), Tag::Scheme);

    // Should have one quantified variable
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1);

    // Body should be the function type
    assert_eq!(engine.pool().scheme_body(scheme), fn_ty);
}

#[test]
fn generalize_does_not_generalize_outer_vars() {
    let mut pool = Pool::new();

    // Create variables at different ranks
    let outer_var = pool.fresh_var_with_rank(Rank::FIRST); // Outer scope
    let inner_var = pool.fresh_var_with_rank(Rank::FIRST.next()); // Inner scope
    let fn_ty = pool.function(&[outer_var], inner_var); // outer -> inner

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope(); // Now at inner rank

    // Generalize at inner rank - only inner_var should be generalized
    let scheme = engine.generalize(fn_ty);

    assert_eq!(engine.pool().tag(scheme), Tag::Scheme);

    // Should have only one quantified variable (inner)
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1);
}

// ========================================
// Instantiation Tests
// ========================================

#[test]
fn instantiate_non_scheme() {
    let mut pool = Pool::new();

    // Create types before engine
    let fn_ty = pool.function(&[Idx::INT], Idx::BOOL);

    let mut engine = UnifyEngine::new(&mut pool);

    // Non-scheme types return unchanged
    let result = engine.instantiate(Idx::INT);
    assert_eq!(result, Idx::INT);

    let result = engine.instantiate(fn_ty);
    assert_eq!(result, fn_ty);
}

#[test]
fn instantiate_identity_scheme() {
    let mut pool = Pool::new();

    // Create a scheme manually: ∀a. a -> a
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let var_id = pool.data(var);
    let fn_ty = pool.function(&[var], var);
    let scheme = pool.scheme(&[var_id], fn_ty);

    // Mark the var as generalized
    *pool.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };

    let mut engine = UnifyEngine::new(&mut pool);

    // Instantiate
    let instance = engine.instantiate(scheme);

    // Should be a function type with fresh variables
    assert_eq!(engine.pool().tag(instance), Tag::Function);

    // Both param and return should be the same fresh variable
    let params = engine.pool().function_params(instance);
    let ret = engine.pool().function_return(instance);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0], ret);

    // The fresh var should be different from the original
    assert_ne!(params[0], var);
}

#[test]
fn instantiate_twice_gives_different_vars() {
    let mut pool = Pool::new();

    // Create scheme: ∀a. a -> a
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let var_id = pool.data(var);
    let fn_ty = pool.function(&[var], var);
    let scheme = pool.scheme(&[var_id], fn_ty);
    *pool.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };

    let mut engine = UnifyEngine::new(&mut pool);

    // Instantiate twice
    let instance1 = engine.instantiate(scheme);
    let instance2 = engine.instantiate(scheme);

    // Both should be function types
    assert_eq!(engine.pool().tag(instance1), Tag::Function);
    assert_eq!(engine.pool().tag(instance2), Tag::Function);

    // But with different fresh variables
    let params1 = engine.pool().function_params(instance1);
    let params2 = engine.pool().function_params(instance2);
    assert_ne!(params1[0], params2[0]);
}

#[test]
fn let_polymorphism_example() {
    // The canonical test: id can be used with different types
    let mut pool = Pool::new();

    // Create id = |x| x at inner rank
    let x = pool.fresh_var_with_rank(Rank::FIRST.next());
    let id_ty = pool.function(&[x], x);
    let x_id = pool.data(x);

    // Create scheme manually (since generalize needs the engine)
    let id_scheme = pool.scheme(&[x_id], id_ty);
    *pool.var_state_mut(x_id) = VarState::Generalized {
        id: x_id,
        name: None,
    };

    let mut engine = UnifyEngine::new(&mut pool);

    // Use id with int
    let id_int = engine.instantiate(id_scheme);
    let params_int = engine.pool().function_params(id_int);
    let param_int = params_int[0];
    assert!(engine.unify(param_int, Idx::INT).is_ok());

    // Use id with str (should get different fresh var)
    let id_str = engine.instantiate(id_scheme);
    let params_str = engine.pool().function_params(id_str);
    let param_str = params_str[0];
    assert!(engine.unify(param_str, Idx::STR).is_ok());

    // Verify: params_int resolved to INT, params_str resolved to STR
    assert_eq!(engine.resolve(param_int), Idx::INT);
    assert_eq!(engine.resolve(param_str), Idx::STR);

    // They should be independent
    assert_ne!(engine.resolve(param_int), engine.resolve(param_str));
}

// ========================================
// Borrowed Reference Tests
// ========================================

#[test]
fn unify_identical_borrowed() {
    let mut pool = Pool::new();
    let b1 = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);
    let b2 = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(b1, b2).is_ok());
}

#[test]
fn unify_borrowed_with_variable_inner() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let b_var = pool.borrowed(var, crate::LifetimeId::STATIC);
    let b_int = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(b_var, b_int).is_ok());
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn unify_borrowed_inner_mismatch() {
    let mut pool = Pool::new();
    let b_int = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);
    let b_str = pool.borrowed(Idx::STR, crate::LifetimeId::STATIC);

    let mut engine = UnifyEngine::new(&mut pool);
    let result = engine.unify(b_int, b_str);
    assert!(matches!(result, Err(UnifyError::Mismatch { .. })));
}

#[test]
fn unify_borrowed_lifetime_mismatch() {
    let mut pool = Pool::new();
    let b_static = pool.borrowed(Idx::INT, crate::LifetimeId::STATIC);
    let b_scoped = pool.borrowed(Idx::INT, crate::LifetimeId::SCOPED);

    let mut engine = UnifyEngine::new(&mut pool);
    let result = engine.unify(b_static, b_scoped);
    assert!(matches!(result, Err(UnifyError::Mismatch { .. })));
}

#[test]
fn occurs_check_finds_var_in_borrowed() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let borrowed_var = pool.borrowed(var, crate::LifetimeId::STATIC);

    let mut engine = UnifyEngine::new(&mut pool);
    let result = engine.unify(var, borrowed_var);
    assert!(matches!(result, Err(UnifyError::InfiniteType { .. })));
}

#[test]
fn generalize_finds_vars_in_borrowed() {
    let mut pool = Pool::new();
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let borrowed_ty = pool.borrowed(var, crate::LifetimeId::STATIC);

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();

    let scheme = engine.generalize(borrowed_ty);

    assert_eq!(engine.pool().tag(scheme), Tag::Scheme);
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1);
}

#[test]
fn substitute_through_borrowed() {
    let mut pool = Pool::new();

    // Create scheme: ∀a. &a
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let var_id = pool.data(var);
    let borrowed_ty = pool.borrowed(var, crate::LifetimeId::STATIC);
    let scheme = pool.scheme(&[var_id], borrowed_ty);
    *pool.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };

    let mut engine = UnifyEngine::new(&mut pool);

    // Instantiate: should replace the inner variable
    let instance = engine.instantiate(scheme);
    assert_eq!(engine.pool().tag(instance), Tag::Borrowed);

    // Inner should be a fresh variable, not the original
    let inner = engine.pool().borrowed_inner(instance);
    assert_ne!(inner, var);
    assert_eq!(engine.pool().tag(inner), Tag::Var);

    // Lifetime should be preserved
    let lt = engine.pool().borrowed_lifetime(instance);
    assert_eq!(lt, crate::LifetimeId::STATIC);
}
