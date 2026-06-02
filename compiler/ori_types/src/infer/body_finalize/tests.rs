//! Tests for the end-of-body defaulting pre-pass.
//!
//! Covers the matrix authored as part of the §11.1 polymorphic-constructor
//! extension: empty-literal defaulting (the original surface) plus the
//! introducer-only walks for `None` / `Ok` / `Err`, with negative pins for
//! `Some` (NOT a defaulting root — `infer_some` reuses the inner's type) and
//! for unbound payload generics under `Ok` / `Err` (payload vars must survive
//! to fire `E2005`, not be silently defaulted to `Never`).
//!
//! Each cell:
//!   1. Constructs a minimal `ExprArena` carrying ONE root expression.
//!   2. Builds an `InferEngine`, marks body inference complete.
//!   3. Pre-populates `expr_types` for the root (and any inner) expression.
//!   4. Invokes `default_unbound_vars_from_empty_literals` against a stub sig.
//!   5. Asserts the resulting type/var state matches the spec for that cell.

use ori_ir::{Expr, ExprArena, ExprKind, Name, Span};
use rustc_hash::FxHashSet;

use crate::output::FunctionSig;
use crate::{Idx, InferEngine, Pool, Tag, VarState};

const SPAN: Span = Span::DUMMY;

/// Build an empty `FunctionSig` returning `int` with no params. Tests that
/// need a non-trivial sig override these fields directly.
fn empty_sig() -> FunctionSig {
    FunctionSig::simple(Name::from_raw(1), vec![], Idx::INT)
}

/// Allocate a root expression of `kind` and drive the defaulting pre-pass
/// against the engine state populated by `setup`. Returns the engine's
/// final view of the root expression's type after defaulting (post-`Link`
/// resolution).
fn run_defaulting<F>(arena_and_root: (ExprArena, ori_ir::ExprId), setup: F) -> (Pool, Idx)
where
    F: FnOnce(&mut InferEngine<'_>, ori_ir::ExprId) -> Idx,
{
    let (arena, root) = arena_and_root;
    let mut pool = Pool::new();
    let mut sig = empty_sig();
    let exempt: FxHashSet<u32> = FxHashSet::default();

    let root_ty = {
        let mut engine = InferEngine::new(&mut pool);
        let ty = setup(&mut engine, root);
        engine.store_type(root.raw() as usize, ty);
        engine.mark_body_inference_complete();
        let mut expr_types = engine.take_expr_types();
        engine.default_unbound_vars_from_empty_literals(&arena, &mut expr_types, &mut sig, &exempt);
        let Some(&ty) = expr_types.get(&(root.raw() as usize)) else {
            panic!("root expr type missing from expr_types");
        };
        ty
    };

    let resolved = pool.resolve_fully(root_ty);
    (pool, resolved)
}

/// Build an arena with a single `None` root expression. Returns the arena
/// and the root's `ExprId`.
fn arena_with_none() -> (ExprArena, ori_ir::ExprId) {
    let mut arena = ExprArena::new();
    let root = arena.alloc_expr(Expr::new(ExprKind::None, SPAN));
    (arena, root)
}

/// Build an arena with a single empty `[]` root expression.
fn arena_with_empty_list() -> (ExprArena, ori_ir::ExprId) {
    let mut arena = ExprArena::new();
    let empty_range = arena.alloc_expr_list([]);
    let root = arena.alloc_expr(Expr::new(ExprKind::List(empty_range), SPAN));
    (arena, root)
}

/// Build an arena with `Ok(<inner_kind>)` and return both ids.
fn arena_with_ok_of(inner_kind: ExprKind) -> (ExprArena, ori_ir::ExprId, ori_ir::ExprId) {
    let mut arena = ExprArena::new();
    let inner = arena.alloc_expr(Expr::new(inner_kind, SPAN));
    let root = arena.alloc_expr(Expr::new(ExprKind::Ok(inner), SPAN));
    (arena, root, inner)
}

/// Build an arena with `Err(<inner_kind>)` and return both ids.
fn arena_with_err_of(inner_kind: ExprKind) -> (ExprArena, ori_ir::ExprId, ori_ir::ExprId) {
    let mut arena = ExprArena::new();
    let inner = arena.alloc_expr(Expr::new(inner_kind, SPAN));
    let root = arena.alloc_expr(Expr::new(ExprKind::Err(inner), SPAN));
    (arena, root, inner)
}

/// Build an arena with `Some(<inner_kind>)` and return both ids.
fn arena_with_some_of(inner_kind: ExprKind) -> (ExprArena, ori_ir::ExprId, ori_ir::ExprId) {
    let mut arena = ExprArena::new();
    let inner = arena.alloc_expr(Expr::new(inner_kind, SPAN));
    let root = arena.alloc_expr(Expr::new(ExprKind::Some(inner), SPAN));
    (arena, root, inner)
}

// Positive cells — defaulting roots flip the introduced fresh var to Never.

/// `None` (no annotation) defaults to `Option<Never>`. The fresh var that
/// `infer_none` would allocate as the inner is the introducer; the
/// `EmptyLiteralRoot` classification walks the full single-child tree and
/// flips it.
#[test]
fn naked_none_defaults_to_option_never() {
    let (pool, resolved) = run_defaulting(arena_with_none(), |engine, _root| {
        let inner = engine.fresh_var();
        engine.infer_option(inner)
    });

    assert_eq!(pool.tag(resolved), Tag::Option, "root must be Option");
    let inner = pool.option_inner(resolved);
    let resolved_inner = pool.resolve_fully(inner);
    assert_eq!(
        resolved_inner,
        Idx::NEVER,
        "None's introducer var must default to Never"
    );
}

/// `let r = Ok(42)` types as `Result<int, Never>`. The err slot's fresh
/// var is the introducer; `IntroducerSlot(ResultSlot::Err)` walks only
/// that slot.
#[test]
fn let_r_eq_ok_42_types_as_result_int_never() {
    // Build `Ok(42)`; the inner expression is an Int literal — typed `int`,
    // contributes no fresh var. The err slot fresh var is the introducer.
    let (arena, root, _inner) = arena_with_ok_of(ExprKind::Int(42));

    let (pool, resolved) = run_defaulting((arena, root), |engine, _root| {
        // Mirror `infer_ok` exactly: inner_ty is the inner expression's type
        // (here `int`); err_ty is a fresh var (the introducer).
        let ok_ty = Idx::INT;
        let err_ty = engine.fresh_var();
        engine.infer_result(ok_ty, err_ty)
    });

    assert_eq!(pool.tag(resolved), Tag::Result);
    assert_eq!(pool.resolve_fully(pool.result_ok(resolved)), Idx::INT);
    assert_eq!(
        pool.resolve_fully(pool.result_err(resolved)),
        Idx::NEVER,
        "Ok's err-slot introducer var must default to Never"
    );
}

/// `let r = Err("boom")` types as `Result<Never, str>`. The ok slot's
/// fresh var is the introducer.
#[test]
fn let_r_eq_err_boom_types_as_result_never_str() {
    let (arena, root, _inner) = arena_with_err_of(ExprKind::String(Name::from_raw(0)));
    let (pool, resolved) = run_defaulting((arena, root), |engine, _root| {
        let ok_ty = engine.fresh_var();
        let err_ty = Idx::STR;
        engine.infer_result(ok_ty, err_ty)
    });

    assert_eq!(pool.tag(resolved), Tag::Result);
    assert_eq!(
        pool.resolve_fully(pool.result_ok(resolved)),
        Idx::NEVER,
        "Err's ok-slot introducer var must default to Never"
    );
    assert_eq!(pool.resolve_fully(pool.result_err(resolved)), Idx::STR);
}

// Negative cells — Some is NOT a defaulting root; payload vars survive.

/// `Some(x)` with an unbound payload var does NOT trigger defaulting at the
/// `Some` level. `infer_some` reuses `infer_expr(inner)` — no fresh var is
/// introduced at the constructor. The payload var must remain `Unbound` so
/// the validator's PC-2 pass fires `E2005`, not silently default to Never.
#[test]
fn some_never_defaulting_root_when_payload_unbound() {
    // The inner expression kind here is irrelevant — we just need a placeholder
    // ExprId in the arena. We seed `expr_types` only for the Some root so the
    // pre-pass walks only that one ExprIndex.
    let (arena, root, _inner) = arena_with_some_of(ExprKind::None);

    let mut pool = Pool::new();
    let mut sig = empty_sig();
    let exempt: FxHashSet<u32> = FxHashSet::default();

    let (payload_var_id, payload_var_idx, root_ty) = {
        let mut engine = InferEngine::new(&mut pool);
        let payload_var = engine.fresh_var();
        let some_ty = engine.infer_option(payload_var);
        engine.store_type(root.raw() as usize, some_ty);
        engine.mark_body_inference_complete();
        let mut expr_types = engine.take_expr_types();
        engine.default_unbound_vars_from_empty_literals(&arena, &mut expr_types, &mut sig, &exempt);
        let var_id = engine.pool().data(payload_var);
        let Some(&root_ty) = expr_types.get(&(root.raw() as usize)) else {
            panic!("Some root type missing from expr_types");
        };
        (var_id, payload_var, root_ty)
    };

    // Some is NOT a defaulting root — the payload var must remain Unbound.
    assert!(
        matches!(pool.var_state(payload_var_id), VarState::Unbound { .. }),
        "Some's payload var was incorrectly defaulted; \
         is_defaulting_root must classify Some as NotARoot"
    );
    let resolved_root = pool.resolve_fully(root_ty);
    assert_eq!(pool.tag(resolved_root), Tag::Option);
    // The inner var must NOT have resolved to Never.
    let resolved_inner = pool.resolve_fully(pool.option_inner(resolved_root));
    assert_ne!(
        resolved_inner,
        Idx::NEVER,
        "Some's payload must not silently default to Never"
    );
    // Sanity: the resolved inner is still the original var idx (no Link).
    let _ = payload_var_idx;
}

/// `Some([])` cascades defaulting through the inner empty-literal's own
/// classification — the inner `[]` IS a defaulting root, so its element
/// var resolves to Never, and the outer `Some` sees the same resolution
/// through unification. Some itself is NOT a root; the cascade works
/// through the inner expression's own entry in `expr_types`.
#[test]
fn naked_some_with_empty_list_cascades_defaulting() {
    // arena: Some(inner) where inner is empty list `[]`
    let mut arena = ExprArena::new();
    let empty_range = arena.alloc_expr_list([]);
    let inner = arena.alloc_expr(Expr::new(ExprKind::List(empty_range), SPAN));
    let root = arena.alloc_expr(Expr::new(ExprKind::Some(inner), SPAN));

    let mut pool = Pool::new();
    let mut sig = empty_sig();
    let exempt: FxHashSet<u32> = FxHashSet::default();

    let (root_ty_after, inner_ty_after) = {
        let mut engine = InferEngine::new(&mut pool);
        // Mirror `infer_some` (which calls `infer_expr(inner)` first):
        // inner's type = `[fresh_elem]` (what `infer_list` of `[]` produces).
        let elem_var = engine.fresh_var();
        let list_ty = engine.pool_mut().list(elem_var);
        engine.store_type(inner.raw() as usize, list_ty);

        // Some's type = Option<list_ty> — uses the same inner_ty per
        // `infer_some`'s `engine.infer_option(inner_ty)`.
        let some_ty = engine.infer_option(list_ty);
        engine.store_type(root.raw() as usize, some_ty);

        engine.mark_body_inference_complete();
        let mut expr_types = engine.take_expr_types();
        engine.default_unbound_vars_from_empty_literals(&arena, &mut expr_types, &mut sig, &exempt);

        let Some(&root_ty) = expr_types.get(&(root.raw() as usize)) else {
            panic!("Some root type missing from expr_types");
        };
        let Some(&inner_ty) = expr_types.get(&(inner.raw() as usize)) else {
            panic!("empty-list inner type missing from expr_types");
        };
        (root_ty, inner_ty)
    };

    // Inner empty list: element resolves to Never.
    let resolved_inner = pool.resolve_fully(inner_ty_after);
    assert_eq!(pool.tag(resolved_inner), Tag::List);
    assert_eq!(
        pool.resolve_fully(pool.list_elem(resolved_inner)),
        Idx::NEVER,
        "empty list's element var must default to Never"
    );

    // Outer Some sees the same resolution via the shared `elem_var` link.
    let resolved_root = pool.resolve_fully(root_ty_after);
    assert_eq!(pool.tag(resolved_root), Tag::Option);
    let outer_inner = pool.resolve_fully(pool.option_inner(resolved_root));
    assert_eq!(pool.tag(outer_inner), Tag::List);
    assert_eq!(
        pool.resolve_fully(pool.list_elem(outer_inner)),
        Idx::NEVER,
        "Some-wrapped empty list element must cascade to Never"
    );
}

// Regression guards — pass-order invariant + introducer-only payload isolation.

/// `default_unbound_vars_from_empty_literals` panics in debug builds when
/// invoked before `mark_body_inference_complete` (pass-order inversion).
/// Guarded to debug builds: the guard is a `debug_assert!`, stripped in
/// release, so the panic only fires when `debug_assertions` is on.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "body_inference_complete invariant violated")]
fn debug_assert_fires_on_pass_order_inversion() {
    let (arena, _root) = arena_with_none();
    let mut pool = Pool::new();
    let mut sig = empty_sig();
    let exempt: FxHashSet<u32> = FxHashSet::default();
    let mut engine = InferEngine::new(&mut pool);
    // Intentionally skip `engine.mark_body_inference_complete()` to invert
    // the contract. The defaulting helper must panic.
    let mut expr_types = engine.take_expr_types();
    engine.default_unbound_vars_from_empty_literals(&arena, &mut expr_types, &mut sig, &exempt);
}

/// `Ok(<unbound payload-generic call>)` defaults the err-slot introducer var
/// but MUST NOT default the unbound payload generic in the ok slot. The
/// payload var lives under `inner_ty` (whatever `infer_expr(inner)`
/// produced) and belongs to the inner expression's own classification —
/// the `IntroducerSlot(ResultSlot::Err)` walk ONLY visits the err slot.
///
/// Regression guard for the introducer-only walk: a full-tree walk would
/// incorrectly default the payload generic to `Never`, silently masking the
/// legitimate `E2005` the validator should fire on the unconstrained
/// payload var.
#[test]
fn ok_introducer_only_walk_preserves_unrelated_payload_generic_e2005() {
    let (arena, root, _inner) = arena_with_ok_of(ExprKind::None);
    let mut pool = Pool::new();
    let mut sig = empty_sig();
    let exempt: FxHashSet<u32> = FxHashSet::default();

    let (payload_var_id, err_var_id, root_ty) = {
        let mut engine = InferEngine::new(&mut pool);
        // Build `Result<payload_var, err_var>` mirroring `infer_ok` shape
        // where `inner_ty = payload_var` (an unbound var the inner call
        // would return).
        let payload_var = engine.fresh_var();
        let err_var = engine.fresh_var();
        let ok_ty = engine.infer_result(payload_var, err_var);
        engine.store_type(root.raw() as usize, ok_ty);
        engine.mark_body_inference_complete();
        let mut expr_types = engine.take_expr_types();
        engine.default_unbound_vars_from_empty_literals(&arena, &mut expr_types, &mut sig, &exempt);
        let payload_id = engine.pool().data(payload_var);
        let err_id = engine.pool().data(err_var);
        let Some(&root_after) = expr_types.get(&(root.raw() as usize)) else {
            panic!("Ok root type missing from expr_types");
        };
        (payload_id, err_id, root_after)
    };

    // Err slot var is the introducer — defaulted to Never.
    assert!(
        matches!(
            pool.var_state(err_var_id),
            VarState::Link { target } if *target == Idx::NEVER
        ),
        "err-slot introducer var must default to Never"
    );

    // Payload (ok slot) var is NOT the introducer — must remain Unbound so
    // the validator's PC-2 pass fires E2005 on it. A full-tree walk would
    // have incorrectly defaulted this var.
    assert!(
        matches!(pool.var_state(payload_var_id), VarState::Unbound { .. }),
        "Ok's ok-slot payload var was incorrectly defaulted by a full-tree walk; \
         introducer-only walk must restrict to the err slot"
    );

    // Sanity: root type still Result with NEVER on err, Var on ok.
    let resolved_root = pool.resolve_fully(root_ty);
    assert_eq!(pool.tag(resolved_root), Tag::Result);
    assert_eq!(
        pool.resolve_fully(pool.result_err(resolved_root)),
        Idx::NEVER
    );
    // Ok slot resolves to the original payload var (still unbound).
    assert_eq!(
        pool.tag(pool.resolve_fully(pool.result_ok(resolved_root))),
        Tag::Var
    );
}

// Empty literal regression — the original §03 surface stays green.

/// Bare `let xs = []` defaults to `[Never]` per the existing empty-literal
/// behavior. Pre-§11.1 cure path; pinned here to guard against the rename
/// or the introducer-only refactor accidentally regressing the original
/// `EmptyLiteralRoot` classification.
#[test]
fn empty_list_defaults_to_list_never() {
    let (pool, resolved) = run_defaulting(arena_with_empty_list(), |engine, _root| {
        let elem_var = engine.fresh_var();
        engine.pool_mut().list(elem_var)
    });

    assert_eq!(pool.tag(resolved), Tag::List);
    assert_eq!(pool.resolve_fully(pool.list_elem(resolved)), Idx::NEVER);
}
