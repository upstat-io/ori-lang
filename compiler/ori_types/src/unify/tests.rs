use super::*;
use crate::{GeneralizedVarState, Tag, VarState};

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

// Regression: a variable nested inside an error-bearing compound type must
// bind through unification. `Error` shares the poison slot (Idx::ERROR), so
// `Result<str, Error>` carries HAS_ERROR; the early-out must still bind the
// free error parameter rather than leaving it dangling (which surfaced a
// spurious E2005 on `let r: Result<str, Error> = Ok("x"); is_ok(result: r)`).
#[test]
fn unify_result_error_binds_nested_variable() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let annotated = pool.result(Idx::STR, Idx::ERROR);
    let inferred = pool.result(Idx::STR, var);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(annotated, inferred).is_ok());
    // The free error parameter must resolve to the Error type, not remain unbound.
    assert_eq!(engine.resolve(var), Idx::ERROR);
}

// Negative pin: cascade suppression is retained — a concrete-vs-error mismatch
// in a non-variable position does NOT surface a diagnostic.
#[test]
fn unify_error_bearing_concrete_mismatch_is_suppressed() {
    let mut pool = Pool::new();
    let a = pool.result(Idx::STR, Idx::ERROR);
    let b = pool.result(Idx::INT, Idx::BOOL);

    let mut engine = UnifyEngine::new(&mut pool);
    // str-vs-int mismatch inside the error-bearing Result is suppressed (no Err).
    assert!(engine.unify(a, b).is_ok());
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

// Generalization Tests

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

    // Body is structurally a function `BoundVar -> BoundVar`, NOT the
    // original `fn_ty` Idx. Generalization rewrites scheme bodies to
    // `Tag::BoundVar` leaves (cell A pins the full shape; this test
    // stays as a scheme-construction smoke check).
    let body = engine.pool().scheme_body(scheme);
    assert_eq!(engine.pool().tag(body), Tag::Function);
    let params: Vec<Idx> = engine.pool().function_params(body);
    let ret = engine.pool().function_return(body);
    assert_eq!(params.len(), 1);
    assert_eq!(engine.pool().tag(params[0]), Tag::BoundVar);
    assert_eq!(engine.pool().tag(ret), Tag::BoundVar);
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

// Scheme-body canonicalization: Generalized -> BoundVar migration.
//
// Target: scheme bodies SHALL contain `Tag::BoundVar` leaves with
// `data = var_id`, not `Tag::Var(VarState::Generalized)`. These cells pin
// the post-migration shape and are RED until `rewrite_body_generalized_to_bound_var`
// lands in `generalize()`.

/// Cell A — poly-lambda single-call at concrete type.
///
/// After `generalize(fn_ty)` on a body containing a fresh generalizable var,
/// the resulting scheme's body carries `Tag::BoundVar` at every position that
/// referenced the generalized var — not `Tag::Var`. Additionally, the
/// `BoundVar`'s `data` field equals the scheme's declared `var_id`
///
#[test]
fn generalize_identity_lambda_body_contains_bound_var_leaves() {
    let mut pool = Pool::new();
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let fn_ty = pool.function(&[var], var); // a -> a

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(fn_ty);

    assert_eq!(engine.pool().tag(scheme), Tag::Scheme);
    let body = engine.pool().scheme_body(scheme);
    assert_eq!(engine.pool().tag(body), Tag::Function);

    let params: Vec<Idx> = engine.pool().function_params(body);
    let ret = engine.pool().function_return(body);
    assert_eq!(params.len(), 1);

    // Post-migration assertions: RED until the BoundVar scheme-body rewrite lands.
    assert_eq!(
        engine.pool().tag(params[0]),
        Tag::BoundVar,
        ": scheme body param must be Tag::BoundVar (not Tag::Var)"
    );
    assert_eq!(
        engine.pool().tag(ret),
        Tag::BoundVar,
        ": scheme body return must be Tag::BoundVar (not Tag::Var)"
    );

    // BoundVar.data == var_id for scheme-declared binders.
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1);
    assert_eq!(
        engine.pool().data(params[0]),
        vars[0],
        "BoundVar.data must equal the scheme's declared var_id"
    );
    assert_eq!(
        engine.pool().data(ret),
        vars[0],
        "BoundVar.data must equal the scheme's declared var_id"
    );
}

/// Cell B — poly-lambda multi-instantiation.
///
/// Instantiating the same scheme twice yields independent fresh `Tag::Var`
/// substitutions at each call site; unifying one does not affect the other.
/// Pins spec §SC-3 (instantiation = fresh Var per bound var) under the new
/// BoundVar-bodied scheme representation.
#[test]
fn generalize_then_instantiate_twice_yields_independent_fresh_vars() {
    let mut pool = Pool::new();
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let fn_ty = pool.function(&[var], var);

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(fn_ty);

    // First instantiation — unify with int.
    let inst1 = engine.instantiate(scheme);
    let inst1_params: Vec<Idx> = engine.pool().function_params(inst1);
    let inst1_ret = engine.pool().function_return(inst1);
    assert!(engine.unify(inst1_params[0], Idx::INT).is_ok());
    assert_eq!(engine.resolve(inst1_ret), Idx::INT);

    // Second instantiation — unify with str. Must NOT be affected by inst1's int.
    let inst2 = engine.instantiate(scheme);
    let inst2_params: Vec<Idx> = engine.pool().function_params(inst2);
    let inst2_ret = engine.pool().function_return(inst2);
    assert!(engine.unify(inst2_params[0], Idx::STR).is_ok());
    assert_eq!(engine.resolve(inst2_ret), Idx::STR);

    // inst1 still resolves to int (independent instantiation contexts).
    assert_eq!(engine.resolve(inst1_params[0]), Idx::INT);
}

/// Cell C — unused poly-lambda.
///
/// Even when the scheme is never instantiated, the body canonicalizes to
/// `Tag::BoundVar` leaves at generalize-time. Body shape is a property of
/// the scheme, not of its use sites.
#[test]
fn generalize_unused_poly_lambda_canonicalizes_to_bound_var_body() {
    let mut pool = Pool::new();
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let fn_ty = pool.function(&[var], var);

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(fn_ty);

    // Scheme never instantiated — body STILL must be canonicalized.
    let body = engine.pool().scheme_body(scheme);
    let params: Vec<Idx> = engine.pool().function_params(body);
    let ret = engine.pool().function_return(body);

    assert_eq!(engine.pool().tag(params[0]), Tag::BoundVar);
    assert_eq!(engine.pool().tag(ret), Tag::BoundVar);
}

/// Cell D — nested poly-lambda.
///
/// `x -> y -> (x, y)` generalizes BOTH x and y; the scheme body has
/// `Tag::BoundVar` at each binder position across the nested function chain,
/// including inside compound types (the tuple).
#[test]
fn generalize_nested_lambda_rewrites_both_binders_to_bound_var() {
    let mut pool = Pool::new();
    let x = pool.fresh_var_with_rank(Rank::FIRST.next());
    let y = pool.fresh_var_with_rank(Rank::FIRST.next());
    let pair_ty = pool.tuple(&[x, y]); // (x, y)
    let inner_fn = pool.function(&[y], pair_ty); // y -> (x, y)
    let outer_fn = pool.function(&[x], inner_fn); // x -> y -> (x, y)

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(outer_fn);

    assert_eq!(engine.pool().tag(scheme), Tag::Scheme);
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 2, "both x and y must be generalized");

    // Outer fn: param(0)=BoundVar, return=inner fn.
    let body = engine.pool().scheme_body(scheme);
    let outer_params: Vec<Idx> = engine.pool().function_params(body);
    let outer_ret = engine.pool().function_return(body);
    assert_eq!(engine.pool().tag(outer_params[0]), Tag::BoundVar);
    assert_eq!(engine.pool().tag(outer_ret), Tag::Function);

    // Inner fn: param(0)=BoundVar, return=Tuple<BoundVar, BoundVar>.
    let inner_params: Vec<Idx> = engine.pool().function_params(outer_ret);
    let inner_ret = engine.pool().function_return(outer_ret);
    assert_eq!(engine.pool().tag(inner_params[0]), Tag::BoundVar);
    assert_eq!(engine.pool().tag(inner_ret), Tag::Tuple);

    let tuple_elems: Vec<Idx> = engine.pool().tuple_elems(inner_ret);
    assert_eq!(tuple_elems.len(), 2);
    assert_eq!(engine.pool().tag(tuple_elems[0]), Tag::BoundVar);
    assert_eq!(engine.pool().tag(tuple_elems[1]), Tag::BoundVar);
}

/// Cell E — poly-lambda return-position polymorphic type.
///
/// `x -> Some(x)` yields scheme `∀a. a -> Option<a>`. Post-generalize, the
/// scheme body's return type is `Option<BoundVar>` — the nested `Tag::Var`
/// inside `Tag::Option` must also be rewritten, exercising the body-walker's
/// recursion through single-child containers.
#[test]
fn generalize_return_position_polymorphic_type_rewrites_nested_var() {
    let mut pool = Pool::new();
    let var = pool.fresh_var_with_rank(Rank::FIRST.next());
    let option_of_var = pool.option(var); // Option<a>
    let fn_ty = pool.function(&[var], option_of_var); // a -> Option<a>

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(fn_ty);

    let body = engine.pool().scheme_body(scheme);
    let params: Vec<Idx> = engine.pool().function_params(body);
    let ret = engine.pool().function_return(body);

    assert_eq!(
        engine.pool().tag(params[0]),
        Tag::BoundVar,
        "param must be BoundVar"
    );
    assert_eq!(
        engine.pool().tag(ret),
        Tag::Option,
        "return must stay Option<_>"
    );

    // Inner child of Option<_> MUST be BoundVar (not Tag::Var).
    let option_child = Idx::from_raw(engine.pool().data(ret));
    assert_eq!(
        engine.pool().tag(option_child),
        Tag::BoundVar,
        "Option's child type must be Tag::BoundVar after body rewrite"
    );
}

// Instantiation Tests

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
    *pool.var_state_mut(var_id) = VarState::Generalized(GeneralizedVarState {
        id: var_id,
        name: None,
    });

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
    *pool.var_state_mut(var_id) = VarState::Generalized(GeneralizedVarState {
        id: var_id,
        name: None,
    });

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
    *pool.var_state_mut(x_id) = VarState::Generalized(GeneralizedVarState {
        id: x_id,
        name: None,
    });

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

// Borrowed Reference Tests

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
    *pool.var_state_mut(var_id) = VarState::Generalized(GeneralizedVarState {
        id: var_id,
        name: None,
    });

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

// DoubleEndedIterator Coercion Tests

#[test]
fn unify_dei_with_iterator_succeeds() {
    let mut pool = Pool::new();
    let dei = pool.double_ended_iterator(Idx::INT);
    let iter = pool.iterator(Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    // DEI coerces to Iterator (same element type)
    assert!(engine.unify(dei, iter).is_ok());
}

#[test]
fn unify_iterator_with_dei_succeeds() {
    let mut pool = Pool::new();
    let iter = pool.iterator(Idx::STR);
    let dei = pool.double_ended_iterator(Idx::STR);

    let mut engine = UnifyEngine::new(&mut pool);
    // Order shouldn't matter for coercion
    assert!(engine.unify(iter, dei).is_ok());
}

#[test]
fn unify_dei_with_iterator_resolves_element_var() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let dei = pool.double_ended_iterator(var);
    let iter = pool.iterator(Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(dei, iter).is_ok());
    // Element type variable should resolve to INT
    assert_eq!(engine.resolve(var), Idx::INT);
}

#[test]
fn unify_dei_element_mismatch_fails() {
    let mut pool = Pool::new();
    let dei = pool.double_ended_iterator(Idx::INT);
    let iter = pool.iterator(Idx::STR);

    let mut engine = UnifyEngine::new(&mut pool);
    // Different element types should fail
    let result = engine.unify(dei, iter);
    assert!(result.is_err());
}

#[test]
fn unify_identical_deis() {
    let mut pool = Pool::new();
    let dei1 = pool.double_ended_iterator(Idx::CHAR);
    let dei2 = pool.double_ended_iterator(Idx::CHAR);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(dei1, dei2).is_ok());
}

#[test]
fn unify_dei_with_variable() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let dei = pool.double_ended_iterator(Idx::INT);

    let mut engine = UnifyEngine::new(&mut pool);
    assert!(engine.unify(var, dei).is_ok());
    let resolved = engine.resolve(var);
    assert_eq!(engine.pool().tag(resolved), Tag::DoubleEndedIterator);
}

#[test]
fn occurs_check_finds_var_in_dei() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let dei_var = pool.double_ended_iterator(var);

    let mut engine = UnifyEngine::new(&mut pool);
    let result = engine.unify(var, dei_var);
    assert!(matches!(result, Err(UnifyError::InfiniteType { .. })));
}

// Generalization compound-tag delegation.
//
// `collect_free_vars_inner` historically open-coded a tag-dispatch ladder over
// the seven simple containers, Map, Result, Borrowed, Function, Tuple, and
// Applied — duplicating the partition `Pool::visit_children` already
// canonicalizes (pool/descriptor.rs). The `_ => {}` catch-all silently dropped
// `Tag::Struct` / `Tag::Enum` / `Tag::Scheme`, missing free vars under those
// shapes (under-generalization). The fix delegates compound recursion to
// `visit_children`, mirroring `check::validators::collect_first_unbound_var`.
//
// Cells below clamp the contract from three sides:
//   1. **Compound-tag matrix** — every container shape currently in the
//      ladder MUST keep generalizing correctly under delegation.
//   2. **Behavior-delta pins** (Struct / Enum / Scheme) — free vars under
//      these tags MUST now be collected. Before the fix these tests fail
//      (`generalize` returns the type unchanged). After the fix they pass.
//   3. **Negative pin** — a contrived shape that demonstrates active
//      delegation: regressing the `_ => visit_children(...)` arm to
//      `_ => {}` would flip the assertion.

/// Cell — compound-tag matrix.
///
/// For every container shape currently enumerated in the ladder, build a
/// monomorphic body containing a fresh inner-rank `Var`, generalize at the
/// inner rank, and assert exactly one var is bound. This pins continuity of
/// behavior across the 13 container tags after delegation.
#[test]
fn generalize_collects_free_var_under_every_compound_tag() {
    use ori_ir::Name;

    // Pre-build all the compound shapes — one per tag in the legacy ladder —
    // each containing a fresh inner-rank `Var`. After generalize we expect
    // exactly one quantified var per shape.
    type CaseBuilder = fn(&mut Pool) -> (Idx, Idx);
    let cases: &[(&str, CaseBuilder)] = &[
        ("List", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.list(v))
        }),
        ("Option", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.option(v))
        }),
        ("Set", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.set(v))
        }),
        ("Channel", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.channel(v))
        }),
        ("Range", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.range(v))
        }),
        ("Iterator", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.iterator(v))
        }),
        ("DoubleEndedIterator", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.double_ended_iterator(v))
        }),
        ("Map", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.map(Idx::STR, v))
        }),
        ("Result", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.result(v, Idx::STR))
        }),
        ("Borrowed", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.borrowed(v, crate::LifetimeId::STATIC))
        }),
        ("Function", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.function(&[Idx::INT], v))
        }),
        ("Tuple", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            (v, p.tuple(&[v, Idx::BOOL]))
        }),
        ("Applied", |p| {
            let v = p.fresh_var_with_rank(Rank::FIRST.next());
            let name = Name::from_raw(1);
            (v, p.applied(name, &[v]))
        }),
    ];

    let mut count = 0;
    for (label, build) in cases {
        let mut pool = Pool::new();
        let (_var, body_ty) = build(&mut pool);

        let mut engine = UnifyEngine::new(&mut pool);
        engine.enter_scope(); // now at FIRST.next()

        let scheme = engine.generalize(body_ty);
        assert_eq!(
            engine.pool().tag(scheme),
            Tag::Scheme,
            "{label}: generalize must produce a scheme",
        );
        let vars = engine.pool().scheme_vars(scheme);
        assert_eq!(
            vars.len(),
            1,
            "{label}: scheme must bind exactly one var (delegation must reach the compound's child)",
        );
        count += 1;
    }
    // Self-verifying matrix completeness:
    // proves the loop visited every cell — a silent skip would be worse than
    // no matrix at all.
    assert_eq!(count, 13, "matrix must visit every cell");
}

/// Cell — behavior delta: `Tag::Struct` body containing a free var.
///
/// Pre-fix: the `_ => {}` arm silently drops `Tag::Struct`; the var is never
/// collected; `generalize` returns the type unchanged → no scheme produced.
/// Post-fix: delegation walks struct fields via `visit_children` → var is
/// collected → scheme binds one var.
///
/// This is the load-bearing semantic pin for the Struct path. Reverting
/// the delegation would flip `Tag::Scheme` → `Tag::Function` here.
#[test]
fn generalize_collects_free_var_under_struct_body() {
    use ori_ir::Name;

    let mut pool = Pool::new();
    let v = pool.fresh_var_with_rank(Rank::FIRST.next());
    // `type S = { f: Var }`
    let struct_ty = pool.struct_type(Name::from_raw(1), &[(Name::from_raw(2), v)]);
    // `() -> S`
    let body_ty = pool.function(&[], struct_ty);

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(body_ty);

    assert_eq!(
        engine.pool().tag(scheme),
        Tag::Scheme,
        "Struct: free var under field MUST be collected (was dropped pre-fix by _ => {{}})",
    );
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1, "Struct: exactly one var must be bound");
}

/// Cell — behavior delta: `Tag::Enum` variant payload containing a free var.
///
/// Same shape of pin as the Struct cell — clamps the Enum recursion path.
#[test]
fn generalize_collects_free_var_under_enum_variant_payload() {
    use crate::EnumVariant;
    use ori_ir::Name;

    let mut pool = Pool::new();
    let v = pool.fresh_var_with_rank(Rank::FIRST.next());
    let variants = [EnumVariant {
        name: Name::from_raw(2),
        field_types: vec![v],
    }];
    let enum_ty = pool.enum_type(Name::from_raw(1), &variants);
    let body_ty = pool.function(&[], enum_ty);

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(body_ty);

    assert_eq!(
        engine.pool().tag(scheme),
        Tag::Scheme,
        "Enum: free var under variant payload MUST be collected (was dropped pre-fix)",
    );
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1, "Enum: exactly one var must be bound");
}

/// Cell — behavior delta: `Tag::Scheme` body containing a free outer-rank var.
///
/// Schemes whose body references a Var from an enclosing rank must propagate
/// that free var to outer generalization. explicitly allows
/// free `Tag::Var` leaves in scheme bodies; delegation must recurse into the
/// scheme body to collect them.
///
/// `Tag::BoundVar` leaves inside the body do NOT trigger collection because
/// they set `HAS_BOUND_VAR`, not `HAS_VAR` (TF-1); the top-level
/// `HAS_VAR` fast-path returns early on a body containing only bound vars.
#[test]
fn generalize_collects_free_var_under_scheme_body() {
    let mut pool = Pool::new();

    // Outer-rank var that will be free inside the inner scheme.
    let outer_var = pool.fresh_var_with_rank(Rank::FIRST);

    // Inner scheme: ∀b. (b) -> outer_var  — body holds a free outer Var.
    let bound_id = pool.allocate_var_id_with_rank(Rank::FIRST.next());
    let bound_node = pool.bound_var(bound_id);
    let inner_body = pool.function(&[bound_node], outer_var);
    let inner_scheme = pool.scheme(&[bound_id], inner_body);

    // Wrap the scheme inside a tuple so we generalize over a structure that
    // contains `Tag::Scheme` as a child — this is the path the legacy `_ => {}`
    // arm dropped.
    let outer_body = pool.tuple(&[inner_scheme]);

    let mut engine = UnifyEngine::new(&mut pool);
    // Stay at Rank::FIRST so outer_var is generalizable.
    let result = engine.generalize(outer_body);

    assert_eq!(
        engine.pool().tag(result),
        Tag::Scheme,
        "Scheme: free outer-rank var inside scheme body MUST be collected (was dropped pre-fix)",
    );
    let vars = engine.pool().scheme_vars(result);
    assert_eq!(
        vars.len(),
        1,
        "Scheme: exactly one var (the outer-rank one) must be bound",
    );
}

/// Negative pin — actively asserts delegation is in place.
///
/// Constructs a deeply nested compound (`[Struct{f: Var}]`) and asserts the
/// scheme binds the var. Reverting the `_ => visit_children(...)` arm to
/// `_ => {}` would drop the struct and leave the var uncollected, returning
/// the original list type unchanged — `Tag::Scheme` assertion would fail.
///
/// This pairs with the positive Struct cell above: positive proves "works",
/// negative proves "delegation is what makes it work".
#[test]
fn generalize_delegation_is_load_bearing_for_nested_struct_in_list() {
    use ori_ir::Name;

    let mut pool = Pool::new();
    let v = pool.fresh_var_with_rank(Rank::FIRST.next());
    let struct_ty = pool.struct_type(Name::from_raw(1), &[(Name::from_raw(2), v)]);
    let list_of_struct = pool.list(struct_ty);

    let mut engine = UnifyEngine::new(&mut pool);
    engine.enter_scope();
    let scheme = engine.generalize(list_of_struct);

    // If the `_ =>` arm dropped the `Tag::Struct` child, `list_of_struct`
    // would have HAS_VAR=true but the Struct under it would be ignored;
    // `generalize` would skip the var and return the type unchanged.
    assert_eq!(
        engine.pool().tag(scheme),
        Tag::Scheme,
        "delegation must recurse List → Struct → field; reverting to _ => {{}} flips this",
    );
    let vars = engine.pool().scheme_vars(scheme);
    assert_eq!(vars.len(), 1);
}

// `instantiate()` on a scheme body carrying a STRUCT field whose field ty is a
// `Tag::BoundVar` must substitute the inline field var to the fresh instantiation
// var via the `var_subst` path. Without a `Tag::Struct` arm in `substitute`
// (`_ => ty`), the field would stay `BoundVar`.
#[test]
fn instantiate_substitutes_inline_struct_bound_var_field() {
    use ori_ir::Name;

    let mut pool = Pool::new();
    let bound_id = pool.allocate_var_id_with_rank(Rank::FIRST.next());
    let bound_node = pool.bound_var(bound_id);
    let struct_body = pool.struct_type(Name::from_raw(11), &[(Name::from_raw(12), bound_node)]);
    let scheme = pool.scheme(&[bound_id], struct_body);

    let mut engine = UnifyEngine::new(&mut pool);
    let instance = engine.instantiate(scheme);

    assert_eq!(
        engine.pool().tag(instance),
        Tag::Struct,
        "instantiated scheme body must remain a Struct; got {instance:?}",
    );
    let (_, field_ty) = engine.pool().struct_field(instance, 0);
    assert_eq!(
        engine.pool().tag(field_ty),
        Tag::Var,
        "the inline struct field BoundVar must be substituted to a fresh Var; got {:?}",
        engine.pool().tag(field_ty),
    );
}

// `instantiate()` on a scheme body carrying an ENUM variant payload that is a
// `Tag::BoundVar` must substitute the inline payload var to the fresh
// instantiation var. Without a `Tag::Enum` arm in `substitute` (`_ => ty`), the
// payload would stay `BoundVar`.
#[test]
fn instantiate_substitutes_inline_enum_bound_var_payload() {
    use ori_ir::Name;

    let mut pool = Pool::new();
    let bound_id = pool.allocate_var_id_with_rank(Rank::FIRST.next());
    let bound_node = pool.bound_var(bound_id);
    let enum_body = pool.enum_type(
        Name::from_raw(21),
        &[crate::EnumVariant {
            name: Name::from_raw(22),
            field_types: vec![bound_node],
        }],
    );
    let scheme = pool.scheme(&[bound_id], enum_body);

    let mut engine = UnifyEngine::new(&mut pool);
    let instance = engine.instantiate(scheme);

    assert_eq!(
        engine.pool().tag(instance),
        Tag::Enum,
        "instantiated scheme body must remain an Enum; got {instance:?}",
    );
    let (_, payloads) = engine.pool().enum_variant(instance, 0);
    assert_eq!(payloads.len(), 1, "variant must keep its single payload");
    assert_eq!(
        engine.pool().tag(payloads[0]),
        Tag::Var,
        "the inline enum payload BoundVar must be substituted to a fresh Var; got {:?}",
        engine.pool().tag(payloads[0]),
    );
}
