//! Tests for [`crate::check::validators::validate_body_types`].
//!
//! Twelve-cell matrix (T1–T12) per §02.4 of
//! `plans/empty-container-typeck-phase-contract/section-02-validator-module.md`:
//!
//! - **Negative cells** (T1–T3): an unbound `Tag::Var` in `expr_types` or in
//!   `FunctionSig.param_types` MUST emit exactly one `E2005` against the
//!   position's span.
//! - **Positive cells** (T4–T7): well-formed types must NOT emit — covers
//!   resolved primitives, `VarState::Link` resolution, `Tag::BoundVar`
//!   scheme bodies, and the `VarState::Generalized` exemption.
//! - **Cascade** (T8): `Tag::Error` short-circuits the walk via `HAS_ERROR`
//!   per `typeck.md §ER-4` / `types.md §TK-3`.
//! - **Determinism** (T9): signature diagnostics precede body diagnostics,
//!   body diagnostics emit in ascending `ExprIndex` order regardless of
//!   `FxHashMap` insertion order.
//! - **Semantic pin for §02.0** (T10): a `Tag::Scheme` wrapping an unbound
//!   `Tag::Var` body MUST fire `E2005`; this test regresses to zero
//!   diagnostics if the scheme `HAS_VAR`-propagation fix is reverted.
//! - **Nested compound** (T11): two-level descent via `Pool::visit_children`
//!   through dedicated compound tags (`Tag::Option` wrapping `Tag::List`).
//! - **Dedup** (T12): multiple unbound vars under one `ExprIndex` collapse
//!   to a single diagnostic per `impl-hygiene.md §Deduplication by (Code,
//!   Span)`.

use rustc_hash::FxHashMap;

use ori_ir::{Name, Span};

use crate::check::validators::validate_body_types;
use crate::output::FunctionSig;
use crate::tag::Tag;
use crate::{ExprIndex, Idx, Pool, TypeCheckError, TypeErrorKind, TypeFlags, VarState};

/// Span returned by the per-`ExprIndex` `span_of` closure in these tests.
/// Distinct from [`SIG_SPAN`] so cells that mix signature and body origins
/// (notably T3 and T9) can tell them apart.
const BODY_SPAN: Span = Span::new(0, 1);

/// Span passed as `sig_span` to [`validate_body_types`] in these tests.
/// Distinct from [`BODY_SPAN`].
const SIG_SPAN: Span = Span::new(100, 101);

/// Build an `FxHashMap<ExprIndex, Idx>` from `entries`, invoke
/// [`validate_body_types`] with the standard test spans, and return the
/// accumulated errors. Keeps each cell focused on its matrix axis rather
/// than map setup boilerplate.
fn run(pool: &Pool, entries: &[(ExprIndex, Idx)], sig: &FunctionSig) -> Vec<TypeCheckError> {
    let mut expr_types: FxHashMap<ExprIndex, Idx> = FxHashMap::default();
    for &(idx, ty) in entries {
        expr_types.insert(idx, ty);
    }
    let mut errors = Vec::new();
    validate_body_types(
        pool,
        &expr_types,
        sig,
        SIG_SPAN,
        &|_| BODY_SPAN,
        &mut errors,
    );
    errors
}

/// A minimal `FunctionSig` with no params and an `int` return — used when
/// the cell exercises body-side behavior only. Built via the canonical
/// `FunctionSig::simple` constructor, so no fixture boilerplate leaks into
/// this file (per §02.4.1 — keep `FunctionSig` construction off this
/// module, use the canonical API).
fn empty_sig() -> FunctionSig {
    FunctionSig::simple(Name::from_raw(1), vec![], Idx::INT)
}

// Negative cells — unbound Tag::Var must emit E2005

/// Spec: `typeck.md §PC-2` — an unbound `Tag::Var` surviving the bodies
/// pass in `expr_types` is a phase-contract violation; the validator emits
/// exactly one `E2005` at the expression's span.
///
/// Plan §02.4 T1 (Negative / Base).
#[test]
fn body_expr_types_with_unbound_var_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();

    let errors = run(&pool, &[(0, var)], &empty_sig());

    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0].kind,
        TypeErrorKind::AmbiguousType { .. }
    ));
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// Spec: `types.md §TK-9` — `Tag::Applied` is the dedicated tag for
/// user-registered generic types (distinct from `Tag::Option`/`Tag::Result`
/// and their dedicated siblings). Using an `Applied(Name, [Var])` exercises
/// the validator's `Pool::visit_children` descent through a user-generic
/// argument list.
///
/// Plan §02.4 T2 (Negative / Applied).
#[test]
fn applied_generic_with_unbound_var_argument_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // Name is synthetic — the validator walks the type structure, it does
    // not require the name to be registered in `TypeRegistry`.
    let my_generic = Name::from_raw(200);
    let applied = pool.applied(my_generic, &[var]);

    let errors = run(&pool, &[(0, applied)], &empty_sig());

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// Spec: `typeck.md §CK-4` — signatures entering the Bodies group may
/// carry fresh `Tag::Var`s for elided annotations; those vars MUST resolve
/// before Bodies exit, and the validator emits at `sig_span` (the function
/// declaration span) when one survives.
///
/// Plan §02.4 T3 (Negative / Signature).
#[test]
fn signature_with_unbound_param_type_emits_at_sig_span() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // @f(x: <unbound>) -> int
    let sig = FunctionSig::simple(Name::from_raw(1), vec![var], Idx::INT);

    let errors = run(&pool, &[], &sig);

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].span, SIG_SPAN,
        "signature-origin diagnostic must emit at sig_span"
    );
}

// Positive cells — fast-path gates and well-formed scheme bodies

/// Spec: `types.md §TF-5` — `!HAS_VAR` short-circuit fires before any walk
/// on a fully-resolved primitive. `int` never violates PC-2.
///
/// Plan §02.4 T4 (Positive / Resolved Int).
#[test]
fn body_expr_types_with_resolved_int_emits_no_diagnostic() {
    let pool = Pool::new();

    let errors = run(&pool, &[(0, Idx::INT)], &empty_sig());

    assert!(errors.is_empty());
}

/// Spec: `types.md §TF-2` — flags are cached at intern time, but
/// `VarState::Link` may mutate later. The validator calls
/// `Pool::resolve_fully` BEFORE the `HAS_VAR` gate at every walk step, so
/// a var linked to `int` after interning resolves through and trips no
/// diagnostic.
///
/// Plan §02.4 T5 (Positive / Linked).
#[test]
fn body_expr_types_with_linked_var_resolves_and_emits_nothing() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // For `Tag::Var`, `data` IS the `var_id` (see `types.md §TY-3` plus the
    // tag-data decoding rule in `types.md` Appendix B).
    let var_id = pool.data(var);
    *pool.var_state_mut(var_id) = VarState::Link { target: Idx::INT };

    let errors = run(&pool, &[(0, var)], &empty_sig());

    assert!(errors.is_empty());
}

/// Spec: `types.md §TF-1` — `Tag::BoundVar` sets `HAS_BOUND_VAR`, NOT
/// `HAS_VAR`. A scheme body referencing a `BoundVar` therefore trips no
/// `HAS_VAR` flag on the outer scheme; the top-level fast-path gate skips
/// silently. This is the legitimate shape of a generalized polymorphic
/// binding after `typeck.md §GN-1` generalization.
///
/// Plan §02.4 T6 (Positive / Scheme `BoundVar`).
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

    let errors = run(&pool, &[(0, scheme)], &empty_sig());

    assert!(errors.is_empty());
}

/// Spec: `typeck.md §GN-1` — a `Tag::Var` in `VarState::Generalized` is a
/// legitimate polymorphic binding, not a PC-2 violation. The validator
/// explicitly exempts this state (see the rationale in
/// `check/validators/mod.rs::collect_first_unbound_var`). Removing this
/// exemption would fire `E2005` on every polymorphic let-binding.
///
/// This is a divergence from `types.md §SC-1`: the implementation stores
/// generalized vars as `Tag::Var(VarState::Generalized)` rather than
/// rewriting them to `Tag::BoundVar`. If a future SC-1 conformance fix
/// rewrites the pool layout, this test becomes tautological (all
/// `Tag::Var` entries would be Unbound) and can be removed.
///
/// Plan §02.4 T7 (Positive / Generalized).
#[test]
fn generalized_var_in_expr_types_emits_no_diagnostic() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    *pool.var_state_mut(var_id) = VarState::Generalized {
        id: var_id,
        name: None,
    };

    let errors = run(&pool, &[(0, var)], &empty_sig());

    assert!(errors.is_empty());
}

// Cascade / Determinism / Semantic pin

/// Spec: `typeck.md §ER-4`, `types.md §TK-3` — `Tag::Error` poisons a type
/// and MUST suppress cascading diagnostics. A tuple carrying BOTH a
/// `Tag::Var` and `Idx::ERROR` propagates both `HAS_VAR` and `HAS_ERROR`;
/// the validator's `HAS_ERROR` short-circuit fires at the top-level gate
/// and the co-located unbound var is silently masked.
///
/// Plan §02.4 T8 (Cascade).
#[test]
fn tuple_with_error_and_unbound_var_suppresses_diagnostic() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // (Var, Error) — `types.md §TF-3` propagates both HAS_VAR and HAS_ERROR.
    let tuple = pool.tuple(&[var, Idx::ERROR]);
    assert!(pool.flags(tuple).contains(TypeFlags::HAS_VAR));
    assert!(pool.flags(tuple).contains(TypeFlags::HAS_ERROR));

    let errors = run(&pool, &[(0, tuple)], &empty_sig());

    assert!(
        errors.is_empty(),
        "HAS_ERROR must suppress diagnostics for co-located unbound vars"
    );
}

/// Spec: `impl-hygiene.md §Pass determinism` — diagnostics must emit in a
/// reproducible order regardless of `FxHashMap` iteration. The validator's
/// contract is: signature positions first (in declaration order), then
/// body expressions in ascending `ExprIndex` order.
///
/// This test exercises both halves of the contract:
///   1. Signature diagnostic precedes body diagnostics.
///   2. Body diagnostics emit in ascending `ExprIndex` order even when
///      the map is populated with the higher index first — the validator's
///      `sort_unstable_by_key` step is the only reason this passes.
///
/// Plan §02.4 T9 (Determinism).
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
    let errors = run(&pool, &[(2, body_var_high), (1, body_var_low)], &sig);

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

/// Semantic pin for `§02.0` (Pool scheme-flag propagation fix).
///
/// A `Tag::Scheme` wrapping an unbound `Tag::Var` body MUST have `HAS_VAR`
/// set on the outer scheme `Idx`. Without the `§02.0` fix extending
/// `PROPAGATE_MASK` through `Tag::Scheme` in `Pool::compute_flags`, the
/// outer flags would be `HAS_VAR=false` and the validator's top-level
/// `!HAS_VAR` gate would return early — the scheme body would never be
/// walked and the PC-2 violation would be invisible.
///
/// This test would FAIL (emit zero diagnostics instead of one) if `§02.0`
/// were reverted. It is the permanent regression guard for the fix.
///
/// Plan §02.4 T10 (Semantic pin for §02.0).
#[test]
fn scheme_wrapping_unbound_var_body_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    // ∀[0]. Var — same shape as
    // `pool::tests::scheme_wrapping_unbound_var_body_propagates_has_var`.
    let scheme = pool.scheme(&[0], var);
    assert!(
        pool.flags(scheme).contains(TypeFlags::HAS_VAR),
        "§02.0 must propagate HAS_VAR through Tag::Scheme; without it \
         this test would silently pass with zero diagnostics"
    );

    let errors = run(&pool, &[(0, scheme)], &empty_sig());

    assert_eq!(
        errors.len(),
        1,
        "scheme body's unbound Tag::Var is a PC-2 violation"
    );
    assert_eq!(errors[0].span, BODY_SPAN);
}

// Nested compound / Dedup

/// Spec: `types.md §TF-3` propagation — compound tags must transitively
/// propagate `HAS_VAR` through nested layers. Uses `Tag::Option` wrapping
/// `Tag::List` wrapping a `Tag::Var` to exercise two levels of dedicated
/// (non-Applied) compound tags, confirming the validator's recursive
/// descent via `Pool::visit_children` reaches leaves at arbitrary depth.
///
/// Plan §02.4 T11 (Nested compound).
#[test]
fn option_list_var_two_level_nesting_emits_one_e2005() {
    let mut pool = Pool::new();
    let var = pool.fresh_var();
    let list_var = pool.list(var);
    let opt_list_var = pool.option(list_var);

    let errors = run(&pool, &[(0, opt_list_var)], &empty_sig());

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].span, BODY_SPAN);
}

/// Spec: `impl-hygiene.md §Deduplication by (Code, Span)` — multiple
/// unbound type variables at a single `ExprIndex` MUST collapse to one
/// `E2005`. The validator sets an `emitted` flag inside its
/// `visit_children` closure to short-circuit further emissions at the
/// same position.
///
/// Uses `Map<Var, Var>` where both key and value types are independently
/// unbound; without dedup, this would emit two diagnostics. The map
/// `Tag::Map` has two children (`types.md §TK-1` — two-child tag range);
/// the first child's unbound var trips the emission, and the `emitted`
/// flag suppresses the second.
///
/// Plan §02.4 T12 (Dedup).
#[test]
fn map_with_two_unbound_vars_emits_one_e2005_not_two() {
    let mut pool = Pool::new();
    let k_var = pool.fresh_var();
    let v_var = pool.fresh_var();
    let map_ty = pool.map(k_var, v_var);

    let errors = run(&pool, &[(0, map_ty)], &empty_sig());

    assert_eq!(
        errors.len(),
        1,
        "dedup must collapse multiple unbound vars under one ExprIndex"
    );
}
