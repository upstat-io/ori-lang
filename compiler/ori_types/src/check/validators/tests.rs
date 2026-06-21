//! Tests for [`crate::check::validators::validate_body_types`].
//!
//! Twelve-cell matrix (T1–T12):
//!
//! - **Negative cells** (T1–T3): an unbound `Tag::Var` in `expr_types` or in
//!   `FunctionSig.param_types` MUST emit exactly one `E2005` against the
//!   position's span.
//! - **Positive cells** (T4–T7): well-formed types must NOT emit — covers
//!   resolved primitives, `VarState::Link` resolution, `Tag::BoundVar`
//!   scheme bodies, and the `VarState::Generalized` exemption.
//! - **Cascade** (T8): `Tag::Error` short-circuits the walk via `HAS_ERROR`.
//! - **Determinism** (T9): signature diagnostics precede body diagnostics,
//!   body diagnostics emit in ascending `ExprIndex` order regardless of
//!   `FxHashMap` insertion order.
//! - **Semantic pin** (T10): a `Tag::Scheme` wrapping an unbound
//!   `Tag::Var` body MUST fire `E2005`; this test regresses to zero
//!   diagnostics if the scheme `HAS_VAR`-propagation fix is reverted.
//! - **Nested compound** (T11): two-level descent via `Pool::visit_children`
//!   through dedicated compound tags (`Tag::Option` wrapping `Tag::List`).
//! - **Dedup** (T12): multiple unbound vars under one `ExprIndex` collapse
//!   to a single diagnostic by (Code, Span).

use rustc_hash::FxHashMap;

use ori_ir::{ExprKind, ExprRange, Name, Span};

use crate::check::validators::{validate_body_types, ValidatorContext};
use crate::output::FunctionSig;
use crate::tag::Tag;
use crate::{
    AmbiguousTypeSite, ExprIndex, Idx, Pool, TypeCheckError, TypeErrorKind, TypeFlags, VarState,
};

/// Span returned by the per-`ExprIndex` `span_of` closure in these tests.
/// Distinct from [`SIG_SPAN`] so cells that mix signature and body origins
/// (notably T3 and T9) can tell them apart.
const BODY_SPAN: Span = Span::new(0, 1);

/// Span passed as `sig_span` to [`validate_body_types`] in these tests.
/// Distinct from [`BODY_SPAN`].
const SIG_SPAN: Span = Span::new(100, 101);

/// Span returned by the per-`(ExprIndex, param_index)` `param_span_of`
/// closure in `test_e2005_message_for_lambda_param`. Distinct from
/// [`BODY_SPAN`] so the narrow-to-param-span contract can be pinned — if
/// the validator falls back to the whole-lambda span, `errors[0].span`
/// would be `BODY_SPAN` and the narrow-span assertion fails.
const PARAM_SPAN: Span = Span::new(15, 16);

/// Build an `FxHashMap<ExprIndex, Idx>` from `entries`, invoke
/// [`validate_body_types`] with the standard test spans, and return the
/// accumulated errors. Keeps each cell focused on its matrix axis rather
/// than map setup boilerplate.
///
/// `scheme_var_ids` is passed through to the validator for the exempt
/// root set. Existing tests pass `&[]` for monomorphic
/// scenarios; scheme-var-exemption tests (T13+) supply real scheme var ids.
fn run(
    pool: &Pool,
    entries: &[(ExprIndex, Idx)],
    sig: &FunctionSig,
    scheme_var_ids: &[u32],
) -> Vec<TypeCheckError> {
    let mut expr_types: FxHashMap<ExprIndex, Idx> = FxHashMap::default();
    for &(idx, ty) in entries {
        expr_types.insert(idx, ty);
    }
    let mut errors = Vec::new();
    let span_of = |_| BODY_SPAN;
    let expr_kind_of = |_| None;
    let param_span_of = |_, _| None;
    let ctx = ValidatorContext {
        span: &span_of,
        kind: &expr_kind_of,
        param_span: &param_span_of,
    };
    validate_body_types(
        pool,
        &expr_types,
        sig,
        SIG_SPAN,
        scheme_var_ids,
        &ctx,
        &mut errors,
    );
    errors
}

/// `run` variant that supplies a per-`ExprIndex` `ExprKind` lookup. Used by
/// the cells that assert site-specific E2005 wording — the validator
/// classifies the error site from the `ExprKind` at the top-level body
/// expression entry point.
fn run_with_expr_kinds(
    pool: &Pool,
    entries: &[(ExprIndex, Idx, Option<ExprKind>)],
    sig: &FunctionSig,
    scheme_var_ids: &[u32],
) -> Vec<TypeCheckError> {
    let no_param_spans = |_, _| None;
    run_with_expr_kinds_and_param_spans(pool, entries, sig, scheme_var_ids, &no_param_spans)
}

/// `run_with_expr_kinds` variant that additionally supplies a
/// `param_span_of` closure. Used by the cells that pin the
/// narrow-to-parameter-span contract for `LambdaParam` sites — the
/// validator must pick up the Lambda's nth parameter's span (not the
/// whole-Lambda span) when an unresolved `Tag::Var` surfaces at parameter
/// slot `n` of the Lambda's inferred function type.
fn run_with_expr_kinds_and_param_spans(
    pool: &Pool,
    entries: &[(ExprIndex, Idx, Option<ExprKind>)],
    sig: &FunctionSig,
    scheme_var_ids: &[u32],
    param_span_of: &dyn Fn(ExprIndex, usize) -> Option<Span>,
) -> Vec<TypeCheckError> {
    let mut expr_types: FxHashMap<ExprIndex, Idx> = FxHashMap::default();
    let mut kinds: FxHashMap<ExprIndex, ExprKind> = FxHashMap::default();
    for &(idx, ty, ref kind) in entries {
        expr_types.insert(idx, ty);
        if let Some(k) = kind {
            kinds.insert(idx, *k);
        }
    }
    let mut errors = Vec::new();
    let span_of = |_| BODY_SPAN;
    let expr_kind_of = |expr_index| kinds.get(&expr_index).copied();
    let ctx = ValidatorContext {
        span: &span_of,
        kind: &expr_kind_of,
        param_span: param_span_of,
    };
    validate_body_types(
        pool,
        &expr_types,
        sig,
        SIG_SPAN,
        scheme_var_ids,
        &ctx,
        &mut errors,
    );
    errors
}

/// A minimal `FunctionSig` with no params and an `int` return — used when
/// the cell exercises body-side behavior only. Built via the canonical
/// `FunctionSig::simple` constructor, so no fixture boilerplate leaks into
/// this file — keep `FunctionSig` construction off this module, use the
/// canonical API.
fn empty_sig() -> FunctionSig {
    FunctionSig::simple(Name::from_raw(1), vec![], Idx::INT)
}

// Negative cells — unbound Tag::Var must emit E2005

/// An unbound `Tag::Var` surviving the bodies pass in `expr_types` is a
/// phase-contract violation; the validator emits exactly one `E2005` at the
/// expression's span.
///
/// T1 (Negative / Base).
#[test]
fn body_expr_types_with_unbound_var_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();

    let errors = run(&pool, &[(0, var)], &empty_sig(), &[]);

    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        TypeErrorKind::AmbiguousType { .. }
    ));
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// `Tag::Applied` is the dedicated tag for user-registered generic types
/// (distinct from `Tag::Option`/`Tag::Result` and their dedicated
/// siblings). Using an `Applied(Name, [Var])` exercises the validator's
/// `Pool::visit_children` descent through a user-generic argument list.
///
/// T2 (Negative / Applied).
#[test]
fn applied_generic_with_unbound_var_argument_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // Name is synthetic — the validator walks the type structure, it does
    // not require the name to be registered in `TypeRegistry`.
    let my_generic = Name::from_raw(200);
    let applied = pool.applied(my_generic, &[var]);

    let errors = run(&pool, &[(0, applied)], &empty_sig(), &[]);

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// Signatures entering the Bodies group may carry fresh `Tag::Var`s for
/// elided annotations; those vars MUST resolve before Bodies exit, and the
/// validator emits at `sig_span` (the function declaration span) when one
/// survives.
///
/// T3 (Negative / Signature).
#[test]
fn signature_with_unbound_param_type_emits_at_sig_span() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // @f(x: <unbound>) -> int
    let sig = FunctionSig::simple(Name::from_raw(1), vec![var], Idx::INT);

    let errors = run(&pool, &[], &sig, &[]);

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].span, SIG_SPAN,
        "signature-origin diagnostic must emit at sig_span"
    );
}

// Positive cells — fast-path gates and well-formed scheme bodies

/// `!HAS_VAR` short-circuit fires before any walk on a fully-resolved
/// primitive. `int` never violates PC-2.
///
/// T4 (Positive / Resolved Int).
#[test]
fn body_expr_types_with_resolved_int_emits_no_diagnostic() {
    let pool = Pool::new();

    let errors = run(&pool, &[(0, Idx::INT)], &empty_sig(), &[]);

    assert!(errors.is_empty());
}

/// Flags are cached at intern time, but `VarState::Link` may mutate later.
/// The validator calls `Pool::resolve_fully` BEFORE the `HAS_VAR` gate at
/// every walk step, so a var linked to `int` after interning resolves
/// through and trips no diagnostic.
///
/// T5 (Positive / Linked).
#[test]
fn body_expr_types_with_linked_var_resolves_and_emits_nothing() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // For `Tag::Var`, `data` IS the `var_id`.
    let var_id = pool.data(var);
    *pool.var_state_mut(var_id) = VarState::Link { target: Idx::INT };

    let errors = run(&pool, &[(0, var)], &empty_sig(), &[]);

    assert!(errors.is_empty());
}

/// `Tag::BoundVar` sets `HAS_BOUND_VAR`, NOT `HAS_VAR`. A scheme body
/// referencing a `BoundVar` therefore trips no `HAS_VAR` flag on the outer
/// scheme; the top-level fast-path gate skips silently. This is the
/// legitimate shape of a generalized polymorphic binding after
/// generalization.
///
/// T6 (Positive / Scheme `BoundVar`).
#[test]
fn scheme_body_with_bound_var_emits_no_diagnostic() {
    let mut pool = Pool::new();
    // ∀[0]. BoundVar(0) — the identity-of-a-polymorphic-value shape.
    let bound_var = pool.intern(Tag::BoundVar, 0);
    let scheme = pool.scheme(&[0], bound_var);
    assert!(
        !pool.flags(scheme).contains(TypeFlags::HAS_VAR),
        "scheme body of BoundVar only must NOT set HAS_VAR"
    );
    assert!(pool.flags(scheme).contains(TypeFlags::HAS_BOUND_VAR));

    let errors = run(&pool, &[(0, scheme)], &empty_sig(), &[]);

    assert!(errors.is_empty());
}

/// Post-`expr_types` port, a `Tag::Var(VarState::Generalized)` surviving in
/// `expr_types` is a partial-migration LEAK: the scheme body was rewritten
/// to `Tag::BoundVar` by `rewrite_generalized_to_bound_var`, but the
/// expression position carrying the lambda's body sub-expression was not
/// re-pointed at the rewritten Idx. Stripping the validator's
/// `VarState::Generalized` arm exposes the leak as `E2005` — a regression
/// alarm, not a false positive.
///
/// Once the post-generalize `expr_types` / `FunctionSig` port ships AND the
/// validator exemption arm is stripped, the only remaining way for a
/// `VarState::Generalized` to reach `expr_types` is a genuine port-side
/// failure, and firing `E2005` is the correct behavior — the validator
/// becomes the leak alarm.
///
/// Cell L of the leak-alarm matrix.
#[test]
fn generalized_var_in_expr_types_emits_e2005_as_leak_alarm() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    *pool.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };

    let errors = run(&pool, &[(0, var)], &empty_sig(), &[]);

    assert_eq!(
        errors.len(),
        1,
        "post-normalization: Tag::Var(Generalized) in expr_types is a leak \
         (the expr_types position was not re-pointed at the rewritten \
         BoundVar Idx); validator strip MUST fire E2005 once per position"
    );
    assert!(
        matches!(errors[0].kind, TypeErrorKind::AmbiguousType { .. }),
        "leak-alarm diagnostic must be E2005 (AmbiguousType), got {:?}",
        errors[0].kind
    );
    assert_eq!(errors[0].span, BODY_SPAN);
}

// Cascade / Determinism / Semantic pin

/// `Tag::Error` poisons a type and MUST suppress cascading diagnostics. A
/// tuple carrying BOTH a `Tag::Var` and `Idx::ERROR` propagates both
/// `HAS_VAR` and `HAS_ERROR`; the validator's `HAS_ERROR` short-circuit
/// fires at the top-level gate and the co-located unbound var is silently
/// masked.
///
/// T8 (Cascade).
#[test]
fn tuple_with_error_and_unbound_var_suppresses_diagnostic() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // (Var, Error) — propagates both HAS_VAR and HAS_ERROR.
    let tuple = pool.tuple(&[var, Idx::ERROR]);
    assert!(pool.flags(tuple).contains(TypeFlags::HAS_VAR));
    assert!(pool.flags(tuple).contains(TypeFlags::HAS_ERROR));

    let errors = run(&pool, &[(0, tuple)], &empty_sig(), &[]);

    assert!(
        errors.is_empty(),
        "HAS_ERROR must suppress diagnostics for co-located unbound vars"
    );
}

/// Diagnostics must emit in a reproducible order regardless of `FxHashMap`
/// iteration. The validator's contract is: signature positions first (in
/// declaration order), then body expressions in ascending `ExprIndex`
/// order.
///
/// This test exercises both halves of the contract:
///   1. Signature diagnostic precedes body diagnostics.
///   2. Body diagnostics emit in ascending `ExprIndex` order even when
///      the map is populated with the higher index first — the validator's
///      `sort_unstable_by_key` step is the only reason this passes.
///
/// T9 (Determinism).
#[test]
fn mixed_sig_and_body_vars_emit_sig_first_then_ascending_expr_index() {
    let mut pool = Pool::new();
    let sig_var = pool.fresh_var();
    let body_var_high = pool.fresh_var();
    let body_var_low = pool.fresh_var();
    // @f(x: <sig_var>) -> int  with two body entries at ExprIndex 1 and 2.
    let sig = FunctionSig::simple(Name::from_raw(1), vec![sig_var], Idx::INT);

    // Insert ExprIndex 2 first so the map's iteration order does not
    // accidentally coincide with the required ascending order.
    let errors = run(&pool, &[(2, body_var_high), (1, body_var_low)], &sig, &[]);

    assert_eq!(errors.len(), 3, "one sig + two body");

    // Signature first.
    assert_eq!(errors[0].span, SIG_SPAN, "sig diagnostic must be errors[0]");

    // Body diagnostics share the same span (the test `span_of` closure
    // returns `BODY_SPAN` for every `ExprIndex`), so their order is
    // verified via the var_id payload carried in `AmbiguousType`.
    let low_id = pool.data(body_var_low);
    let high_id = pool.data(body_var_high);
    match &errors[1].kind {
        TypeErrorKind::AmbiguousType { var_id, .. } => {
            assert_eq!(
                *var_id, low_id,
                "ExprIndex 1 (body_var_low) must precede ExprIndex 2"
            );
        }
        other => panic!("expected AmbiguousType at errors[1], got {other:?}"),
    }
    match &errors[2].kind {
        TypeErrorKind::AmbiguousType { var_id, .. } => {
            assert_eq!(
                *var_id, high_id,
                "ExprIndex 2 (body_var_high) must follow ExprIndex 1"
            );
        }
        other => panic!("expected AmbiguousType at errors[2], got {other:?}"),
    }
}

/// Semantic pin for Pool scheme-flag propagation.
///
/// A `Tag::Scheme` wrapping an unbound `Tag::Var` body MUST have `HAS_VAR`
/// set on the outer scheme `Idx`. Without `PROPAGATE_MASK` extending
/// through `Tag::Scheme` in `Pool::compute_flags`, the outer flags would be
/// `HAS_VAR=false` and the validator's top-level `!HAS_VAR` gate would
/// return early — the scheme body would never be walked and the PC-2
/// violation would be invisible.
///
/// This test would FAIL (emit zero diagnostics instead of one) if the
/// propagation were reverted. It is the permanent regression guard.
///
/// T10 (Semantic pin for scheme-flag propagation).
#[test]
fn scheme_wrapping_unbound_var_body_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // ∀[0]. Var — same shape as
    // `pool::tests::scheme_wrapping_unbound_var_body_propagates_has_var`.
    let scheme = pool.scheme(&[0], var);
    assert!(
        pool.flags(scheme).contains(TypeFlags::HAS_VAR),
        "scheme-flag propagation must propagate HAS_VAR through Tag::Scheme; \
         without it this test would silently pass with zero diagnostics"
    );

    let errors = run(&pool, &[(0, scheme)], &empty_sig(), &[]);

    assert_eq!(
        errors.len(),
        1,
        "scheme body's unbound Tag::Var is a PC-2 violation"
    );
    assert_eq!(errors[0].span, BODY_SPAN);
}

// Nested compound / Dedup

/// Compound tags must transitively propagate `HAS_VAR` through nested
/// layers. Uses `Tag::Option` wrapping `Tag::List` wrapping a `Tag::Var` to
/// exercise two levels of dedicated (non-Applied) compound tags, confirming
/// the validator's recursive descent via `Pool::visit_children` reaches
/// leaves at arbitrary depth.
///
/// T11 (Nested compound).
#[test]
fn option_list_var_two_level_nesting_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let list_var = pool.list(var);
    let opt_list_var = pool.option(list_var);

    let errors = run(&pool, &[(0, opt_list_var)], &empty_sig(), &[]);

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// Multiple unbound type variables at a single `ExprIndex` MUST collapse to
/// one `E2005`. The validator sets an `emitted` flag inside its
/// `visit_children` closure to short-circuit further emissions at the
/// same position.
///
/// Uses `Map<Var, Var>` where both key and value types are independently
/// unbound; without dedup, this would emit two diagnostics. The map
/// `Tag::Map` has two children;
/// the first child's unbound var trips the emission, and the `emitted`
/// flag suppresses the second.
///
/// T12 (Dedup).
#[test]
fn map_with_two_unbound_vars_emits_one_e2005_not_two() {
    let mut pool = Pool::new();
    let k_var = pool.fresh_var();
    let v_var = pool.fresh_var();
    let map_ty = pool.map(k_var, v_var);

    let errors = run(&pool, &[(0, map_ty)], &empty_sig(), &[]);

    assert_eq!(
        errors.len(),
        1,
        "dedup must collapse multiple unbound vars under one ExprIndex"
    );
}

// Scheme-var exemption tests (T13–T16)

/// Semantic pin for a fresh instantiation var that became the
/// union-find root of a scheme var's equivalence class must NOT emit E2005.
///
/// Simulates `@apply_identity<T>(x: T) = identity(x: x)` where:
/// - `scheme_var_id=0` is the caller's `T`
/// - `fresh_var` (`var_id=1`) is the instantiation var from the `identity` call
/// - union-find made `fresh_var` the root (scheme var linked to fresh var)
///
/// Without the exempt-root-set fix, the validator sees `var_id=1` as
/// `VarState::Unbound` and not in `scheme_var_ids=[0]` → false E2005.
/// With the fix, the exempt set resolves `scheme_var_id=0`'s root to
/// `var_id=1` and exempts it.
#[test]
fn scheme_var_root_is_fresh_instantiation_var_no_false_e2005() {
    let mut pool = Pool::new();
    // scheme var (T) — `var_id=0`
    let scheme_var = pool.fresh_var();
    let scheme_var_id = pool.data(scheme_var);
    // fresh instantiation var — `var_id=1`
    let fresh_var = pool.fresh_var();
    let _fresh_var_id = pool.data(fresh_var);

    // Simulate union-find making `fresh_var` the root:
    // `scheme_var` links to `fresh_var` (`fresh_var` stays Unbound as root).
    *pool.var_state_mut(scheme_var_id) = VarState::Link { target: fresh_var };

    // Body contains the `fresh_var` (the root of the equivalence class).
    let errors = run(&pool, &[(0, fresh_var)], &empty_sig(), &[scheme_var_id]);

    assert!(
        errors.is_empty(),
        "fresh instantiation var that is the root of a scheme var's \
         equivalence class must be exempted"
    );
}

/// Multiple scheme vars with multiple fresh vars — all exempt.
///
/// Simulates `@f<A, B>(x: A, y: B) = g(x: x, y: y)` where `g` is also
/// generic and each call-site instantiation creates fresh vars that
/// become roots.
#[test]
fn multiple_scheme_vars_with_fresh_roots_all_exempt() {
    let mut pool = Pool::new();
    // scheme var A (var_id=0)
    let sv_a = pool.fresh_var();
    let sv_a_id = pool.data(sv_a);
    // scheme var B (var_id=1)
    let sv_b = pool.fresh_var();
    let sv_b_id = pool.data(sv_b);
    // fresh instantiation vars (roots)
    let fresh_a = pool.fresh_var();
    let fresh_b = pool.fresh_var();

    // Union-find: scheme vars link to fresh vars.
    *pool.var_state_mut(sv_a_id) = VarState::Link { target: fresh_a };
    *pool.var_state_mut(sv_b_id) = VarState::Link { target: fresh_b };

    // Body contains both fresh vars as roots.
    let errors = run(
        &pool,
        &[(0, fresh_a), (1, fresh_b)],
        &empty_sig(),
        &[sv_a_id, sv_b_id],
    );

    assert!(errors.is_empty(), "both fresh roots must be exempted");
}

/// Nested generic calls — A calls B calls C. The chain of fresh vars
/// through union-find all collapse via `resolve_fully` and the exempt
/// set covers the final root.
#[test]
fn nested_generic_calls_chain_through_union_find() {
    let mut pool = Pool::new();
    // scheme var T (`var_id=0`) — the caller's type param
    let sv = pool.fresh_var();
    let sv_id = pool.data(sv);
    // fresh var from B's instantiation (`var_id=1`)
    let fresh_b = pool.fresh_var();
    let fresh_b_id = pool.data(fresh_b);
    // fresh var from C's instantiation (`var_id=2`) — final root
    let fresh_c = pool.fresh_var();

    // Chain: sv → fresh_b → fresh_c (`fresh_c` is the ultimate root).
    *pool.var_state_mut(sv_id) = VarState::Link { target: fresh_b };
    *pool.var_state_mut(fresh_b_id) = VarState::Link { target: fresh_c };

    // Body contains the final root (`fresh_c`).
    let errors = run(&pool, &[(0, fresh_c)], &empty_sig(), &[sv_id]);

    assert!(
        errors.is_empty(),
        "transitive chain root must be in exempt set"
    );
}

/// Negative pin: a genuinely unbound var in the same generic body as
/// exempt scheme vars MUST still emit E2005. The exempt set must not
/// over-exempt.
///
/// Simulates a body where one expr's type resolved to a scheme var root
/// (exempt) and another expr's type is a genuinely unresolved inference
/// variable (not exempt → E2005).
#[test]
fn genuine_unbound_var_in_generic_body_still_emits_e2005() {
    let mut pool = Pool::new();
    // scheme var T (`var_id=0`)
    let sv = pool.fresh_var();
    let sv_id = pool.data(sv);
    // fresh instantiation var for T's root (`var_id=1`) — exempt
    let fresh_root = pool.fresh_var();
    // genuinely unbound var (`var_id=2`) — NOT in any scheme var's class
    let genuine_unbound = pool.fresh_var();

    // Union-find: scheme var links to `fresh_root`.
    *pool.var_state_mut(sv_id) = VarState::Link { target: fresh_root };

    // Body has both: exempt root at ExprIndex 0, genuine unbound at 1.
    let errors = run(
        &pool,
        &[(0, fresh_root), (1, genuine_unbound)],
        &empty_sig(),
        &[sv_id],
    );

    assert_eq!(
        errors.len(),
        1,
        "genuinely unbound var must still emit E2005 even when scheme \
         vars are exempt"
    );
    // Verify it's the genuine unbound var, not the exempt one.
    let genuine_id = pool.data(genuine_unbound);
    match &errors[0].kind {
        TypeErrorKind::AmbiguousType { var_id, .. } => {
            assert_eq!(
                *var_id, genuine_id,
                "E2005 must be for the genuinely unbound var, not the exempt root"
            );
        }
        other => panic!("expected AmbiguousType, got {other:?}"),
    }
}

// Poly-lambda return-type position semantic pins (T17-T19).
//
// These three tests pin the typeck-boundary invariant for polymorphic
// lambda return-type positions: a poly-lambda return may carry
// `Tag::BoundVar` (the SC-1 target shape) OR `Tag::Var(VarState::Generalized)`
// (the currently shipped-pool divergence documented in
// `collect_first_unbound_var`), and BOTH are legitimate. A surviving
// `Tag::Var(VarState::Unbound)` is a genuine PC-2 violation and MUST fire
// `E2005`, even when wrapped inside a `Tag::Function` / `Tag::Scheme` chain
// that matches the poly-lambda shape.
//
// The pool construction `Scheme([id], Function([Var], Var))` mirrors the
// typed IR shape produced by `check_function_bodies` for a polymorphic
// lambda like `(x -> x)` in a generic context — the two-level wrap
// exercises `HAS_VAR` propagation through `Tag::Function` as well as
// `Tag::Scheme`, distinct from T10's direct `Scheme([0], Var)` shape.

/// The SC-1 target shape for a polymorphic lambda return is `Tag::BoundVar`.
/// `BoundVar` sets `HAS_BOUND_VAR`, NOT `HAS_VAR` (TF-1: presence flags), so
/// the outer scheme short-circuits at the validator's top-level
/// `!HAS_VAR` gate and emits no diagnostic. This is the legitimate
/// spec-target form of a generalized polymorphic lambda.
///
/// Semantic pin for the poly-lambda-return code path once a fix
/// produces the target `BoundVar` shape rather than the currently
/// shipped `Var(Generalized)` divergence.
///
/// Not the enforcement point.
#[test]
fn polylambda_return_type_with_boundvar_emits_no_diagnostic() {
    let mut pool = Pool::new();
    // ∀[0]. (BoundVar(0)) -> BoundVar(0) — identity poly-lambda spec shape.
    let bound = pool.intern(Tag::BoundVar, 0);
    let lambda_ty = pool.function(&[bound], bound);
    let scheme = pool.scheme(&[0], lambda_ty);
    assert!(
        !pool.flags(scheme).contains(TypeFlags::HAS_VAR),
        "scheme over BoundVar-returning lambda must NOT set HAS_VAR per TF-1"
    );
    assert!(pool.flags(scheme).contains(TypeFlags::HAS_BOUND_VAR));

    let errors = run(&pool, &[(0, scheme)], &empty_sig(), &[]);

    assert!(errors.is_empty());
}

/// Post-`expr_types` port, a polymorphic-lambda return position carrying a
/// `Tag::Var(VarState::Generalized)` inside its `Tag::Scheme` wrapper is a
/// partial-migration LEAK — the target shape is `Tag::BoundVar` all the way
/// down (covered by `polylambda_return_type_with_boundvar_emits_no_diagnostic`
/// above), and any surviving `Tag::Var` inside a polymorphic scheme body
/// after the rewrite pass runs is a genuine SC-1 divergence. Stripping the
/// validator's `VarState::Generalized` exemption exposes the leak as
/// `E2005` via the `HAS_VAR` propagation through `Tag::Scheme` →
/// `Tag::Function` → `Tag::Var`.
///
/// Once the `expr_types` / `FunctionSig` port AND the validator exemption
/// strip land, the only way a `Generalized` var reaches a scheme-wrapped
/// expression position is a genuine port-side failure, and firing `E2005`
/// is the leak alarm's correct behavior.
///
/// Cell L of the leak-alarm matrix.
#[test]
fn polylambda_return_type_with_generalized_var_emits_e2005_as_leak_alarm() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    *pool.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };
    // ∀[var_id]. (Var(Generalized)) -> Var(Generalized) — post-normalization
    // this shape SHOULD NOT reach the validator: the rewrite pass lands
    // `Tag::BoundVar` at every position, and any surviving `Tag::Var`
    // inside a scheme body is a port-side failure.
    let lambda_ty = pool.function(&[var], var);
    let scheme = pool.scheme(&[var_id], lambda_ty);
    assert!(
        pool.flags(scheme).contains(TypeFlags::HAS_VAR),
        "scheme over Var(Generalized)-returning lambda MUST set HAS_VAR \
         because Var carries HAS_VAR per TF-1 — this is the flag the \
         leak-alarm relies on to walk into the scheme body and locate \
         the surviving Tag::Var"
    );

    let errors = run(&pool, &[(0, scheme)], &empty_sig(), &[]);

    assert_eq!(
        errors.len(),
        1,
        "post-normalization: Tag::Var(Generalized) inside a scheme body is a \
         port-side leak; validator strip MUST fire exactly one E2005 at \
         the expression position"
    );
    assert!(
        matches!(errors[0].kind, TypeErrorKind::AmbiguousType { .. }),
        "leak-alarm diagnostic must be E2005 (AmbiguousType), got {:?}",
        errors[0].kind
    );
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// A polymorphic-lambda return-type position carrying a surviving
/// `Tag::Var(VarState::Unbound)` is a genuine PC-2 violation and MUST fire
/// `E2005`. This is the negative pin:
/// if any future refactor either (a) extends the
/// `VarState::Generalized` exemption to cover `Unbound` at this position,
/// or (b) silently drops the PC-2 walk inside `Tag::Function` /
/// `Tag::Scheme`, this test fails — catching the over-exemption and
/// preventing the INVERTED-TDD regression
/// (the canonical example is exactly a PC-2 validator with a gate added
/// to silence failing generic-body tests).
///
/// Distinct from T10 (`scheme_wrapping_unbound_var_body_emits_one_e2005`)
/// which exercises `Scheme([0], Var)` direct wrap — this test threads the
/// var through an additional `Tag::Function` layer, exercising `HAS_VAR`
/// propagation through the composite Function-in-Scheme shape that models
/// the typed IR's poly-lambda representation.
///
/// Not the enforcement point.
#[test]
fn polylambda_return_type_with_unbound_var_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // ∀[0]. (Var(Unbound)) -> Var(Unbound) — the PC-2 violation shape.
    let lambda_ty = pool.function(&[var], var);
    let scheme = pool.scheme(&[0], lambda_ty);
    assert!(pool.flags(scheme).contains(TypeFlags::HAS_VAR));

    // scheme_var_ids=&[] disables the exempt root set — any Unbound var in
    // the scheme body is treated as a genuine inference failure, not a
    // scheme-parameter var.
    let errors = run(&pool, &[(0, scheme)], &empty_sig(), &[]);

    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        TypeErrorKind::AmbiguousType { .. }
    ));
    assert_eq!(errors[0].span, BODY_SPAN);
}

// Empty-container fault-tolerance + semantic/negative pins. Tests are
// synthetic unit tests that drive `validate_body_types` directly, mirroring
// the T1–T12 harness.

/// Fault-tolerance — two unbound `Tag::Var`s at DISTINCT `ExprIndex`
/// positions MUST emit two independent `E2005` diagnostics (no
/// cross-ExprIndex dedup). Distinct from T12
/// (`map_with_two_unbound_vars_emits_one_e2005_not_two`) which verifies
/// same-ExprIndex collapse — this test is the count-2 companion for the
/// fault-tolerance spec file `empty_list_multiple_unannotated.ori`; the
/// spec-side `#compile_fail(code: "E2005")` and this Rust-side
/// `diagnostics.len() == 2` together anchor the multi-error
/// fault-tolerance contract.
///
/// Models the repro `let xs = []; let ys = []; xs.len() + ys.len()`: two
/// distinct let-bindings with independent unbound element vars, each
/// carried by its own `ExprIndex`.
///
/// T13.
#[test]
fn validate_body_types_emits_one_e2005_per_unbound_var() {
    let mut pool = Pool::new();
    let xs_elem = pool.fresh_var();
    let ys_elem = pool.fresh_var();
    // Two bindings at distinct ExprIndex positions — no dedup across indices.
    let errors = run(&pool, &[(0, xs_elem), (1, ys_elem)], &empty_sig(), &[]);

    assert_eq!(
        errors.len(),
        2,
        "distinct ExprIndices must each emit one diagnostic; dedup is \
         per-ExprIndex (see T12), not cross-ExprIndex"
    );
    for err in &errors {
        assert!(
            matches!(err.kind, TypeErrorKind::AmbiguousType { .. }),
            "both diagnostics must be E2005 (AmbiguousType), got {:?}",
            err.kind
        );
        assert_eq!(err.span, BODY_SPAN);
    }
}

/// Semantic pin SP-1: the validator-skips-codegen-leak shape — an unbound
/// element `Tag::Var` inside a `Tag::List` reaches `expr_types` and is
/// rejected at the typeck boundary with `E2005`
/// (`TypeErrorKind::AmbiguousType`). Once the validator emits E2005, the
/// driver's codegen gate suppresses emission — the "not a codegen error"
/// half of the pin is the diagnostic kind itself: an `AmbiguousType` is a
/// typeck-origin diagnostic, not a codegen-origin one.
///
/// Reverting the Value Restriction or the bodies-pass wiring allows the
/// element var to survive to codegen and surface as an "unresolved type
/// variable at codegen" failure instead; this pin asserts the E2005 path
/// is the one that fires.
///
/// SP-1.
#[test]
fn empty_list_bare_use_emits_ambiguous_type_before_codegen() {
    let mut pool = Pool::new();
    let elem_var = pool.fresh_var();
    let list_var = pool.list(elem_var);

    let errors = run(&pool, &[(0, list_var)], &empty_sig(), &[]);

    assert_eq!(errors.len(), 1, "bare `[]` element var must emit one E2005");
    assert!(
        matches!(errors[0].kind, TypeErrorKind::AmbiguousType { .. }),
        "diagnostic kind is E2005 (AmbiguousType) — a typeck-origin error, \
         proving the validator intercepted before codegen; got {:?}",
        errors[0].kind
    );
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// Semantic pin SP-2: `Tag::Error` at one `ExprIndex` MUST NOT spuriously
/// induce an `E2005` at a sibling well-formed `ExprIndex`. Verifies the
/// validator is monotone (ER-2: monotone recovery) across sibling
/// expression positions — error presence in one `ExprIndex` neither
/// propagates to nor suppresses a CONCRETE resolved type at another
/// `ExprIndex`.
///
/// Distinct from T8 (`tuple_with_error_and_unbound_var_suppresses_diagnostic`)
/// which verifies intra-type cascade suppression; SP-2 verifies
/// cross-ExprIndex monotonicity: the Error-typed slot emits nothing (it is
/// poison per TK-3: Error tag), and the int-typed slot emits nothing (it is
/// fully resolved per T4's fast-path).
///
/// SP-2.
#[test]
fn error_typed_expr_with_sibling_wellformed_expr_emits_no_diagnostic() {
    let pool = Pool::new();

    let errors = run(&pool, &[(0, Idx::ERROR), (1, Idx::INT)], &empty_sig(), &[]);

    assert!(
        errors.is_empty(),
        "monotone recovery: Error-typed sibling does not cascade into a \
         false-positive E2005 on the well-formed int slot, and the \
         Error-typed slot itself emits nothing"
    );
}

/// Negative pin NP-1: an unannotated signature parameter carries a fresh
/// `Tag::Var` that the `ori_types::check::bodies` end-of-body defaulting
/// pre-pass does NOT cover — PC-2: defaulting is scoped to vars reachable
/// from empty-literal expression roots. An unannotated param
/// `@f (x) -> int = 0` is unreachable from any empty-literal root; the
/// validator catches it at `SIG_SPAN` with `E2005`.
///
/// Companion spec file:
/// `compiler_repo/tests/spec/types/empty_literals/negative_unrelated_signature_var_still_errors.ori`.
/// The Rust pin proves the diagnostic identity (E2005 at `sig_span`),
/// which the `.ori` file cannot assert directly beyond the error-code match.
///
/// Distinct from T3 (`signature_with_unbound_param_type_emits_at_sig_span`)
/// which establishes the baseline sig-position emission. NP-1 explicitly
/// documents the defaulting exclusion — defaulting is scope-by-var, not
/// scope-by-position; sig-position vars stay Unbound and fire E2005.
///
/// NP-1.
#[test]
fn unannotated_signature_param_emits_ambiguous_type_not_codegen_error() {
    let mut pool = Pool::new();
    let param_var = pool.fresh_var();
    // @f(x: <unbound>) -> int — sig-position Var, no empty-literal root
    // anywhere in the body, so the empty-literal defaulting cannot touch it.
    let sig = FunctionSig::simple(Name::from_raw(1), vec![param_var], Idx::INT);

    let errors = run(&pool, &[], &sig, &[]);

    assert_eq!(errors.len(), 1, "sig-position Var fires exactly once");
    assert!(
        matches!(errors[0].kind, TypeErrorKind::AmbiguousType { .. }),
        "diagnostic is E2005 (AmbiguousType), a typeck-origin error — \
         proving the defense-in-depth validator catches sig-position Vars \
         before codegen ever sees them; got {:?}",
        errors[0].kind
    );
    assert_eq!(
        errors[0].span, SIG_SPAN,
        "sig-origin diagnostic must emit at sig_span (defaulting does not \
         cover sig positions; they stay Unbound)"
    );
}

/// Negative pin NP-2: a `Tag::Scheme` body containing a captured outer
/// `Tag::Var` (`var_id` NOT in the scheme's bound-var list) MUST emit
/// `E2005`. Prevents an over-eager "skip Scheme" fix that would miss
/// captured-outer-var violations — the validator MUST walk scheme bodies
/// and check every Unbound Var against the scheme's bound list.
///
/// Distinct from T6 (`scheme_body_with_bound_var_emits_no_diagnostic` —
/// a `BoundVar` body, no unbound anywhere) and T10
/// (`scheme_wrapping_unbound_var_body_emits_one_e2005` — an Unbound var
/// with `var_id` IN the scheme's bound list, exercising the
/// `PROPAGATE_MASK` propagation fix). NP-2 uses a DIFFERENT `var_id` than
/// the scheme's bound
/// list: the scheme binds a synthetic id derived via
/// `outer_var_id.wrapping_add(1000)` — guaranteed distinct from
/// `outer_var_id` (belt-and-suspenders `assert_ne!` inside this test) — while
/// the body references the freshly-allocated outer Var. The Var is
/// therefore a genuine capture (not in the scheme's bound list) and fires
/// E2005.
///
/// NP-2.
#[test]
fn scheme_body_with_captured_outer_var_emits_one_e2005() {
    let mut pool = Pool::new();
    let outer_var = pool.fresh_var();
    let outer_var_id = pool.data(outer_var);
    // Scheme binds a synthetic id that is guaranteed distinct from
    // `outer_var_id` — the outer Var is captured from an enclosing scope,
    // NOT bound by this scheme.
    let synthetic_bound_id: u32 = outer_var_id.wrapping_add(1000);
    assert_ne!(
        synthetic_bound_id, outer_var_id,
        "synthetic bound id must be distinct from the captured var's id"
    );
    let scheme = pool.scheme(&[synthetic_bound_id], outer_var);
    assert!(
        pool.flags(scheme).contains(TypeFlags::HAS_VAR),
        "scheme body with captured Unbound var must carry HAS_VAR via \
         PROPAGATE_MASK (TF-3)"
    );

    let errors = run(&pool, &[(0, scheme)], &empty_sig(), &[]);

    assert_eq!(
        errors.len(),
        1,
        "captured outer Var (var_id not in scheme bounds) is a PC-2 \
         violation and MUST emit E2005 — a 'skip schemes entirely' fix \
         would miss this"
    );
    assert!(matches!(
        errors[0].kind,
        TypeErrorKind::AmbiguousType { .. }
    ));
    assert_eq!(errors[0].span, BODY_SPAN);
}

// E2005 diagnostic dispatch by ExprKind.
//
// Three cells that pin the site-specific wording. Positive pins T17 + T18
// assert the specialized message text for empty-list and lambda-param
// sites. Negative pin T19 asserts that a signature-position var (no
// ExprKind in scope) still produces the generic
// `"cannot infer type in expression"` wording.

/// Positive pin: empty-list literal site produces the specialized
/// `"cannot infer the type of this empty list"` wording with a
/// `[int]`-annotation hint, and the span points at the list expression.
///
/// Drives the specialized wording and span discipline. Fails if (a)
/// `AmbiguousTypeSite::EmptyList` is not selected at a bare `[]` site, (b)
/// the message text drifts, or (c) the diagnostic's span does not match
/// the body-expression span supplied by `span_of`.
#[test]
fn test_e2005_message_for_empty_list() {
    let mut pool = Pool::new();
    let elem_var = pool.fresh_var();
    let list_var = pool.list(elem_var);
    // ExprKind::List(empty range) — the bare `[]` shape.
    let empty_list_kind = ExprKind::List(ExprRange::EMPTY);

    let errors = run_with_expr_kinds(
        &pool,
        &[(0, list_var, Some(empty_list_kind))],
        &empty_sig(),
        &[],
    );

    assert_eq!(errors.len(), 1, "one E2005 per unbound element var");
    assert_eq!(
        errors[0].span, BODY_SPAN,
        "span must match the List ExprId's span"
    );
    match &errors[0].kind {
        TypeErrorKind::AmbiguousType { site, .. } => {
            assert_eq!(
                *site,
                AmbiguousTypeSite::EmptyList,
                "validator must select EmptyList site for ExprKind::List([])"
            );
        }
        other => panic!("expected AmbiguousType, got {other:?}"),
    }
    assert_eq!(
        errors[0].message(),
        "cannot infer the type of this empty list; \
         add a type annotation like `let x: [int] = []`",
        "specialized wording for empty-list site"
    );
}

/// Positive pin: lambda-parameter site produces the specialized
/// `"cannot infer the type of this closure parameter"` wording with a
/// typed-lambda-annotation hint, and the primary diagnostic span narrows
/// to the specific parameter token (NOT the whole-Lambda span).
///
/// Drives the specialized wording and span discipline. Fails if (a)
/// `AmbiguousTypeSite::LambdaParam` is not selected at a `Lambda` site, (b)
/// the message text drifts, or (c) the diagnostic's span falls back to the
/// whole-Lambda span instead of narrowing to [`PARAM_SPAN`] — a
/// whole-Lambda span is too wide for an actionable "add a type annotation
/// to this parameter" message.
///
/// The lambda's inferred type is `(Var) -> int` — a `Tag::Function` with
/// a single unresolved-var parameter at slot 0. The validator must:
///   1. Classify the site as `LambdaParam` from `ExprKind::Lambda`.
///   2. Walk the function sig's param types and find `HAS_VAR` at slot 0.
///   3. Call `param_span_of(lambda_expr_idx, 0)` to get the narrow span.
///   4. Emit E2005 at [`PARAM_SPAN`] (NOT [`BODY_SPAN`]).
///
/// [`PARAM_SPAN`] is deliberately distinct from [`BODY_SPAN`] so a
/// validator falling back to `span_of(expr_idx)` (whole-Lambda span)
/// reports `BODY_SPAN` and this assertion fails — confirming the test
/// actively pins the narrow-span contract (per INVERTED-TDD avoidance).
#[test]
fn test_e2005_message_for_lambda_param() {
    let mut pool = Pool::new();
    let param_var = pool.fresh_var();
    // Lambda inferred type: `(Var) -> int` — function with a single
    // unresolved param at slot 0. This is the type stored in
    // `expr_types[lambda_expr_idx]` by InferEngine for a lambda whose
    // parameter type could not be resolved.
    let lambda_ty = pool.function(&[param_var], Idx::INT);
    // ExprKind::Lambda — site-classification trigger. Actual
    // params/body/ret_ty values don't need to be arena-resolvable here —
    // the validator only (a) dispatches on the discriminant for site
    // classification and (b) calls `param_span_of(expr_idx, i)` for the
    // narrow span, which the test wires below.
    let lambda_kind = ExprKind::Lambda {
        params: ori_ir::ParamRange::EMPTY,
        ret_ty: ori_ir::ParsedTypeId::INVALID,
        body: ori_ir::ExprId::INVALID,
    };

    // Wire `param_span_of` to return `PARAM_SPAN` for (lambda_expr_idx=0,
    // param_index=0). Any other (expr_idx, i) returns `None` — the
    // validator falls back to the whole-Lambda span only when the Lambda's
    // return slot is the unresolved one, which is not the case here.
    let errors = run_with_expr_kinds_and_param_spans(
        &pool,
        &[(0, lambda_ty, Some(lambda_kind))],
        &empty_sig(),
        &[],
        &|expr_index, param_index| {
            if expr_index == 0 && param_index == 0 {
                Some(PARAM_SPAN)
            } else {
                None
            }
        },
    );

    assert_eq!(errors.len(), 1, "one E2005 per unbound lambda-param var");
    assert_ne!(
        PARAM_SPAN, BODY_SPAN,
        "test setup: PARAM_SPAN must be distinct from BODY_SPAN so the \
         narrow-span assertion actively pins the narrowing contract — \
         a validator falling back to span_of(expr_idx) would report \
         BODY_SPAN and this test would fail"
    );
    assert_eq!(
        errors[0].span, PARAM_SPAN,
        "span must narrow to the parameter token \
         (PARAM_SPAN), NOT the whole-Lambda span (BODY_SPAN)"
    );
    match &errors[0].kind {
        TypeErrorKind::AmbiguousType { site, .. } => {
            assert_eq!(
                *site,
                AmbiguousTypeSite::LambdaParam,
                "validator must select LambdaParam site for ExprKind::Lambda"
            );
        }
        other => panic!("expected AmbiguousType, got {other:?}"),
    }
    assert_eq!(
        errors[0].message(),
        "cannot infer the type of this closure parameter; \
         add a full typed-lambda annotation like `(x: int) -> ReturnT = body`",
        "specialized wording for lambda-param site"
    );
}

/// Negative pin: an unbound `Tag::Var` in `FunctionSig.param_types[0]` (a
/// signature position, not a lambda body) STILL produces the generic
/// `"cannot infer type in expression"` wording.
///
/// Guards against over-eager specialized dispatch — regular unannotated
/// function parameters must NOT pick up the typed-lambda wording.
/// Signature positions have no `ExprKind` in
/// scope (the validator cannot look one up from a `FunctionSig` position),
/// so the site defaults to `AmbiguousTypeSite::Expression`.
#[test]
fn test_e2005_message_falls_back_to_generic_for_signature_var() {
    let mut pool = Pool::new();
    let param_var = pool.fresh_var();
    // @f(x: <unbound>) -> int — sig-position Var, no ExprKind in scope.
    let sig = FunctionSig::simple(Name::from_raw(1), vec![param_var], Idx::INT);

    let errors = run(&pool, &[], &sig, &[]);

    assert_eq!(errors.len(), 1, "sig-position var fires exactly once");
    assert_eq!(
        errors[0].span, SIG_SPAN,
        "sig-origin diagnostic must emit at sig_span"
    );
    match &errors[0].kind {
        TypeErrorKind::AmbiguousType { site, .. } => {
            assert_eq!(
                *site,
                AmbiguousTypeSite::Expression,
                "sig positions (no ExprKind in scope) must default to the \
                 generic Expression site — not LambdaParam"
            );
        }
        other => panic!("expected AmbiguousType, got {other:?}"),
    }
    assert_eq!(
        errors[0].message(),
        "cannot infer type in expression",
        "sig-position vars stay on generic wording"
    );
}
