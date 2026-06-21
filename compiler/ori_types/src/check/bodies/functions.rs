//! Function and test body type checking.
//!
//! Owns `check_function_bodies` (Pass 2) and `check_test_bodies` (Pass 3) and
//! their private body-checking helpers. See `bodies/mod.rs` for the architecture
//! docstring that covers all four body passes.

use ori_ir::{ExprArena, ExprId, ExprKind, Function, Module, Name, TestDef};
use rustc_hash::FxHashSet;

use crate::check::validators::build_exempt_var_ids;
use crate::check::ModuleChecker;
use crate::{
    check_expr, infer_expr, ContextKind, Expected, ExpectedOrigin, Idx, InferEngine, TypeCheckError,
};

/// Check all function bodies.
///
/// This pass runs after signature collection (Pass 1). Each function body
/// is type-checked against its declared return type.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.functions.len()))]
pub fn check_function_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for func in &module.functions {
        check_function(checker, func);
    }
}

/// Type check a single function body.
#[expect(
    clippy::too_many_lines,
    reason = "Per-function body-check pipeline — line count tracks the number of sequential phases (sig clone, exempt-set build, child-env, param/const/capability binding, fn_type build, span capture, with_function_scope closure containing engine setup + bound registration + capability bounds + guard check + body check + defaulting + normalization, finalize_body_and_export, sig writeback). Each phase is necessary and ordered; extracting into helpers would multiply borrows of `checker`/`sig`/`engine` across function boundaries without structural win."
)]
fn check_function(checker: &mut ModuleChecker<'_>, func: &Function) {
    // Look up the pre-collected signature. Cloned into a mutable local so the
    // end-of-body defaulting pass can refresh `param_types`,
    // `return_type`, and Merkle hashes before `validate_body_types` runs and
    // before the sig is written back to `checker.signatures`.
    let Some(mut sig) = checker.get_signature(func.name).cloned() else {
        // This should never happen if Pass 1 ran correctly
        checker.error_undefined(func.name, func.span);
        return;
    };

    // Build the exempt var-id set once, before entering the inference
    // closure. The engine method receives `&FxHashSet<u32>` — avoids an
    // `infer → check::validators` upward import. Capability marker var_ids
    // are merged in after the capability loop below.
    let mut exempt = build_exempt_var_ids(checker.pool(), &sig.scheme_var_ids);

    // Create child environment from frozen base
    let Some(child_env) = checker.child_of_base() else {
        // Base env not frozen - internal error
        return;
    };

    // Bind parameters in the child environment
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Bind const generic parameters as their declared type.
    // E.g., for `@f<$N: int>`, bind N -> int so the body can reference N.
    for cp in &sig.const_params {
        param_env.bind(cp.name, cp.const_type);
    }

    // Bind capability names as fresh type variables so the body can reference
    // them (e.g., `@f () -> int uses Value = Value`). The concrete type is
    // provided by the caller via `with...in`, so cap_ty MUST stay a unifiable
    // `Tag::Var` (a `RigidVar` would refuse to unify with the concrete provider
    // per UN-6, breaking capability provision).
    //
    // The capability trait is registered as a bound on cap_ty (below) so a
    // body-internal `Cap.method(...)` dispatches via the rigid-receiver
    // bound-chain. A no-self capability call (`Http.get(...)`) leaves the cap
    // receiver var otherwise unconstrained; rather than force it concrete (which
    // would break provision), its var_id is added to the `validate_body_types`
    // exempt set below so it does not surface a spurious E2005 — a capability
    // namespace is a parametric marker, not a genuine inference hole.
    let mut capability_var_bounds: Vec<(u32, Name)> = Vec::new();
    let mut capability_var_ids: Vec<u32> = Vec::new();
    for &cap_name in &sig.capabilities {
        let cap_ty = checker.pool_mut().fresh_var();
        param_env.bind(cap_name, cap_ty);
        let var_id = checker.pool().data(cap_ty);
        capability_var_bounds.push((var_id, cap_name));
        capability_var_ids.push(var_id);
    }
    // Exempt capability marker vars from the `validate_body_types` E2005 check:
    // a no-self capability call leaves its receiver var unconstrained by design.
    exempt.extend(capability_var_ids.iter().copied());

    // Build function type for recursion support
    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);

    // Extract capabilities for scope context
    let capabilities: FxHashSet<Name> = sig.capabilities.iter().copied().collect();

    // Get spans before entering the checking scope
    let guard_span = func.guard.map(|id| checker.arena().get_expr(id).span);
    let body_span = checker.arena().get_expr(func.body).span;

    // Check body with function scope context
    let func_name = func.name;
    let (
        expr_types,
        errors,
        warnings,
        pat_resolutions,
        mono_instances,
        mono_dispatch_pre_dedup,
        deferred_mono_calls,
        composed_burdens,
        assign_desugars,
    ) = checker.with_function_scope(fn_type, capabilities, |c| {
        // Get arena reference (lifetime 'a, not tied to c borrow)
        let arena = c.arena();

        // Create inference engine with prepared environment
        let mut engine = c.create_engine_with_env(param_env);

        // Set self type for recursive calls (self() in patterns like recurse)
        engine.set_self_type(fn_type);

        // Track current function for deferred mono call recording
        engine.set_current_function(Some(func_name));

        // Register top-level function generic-param trait bounds on the
        // engine so body-internal method-call dispatch on
        // `Tag::Var`/`Tag::RigidVar` receivers can walk the bound chain
        // and resolve trait methods. Two sources to merge:
        //
        // 1. Inline bounds: `@f<T: Clone>(val: T)` — bounds in
        //    `sig.type_param_bounds` (parallel to `sig.scheme_var_ids`).
        // 2. Where-clauses: `@f<T> (val: T) where T: Eq, T: Hashable` —
        //    bounds in `sig.where_clauses` keyed by `param: Name` which
        //    matches an entry in `sig.type_params` (parallel to
        //    `sig.scheme_var_ids`). Projection-style where-clauses
        //    (`T.Item: Eq`) are skipped — they constrain associated
        //    types, not the type-param itself.
        for (i, &var_id) in sig.scheme_var_ids.iter().enumerate() {
            if let Some(bounds) = sig.type_param_bounds.get(i) {
                for &trait_name in bounds {
                    engine.bind_rigid_bound_by_var_id(var_id, trait_name);
                }
            }
        }
        for wc in &sig.where_clauses {
            if wc.projection.is_some() {
                continue;
            }
            if let Some(idx) = sig.type_params.iter().position(|&n| n == wc.param) {
                if let Some(&var_id) = sig.scheme_var_ids.get(idx) {
                    for &trait_name in &wc.bounds {
                        engine.bind_rigid_bound_by_var_id(var_id, trait_name);
                    }
                }
            }
        }

        // Register each capability trait as a bound on its cap_ty
        // fresh_var. Body-internal `Cap.method(...)` then routes through
        // the bound-chain dispatch.
        for &(var_id, cap_name) in &capability_var_bounds {
            engine.bind_rigid_bound_by_var_id(var_id, cap_name);
        }

        // Push context for better error messages
        engine.push_context(ContextKind::FunctionReturn {
            func_name: Some(func_name),
        });

        // Check guard expression if present
        if let (Some(guard_id), Some(span)) = (func.guard, guard_span) {
            let guard_ty = infer_expr(&mut engine, arena, guard_id);
            let expected_bool = Expected {
                ty: Idx::BOOL,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::MatchArmGuard { arm_index: 0 }, // Reuse guard context
                },
            };
            let _ = engine.check_type(guard_ty, &expected_bool, span);
        }

        // Spec: Clause 10 (Function-Level Contracts).
        // pre() contracts: scope check (E2047) precedes type check (E2044).
        validate_pre_contracts(&mut engine, arena, func);

        // Check body against declared return type (bidirectional)
        let expected = Expected {
            ty: sig.return_type,
            origin: ExpectedOrigin::Context {
                span: body_span,
                kind: ContextKind::FunctionReturn {
                    func_name: Some(func_name),
                },
            },
        };
        let _body_ty = check_expr(&mut engine, arena, func.body, &expected, body_span);

        // BUG-04-190: type-check each parameter's default expression against the
        // param's declared type so the default carries resolved type info. Canon
        // fills the default at omitting (positional / zero-arg) call sites; without
        // a recorded type a composite default (e.g. `[9, 9]`) lowers with Tag::Error
        // and the LLVM backend rejects the list-literal construction.
        for (i, param) in arena.get_params(func.params).iter().enumerate() {
            if let (Some(default_id), Some(&param_ty)) = (param.default, sig.param_types.get(i)) {
                let default_span = arena.get_expr(default_id).span;
                let default_expected = Expected {
                    ty: param_ty,
                    origin: ExpectedOrigin::Annotation {
                        name: param.name,
                        span: default_span,
                    },
                };
                let _ = check_expr(
                    &mut engine,
                    arena,
                    default_id,
                    &default_expected,
                    default_span,
                );
            }
        }

        // Spec: Clause 10 (Function-Level Contracts).
        // post() contracts: void-return rejection (E2046). Runs after body
        // inference so sig.return_type is fully resolved.
        validate_post_contracts(&mut engine, arena, func, sig.return_type);

        engine.pop_context();

        // Mark body inference complete before the defaulting pre-pass runs.
        // Guards a pass-order inversion via debug_assert in the defaulting
        // helpers; any reordering that moves defaulting before body inference
        // finishes panics in debug builds instead of silently defaulting vars
        // that bidirectional propagation should have pinned.
        engine.mark_body_inference_complete();

        // default unbound vars reachable from empty-literal
        // expr roots to `Idx::NEVER` before exporting `expr_types` and
        // before `validate_body_types` runs. Mutates `sig.param_types`,
        // `sig.return_type`, and refreshes `param_hashes` / `return_hash`
        // via `substitute_in_pool` + direct-assign to `VarState::Link`.
        let mut expr_types = engine.take_expr_types();
        engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, &mut sig, &exempt);

        // Normalize `Tag::Var(Generalized)` leaves in `expr_types` / sig
        // positions to `Tag::BoundVar`. Drains `pending_generalized_vars` from
        // inner let-polymorphism AND rewrites the sig's scheme var
        // ids (populated by signatures pass for top-level polymorphic
        // functions). MUST run after defaulting (keeps `Idx::NEVER`
        // substitutions intact) and before `validate_body_types`
        // (validator's `Generalized` exemption is stripped — the
        // rewrite is now the only path keeping scheme vars legitimate).
        engine.normalize_body_generalized_to_bound_var_sig(&mut expr_types, &mut sig);

        // Deep-resolve var-links in every body type into the pool BEFORE the
        // burden sweep in `take_composed_burdens`. A late-resolved
        // generic-builtin instantiation (e.g. inline `Ok([..])` typed
        // `Result<[int], Var(k)→int>`) is stored as a `HAS_VAR` Idx that the
        // sweep's `collect_candidate_indices` excludes; its deep-resolved
        // var-free form is otherwise minted only at ARC lowering, after the
        // sweep, so its `UserBurdenSpec` is never composed and the scope-exit
        // dec is dropped (Spec: Annex E §AIMS RL-2 — dec at last use).
        // `substitute_in_pool` follows `VarState::Link` and reinterns, minting
        // the resolved form the sweep then composes.
        engine.compose_body_type_burdens(&expr_types);

        (
            expr_types,
            engine.take_errors(),
            engine.take_warnings(),
            engine.take_pattern_resolutions(),
            engine.take_mono_instances(),
            engine.take_mono_dispatch_pre_dedup(),
            engine.take_deferred_mono_calls(),
            engine.take_composed_burdens(),
            engine.take_assign_desugars(),
        )
    });

    // Shared PC-2 validation + store/push/accumulate spine.
    super::finalize_body_and_export(
        checker,
        &sig,
        func.span,
        func.body,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            mono_dispatch_pre_dedup,
            deferred_mono_calls,
            composed_burdens,
            capability_exempt_var_ids: capability_var_ids,
            assign_desugars,
        },
    );

    // Write the defaulted signature back to the checker's signature map so
    // cross-function lookups and the cross-module identity channel
    // (param_hashes / return_hash) see the post-defaulted types instead of
    // the pre-defaulted ones. Prevents incremental-cache divergence on
    // re-checks.
    checker.signatures.insert(func.name, sig);
}

/// Check all test bodies.
///
/// Tests are similar to functions but:
/// - Always return unit (void)
/// - May have special test parameters
#[tracing::instrument(level = "debug", skip_all, fields(count = module.tests.len()))]
pub fn check_test_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for test in &module.tests {
        check_test(checker, test);
    }
}

/// Type check a single test body.
fn check_test(checker: &mut ModuleChecker<'_>, test: &TestDef) {
    // Look up pre-collected signature. Cloned into a mutable local so the
    // defaulting pass can refresh `param_types` / `return_type` /
    // Merkle hashes before export.
    let Some(mut sig) = checker.get_signature(test.name).cloned() else {
        checker.error_undefined(test.name, test.span);
        return;
    };

    // Create child environment
    let Some(child_env) = checker.child_of_base() else {
        return;
    };

    // Bind parameters
    let mut param_env = child_env;
    for (name, ty) in sig.param_names.iter().zip(&sig.param_types) {
        param_env.bind(*name, *ty);
    }

    // Build the exempt var-id set before the engine takes a mut borrow.
    // See check_function for the rationale — engine method receives
    // `&FxHashSet<u32>` to avoid an `infer → check::validators` upward import.
    let exempt = build_exempt_var_ids(checker.pool(), &sig.scheme_var_ids);

    // Get arena reference (lifetime 'a, not tied to checker borrow)
    let arena = checker.arena();

    // Create inference engine and check body
    let fn_type = checker
        .pool_mut()
        .function(&sig.param_types, sig.return_type);
    let mut engine = checker.create_engine_with_env(param_env);
    engine.set_self_type(fn_type);

    // Push test context
    engine.push_context(ContextKind::TestBody);

    // Check body against declared return type (bidirectional)
    let body_span = arena.get_expr(test.body).span;
    let expected = Expected {
        ty: sig.return_type,
        origin: ExpectedOrigin::Context {
            span: body_span,
            kind: ContextKind::FunctionReturn {
                func_name: Some(test.name),
            },
        },
    };
    let _body_ty = check_expr(&mut engine, arena, test.body, &expected, body_span);

    engine.pop_context();

    // Mark body inference complete before the defaulting pre-pass runs;
    // defaulting helpers debug-assert this flag (see `check_function`).
    engine.mark_body_inference_complete();

    // default unbound vars reachable from empty-literal expr roots before
    // exporting types, so tests with empty literals type-check without E2005.
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_from_empty_literals(arena, &mut expr_types, &mut sig, &exempt);

    // Normalize scheme vars to `Tag::BoundVar`.
    // See `check_function` for the full rationale.
    engine.normalize_body_generalized_to_bound_var_sig(&mut expr_types, &mut sig);

    // Deep-resolve var-links so late-resolved generic-builtin instantiations are
    // var-free in the exported IR + composed by the burden sweep (see the
    // main-body path above + `intern_link_resolved_body_types`).
    engine.compose_body_type_burdens(&expr_types);

    // Extract results
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    let pat_resolutions = engine.take_pattern_resolutions();
    let mono_instances = engine.take_mono_instances();
    let mono_dispatch_pre_dedup = engine.take_mono_dispatch_pre_dedup();
    let deferred_mono_calls = engine.take_deferred_mono_calls();
    let composed_burdens = engine.take_composed_burdens();
    let assign_desugars = engine.take_assign_desugars();

    // Shared PC-2 validation + store/push/accumulate spine.
    super::finalize_body_and_export(
        checker,
        &sig,
        test.span,
        test.body,
        super::BodyOutputs {
            expr_types,
            errors,
            warnings,
            pat_resolutions,
            mono_instances,
            mono_dispatch_pre_dedup,
            deferred_mono_calls,
            composed_burdens,
            capability_exempt_var_ids: Vec::new(),
            assign_desugars,
        },
    );

    // write the defaulted test signature back so the hash channel
    // (cross-module identity) reflects post-defaulted types, matching
    // check_function's behavior.
    checker.signatures.insert(test.name, sig);
}

/// Validate every `pre()` contract on `func`.
///
/// Spec: Clause 10 (Function-Level Contracts).
///
/// Two checks per contract, in order:
///
/// 1. **Scope (E2047)** — every `Ident` in the contract expression MUST
///    resolve in the engine's environment, which carries function parameters,
///    capability bindings, and (via parent chain) module-level signatures and
///    constants. Free names emit E2047.
/// 2. **Type (E2044)** — if scope passed, infer the condition's type and
///    require `bool`. Skipped when scope check failed to avoid cascading
///    diagnostics (ER-4: follow-on suppression).
fn validate_pre_contracts(engine: &mut InferEngine<'_>, arena: &ExprArena, func: &Function) {
    for contract in &func.pre_contracts {
        let scope_errors = collect_pre_contract_scope_errors(engine, arena, contract.condition);
        if !scope_errors.is_empty() {
            for err in scope_errors {
                engine.push_error(err);
            }
            continue;
        }

        let cond_ty = infer_expr(engine, arena, contract.condition);
        let resolved = engine.resolve(cond_ty);
        if resolved != Idx::BOOL && resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::pre_contract_not_bool(
                contract.span,
                resolved,
            ));
        }
    }
}

/// Validate every `post()` contract on `func`.
///
/// Spec: Clause 10 (Function-Level Contracts).
///
/// `post()` is rejected when the function returns `void` (E2046) — there is
/// no result value for the post-lambda to bind. The parser guarantees every
/// `PostContract` carries a lambda-shaped binder (one or more params before
/// `->`); see `parse_post_contract_params` in `ori_parse/src/grammar/item/
/// function/mod.rs`. Non-lambda forms like `post(true)` are rejected at parse
/// time with E1002, so no E2045 emission is reachable here.
fn validate_post_contracts(
    engine: &mut InferEngine<'_>,
    _arena: &ExprArena,
    func: &Function,
    return_ty: Idx,
) {
    let resolved_return = engine.resolve(return_ty);
    let returns_void = resolved_return == Idx::UNIT;
    for contract in &func.post_contracts {
        if returns_void {
            engine.push_error(TypeCheckError::post_contract_void_return(contract.span));
        }
    }
}

/// Walk the pre-condition expression, collecting an E2047 error for every
/// identifier that is not bound in the engine's current environment.
///
/// The walk treats `ExprKind::Ident` as the only externally-bound reference
/// shape. `Const(_)` references resolve through a separate const map and are
/// out of scope here. Compound nodes recurse via this helper; arena lookup is
/// the only side effect.
fn collect_pre_contract_scope_errors(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
) -> Vec<TypeCheckError> {
    let mut errors = Vec::new();
    walk_pre_contract_expr(engine, arena, expr_id, &mut errors);
    errors
}

fn walk_pre_contract_expr(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    errors: &mut Vec<TypeCheckError>,
) {
    let expr = arena.get_expr(expr_id);
    match expr.kind {
        ExprKind::Ident(name) => {
            if engine.env().lookup(name).is_none() {
                errors.push(TypeCheckError::pre_contract_unknown_ident(expr.span, name));
            }
        }
        ExprKind::Binary { left, right, .. } => {
            walk_pre_contract_expr(engine, arena, left, errors);
            walk_pre_contract_expr(engine, arena, right, errors);
        }
        ExprKind::Unary { operand, .. } => {
            walk_pre_contract_expr(engine, arena, operand, errors);
        }
        ExprKind::Field { receiver, .. } => {
            walk_pre_contract_expr(engine, arena, receiver, errors);
        }
        ExprKind::Index { receiver, index } => {
            walk_pre_contract_expr(engine, arena, receiver, errors);
            walk_pre_contract_expr(engine, arena, index, errors);
        }
        ExprKind::Call { func, args } => {
            walk_pre_contract_expr(engine, arena, func, errors);
            for &arg_id in arena.get_expr_list(args) {
                walk_pre_contract_expr(engine, arena, arg_id, errors);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk_pre_contract_expr(engine, arena, receiver, errors);
            for &arg_id in arena.get_expr_list(args) {
                walk_pre_contract_expr(engine, arena, arg_id, errors);
            }
        }
        _ => {
            // Other expression shapes (literals, struct/list/map literals,
            // control-flow forms, lambdas) either contain no free identifier
            // references or introduce their own binders. The scope check is
            // intentionally narrow — surface the unknown-ident case for the
            // common shapes (comparisons, calls, field access) and let
            // `infer_expr` handle complex forms via E2003 as a fallback.
        }
    }
}
