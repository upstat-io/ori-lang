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
//! `EDROP_PARTIAL_MOVE` (reserved at `E2044`).
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

use crate::output::FunctionSig;
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
///
/// `_sig` is currently unused but accepted to mirror
/// [`crate::check::validators::validate_body_types`]'s signature and
/// provide a hook for future signature-position partial-move checks
/// (e.g., when method generic parameters can carry move-state metadata).
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the crate-wide choice for expr_types; \
              callers are internal and always use FxHashMap"
)]
pub fn validate_partial_move(
    pool: &Pool,
    arena: &ExprArena,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    _sig: &FunctionSig,
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
            // call-wrapped initializers fall outside §03.4 scope.
            if let Some((agg, field, span)) = direct_projection(ctx, init) {
                if is_non_drop_aggregate_for(ctx, init_receiver(ctx, init)) {
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
/// direct `Ident(v).field` projection. `None` otherwise.
fn direct_projection(ctx: &WalkCtx<'_>, init: ExprId) -> Option<(Name, Name, Span)> {
    if init == ExprId::INVALID {
        return None;
    }
    let expr = ctx.arena.get_expr(init);
    if let ExprKind::Field { receiver, field } = expr.kind {
        if let Some(agg) = aggregate_name_of(ctx, receiver) {
            return Some((agg, field, expr.span));
        }
    }
    None
}

/// Return the receiver `ExprId` of a `let`-binding initializer that is
/// a direct field projection. Returns `ExprId::INVALID` otherwise — the
/// caller-side [`is_non_drop_aggregate_for`] short-circuits on invalid
/// ids.
fn init_receiver(ctx: &WalkCtx<'_>, init: ExprId) -> ExprId {
    if init == ExprId::INVALID {
        return ExprId::INVALID;
    }
    if let ExprKind::Field { receiver, .. } = ctx.arena.expr_kind(init) {
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
            if let Some((agg, field, span)) = direct_projection(ctx, init) {
                if is_non_drop_aggregate_for(ctx, init_receiver(ctx, init)) {
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
    let children = collect_child_ids(ctx.arena, expr_id);
    for c in children {
        collect_projections(ctx, c, out);
    }
}

/// Walk every child of an expression for the conditional-partial-move
/// top-level pass. Used for non-`If`/`Match` recursion.
fn walk_children(ctx: &mut WalkCtx<'_>, expr_id: ExprId) {
    let arena = ctx.arena;
    let children = collect_child_ids(arena, expr_id);
    for c in children {
        walk_expr(ctx, c);
    }
}

/// Return the list of every [`ExprId`] reachable in one structural step
/// from `expr_id`. Mirrors the relevant arms of
/// [`ori_ir::visitor::walk_expr`] but specialized for the subset of
/// [`ExprKind`] variants relevant to this validator (anything that can
/// contain projections or nested conditionals).
fn collect_child_ids(arena: &ExprArena, expr_id: ExprId) -> Vec<ExprId> {
    // ExprKind is Copy.
    let kind = *arena.expr_kind(expr_id);
    let mut out: Vec<ExprId> = Vec::new();
    push_children_for_kind(arena, kind, &mut out);
    out
}

/// Push every relevant child expression for `kind` into `out`.
///
/// Extracted from [`collect_child_ids`] to keep cognitive complexity in
/// each arm bounded; per-variant arms below dispatch into small per-shape
/// helpers ([`push_call_children`], [`push_call_named_children`],
/// [`push_expr_list_children`], [`push_list_element_children`],
/// [`push_map_entry_children`], [`push_map_element_children`],
/// [`push_struct_children`], [`push_struct_lit_field_children`]) that
/// own each variant family.
#[expect(
    clippy::too_many_lines,
    reason = "Dispatch table over every relevant ExprKind variant; \
              splitting further would require touching the rust-analyzer-driven \
              registration sync points called out in impl-hygiene's Registration \
              Sync Points section, which is the wrong cure — this function IS the \
              registration."
)]
fn push_children_for_kind(arena: &ExprArena, kind: ExprKind, out: &mut Vec<ExprId>) {
    match kind {
        ExprKind::Binary { left, right, .. } => {
            out.push(left);
            out.push(right);
        }
        ExprKind::Unary { operand, .. } => out.push(operand),
        ExprKind::Call { func, args }
        | ExprKind::MethodCall {
            receiver: func,
            args,
            ..
        } => push_call_children(arena, func, args, out),
        ExprKind::CallNamed { func, args }
        | ExprKind::MethodCallNamed {
            receiver: func,
            args,
            ..
        } => push_call_named_children(arena, func, args, out),
        ExprKind::Field { receiver, .. } => out.push(receiver),
        ExprKind::Index { receiver, index } => {
            out.push(receiver);
            out.push(index);
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            out.push(cond);
            out.push(then_branch);
            out.push(else_branch);
        }
        ExprKind::Match { scrutinee, arms } => {
            out.push(scrutinee);
            for arm in arena.get_arms(arms) {
                if let Some(g) = arm.guard {
                    out.push(g);
                }
                out.push(arm.body);
            }
        }
        ExprKind::For {
            iter, guard, body, ..
        } => {
            out.push(iter);
            if guard != ExprId::INVALID {
                out.push(guard);
            }
            out.push(body);
        }
        ExprKind::Loop { body, .. } => out.push(body),
        ExprKind::Block { stmts, result } => {
            for stmt in arena.get_stmt_range(stmts) {
                match stmt.kind {
                    ori_ir::StmtKind::Expr(id) | ori_ir::StmtKind::Let { init: id, .. } => {
                        out.push(id);
                    }
                }
            }
            if result != ExprId::INVALID {
                out.push(result);
            }
        }
        ExprKind::Let { init, .. } | ExprKind::Lambda { body: init, .. } => out.push(init),
        ExprKind::List(range) | ExprKind::Tuple(range) => {
            push_expr_list_children(arena, range, out);
        }
        ExprKind::ListWithSpread(range) => {
            push_list_element_children(arena, range, out);
        }
        ExprKind::Map(range) => push_map_entry_children(arena, range, out),
        ExprKind::MapWithSpread(range) => push_map_element_children(arena, range, out),
        ExprKind::Struct { fields, .. } => push_struct_children(arena, fields, out),
        ExprKind::StructWithSpread { fields, .. } => {
            push_struct_lit_field_children(arena, fields, out);
        }
        ExprKind::Range {
            start, end, step, ..
        } => {
            for id in [start, end, step] {
                if id != ExprId::INVALID {
                    out.push(id);
                }
            }
        }
        ExprKind::Ok(inner)
        | ExprKind::Err(inner)
        | ExprKind::Some(inner)
        | ExprKind::Await(inner)
        | ExprKind::Try(inner)
        | ExprKind::Unsafe(inner) => {
            if inner != ExprId::INVALID {
                out.push(inner);
            }
        }
        ExprKind::Break { value, .. } | ExprKind::Continue { value, .. } => {
            if value != ExprId::INVALID {
                out.push(value);
            }
        }
        ExprKind::Cast { expr, .. } => out.push(expr),
        ExprKind::Assign { target, value } => {
            out.push(target);
            out.push(value);
        }
        ExprKind::WithCapability { provider, body, .. } => {
            out.push(provider);
            out.push(body);
        }
        // Leaf / non-relevant variants — no child IDs that can carry
        // projections (FunctionSeq / FunctionExp / TemplateLiteral land
        // here because their projection-bearing surfaces are checked
        // post-canonicalization).
        _ => {}
    }
}

/// Push children of [`ExprKind::Call`] / [`ExprKind::MethodCall`] (positional args).
fn push_call_children(
    arena: &ExprArena,
    callee: ExprId,
    args: ori_ir::ExprRange,
    out: &mut Vec<ExprId>,
) {
    out.push(callee);
    for &id in arena.get_expr_list(args) {
        out.push(id);
    }
}

/// Push children of [`ExprKind::CallNamed`] / [`ExprKind::MethodCallNamed`] (named args).
fn push_call_named_children(
    arena: &ExprArena,
    callee: ExprId,
    args: ori_ir::CallArgRange,
    out: &mut Vec<ExprId>,
) {
    out.push(callee);
    for a in arena.get_call_args(args) {
        out.push(a.value);
    }
}

/// Push children from a bare [`ori_ir::ExprRange`] ([`ExprKind::List`] / [`ExprKind::Tuple`]).
fn push_expr_list_children(arena: &ExprArena, range: ori_ir::ExprRange, out: &mut Vec<ExprId>) {
    for &id in arena.get_expr_list(range) {
        out.push(id);
    }
}

/// Push children from a [`ori_ir::ListElementRange`] ([`ExprKind::ListWithSpread`]).
fn push_list_element_children(
    arena: &ExprArena,
    range: ori_ir::ListElementRange,
    out: &mut Vec<ExprId>,
) {
    for el in arena.get_list_elements(range) {
        match el {
            ori_ir::ListElement::Expr { expr, .. } | ori_ir::ListElement::Spread { expr, .. } => {
                out.push(*expr);
            }
        }
    }
}

/// Push children from a [`ori_ir::MapEntryRange`] (plain [`ExprKind::Map`] literal).
fn push_map_entry_children(arena: &ExprArena, range: ori_ir::MapEntryRange, out: &mut Vec<ExprId>) {
    for entry in arena.get_map_entries(range) {
        out.push(entry.key);
        out.push(entry.value);
    }
}

/// Push children from a [`ori_ir::MapElementRange`] ([`ExprKind::MapWithSpread`]).
fn push_map_element_children(
    arena: &ExprArena,
    range: ori_ir::MapElementRange,
    out: &mut Vec<ExprId>,
) {
    for el in arena.get_map_elements(range) {
        match el {
            ori_ir::MapElement::Entry(e) => {
                out.push(e.key);
                out.push(e.value);
            }
            ori_ir::MapElement::Spread { expr, .. } => out.push(*expr),
        }
    }
}

/// Push children from a [`ori_ir::FieldInitRange`] (plain [`ExprKind::Struct`] literal).
fn push_struct_children(arena: &ExprArena, range: ori_ir::FieldInitRange, out: &mut Vec<ExprId>) {
    for f in arena.get_field_inits(range) {
        if let Some(v) = f.value {
            out.push(v);
        }
    }
}

/// Push children from a [`ori_ir::StructLitFieldRange`] ([`ExprKind::StructWithSpread`]).
fn push_struct_lit_field_children(
    arena: &ExprArena,
    range: ori_ir::StructLitFieldRange,
    out: &mut Vec<ExprId>,
) {
    for f in arena.get_struct_lit_fields(range) {
        match f {
            ori_ir::StructLitField::Field(init) => {
                if let Some(v) = init.value {
                    out.push(v);
                }
            }
            ori_ir::StructLitField::Spread { expr, .. } => out.push(*expr),
        }
    }
}

/// Return the surface name of `expr` when it is a bare `Ident(name)`.
/// `None` for any other shape — the partial-move check's scope is
/// direct-aggregate projection (`v.field`); `f(v).field` or
/// `(v as T).field` do not match the per-CFG-path invariant.
fn aggregate_name_of(ctx: &WalkCtx<'_>, expr: ExprId) -> Option<Name> {
    if expr == ExprId::INVALID {
        return None;
    }
    match ctx.arena.expr_kind(expr) {
        ExprKind::Ident(name) => Some(*name),
        _ => None,
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
trait ExprIdRawIndex {
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
