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
