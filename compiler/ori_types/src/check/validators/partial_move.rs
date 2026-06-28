//! Conditional partial-move rejection — producer side of `E2043`.
//!
//! Phase 5 ARC lowering relies on the invariant that `moved_out_fields[v]`
//! is statically computable per-CFG-path for every owned aggregate `v`.
//! Conditional partial moves — where a field is projected on one branch
//! of an `if`/`match` but not on a sibling branch — violate that
//! invariant: resolving them at the join would require fixpoint dataflow
//! over a lattice, which the trivial-emission goal of Phase 5 forbids.
//!
//! This validator rejects such patterns at type-check time so Phase 5
//! never sees them. The scope is non-Drop owned aggregates (Struct,
//! Tuple, Enum, user-defined Named types). Drop types are governed by
//! `EDROP_PARTIAL_MOVE` (E2048) and `EVALUE_DROP_CONFLICT` (E2049).
//!
//! # Algorithm
//!
//! Walks every body expression's AST. For each `If`/`Match`, collects
//! the set of `(aggregate_var, field_name)` pairs each branch projects
//! via `Field { receiver: Ident(v), field }`. If a pair appears
//! asymmetrically — present on some branches, absent on others — emit
//! `E2043` at the branch site that introduced the asymmetric projection.
//!
//! The walk is structural and bounded by the AST size: no CFG, no
//! fixpoint, no lattice consultation.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::validators::expr_children::child_ids;
use crate::tag::Tag;
use crate::{ExprIndex, Idx, Pool, TypeCheckError};

/// Validate that every body expression is free of conditional partial
/// moves on non-Drop owned aggregates.
///
/// Walks `body_root`'s AST top-down. At every `If`/`Match` node, collects
/// the projection sets each branch produces and compares them. Asymmetric
/// projections emit `E2043`.
///
/// `expr_types` is the inference output for this body — used to resolve
/// each aggregate variable's type and gate the check on non-Drop owned
/// aggregates (see [`is_non_drop_aggregate_for`]).
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the crate-wide choice for expr_types; \
              callers are internal and always use FxHashMap"
)]
pub fn validate_partial_move(
    pool: &Pool,
    arena: &ExprArena,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    body_root: ExprId,
    errors: &mut Vec<TypeCheckError>,
) {
    if body_root == ExprId::INVALID {
        return;
    }
    let mut ctx = WalkCtx {
        pool,
        arena,
        expr_types,
        errors,
    };
    walk_expr(&mut ctx, body_root);
}

/// Per-validator walk state.
struct WalkCtx<'a> {
    pool: &'a Pool,
    arena: &'a ExprArena,
    expr_types: &'a FxHashMap<ExprIndex, Idx>,
    errors: &'a mut Vec<TypeCheckError>,
}

/// Walk a single expression node, recursing into children + emitting
/// conditional-partial-move diagnostics at `If`/`Match` boundaries.
fn walk_expr(ctx: &mut WalkCtx<'_>, expr_id: ExprId) {
    if expr_id == ExprId::INVALID {
        return;
    }
    // ExprKind is Copy.
    let kind = *ctx.arena.expr_kind(expr_id);
    match kind {
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            // Recurse into cond first — projections in cond run
            // unconditionally relative to the if-arms below.
            walk_expr(ctx, cond);
            check_conditional_branches(ctx, &[then_branch, else_branch]);
        }
        ExprKind::Match { scrutinee, arms } => {
            walk_expr(ctx, scrutinee);
            let arm_bodies: Vec<ExprId> = ctx.arena.get_arms(arms).iter().map(|a| a.body).collect();
            check_conditional_branches(ctx, &arm_bodies);
        }
        _ => walk_children(ctx, expr_id),
    }
}

/// Collect each branch's projection set, compare for asymmetry, and emit
/// `E2043` for any aggregate whose projected field set differs across
/// branches.
///
/// For branches that are `ExprId::INVALID` (an `if cond then ...` form
/// with no else), treat the missing branch as projecting the empty set —
/// any projection in the other branch is a conditional move.
fn check_conditional_branches(ctx: &mut WalkCtx<'_>, branches: &[ExprId]) {
    // Per-branch projection set, indexed by branch position.
    let mut per_branch: Vec<FxHashMap<(Name, Name), Span>> = Vec::with_capacity(branches.len());
    for &b in branches {
        let mut set: FxHashMap<(Name, Name), Span> = FxHashMap::default();
        if b != ExprId::INVALID {
            collect_projections(ctx, b, &mut set);
        }
        per_branch.push(set);
    }

    // Union of all projection keys across branches.
    let mut keys: FxHashSet<(Name, Name)> = FxHashSet::default();
    for set in &per_branch {
        keys.extend(set.keys().copied());
    }

    // Sort keys for deterministic diagnostic ordering.
    let mut keys: Vec<(Name, Name)> = keys.into_iter().collect();
    keys.sort_unstable_by_key(|(agg, fld)| (agg.raw(), fld.raw()));

    for (agg, fld) in keys {
        // For each branch, does it project (agg, fld)?
        let presence: Vec<bool> = per_branch
            .iter()
            .map(|set| set.contains_key(&(agg, fld)))
            .collect();
        // Asymmetric ⟺ at least one true AND at least one false.
        let any = presence.iter().any(|&p| p);
        let all = presence.iter().all(|&p| p);
        if any && !all {
            // Find the first branch carrying the projection — its
            // span attributes the diagnostic.
            if let Some((branch_idx, _)) = presence.iter().enumerate().find(|(_, &p)| p) {
                if let Some(&span) = per_branch[branch_idx].get(&(agg, fld)) {
                    ctx.errors
                        .push(TypeCheckError::conditional_partial_move(span, agg, fld));
                }
            }
        }
    }

    // After comparing this if/match's own branches, recurse into every
    // branch so nested conditionals are checked too.
    for &b in branches {
        if b != ExprId::INVALID {
            walk_expr(ctx, b);
        }
    }
}

/// Collect every `(aggregate_name, field_name)` partial-move rooted in
/// `expr_id`'s subtree.
///
/// **Scope (narrow):** the partial-move rule rejects the specific shape
/// `let f = v.field` — a `let` binding whose initializer is a direct
/// projection of an in-scope aggregate variable. Field accesses that
/// appear as read-only operands (e.g. inside arithmetic, comparisons,
/// arguments, struct-literal field values, spread expressions) do NOT
/// move the field and are out of scope. Phase 5 ARC lowering uses
/// AIMS-side analysis to distinguish read vs move on those forms;
/// typeck cannot, so the validator restricts itself to the unambiguous
/// `let`-binding shape.
///
/// Stops descending at nested `If`/`Match` nodes — those have their own
/// per-branch check via [`check_conditional_branches`], and counting
/// their inner partial moves here would double-attribute the asymmetry.
/// The recursive walk that visits the nested conditional later handles
/// its own branches independently.
fn collect_projections(
    ctx: &WalkCtx<'_>,
    expr_id: ExprId,
    out: &mut FxHashMap<(Name, Name), Span>,
) {
    if expr_id == ExprId::INVALID {
        return;
    }
    let expr = ctx.arena.get_expr(expr_id);
    match expr.kind {
        ExprKind::Let { init, .. } => {
            // The canonical partial-move shape: `let f = v.field`.
            // Recognise direct projection initializers only — chained or
            // call-wrapped initializers fall outside the partial-move scope.
            if let Some((agg, field, span)) = direct_projection(ctx.arena, init) {
                if is_non_drop_aggregate_for(ctx, init_receiver(ctx.arena, init)) {
                    out.insert((agg, field), span);
                }
            }
            // Recurse into the initializer to find nested let-bindings
            // (e.g. `let f = { let g = v.field; g }`).
            collect_projections_in_children(ctx, expr_id, out);
        }
        ExprKind::If { .. } | ExprKind::Match { .. } => {
            // Nested conditional — checked independently when the
            // top-level walk reaches it. Do NOT descend here.
        }
        _ => {
            // Block statements descend into their stmt-list, so a
            // `let f = v.field;` written as a block statement (the
            // common form) routes back through this function via
            // `Block.stmts` → `StmtKind::Let { init, .. }`. The same
            // `direct_projection`/`is_non_drop_aggregate_for` checks
            // run on each let initializer.
            collect_projections_from_stmts(ctx, expr_id, out);
            collect_projections_in_children(ctx, expr_id, out);
        }
    }
}

/// Return `Some((aggregate_name, field_name, span))` when `init` is a
/// direct `Ident(v).field` projection. `None` otherwise — the partial-move
/// checks' scope is direct-aggregate projection (`v.field`); `f(v).field`
/// or `(v as T).field` do not match the per-CFG-path invariant. Shared by
/// the E2043 ([`validate_partial_move`]) and E2048
/// ([`validate_drop_partial_move`]) detectors.
pub(super) fn direct_projection(arena: &ExprArena, init: ExprId) -> Option<(Name, Name, Span)> {
    if init == ExprId::INVALID {
        return None;
    }
    let expr = arena.get_expr(init);
    if let ExprKind::Field { receiver, field } = expr.kind {
        if let ExprKind::Ident(agg) = arena.expr_kind(receiver) {
            return Some((*agg, field, expr.span));
        }
    }
    None
}

/// Return the receiver `ExprId` of a `let`-binding initializer that is
/// a direct field projection. Returns `ExprId::INVALID` otherwise — the
/// caller-side aggregate gate short-circuits on invalid ids. Shared by
/// both partial-move detectors.
pub(super) fn init_receiver(arena: &ExprArena, init: ExprId) -> ExprId {
    if init == ExprId::INVALID {
        return ExprId::INVALID;
    }
    if let ExprKind::Field { receiver, .. } = arena.expr_kind(init) {
        *receiver
    } else {
        ExprId::INVALID
    }
}

/// Walk `expr_id` if it is a `Block` and run [`collect_projections`] on
/// every `let`-statement initializer in the block. This is the
/// statement-form mirror of the `ExprKind::Let` arm in
/// [`collect_projections`]; a block of statements where each `let
/// $a = v.field;` line carries a partial move lands here.
fn collect_projections_from_stmts(
    ctx: &WalkCtx<'_>,
    expr_id: ExprId,
    out: &mut FxHashMap<(Name, Name), Span>,
) {
    let ExprKind::Block { stmts, .. } = *ctx.arena.expr_kind(expr_id) else {
        return;
    };
    for stmt in ctx.arena.get_stmt_range(stmts) {
        if let ori_ir::StmtKind::Let { init, .. } = stmt.kind {
            if let Some((agg, field, span)) = direct_projection(ctx.arena, init) {
                if is_non_drop_aggregate_for(ctx, init_receiver(ctx.arena, init)) {
                    out.insert((agg, field), span);
                }
            }
        }
    }
}

/// Helper: recurse into every child expression of `expr_id` and collect
/// projections. Mirrors [`walk_children`] but threads the projection
/// accumulator.
fn collect_projections_in_children(
    ctx: &WalkCtx<'_>,
    expr_id: ExprId,
    out: &mut FxHashMap<(Name, Name), Span>,
) {
    let children = child_ids(ctx.arena, expr_id);
    for c in children {
        collect_projections(ctx, c, out);
    }
}

/// Walk every child of an expression for the conditional-partial-move
/// top-level pass. Used for non-`If`/`Match` recursion.
fn walk_children(ctx: &mut WalkCtx<'_>, expr_id: ExprId) {
    let arena = ctx.arena;
    let children = child_ids(arena, expr_id);
    for c in children {
        walk_expr(ctx, c);
    }
}

/// Return `true` iff `receiver`'s inferred type is a non-Drop owned
/// aggregate (Struct, Tuple, Enum, user-defined Named type).
///
/// Primitive scalars (`int`, `float`, etc.), borrowed projections, and
/// collection types (`[T]`, `{K: V}`, `Set<T>`, `str`) all fall outside
/// the partial-move check's scope: scalars carry no burden; collections
/// own a heap buffer but field projection isn't expressible at the
/// surface for them.
///
/// Drop-trait detection: the prelude's `Drop` trait is not yet wired
/// into the trait registry for user-type lookup. Until
/// `EDROP_PARTIAL_MOVE` infrastructure ships, every user-defined
/// aggregate is treated as non-Drop. This is the conservative default:
/// rejecting too few conditional partial moves is a Phase 5 invariant
/// violation (silent miscompilation); rejecting too many is a
/// transient over-restriction.
fn is_non_drop_aggregate_for(ctx: &WalkCtx<'_>, expr: ExprId) -> bool {
    let Some(&ty) = ctx.expr_types.get(&expr.raw_index()) else {
        return false;
    };
    let resolved = ctx.pool.resolve_fully(ty);
    if resolved == Idx::ERROR {
        return false;
    }
    matches!(ctx.pool.tag(resolved), Tag::Struct | Tag::Tuple | Tag::Enum)
}

/// Convenience adapter exposing [`ExprId::raw`] as the [`ExprIndex`] key
/// used in `expr_types`. [`ExprIndex`] is `usize`.
pub(super) trait ExprIdRawIndex {
    fn raw_index(&self) -> ExprIndex;
}

impl ExprIdRawIndex for ExprId {
    #[inline]
    fn raw_index(&self) -> ExprIndex {
        self.raw() as ExprIndex
    }
}

#[cfg(test)]
mod tests;
