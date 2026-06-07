use super::*;
use crate::{
    check::test_utils::parse_and_check, check::ModuleChecker, Tag, TypeEnv, TypeErrorKind, VarState,
};
use ori_ir::{ExprArena, Module, StringInterner};

use ori_lexer::lex;
use ori_parse::parse;

use crate::{check_module_with_pool, Pool, TypeCheckResult};

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
/// via `validate_body_types` wired in §03.1.
///
/// `x` in `@f (x) -> int = 0` is an unannotated parameter — its type is a
/// fresh `Tag::Var` in `FunctionSig.param_types`. The body `0` never uses
/// `x`, so the var is never constrained. The end-of-body defaulting pass only
/// targets empty-literal-reachable vars; unannotated params survive it and
/// must be caught by `validate_body_types` at the sig-position check.
///
/// Note: originally tested with `let ages = []` (unannotated empty list). That
/// case now defaults to `[Never]` via the end-of-body defaulting
/// pass and no longer fires E2005 — an unannotated param is the simplest
/// remaining case that survives defaulting.
#[test]
fn check_function_with_unannotated_param_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check("@f (x) -> int = 0;");
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
/// typed IR to downstream phases (PC-2).
///
/// A block-wrapped lambda `{ x -> x }` is not a direct lambda binding and is
/// NOT generalizable per (Value Restriction). The parameter
/// `x`'s type remains an unbound `Tag::Var`. No empty literals exist, so the
/// end-of-body defaulting pass does not fire — only `validate_body_types`
/// catches the surviving var in `expr_types`.
///
/// Test bodies have `() -> void` signatures (no unannotated params), so
/// the only path for a surviving var is through body expressions.
#[test]
fn check_test_with_ungeneralizable_lambda_body_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(
        "@target () -> void = ();\n@test_fn tests @target () -> void = { let _ = { x -> x }; () }",
    );
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
/// parameter `x` lives in `FunctionSig.param_types` as a fresh `Tag::Var`,
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
    let (result, _interner) =
        parse_and_check("type Foo = { f: int };\nimpl Foo { @process (self, x) -> void = { () } }");
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
/// sig-position test above.
///
/// A block-wrapped lambda `{ x -> x }` is ungeneralizable per
/// (Value Restriction); its parameter stays `Tag::Var`.
/// The enclosing impl method is `(self) -> void`, so no sig vars exist —
/// the only path to a surviving var is through body `expr_types`, which
/// `validate_body_types` must walk.
#[test]
fn check_impl_method_with_ungeneralizable_body_lambda_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(
        "type Foo = { f: int };\nimpl Foo { @m (self) -> void = { let _ = { x -> x }; () } }",
    );
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

/// Regression: a def-impl method with an unannotated parameter must produce
/// `E2005` (`AmbiguousType`) at typeck via `check_def_impl_method` (Pass 5 per
/// CK-1). Exercises the sig-position validator walk after `run_validator`
/// wires into `check_def_impl_method`.
///
/// The unannotated parameter `x` lives in the temporary `FunctionSig.param_types`
/// as a fresh `Tag::Var` (allocated at `impls.rs` `None => fresh_var()` for
/// parameters without an AST annotation), and the body `{ () }` never uses `x`
/// so the var is never constrained. `def_impl` methods construct the sig
/// validator-locally (no `register_impl_sig` call), so this test distinguishes
/// "validator wired" from "temporary sig correctly covers param/return positions."
#[test]
fn check_def_impl_method_with_unannotated_param_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(
        "trait Processable { @process (x) -> void; }\npub def impl Processable { @process (x) -> void = { () } }",
    );
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
/// sig-position test above.
///
/// A block-wrapped lambda `{ x -> x }` is ungeneralizable per
/// (Value Restriction); its parameter stays `Tag::Var`.
/// The enclosing def-impl method is `() -> void`, so no sig vars exist — the
/// only path to a surviving var is through body `expr_types`, which
/// `validate_body_types` must walk via `run_validator`.
#[test]
fn check_def_impl_method_with_ungeneralizable_body_lambda_emits_ambiguous_type() {
    let (result, _interner) = parse_and_check(
        "trait Processable { @process () -> void; }\npub def impl Processable { @process () -> void = { let _ = { x -> x }; () } }",
    );
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
    let (result, _interner) = parse_and_check(
        "trait Greetable { @greet () -> str; }\npub def impl Greetable { @greet () -> str = { \"hi\" } }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for well-typed def-impl body, got: {:?}",
        result.typed.errors
    );
}

/// Pins the §09.2 fix: `def impl Trait { @m (self) -> int = { 99 } }`
/// compiles clean because `check_def_impl_method` wraps body-checking with
/// `with_impl_scope(self_rigid, ...)` (`impls.rs:572`) so `Self` resolves to
/// a registered `Tag::RigidVar` allocated before param-type resolution
/// (`impls.rs:492`), NOT a fabricated fresh `Tag::Var`. Without §09.2, the
/// `(self)` parameter's `None if p.name == self_kw` arm would fall through
/// to a fresh var that is never constrained, surfacing as `E2005` at
/// validator time.
#[test]
fn test_def_impl_method_body_binds_self_to_registered_rigid_var() {
    let (result, _interner) = parse_and_check(
        "trait Identity { @m (self) -> int; }\npub def impl Identity { @m (self) -> int = { 99 } }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for def-impl with (self) param body, got: {:?}",
        result.typed.errors
    );
}

/// Pins method-call dispatch on `self` inside a def-impl body. The body
/// `self.inner()` requires `Self` to resolve to the def-impl's registered
/// `Tag::RigidVar` so trait-method lookup walks the bound-chain (§10.1) and
/// finds `inner` on the same `Identity` trait. Verifies §09.2 is more than a
/// type-binding fix — it makes `self`-receiver dispatch reachable.
#[test]
fn test_def_impl_method_body_dispatches_self_method_calls() {
    let (result, _interner) = parse_and_check(
        "trait Identity { @inner (self) -> int; @outer (self) -> int; }\npub def impl Identity { @inner (self) -> int = { 1 } @outer (self) -> int = { self.inner() } }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for def-impl self-method dispatch, got: {:?}",
        result.typed.errors
    );
}

/// Negative control: a def-impl method WITHOUT a `self` parameter must
/// remain well-typed under §09.2 — the `with_impl_scope` wrap is still
/// applied (the registered `Tag::RigidVar` is available), but the body
/// never references `Self`, so nothing triggers `impl_self_type()`. Pins
/// the false-positive boundary: §09.2 must not regress no-self def-impl
/// methods (e.g., trait-level associated-style functions).
#[test]
fn test_def_impl_without_self_param_does_not_bind_self() {
    let (result, _interner) = parse_and_check(
        "trait Constant { @m () -> int; }\npub def impl Constant { @m () -> int = { 42 } }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for def-impl without self param, got: {:?}",
        result.typed.errors
    );
}

/// Pins the §09.4 fix: an untyped lambda passed to `[int].map(...)` infers
/// its parameter from the receiver's element type via
/// `unify_closure_param_with_iterator_elem` (`closure_unify.rs:127-149`)
/// dispatched from `unify_higher_order_constraints` in `method_call.rs`.
/// Without §09.4, the unannotated `x` would stay as a fresh `Tag::Var`,
/// surfacing as `E2005` after `+ 1` constrains it only as some addable
/// type. With §09.4, the receiver `[int]` propagates `int` to `x` before
/// the body checks, so the body is well-typed and no error fires.
#[test]
fn test_lambda_param_inferred_from_list_map_receiver() {
    let (result, _interner) =
        parse_and_check("@process (xs: [int]) -> [int] = { xs.map(x -> x + 1) }");
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
    let (result, _interner) =
        parse_and_check("@process (xs: [int]) -> [int] = { xs.filter(x -> x > 0) }");
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
/// 2-param case alongside the 1-param `.map`/`.filter` cases above.
#[test]
fn test_lambda_params_inferred_from_list_fold_receiver() {
    let (result, _interner) = parse_and_check(
        "@process (xs: [int]) -> int = { xs.fold(initial: 0, (acc, x) -> acc + x) }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for [int].fold(0, (acc, x) -> acc + x), got: {:?}",
        result.typed.errors
    );
}

/// End-to-end regression pin for the repro program (`let ages = [];
/// ages = ages.push(value: 10); ages.len() == 1`).
///
/// Originally this program reached codegen with an unresolved `Tag::Var`
/// element type and surfaced as "unresolved type variable at codegen" in
/// `ori_llvm/type_info/store.rs`. The combined effect of Sections 01
/// (Value Restriction), 02 (`validate_body_types` + end-of-body defaulting),
/// 03.1–03.4 (wiring the validator into all four body passes), and the
/// defaulting pre-pass means the empty-list element `Tag::Var`
/// is resolved via unification with `push(value: 10)` (`T := int`) BEFORE
/// defaulting even runs — the program now type-checks cleanly and is safe
/// to hand to codegen (PC-2).
///
/// The test pins end-to-end typeck behavior: the full input flows
/// through every pass without diagnostics. Its companion AOT test
/// (`annotated_empty_list_with_push_and_len_compiles_and_exits_zero` in
/// `compiler/ori_llvm/tests/aot/empty_container.rs`) pins the runtime half —
/// exit 0 via both the interpreter and AOT paths.
///
/// Deliberately does NOT assert on E2005: after the defaulting
/// pass landed, inference resolves the element type via `push`'s `value: T`
/// constraint, so the validator's `PC-2` rejection path is not exercised
/// here. Canonical validator coverage for the four body passes lives in
/// the `unannotated_param` / `ungeneralizable_body_lambda` tests above
/// (§03.1–§03.4). This test is complementary: it confirms the original
/// bug's surface repro is fixed end-to-end.
#[test]
fn empty_list_with_push_and_len_typechecks_without_errors_end_to_end() {
    let (result, _interner) = parse_and_check(
        "@main () -> int = { let ages = []; ages = ages.push(value: 10); if ages.len() == 1 then 0 else 1 }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "empty list push repro must type-check cleanly after end-of-body defaulting pre-pass, got: {:?}",
        result.typed.errors
    );
}

// §08.3b.1 TDD cells (H, I, K). Pin the post-§08.3b migration contract:
// after generalization, every `Tag::Var(VarState::Generalized)` leaf in
// `InferOutput.expr_types` and every top-level polymorphic function's
// `FunctionSig.param_types` / `return_type` MUST have been rewritten to
// `Tag::BoundVar(var_id)` per. These cells currently
// FAIL (RED) — the end-of-body-group normalization pass that re-points
// those storage locations at the already-rewritten scheme-body Idxs
// does not yet exist. See
//
// §08.3b.1 TDD matrix (cells H / I / K).

/// Cell H — `expr_types` port (positive pin for §SC-1 target shape).
///
/// Program `@f () -> int = { let $id = x -> x; id(42) }` generalizes
/// `$id` to `∀a. a -> a`. `§08.3b` rewrites the scheme body to
/// `Tag::Function([BoundVar(0)], BoundVar(0))`, but the lambda's body
/// sub-expression `x` still has an `expr_types[lambda_body_expr] =
/// Tag::Var(Generalized)` entry because the rewrite never re-points
/// `expr_types`. The §08.3b.1 normalization pass MUST walk
/// `expr_types` and substitute the pre-generalize `Tag::Var` leaves
/// with `Tag::BoundVar` matching the scheme's declared var ids.
///
/// This test scans every `expr_types` entry for a surviving
/// `Tag::Var(VarState::Generalized)` and asserts zero are present
/// post-typeck. Today the scan finds at least one such entry — the
/// lambda body `x` — and the assertion fails (RED).
#[test]
fn expr_types_port_lambda_body_is_bound_var() {
    let (result, pool, _interner) =
        parse_and_check_with_pool("@f () -> int = { let $id = x -> x; id(42) }");

    assert!(
        result.typed.errors.is_empty(),
        "cell H input must typecheck cleanly (errors would mask the shape check): {:?}",
        result.typed.errors
    );

    // Scan every `expr_types` entry for a surviving `Tag::Var(Generalized)`.
    // Every position referencing a generalized scheme var MUST have been
    // re-pointed at a `Tag::BoundVar` Idx by the §08.3b.1 pass.
    let mut generalized_positions: Vec<(usize, Idx)> = Vec::new();
    for (expr_idx, ty) in result.typed.expr_types.iter().enumerate() {
        scan_for_generalized_var_leaves(&pool, *ty, &mut |leaf| {
            generalized_positions.push((expr_idx, leaf));
        });
    }

    assert!(
        generalized_positions.is_empty(),
        ": no `expr_types` entry may carry a Tag::Var(Generalized) \
         leaf post-typeck — the §08.3b.1 normalization pass must substitute every \
         scheme-var leaf with Tag::BoundVar matching the enclosing scheme's \
         declared var ids. Offending positions: {generalized_positions:?}"
    );
}

/// Cell I — `FunctionSig` port (positive pin for §SC-1 target shape).
///
/// Top-level polymorphic function `@id<T> (x: T) -> T = x` collects its
/// signature during pass 1 (`CK-4`) as `FunctionSig.param_types = [Var(a)]`,
/// `return_type = Var(a)` — fresh type variables for the parameter and
/// return. Generalization rewrites the scheme body but never updates the
/// exported `FunctionSig.param_types` / `return_type`; those Idxs still
/// point at the original `Tag::Var` leaves whose `var_state` was mutated
/// to `Generalized` in place. The §08.3b.1 normalization pass MUST
/// re-point both positions at `Tag::BoundVar` Idxs.
///
/// This test resolves `param_types[0]` and `return_type` via
/// `pool.resolve_fully` and asserts each has `Tag::BoundVar`. Today both
/// resolve to `Tag::Var(Generalized)` and the assertion fails (RED).
#[test]
fn function_sig_port_top_level_polymorphic_function() {
    let (result, pool, interner) = parse_and_check_with_pool("@id<T> (x: T) -> T = x;");

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

/// Cell K — validator strip (positive pin / regression guard).
///
/// Program `@f () -> int = { let $id = x -> x; id(42) }` typechecks with
/// ZERO `E2005` diagnostics both before AND after §08.3b.1 lands — but
/// the mechanism differs:
///
/// - **Today (pre-port)**: `validate_body_types` exempts
///   `Tag::Var(VarState::Generalized)` via the partial-migration
///   exemption arm in `validators/mod.rs::collect_first_unbound_var`.
///   The generalized vars reach the validator unchanged and are silently
///   skipped.
/// - **After §08.3b.1 port + strip**: the `expr_types` normalization pass
///   re-points every `Tag::Var(Generalized)` leaf at a `Tag::BoundVar`,
///   then the validator's `Generalized` exemption arm is stripped. The
///   validator walks the same positions and short-circuits at the
///   `Tag::BoundVar` arm (TF-1).
///
/// This cell pins the invariant that BOTH mechanisms keep this
/// well-formed polymorphic let-binding program diagnostic-free. If the
/// §08.3b.1 strip+port ever regresses the rewrite coverage (e.g., the
/// port pass misses an `expr_types` leaf), this test flips RED because
/// the surviving `Generalized` var reaches the stripped validator and
/// fires `E2005`. Today the test is GREEN because the current exemption
/// arm silences the leak.
#[test]
fn validator_strip_polylambda_typechecks_no_e2005() {
    let (result, _interner) = parse_and_check("@f () -> int = { let $id = x -> x; id(42) }");

    let ambiguous_count = result
        .typed
        .errors
        .iter()
        .filter(|e| matches!(e.kind, TypeErrorKind::AmbiguousType { .. }))
        .count();

    assert_eq!(
        ambiguous_count, 0,
        "cell K: `let $id = x -> x; id(42)` must typecheck with zero E2005 \
         diagnostics regardless of which §08.3b.1 mechanism (Generalized \
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
/// canonical walker used by `validate_body_types`
///
fn scan_for_generalized_var_leaves(pool: &Pool, ty: Idx, report: &mut dyn FnMut(Idx)) {
    let resolved = pool.resolve_fully(ty);
    match pool.tag(resolved) {
        Tag::Var => {
            let var_id = pool.data(resolved);
            if let VarState::Generalized { .. } = pool.var_state(var_id) {
                report(resolved);
            }
        }
        // BoundVar leaves are the §SC-1 target shape — do NOT report.
        Tag::BoundVar | Tag::RigidVar => {}
        _ => {
            pool.visit_children(resolved, |child| {
                scan_for_generalized_var_leaves(pool, child, report);
            });
        }
    }
}

// ============================================================================
// §09.5 method-call return BD-2 — 5-cell TDD matrix
// ============================================================================
//
// Pins the BD-2 gate that propagates an outer `Check(T)` annotation into a
// method-call's generic-return slot at typeck time. Without §09.5 the LHS
// annotation does NOT flow through `infer_method_call`'s synth-then-check
// shape, leaving the call's return as an unresolved `Tag::Var` that surfaces
// downstream as `E2005` on subsequent field/method access.
//
// Tests written BEFORE the fix per CLAUDE.md §TDD step 4 — they MUST fail on
// the current tree (cell 1/3/4 surface E2005 / E2036; cells 2/5 may pass
// already as regression baselines). The §05.5 implementation collapses all
// failures.

/// §09.5 cell 1 (positive): `let e: Error = msg.into()` compiles clean and
/// `e.message` resolves to `str`. The LHS annotation `Error` must propagate
/// through `into()`'s generic return slot via BD-2 so `e` is bound to
/// `Error` (not a fresh `Tag::Var`). Pins the `str_to_error.ori` failure
/// shape verbatim.
#[test]
fn test_method_call_return_bd2_into_to_error_resolves_field_access() {
    let (result, _interner) = parse_and_check(
        "@f () -> str = { let msg: str = \"x\"; let e: Error = msg.into(); e.message }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for let e: Error = msg.into() + e.message, got: {:?}",
        result.typed.errors
    );
}

/// §09.5 cell 2 (regression — collect default): the existing `[T]` default
/// for `.collect()` remains clean. The new `MethodCall` BD-2 gate must NOT
/// regress the no-Set-annotation collect path that has always worked via
/// the registered `Collect` impl's default return.
#[test]
fn test_method_call_return_bd2_collect_default_to_list_unchanged() {
    let (result, _interner) =
        parse_and_check("@f () -> [int] = { [1, 2, 3].iter().map(x -> x + 1).collect() }");
    assert!(
        result.typed.errors.is_empty(),
        "expected [int].iter().map.collect() to remain clean, got: {:?}",
        result.typed.errors
    );
}

/// §09.5 cell 3 (negative): `let n: int = msg.into()` where no `Into<int>`
/// impl exists on `str` must surface a diagnostic. Either `E2036`
/// (`IntoNotImplemented`) at method-resolution time OR `E2001` (Mismatch)
/// once LHS-driven instantiation finds no matching impl. The gate must
/// NOT silently produce `int` and mask the missing-impl error.
#[test]
#[ignore = "BUG-02-028: lookup_impl_method accepts generic Into<T> blanket affirmatively without verifying concrete T binding has matching impl on receiver; emit_into_not_implemented at method_call.rs:85 only fires when lookup returns None. Orthogonal to §09.5 BD-2 propagation."]
fn test_method_call_return_bd2_no_impl_reports_diagnostic() {
    let (result, _interner) =
        parse_and_check("@f () -> void = { let msg: str = \"x\"; let _n: int = msg.into(); () }");
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

/// §09.5 cell 4 (regression — no annotation): without LHS annotation the
/// method-call BD-2 gate MUST NOT fire — the synth path stays unchanged.
/// `let _e = msg.into()` (no target type, unused binding) currently
/// resolves `into`'s generic return as a fresh var via the existing synth
/// path; the §09.5 gate's `expected.has_expectation()` short-circuit
/// (parallels §09.3 `check_ok` early-return) MUST preserve this — no new
/// errors introduced. Pins the gate's no-spurious-fire invariant.
#[test]
fn test_method_call_return_bd2_no_annotation_falls_through_to_synth() {
    let (result, _interner) =
        parse_and_check("@f (msg: str) -> str = { let _e = msg.into(); \"\" }");
    assert!(
        result.typed.errors.is_empty(),
        "expected no NEW errors for ungated msg.into() (no-annotation path stays synth-only), got: {:?}",
        result.typed.errors
    );
}

/// §09.5 cell 6 (positive — user-defined Convert<T>): the gate must
/// propagate LHS annotation into a USER-DEFINED generic-return trait
/// method's slot. Independent of builtin/prelude types (Into/Error/collect)
/// — pins the gate's correctness on user-defined registered types alone.
/// If this cell passes post-fix while cells 1/5 (Error-typed) still fail,
/// the gate is correct and Error-typed cells are blocked by a separate
/// defect (Error not registered in `TypeRegistry`; falls through to
/// `fresh_named_var` at `type_resolution.rs:175`).
#[test]
#[ignore = "BUG-02-030: parse error on `impl <primitive>: Trait<...>` syntax (E1002 expected identifier). Blocks user-defined-trait-on-primitive verification of §09.5 BD-2 gate; gate itself works correctly on Error/Into case (cell 1)."]
fn test_method_call_return_bd2_user_convert_propagates_to_payload_field() {
    let source = "trait Convert<T> { @to_t (self) -> T }\n\
                  type MyErr = { msg: str }\n\
                  impl str: Convert<MyErr> { @to_t (self) -> MyErr = MyErr { msg: self } }\n\
                  @f () -> str = { let e: MyErr = \"x\".to_t(); e.msg }\n";
    let (result, _interner) = parse_and_check(source);
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for user-defined Convert<MyErr> with LHS annotation, got: {:?}",
        result.typed.errors
    );
}

/// §09.5 cell 5 (interaction — nested in `map_err`): the LHS annotation
/// `Result<int, Error>` propagates through `map_err`'s `(E) -> E2` closure
/// to the closure's return position; the closure body's `msg.into()` then
/// receives `Check(Error)` and instantiates `into<T = Error>` correctly.
/// Pins recursive-propagation: BD-2 composes across generic-return
/// method-calls AND closure-return propagation (§09.4 wired
/// closure-param-from-receiver; this pins the closure-return direction).
#[test]
#[ignore = "BUG-02-029: closure-return BD-2 propagation absent — outer Check(Result<T,E2>) does not flow through map_err's (E) -> E2 closure signature into closure body. §09.4 wired closure-param-from-receiver; symmetric closure-return-from-outer-expected gap. Sibling cure surface to §09.4 in unify_higher_order_constraints."]
fn test_method_call_return_bd2_nested_into_in_map_err_closure() {
    let (result, _interner) = parse_and_check(
        "@f (ok_int: Result<int, str>) -> Result<int, Error> = { ok_int.map_err(msg -> msg.into()) }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for map_err(msg -> msg.into()) into Result<int, Error>, got: {:?}",
        result.typed.errors
    );
}

// ===========================================================================
// §06.5 unknown-method diagnostic — silent-poison class closure (BUG-02-044 +
// §10.1 rigid-receiver negative). A genuine NotFound method lookup on a
// diagnosable receiver must emit a diagnostic, NOT silently poison via
// Idx::ERROR. (/improve-tooling-adjacent compiler fix 2026-06-07.)
// ===========================================================================

#[test]
fn test_builtin_assoc_fn_on_concrete_receiver_no_spurious_error() {
    // Negative clamp on the §06.5 emit's SCOPE: a `NotFound` on a CONCRETE
    // receiver must NOT emit. Typeck's concrete-receiver dispatch is incomplete
    // (builtin trait assoc-fns like `int.default()`, field-callables like
    // `s.transform(...)`, builtin collection methods like `list.updated(...)`),
    // and the evaluator resolves these via its own dispatch. Emitting on a
    // concrete miss false-positives every such legitimate call. `int.default()`
    // is the canonical case: it has no typeck-registry entry yet, so
    // `lookup_impl_method` returns NotFound, but it is a valid call (Default is
    // implemented for every primitive) — the emit must stay silent here. The
    // genuine concrete-unknown case (BUG-02-044, `{str:int}.map(...)`) reverts to
    // silent too; its cure is blocked on completing concrete-receiver dispatch so
    // a miss reliably implies genuine absence. This pin fails if the emit is
    // ever re-broadened to concrete receivers.
    let (result, _interner) = parse_and_check("@f () -> int = int.default();");
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
    // §10.1 negative case: `@f<T>(x: T)` has no bound providing `hello`, so
    // `x.hello()` must report a method-not-found (with an add-a-bound hint),
    // not silently accept.
    let (result, _interner) =
        parse_and_check("trait Greet { @hello (self) -> str }\n@f<T> (x: T) -> str = x.hello();");
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
    let (result, _interner) = parse_and_check(
        "trait Http { @get (url: str) -> str }\n\
         @fetch (url: str) -> str uses Http = Http.get(url: url);",
    );
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
    // §10.2: a no-self capability/associated method call (`Http.get(url:)`,
    // `@get` has no `self`) on a capability namespace must type-check with ZERO
    // errors. The cap marker var stays a unifiable `Tag::Var` (so caller-side
    // `with...in` provision unifies it with the concrete provider) and its
    // var_id is exempt from the `validate_body_types` E2005 check, so the
    // otherwise-unconstrained no-self receiver var does not surface a spurious
    // "cannot infer type". This pin fails if the cap exempt regresses (E2005
    // returns) or if cap_ty is forced to a non-unifiable RigidVar.
    let (result, _interner) = parse_and_check(
        "trait Http { @get (url: str) -> str }\n\
         @fetch (url: str) -> str uses Http = Http.get(url: url);",
    );
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
    // §10.1 bound-chain — no spurious method-not-found from the §06.5 emit.
    let (result, _interner) = parse_and_check(
        "trait Greet { @hello (self) -> str }\n@f<T: Greet> (x: T) -> str = x.hello();",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for bounded `x.hello()`, got: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_unbounded_impl_level_generic_receiver_reports_error() {
    // §10.1.2 negative: an IMPL-level generic param (`impl<T>`, no bound) used as
    // a method receiver inside a method body must surface a method-not-found —
    // same invariant as the function-level `@f<T>` case. Before impl-level
    // generics were allocated as RigidVars (in `check_impl_block`), `x: T`
    // resolved to an unresolved `Tag::Named` that the rigid-emit skipped, so the
    // unbounded call was silently accepted. This pin fails if impl-level generics
    // regress to `Tag::Named` resolution.
    let (result, _interner) = parse_and_check(
        "trait Greet { @hello (self) -> str }\n\
         type Box<T> = { inner: T }\n\
         impl<T> Box<T> { @greet (self, x: T) -> str = x.hello(); }",
    );
    assert!(
        !result.typed.errors.is_empty(),
        "expected a method-not-found diagnostic for `x.hello()` on an unbounded \
         impl-level generic param, got none (silent accept)"
    );
}

#[test]
fn test_bounded_impl_level_generic_receiver_resolves_clean() {
    // §10.1.2 positive boundary: with the impl-level `T: Greet` bound registered
    // on the impl RigidVar, `x.hello()` resolves via the §10.1 bound-chain in
    // typeck itself (not merely masked by the evaluator's dispatch) — no spurious
    // method-not-found.
    let (result, _interner) = parse_and_check(
        "trait Greet { @hello (self) -> str }\n\
         type Box<T> = { inner: T }\n\
         impl<T: Greet> Box<T> { @greet (self, x: T) -> str = x.hello(); }",
    );
    assert!(
        result.typed.errors.is_empty(),
        "expected no errors for bounded impl-level `x.hello()`, got: {:?}",
        result.typed.errors
    );
}
