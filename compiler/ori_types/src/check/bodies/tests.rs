use super::*;

#[cfg(target_pointer_width = "64")]
#[test]
#[should_panic(expected = "keep inference expression keys sourced from ExprId")]
fn validator_rejects_expression_indices_outside_expr_id_range() {
    validator_expr_id(u32::MAX as usize + 1);
}
use crate::{
    check::test_utils::parse_and_check, check::ModuleChecker, Tag, TypeEnv, TypeErrorKind, VarState,
};
use ori_ir::{ExprArena, Module, StringInterner};

use ori_lexer::lex;
use ori_parse::parse;

use crate::{check_module_with_pool, Pool, TypeCheckResult};

fn fixture_without_trailing_newline(source: &'static str) -> &'static str {
    let Some(source) = source.strip_suffix('\n') else {
        panic!("committed Ori fixtures end with a newline");
    };
    source
}

/// Parse-and-check harness variant that returns the pool alongside the
/// `TypeCheckResult`, so cells that inspect post-typeck `Tag`/`VarState`
/// shapes can read the interned types directly.
fn parse_and_check_with_pool(source: &str) -> (TypeCheckResult, Pool, StringInterner) {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let (result, pool) = check_module_with_pool(&parsed.module, &parsed.arena, &interner);
    (result, pool, interner)
}

#[test]
fn catch_direct_loop_block_infers_result_value() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/catch_direct_loop_block_infers_result_value.ori"
    )));

    assert!(
        result.typed.errors.is_empty(),
        "catch must infer its successful value from the direct expression: {:?}",
        result.typed.errors
    );
}

#[test]
fn catch_lambda_expression_rejects_result_value_annotation() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/catch_lambda_expression_rejects_result_value_annotation.ori"
    )));

    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|error| matches!(error.kind, TypeErrorKind::Mismatch { .. })),
        "catch must preserve a lambda expression as the Result payload: {:?}",
        result.typed.errors
    );
}

#[test]
fn check_module_with_no_function_bodies_produces_no_errors() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Freeze base env (simulating Pass 1)
    checker.freeze_base_env(TypeEnv::new());

    let module = Module::default();

    // These should not panic with empty module
    check_function_bodies(&mut checker, &module);
    check_test_bodies(&mut checker, &module);
    check_impl_bodies(&mut checker, &module);
    check_def_impl_bodies(&mut checker, &module);

    assert!(!checker.has_errors());
}

/// Regression: a function with an unannotated parameter must produce `E2005`
/// (`AmbiguousType`) at typeck. Exercises `check_function` (Pass 2 per CK-1)
/// via `validate_body_types`.
///
/// `x` in `@f (x) -> int = 0` is an unannotated parameter — its type is a
/// fresh `Tag::Var` in `FunctionSig.param_types`. The body `0` never uses
/// `x`, so the var is never constrained. The end-of-body defaulting pass only
/// targets empty-literal-reachable vars; unannotated params survive it and
/// must be caught by `validate_body_types` at the sig-position check.
///
#[test]
fn check_function_with_unannotated_param_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_function_with_unannotated_param_emits_ambiguous_type.ori"
    )));
    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType, got: {:?}",
        result.typed.errors
    );
}

/// Regression: an unresolved `Tag::Var` in a test body must produce `E2005`
/// (`AmbiguousType`) at typeck via `check_test` (Pass 3 per CK-1). Without
/// `validate_body_types` wired into `check_test`, surviving vars leak through
/// typed IR to consuming phases (PC-2).
///
/// A block-wrapped lambda `{ x -> x }` is not a direct lambda binding and is
/// NOT generalizable per the Value Restriction. The parameter
/// `x`'s type remains an unbound `Tag::Var`. No empty literals exist, so the
/// end-of-body defaulting pass does not fire — only `validate_body_types`
/// catches the surviving var in `expr_types`.
///
/// Test bodies have `() -> void` signatures (no unannotated params), so
/// the only path for a surviving var is through body expressions.
#[test]
fn check_test_with_ungeneralizable_lambda_body_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_test_with_ungeneralizable_lambda_body_emits_ambiguous_type.ori"
    )));
    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType in test body, got: {:?}",
        result.typed.errors
    );
}

/// Regression: an impl method with an unannotated parameter must produce
/// `E2005` (`AmbiguousType`) at typeck via `check_impl_method` (Pass 4 per
/// CK-1). Exercises the sig-position validator walk: the unannotated
/// `FunctionSig.param_types` contains parameter `x` as a fresh `Tag::Var`,
/// and the body `()` never uses `x` so the var is never constrained.
///
/// The end-of-body defaulting pass (`default_unbound_vars_in_scope`)
/// targets ONLY vars reachable from empty-literal expression roots; an
/// unannotated param with no body references does not flow into that set
/// and must be caught by `validate_body_types` at sig-position coverage.
///
/// If the validator is wired but walks only body `expr_types` (skipping
/// `sig.param_types` + `sig.return_type`), this test still fails — the
/// test distinguishes "validator present" from "validator correctly walks
/// sig positions."
#[test]
fn check_impl_method_with_unannotated_param_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_impl_method_with_unannotated_param_emits_ambiguous_type.ori"
    )));
    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 for unannotated parameter in impl method sig, got: {:?}",
        result.typed.errors
    );
}

/// Regression: an impl method body carrying an unresolved `Tag::Var`
/// through `expr_types` must produce `E2005` via `check_impl_method`.
/// Exercises the body-position validator walk complement to the
/// signature-position case.
///
/// A block-wrapped lambda `{ x -> x }` is ungeneralizable under the Value
/// Restriction; its parameter stays `Tag::Var`.
/// The enclosing impl method is `(self) -> void`, so no sig vars exist —
/// the only path to a surviving var is through body `expr_types`, which
/// `validate_body_types` must walk.
#[test]
fn check_impl_method_with_ungeneralizable_body_lambda_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_impl_method_with_ungeneralizable_body_lambda_emits_ambiguous_type.ori"
    )));
    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType in impl method body, got: {:?}",
        result.typed.errors
    );
}

fn assert_exact_user_drop_role(source: &str, drop_impl_index: usize) {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    assert!(
        parsed.errors.is_empty(),
        "drop-role fixture must parse: {:?}",
        parsed.errors
    );
    let drop_body = parsed.module.impls[drop_impl_index].methods[0].body;
    let (result, pool) = check_module_with_pool(&parsed.module, &parsed.arena, &interner);
    assert!(
        result.typed.errors.is_empty(),
        "drop-role fixture must type-check: {:?}",
        result.typed.errors
    );

    let user_drop_sigs: Vec<_> = result
        .typed
        .impl_sigs
        .iter()
        .filter(|entry| matches!(entry.role, crate::ImplMethodRole::UserDrop { .. }))
        .collect();
    assert_eq!(
        user_drop_sigs.len(),
        1,
        "only the exact Drop trait method may carry UserDrop: {:?}",
        result.typed.impl_sigs
    );
    let user_drop = user_drop_sigs[0];
    assert_eq!(
        user_drop.id,
        crate::ImplMethodId::new(drop_impl_index, drop_body),
        "the role must attach to the exact Drop impl body, independent of ordinary dispatch"
    );
    assert!(
        result
            .typed
            .impl_sigs
            .iter()
            .filter(|entry| entry.id != user_drop.id)
            .all(|entry| entry.role == crate::ImplMethodRole::Ordinary),
        "same-spelled inherent and other-trait methods must remain ordinary"
    );

    let registry = crate::TypeRegistry::from_typed_exports(
        result.typed.types.clone(),
        result.typed.collection_burdens.clone(),
    );
    let burden_type = result
        .typed
        .types
        .iter()
        .find(|entry| pool.resolve_fully(entry.idx) == pool.resolve_fully(user_drop.receiver))
        .map_or(user_drop.receiver, |entry| entry.idx);
    let expected = registry
        .burden(burden_type)
        .and_then(|burden| burden.user_drop);
    let crate::ImplMethodRole::UserDrop { logical } = user_drop.role else {
        unreachable!("filtered to UserDrop")
    };
    assert_eq!(
        Some(logical),
        expected,
        "the exported method role must carry the registry's exact logical burden identity"
    );
}

#[test]
fn user_drop_role_ignores_preceding_same_named_methods() {
    assert_exact_user_drop_role(
        fixture_without_trailing_newline(include_str!(
            "fixtures/user_drop_role_ignores_preceding_same_named_methods.ori"
        )),
        2,
    );
}

#[test]
fn user_drop_role_ignores_following_same_named_methods() {
    assert_exact_user_drop_role(
        fixture_without_trailing_newline(include_str!(
            "fixtures/user_drop_role_ignores_following_same_named_methods.ori"
        )),
        0,
    );
}

#[test]
fn imported_drop_trait_assigns_exact_role_amid_inherent_name_collision() {
    let interner = StringInterner::new();
    let prelude_tokens = lex(fixture_without_trailing_newline(include_str!("fixtures/imported_drop_trait_assigns_exact_role_amid_inherent_name_collision_prelude.ori")), &interner);
    let prelude = parse(&prelude_tokens, &interner);
    assert!(prelude.errors.is_empty());
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/imported_drop_trait_assigns_exact_role_amid_inherent_name_collision_source.ori"
    ));
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    assert!(parsed.errors.is_empty());
    let (result, _) =
        crate::check_module_with_imports(&parsed.module, &parsed.arena, &interner, |checker| {
            checker.register_imported_traits(&prelude.module, &prelude.arena);
        });
    assert!(
        result.typed.errors.is_empty(),
        "imported Drop fixture must type-check: {:?}",
        result.typed.errors
    );
    assert_eq!(
        result
            .typed
            .impl_sigs
            .iter()
            .filter(|entry| matches!(entry.role, crate::ImplMethodRole::UserDrop { .. }))
            .count(),
        1,
        "the exact imported Drop trait must still mint one semantic role: {:?}",
        result.typed.impl_sigs
    );
}

/// Regression: a def-impl method with an unannotated parameter must produce
/// `E2005` (`AmbiguousType`) at typeck via `check_def_impl_method` (Pass 5 per
/// CK-1). Exercises the sig-position validator walk after `run_validator`
/// wires into `check_def_impl_method`.
///
/// The unannotated parameter `x` is a fresh `Tag::Var` in
/// `FunctionSig.param_types`, and the body never constrains it. Def-impl methods
/// construct this signature locally, so the validator must inspect its
/// parameter and return positions directly.
#[test]
fn check_def_impl_method_with_unannotated_param_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_def_impl_method_with_unannotated_param_emits_ambiguous_type.ori"
    )));
    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 for unannotated parameter in def-impl method sig, got: {:?}",
        result.typed.errors
    );
}

/// Regression: a def-impl method body carrying an unresolved `Tag::Var`
/// through `expr_types` must produce `E2005` via `check_def_impl_method`.
/// Exercises the body-position validator walk — the complement of the
/// signature-position case.
///
/// A block-wrapped lambda `{ x -> x }` is ungeneralizable under the Value
/// Restriction; its parameter stays `Tag::Var`.
/// The enclosing def-impl method is `() -> void`, so no sig vars exist — the
/// only path to a surviving var is through body `expr_types`, which
/// `validate_body_types` must walk via `run_validator`.
#[test]
fn check_def_impl_method_with_ungeneralizable_body_lambda_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_def_impl_method_with_ungeneralizable_body_lambda_emits_ambiguous_type.ori"
    )));
    assert!(
        result
            .typed
            .errors
            .iter()
            .any(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. })),
        "expected E2005 AmbiguousType in def-impl method body, got: {:?}",
        result.typed.errors
    );
}

/// Negative control:
/// a well-typed def-impl method body must NOT produce `E2005` (or any typeck
/// error) after `run_validator` wires in. Pins the false-positive boundary —
/// if the validator is too aggressive and fires on fully-resolved body types
/// or signature positions, this test catches the regression.
///
/// The trait and def-impl pair declare `@greet () -> str`; the body is a
/// concrete string literal, so `expr_types` contains no `Tag::Var` and the
/// sig positions are all resolved (`() -> str`). The validator must be
/// silent on this input.
#[test]
fn check_def_impl_method_with_well_typed_body_produces_no_errors() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/check_def_impl_method_with_well_typed_body_produces_no_errors.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for well-typed def-impl body, got: {:?}",
        result.typed.errors
    );
}

/// Pins the def-impl Self binding: `def impl Trait { @m (self) -> int = { 99 } }`
/// compiles clean because `check_def_impl_method` wraps body-checking with
/// `with_impl_scope(self_rigid, ...)` so `Self` resolves to a registered
/// `Tag::RigidVar` allocated before param-type resolution, NOT a fabricated
/// fresh `Tag::Var`. Without that wrap, the `(self)` parameter's
/// `None if p.name == self_kw` arm would fall through to a fresh var that
/// is never constrained, surfacing as `E2005` at validator time.
#[test]
fn test_def_impl_method_body_binds_self_to_registered_rigid_var() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/def_impl_method_body_binds_self_to_registered_rigid_var.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for def-impl with (self) param body, got: {:?}",
        result.typed.errors
    );
}

/// Pins method-call dispatch on `self` inside a def-impl body. The body
/// `self.inner()` requires `Self` to resolve to the def-impl's registered
/// `Tag::RigidVar` so trait-method lookup walks the bound-chain and finds
/// `inner` on the same `Identity` trait. Verifies the Self binding is more
/// than a type-binding fix — it makes `self`-receiver dispatch reachable.
#[test]
fn test_def_impl_method_body_dispatches_self_method_calls() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/def_impl_method_body_dispatches_self_method_calls.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for def-impl self-method dispatch, got: {:?}",
        result.typed.errors
    );
}

/// Negative control: a def-impl method WITHOUT a `self` parameter must
/// remain well-typed — the `with_impl_scope` wrap is still applied (the
/// registered `Tag::RigidVar` is available), but the body never references
/// `Self`, so nothing triggers `impl_self_type()`. Pins the false-positive
/// boundary: the Self binding must not regress no-self def-impl methods
/// (e.g., trait-level associated-style functions).
#[test]
fn test_def_impl_without_self_param_does_not_bind_self() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/def_impl_without_self_param_does_not_bind_self.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for def-impl without self param, got: {:?}",
        result.typed.errors
    );
}

/// Pins receiver-element propagation: an untyped lambda passed to
/// `[int].map(...)` infers its parameter from the receiver's element type
/// via `unify_closure_param_with_iterator_elem` dispatched from
/// `unify_higher_order_constraints`. Without it, the unannotated `x` would
/// stay as a fresh `Tag::Var`, surfacing as `E2005` after `+ 1` constrains
/// it only as some addable type. With it, the receiver `[int]` propagates
/// `int` to `x` before the body checks, so the body is well-typed and no
/// error fires.
#[test]
fn test_lambda_param_inferred_from_list_map_receiver() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/lambda_param_inferred_from_list_map_receiver.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for [int].map(x -> x + 1), got: {:?}",
        result.typed.errors
    );
}

/// Companion to `test_lambda_param_inferred_from_list_map_receiver` — pins
/// the same propagation through `.filter`, which dispatches via the same
/// `unify_higher_order_constraints` path but with a `(T) -> bool`
/// closure shape rather than `(T) -> U`. Different dispatch arm, same
/// receiver-element propagation invariant.
#[test]
fn test_lambda_param_inferred_from_list_filter_receiver() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/lambda_param_inferred_from_list_filter_receiver.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for [int].filter(x -> x > 0), got: {:?}",
        result.typed.errors
    );
}

/// Multi-arity dispatch cell: `[int].fold(initial, (acc, x) -> ...)` — the
/// fold dispatch arm in `unify_higher_order_constraints` (`closure_unify.rs`
/// §`unify_fold_constraints`) propagates BOTH the `initial` value's type
/// AND the receiver's element type to the lambda's two params. Pins the
/// two-parameter case alongside one-parameter `.map` and `.filter` cases.
#[test]
fn test_lambda_params_inferred_from_list_fold_receiver() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/lambda_params_inferred_from_list_fold_receiver.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for [int].fold(0, (acc, x) -> acc + x), got: {:?}",
        result.typed.errors
    );
}

/// End-to-end type-check test for `let ages = [];
/// ages = ages.push(value: 10); ages.len() == 1`).
///
/// The `push` value constrains the empty list's element variable to `int`
/// before end-of-body defaulting. The complete input must pass every body
/// check without an unresolved `Tag::Var` or E2005 diagnostic.
#[test]
fn empty_list_with_push_and_len_typechecks_without_errors_end_to_end() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/empty_list_with_push_and_len_typechecks_without_errors_end_to_end.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "empty list push repro must type-check cleanly after end-of-body defaulting pre-pass, got: {:?}",
        result.typed.errors
    );
}

// Generalization normalization contract: every
// `Tag::Var(VarState::Generalized)` leaf in
// `InferOutput.expr_types` and every top-level polymorphic function's
// `FunctionSig.param_types` and `return_type` is rewritten to
// `Tag::BoundVar(var_id)` before body export.

/// Cell H — `expr_types` port (positive pin for the SC-1 target shape).
///
/// Program `@f () -> int = { let $id = x -> x; id(42) }` generalizes
/// `$id` to `∀a. a -> a`. Generalization rewrites the scheme body to
/// `Tag::Function([BoundVar(0)], BoundVar(0))`, but the lambda's body
/// sub-expression `x` still has an `expr_types[lambda_body_expr] =
/// Tag::Var(Generalized)` entry because the rewrite never re-points
/// `expr_types`. The normalization pass MUST walk
/// `expr_types` and substitute the pre-generalize `Tag::Var` leaves
/// with `Tag::BoundVar` matching the scheme's declared var ids.
///
/// This test scans every `expr_types` entry and rejects any surviving
/// `Tag::Var(VarState::Generalized)` after type checking.
#[test]
fn expr_types_port_lambda_body_is_bound_var() {
    let (result, pool, _interner) = parse_and_check_with_pool(fixture_without_trailing_newline(
        include_str!("fixtures/expr_types_port_lambda_body_is_bound_var.ori"),
    ));

    assert!(
        result.typed.errors.is_empty(),
        "cell H input must typecheck cleanly (errors would mask the shape check): {:?}",
        result.typed.errors
    );

    // Scan every `expr_types` entry for a surviving `Tag::Var(Generalized)`.
    // Every position referencing a generalized scheme var MUST have been
    // re-pointed at a `Tag::BoundVar` Idx by the normalization pass.
    let mut generalized_positions: Vec<(usize, Idx)> = Vec::new();
    for (expr_idx, ty) in result.typed.expr_types.iter().enumerate() {
        scan_for_generalized_var_leaves(&pool, *ty, &mut |leaf| {
            generalized_positions.push((expr_idx, leaf));
        });
    }

    assert!(
        generalized_positions.is_empty(),
        ": no `expr_types` entry may carry a Tag::Var(Generalized) \
         leaf post-typeck — the normalization pass must substitute every \
         scheme-var leaf with Tag::BoundVar matching the enclosing scheme's \
         declared var ids. Offending positions: {generalized_positions:?}"
    );
}

/// Cell I — `FunctionSig` port (positive pin for the SC-1 target shape).
///
/// Top-level polymorphic function `@id<T> (x: T) -> T = x` collects its
/// signature during pass 1 (`CK-4`) as `FunctionSig.param_types = [Var(a)]`,
/// `return_type = Var(a)` — fresh type variables for the parameter and
/// return. Generalization rewrites the scheme body but never updates the
/// exported `FunctionSig.param_types` / `return_type`; those Idxs still
/// point at the original `Tag::Var` leaves whose `var_state` was mutated
/// to `Generalized` in place. The normalization pass MUST
/// re-point both positions at `Tag::BoundVar` Idxs.
///
/// This test resolves `param_types[0]` and `return_type` via
/// `pool.resolve_fully` and requires each position to contain `Tag::BoundVar`.
#[test]
fn function_sig_port_top_level_polymorphic_function() {
    let (result, pool, interner) = parse_and_check_with_pool(fixture_without_trailing_newline(
        include_str!("fixtures/function_sig_port_top_level_polymorphic_function.ori"),
    ));

    assert!(
        result.typed.errors.is_empty(),
        "cell I input must typecheck cleanly: {:?}",
        result.typed.errors
    );

    let id_name = interner.intern("id");
    let Some(sig) = result.typed.function(id_name) else {
        panic!("cell I: @id signature must be collected by pass 1 (CK-4)")
    };

    assert_eq!(
        sig.param_types.len(),
        1,
        "cell I: @id<T>(x: T) has exactly one parameter"
    );

    // param_types[0] must be Tag::BoundVar post-typeck.
    let param_ty = pool.resolve_fully(sig.param_types[0]);
    assert_eq!(
        pool.tag(param_ty),
        Tag::BoundVar,
        ": FunctionSig.param_types[0] for a top-level polymorphic \
         function must be Tag::BoundVar post-typeck, got {:?} (data={})",
        pool.tag(param_ty),
        pool.data(param_ty)
    );

    // return_type must also be Tag::BoundVar.
    let ret_ty = pool.resolve_fully(sig.return_type);
    assert_eq!(
        pool.tag(ret_ty),
        Tag::BoundVar,
        ": FunctionSig.return_type for a top-level polymorphic \
         function must be Tag::BoundVar post-typeck, got {:?} (data={})",
        pool.tag(ret_ty),
        pool.data(ret_ty)
    );

    // Both positions must reference the SAME scheme-declared var_id (the
    // function is `T -> T`, not `T -> U`). sig.scheme_var_ids lists the
    // generalized var ids for this function.
    assert_eq!(
        sig.scheme_var_ids.len(),
        1,
        "cell I: @id<T> has exactly one scheme-quantified var"
    );
    let scheme_var_id = sig.scheme_var_ids[0];
    assert_eq!(
        pool.data(param_ty),
        scheme_var_id,
        "cell I: BoundVar.data on param_types[0] must equal the scheme-declared var_id"
    );
    assert_eq!(
        pool.data(ret_ty),
        scheme_var_id,
        "cell I: BoundVar.data on return_type must equal the scheme-declared var_id"
    );
}

/// Cell K verifies generalized-lambda normalization before validation.
///
/// The well-formed polymorphic binding in this fixture must contain only
/// `Tag::BoundVar` leaves at validation and produce no E2005 diagnostic.
#[test]
fn validator_strip_polylambda_typechecks_no_e2005() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/expr_types_port_lambda_body_is_bound_var.ori"
    )));

    let ambiguous_count = result
        .typed
        .errors
        .iter()
        .filter(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. }))
        .count();

    assert_eq!(
        ambiguous_count, 0,
        "cell K: `let $id = x -> x; id(42)` must typecheck with zero E2005 \
         diagnostics regardless of which mechanism (Generalized \
         exemption OR BoundVar rewrite) is active. All errors: {:?}",
        result.typed.errors
    );
}

/// Recursively scan a resolved type tree for any `Tag::Var` leaf whose
/// `VarState` is `Generalized`. Reports each hit via `report`. Used by
/// cell H to assert zero generalized-var leaves in `expr_types`
/// post-typeck.
///
/// Delegates compound-tag traversal to `Pool::visit_children` — the same
/// canonical walker used by `validate_body_types`.
fn scan_for_generalized_var_leaves(pool: &Pool, ty: Idx, report: &mut dyn FnMut(Idx)) {
    let resolved = pool.resolve_fully(ty);
    match pool.tag(resolved) {
        Tag::Var => {
            let var_id = pool.data(resolved);
            if let VarState::Generalized(_) = pool.var_state(var_id) {
                report(resolved);
            }
        }
        // BoundVar leaves are the SC-1 target shape — do NOT report.
        Tag::BoundVar | Tag::RigidVar => {}
        _ => {
            pool.visit_children(resolved, |child| {
                scan_for_generalized_var_leaves(pool, child, report);
            });
        }
    }
}

// Method-call return annotation propagation.
//
// An outer `Check(T)` annotation constrains a method call's generic return
// slot during type checking. The annotation prevents unresolved `Tag::Var`
// leaves from reaching later field or method access.

/// Cell 1 (positive): `let e: Error = msg.into()` compiles clean and
/// `e.message` resolves to `str`. The LHS annotation `Error` must propagate
/// through `into()`'s generic return slot via BD-2 so `e` is bound to
/// `Error` (not a fresh `Tag::Var`). Pins the `str_to_error.ori` failure
/// shape verbatim.
#[test]
fn test_method_call_return_bd2_into_to_error_resolves_field_access() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/method_call_return_bd2_into_to_error_resolves_field_access.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for let e: Error = msg.into() + e.message, got: {:?}",
        result.typed.errors
    );
}

/// Cell 2 (regression — collect default): the existing `[T]` default
/// for `.collect()` remains clean. The new `MethodCall` BD-2 gate must NOT
/// regress the no-Set-annotation collect path that has always worked via
/// the registered `Collect` impl's default return.
#[test]
fn test_method_call_return_bd2_collect_default_to_list_unchanged() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/method_call_return_bd2_collect_default_to_list_unchanged.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected [int].iter().map.collect() to remain clean, got: {:?}",
        result.typed.errors
    );
}

/// Cell 3 (negative): `let n: int = msg.into()` where no `Into<int>`
/// impl exists on `str` must surface a diagnostic. Either `E2036`
/// (`IntoNotImplemented`) at method-resolution time OR `E2001` (Mismatch)
/// once LHS-driven instantiation finds no matching impl. The gate must
/// NOT silently produce `int` and mask the missing-impl error.
#[test]
fn test_method_call_return_bd2_no_impl_reports_diagnostic() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/method_call_return_bd2_no_impl_reports_diagnostic.ori"
    )));
    let has_diagnostic = result.typed.errors.iter().any(|e| {
        matches!(
            e.kind,
            TypeErrorKind::IntoNotImplemented { .. } | TypeErrorKind::Mismatch { .. }
        )
    });
    assert!(
        has_diagnostic,
        "expected E2036 or E2001 for let _n: int = msg.into() with no Into<int> impl, got: {:?}",
        result.typed.errors
    );
}

/// Without an LHS annotation, `msg.into()` synthesizes a fresh return variable.
/// The BD-2 gate requires an expectation and must not emit a diagnostic here.
#[test]
fn test_method_call_return_bd2_no_annotation_falls_through_to_synth() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/method_call_return_bd2_no_annotation_falls_through_to_synth.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no NEW errors for ungated msg.into() (no-annotation path stays synth-only), got: {:?}",
        result.typed.errors
    );
}

/// Cell 6 (positive — user-defined Convert<T>): the gate must
/// propagate LHS annotation into a USER-DEFINED generic-return trait
/// method's slot. Independent of builtin/prelude types (Into/Error/collect)
/// — this isolates propagation through user-defined registered types.
#[test]
fn test_method_call_return_bd2_user_convert_propagates_to_payload_field() {
    let source = include_str!(
        "fixtures/method_call_return_bd2_user_convert_propagates_to_payload_field.ori"
    );
    let (result, _interner) = parse_and_check(source);
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for user-defined Convert<MyErr> with LHS annotation, got: {:?}",
        result.typed.errors
    );
}

/// Cell 5 (interaction — nested in `map_err`): the LHS annotation
/// `Result<int, Error>` propagates through `map_err`'s `(E) -> E2` closure
/// to the closure's return position; the closure body's `msg.into()` then
/// receives `Check(Error)` and instantiates `into<T = Error>` correctly.
/// Pins recursive-propagation: BD-2 composes across generic-return
/// method calls and closure-return propagation.
#[test]
fn test_method_call_return_bd2_nested_into_in_map_err_closure() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/method_call_return_bd2_nested_into_in_map_err_closure.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for map_err(msg -> msg.into()) into Result<int, Error>, got: {:?}",
        result.typed.errors
    );
}

// Unknown-method diagnostic — silent-poison class closure (concrete-receiver
// case + rigid-receiver negative). A genuine NotFound method lookup on a
// diagnosable receiver must emit a diagnostic, NOT silently poison via
// Idx::ERROR.

#[test]
fn test_builtin_assoc_fn_on_concrete_receiver_no_spurious_error() {
    // Negative clamp on the emit's SCOPE: a `NotFound` on a CONCRETE
    // receiver must NOT emit. Typeck's concrete-receiver dispatch is incomplete
    // (builtin trait assoc-fns like `int.default()`, field-callables like
    // `s.transform(...)`, builtin collection methods like `list.updated(...)`),
    // and the evaluator resolves these via its own dispatch. Emitting on a
    // concrete miss false-positives every such legitimate call. `int.default()`
    // is the canonical case: it has no typeck-registry entry yet, so
    // `lookup_impl_method` returns NotFound, but it is a valid call (Default is
    // implemented for every primitive) — the emit must stay silent here. The
    // genuine concrete-unknown case (`{str:int}.map(...)`) reverts to
    // silent too; its cure depends on completing concrete-receiver dispatch so
    // a miss reliably implies genuine absence. This pin fails if the emit is
    // ever re-broadened to concrete receivers.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/builtin_assoc_fn_on_concrete_receiver_no_spurious_error.ori"
    )));
    assert!(
        !result
            .typed
            .errors
            .iter()
            .any(|e| format!("{e:?}").contains("no method")),
        "concrete-receiver builtin assoc-fn `int.default()` must NOT emit a \
         method-not-found diagnostic (typeck dispatch gap, not genuine absence); \
         got: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_unknown_method_on_unbounded_rigid_receiver_reports_error() {
    // Negative case: `@f<T>(x: T)` has no bound providing `hello`, so
    // `x.hello()` must report a method-not-found (with an add-a-bound hint),
    // not silently accept.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/unknown_method_on_unbounded_rigid_receiver_reports_error.ori"
    )));
    assert!(
        !result.typed.errors.is_empty(),
        "expected a diagnostic for `x.hello()` on an unbounded generic, got none (silent accept)"
    );
}

#[test]
fn test_capability_namespace_receiver_no_spurious_error() {
    // Discriminator clamp: a named-`Tag::Var` receiver whose name is a REGISTERED
    // TRAIT is a capability/trait-namespace call (`Http.get(url:)`), NOT a generic
    // type parameter. Its proper resolution is the capability/trait-associated
    // path (CP-3 target-only, incomplete in typeck), so a NotFound must DEFER, not
    // diagnose. Without the trait-name exclusion in `is_named_generic_var`, this
    // mis-emits a method-not-found / arity error on every capability call. This
    // pin fails if the discriminator regresses to name-presence alone.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/capability_namespace_receiver_no_spurious_error.ori"
    )));
    assert!(
        !result
            .typed
            .errors
            .iter()
            .any(|e| format!("{e:?}").contains("no method")),
        "capability-namespace call `Http.get(url:)` must NOT emit a \
         method-not-found diagnostic (trait namespace, not a generic param); \
         got: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_capability_no_self_method_dispatch_resolves_clean() {
    // A no-self capability/associated method call (`Http.get(url:)`,
    // `@get` has no `self`) on a capability namespace must type-check with ZERO
    // errors. The cap marker var stays a unifiable `Tag::Var` (so caller-side
    // `with...in` provision unifies it with the concrete provider) and its
    // var_id is exempt from the `validate_body_types` E2005 check, so the
    // otherwise-unconstrained no-self receiver var does not surface a spurious
    // "cannot infer type". This pin fails if the cap exempt regresses (E2005
    // returns) or if cap_ty is forced to a non-unifiable RigidVar.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/capability_namespace_receiver_no_spurious_error.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected zero errors for no-self capability dispatch `Http.get(url:)`, \
         got: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_method_on_bounded_rigid_receiver_resolves_clean() {
    // Positive boundary: with the `T: Greet` bound, `x.hello()` resolves via the
    // bound-chain — no spurious method-not-found from the emit.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/method_on_bounded_rigid_receiver_resolves_clean.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for bounded `x.hello()`, got: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_unbounded_impl_level_generic_receiver_reports_error() {
    // An unbounded impl-level generic receiver produces method-not-found, just
    // like an unbounded function-level generic receiver.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/unbounded_impl_level_generic_receiver_reports_error.ori"
    )));
    assert!(
        !result.typed.errors.is_empty(),
        "expected a method-not-found diagnostic for `x.hello()` on an unbounded \
         impl-level generic param, got none (silent accept)"
    );
}

#[test]
fn test_bounded_impl_level_generic_receiver_resolves_clean() {
    // Positive boundary: with the impl-level `T: Greet` bound registered
    // on the impl RigidVar, `x.hello()` resolves via the bound-chain in
    // typeck itself (not merely masked by the evaluator's dispatch) — no spurious
    // method-not-found.
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/bounded_impl_level_generic_receiver_resolves_clean.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for bounded impl-level `x.hello()`, got: {:?}",
        result.typed.errors
    );
}

/// Poison negative pin (matrix cell #7): a genuinely-poisoned
/// subexpression (`1 + unknown_var` with `unknown_var` unbound) must surface
/// EXACTLY ONE error (the unbound identifier) and suppress the cascade —
/// poison on `Idx::ERROR` keeps absorbing per UN-4. Separating the user-`Error`
/// type from the poison sentinel does not change this behavior.
#[test]
fn test_poison_unbound_ident_suppresses_cascade() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/poison_unbound_ident_suppresses_cascade.ori"
    )));
    assert_eq!(
        result.typed.errors.len(),
        1,
        "poison must suppress the arithmetic cascade — exactly one (unbound-ident) error expected, got: {:?}",
        result.typed.errors
    );
}

/// Poison negative pin (matrix cell #8): a poisoned element inside a
/// compound (list) literal must NOT add a secondary diagnostic on the compound
/// — `HAS_ERROR` propagates and UN-4 absorbs at the compound level.
#[test]
fn test_poison_in_compound_literal_no_secondary_diagnostic() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/poison_in_compound_literal_no_secondary_diagnostic.ori"
    )));
    assert_eq!(
        result.typed.errors.len(),
        1,
        "poisoned list element must not cascade onto the compound — exactly one error expected, got: {:?}",
        result.typed.errors
    );
}

/// Poison negative pin (matrix cell #9): a `match` whose scrutinee is
/// a poisoned (unbound-ident) subexpression must surface EXACTLY ONE error (the
/// unbound identifier) and NOT cascade onto the arms — poison on `Idx::ERROR`
/// absorbs the arm-type unification per UN-4. Separating the user-`Error` struct
/// from the poison sentinel MUST NOT make `match` on poison cascade.
#[test]
fn test_poison_match_scrutinee_no_arm_cascade() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/poison_match_scrutinee_no_arm_cascade.ori"
    )));
    assert_eq!(
        result.typed.errors.len(),
        1,
        "poisoned match scrutinee must not cascade onto the arms — exactly one error expected, got: {:?}",
        result.typed.errors
    );
}

/// Poison negative pin (matrix cell #13): field access on a poisoned
/// (unbound-ident) receiver must surface EXACTLY ONE error (the unbound
/// identifier) and NOT cascade through the `Tag::Error` field-access arm.
/// Separating the user-`Error` struct from the poison sentinel re-routes the
/// USER-`Error` `.message` path to normal struct-field resolution, but a POISON
/// receiver still hits `Idx::ERROR` — its field access must stay cascade-free.
#[test]
fn test_poison_field_access_no_cascade() {
    // `-> str` matches the `.message -> str` field-access result, isolating the
    // no-cascade property: only the unbound-ident error remains (a non-`str`
    // return would add a legitimate result-type mismatch, not a cascade).
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/poison_field_access_no_cascade.ori"
    )));
    assert_eq!(
        result.typed.errors.len(),
        1,
        "poisoned receiver field access must not cascade — exactly one error expected, got: {:?}",
        result.typed.errors
    );
}

#[test]
fn module_error_variant_shadows_builtin_error_constructor() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/module_error_variant_shadows_builtin_error_constructor.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "module `Error` variant must shadow the universe builtin: {:?}",
        result.typed.errors
    );
}

#[test]
fn builtin_error_constructor_remains_resolvable_without_shadow() {
    let (result, _interner) = parse_and_check(fixture_without_trailing_newline(include_str!(
        "fixtures/builtin_error_constructor_remains_resolvable_without_shadow.ori"
    )));
    assert!(
        result.typed.errors.is_empty(),
        "builtin Error constructor must remain available: {:?}",
        result.typed.errors
    );
}
