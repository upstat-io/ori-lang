use super::*;
use crate::{check::ModuleChecker, check_module, TypeCheckResult, TypeEnv, TypeErrorKind};
use ori_ir::{ExprArena, Module, StringInterner};
use ori_lexer::lex;
use ori_parse::parse;

/// Parse-and-check harness used by body-pass regression tests.
///
/// Duplicates the helper in `check/api/tests.rs`. After §03.1–§03.4 wire the
/// validator into all four body passes, `/improve-tooling` close-out should
/// extract a shared `#[cfg(test)] pub(crate)` helper module rather than
/// maintain two copies.
fn parse_and_check(source: &str) -> (TypeCheckResult, StringInterner) {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let result = check_module(&parsed.module, &parsed.arena, &interner);
    (result, interner)
}

#[test]
fn check_empty_module_bodies() {
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
/// typed IR to downstream phases (`typeck.md §PC-2`).
///
/// A block-wrapped lambda `{ x -> x }` is not a direct lambda binding and is
/// NOT generalizable per `typeck.md §GN-3` (Value Restriction). The parameter
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
/// `typeck.md §GN-3` (Value Restriction); its parameter stays `Tag::Var`.
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
/// `typeck.md §GN-3` (Value Restriction); its parameter stays `Tag::Var`.
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

/// Negative control (pairs with the two positive tests above per
/// `tests.md §Negative Testing Protocol` positive + negative pairing rule):
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
/// to hand to codegen (`typeck.md §PC-2`).
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
