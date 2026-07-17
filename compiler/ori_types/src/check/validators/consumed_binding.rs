//! Use-after-`drop_early` rejection — producer side of `E2054`.
//!
//! Spec Clause 13 §13.7 (Variable Lifetime): `drop_early(value: x)` consumes
//! `x` and runs its drop immediately; the binding is inaccessible afterward.
//! Any later use reads reclaimed memory (use-after-free), so it MUST be a
//! compile-time error. A re-binding `let` shadowing the name restores
//! accessibility (a fresh binding).
//!
//! # Algorithm
//!
//! A forward execution-order walk threading a consumed-binding set
//! (`Name -> consume Span`) through the body:
//!
//! - `drop_early(value: x)` for a bare identifier `x` marks `x` consumed; a
//!   second consume of an already-consumed `x` (a double `drop_early`) is
//!   itself a use-after-consume and emits `E2054`.
//! - A bare `Ident(x)` where `x` is consumed emits `E2054`.
//! - `let`/destructure binding a name un-consumes it (a fresh binding shadows
//!   the consumed one).
//! - `if` / `match` join branch consumed-sets by UNION (consumed-on-any-path):
//!   a later use of a binding consumed on some reaching path is rejected.
//! - Loops (`for` / `loop` / `while`) run a dry pass to compute the
//!   loop-carried consumed set, then a second emitting pass seeded with it so a
//!   use in iteration N+1 of a binding consumed in iteration N is caught.
//!
//! The walk is bounded by the AST size: no CFG, no fixpoint, no lattice.

mod support;

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};
use rustc_hash::FxHashMap;

use crate::check::validators::expr_children::child_ids;
use crate::TypeCheckError;
use support::{
    binding_pattern_bound_names, drop_early_named_target, drop_early_positional_target,
    match_pattern_bound_names, record_consume, unbind_binding_pattern,
};

/// Consumed-binding map: surface name -> span of the `drop_early` that
/// consumed it.
type Consumed = FxHashMap<Name, Span>;

/// Validate that no binding is used after a `drop_early` consumed it.
///
/// `drop_early_name` is the interned prelude name `drop_early`; the validator
/// recognises `drop_early(value: x)` by callee identity. When the name is not
/// present in any call the walk is a no-op.
pub fn validate_consumed_binding(
    arena: &ExprArena,
    drop_early_name: Name,
    body_root: ExprId,
    errors: &mut Vec<TypeCheckError>,
) {
    if body_root == ExprId::INVALID {
        return;
    }
    let ctx = WalkCtx {
        arena,
        drop_early_name,
    };
    let mut consumed: Consumed = FxHashMap::default();
    thread_expr(&ctx, body_root, &mut consumed, errors);
}

/// Immutable per-validator walk state. The consumed set + error sink are
/// threaded as `&mut` params so branch/loop passes can fork and rejoin them.
struct WalkCtx<'a> {
    arena: &'a ExprArena,
    drop_early_name: Name,
}

/// Thread one expression in execution order, updating `consumed` and emitting
/// `E2054` for any use of a consumed binding.
fn thread_expr(
    ctx: &WalkCtx<'_>,
    expr_id: ExprId,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    if expr_id == ExprId::INVALID {
        return;
    }
    // ExprKind is Copy.
    let kind = *ctx.arena.expr_kind(expr_id);
    match kind {
        ExprKind::Ident(name) => {
            if consumed.contains_key(&name) {
                let span = ctx.arena.get_expr(expr_id).span;
                errors.push(TypeCheckError::use_after_drop_early(span, name));
            }
        }
        ExprKind::CallNamed { func, args } => {
            if let Some((bound, span)) = drop_early_named_target(ctx, func, args) {
                record_consume(bound, span, consumed, errors);
            } else {
                thread_children(ctx, expr_id, consumed, errors);
            }
        }
        ExprKind::Call { func, args } => {
            // Positional `drop_early(x)` — the shipped checker validates calls
            // positionally, so `drop_early(x)` (no `value:`) type-checks and
            // lowers to `Call`, not `CallNamed`. Recognise the consume here too.
            if let Some((bound, span)) = drop_early_positional_target(ctx, func, args) {
                record_consume(bound, span, consumed, errors);
            } else {
                thread_children(ctx, expr_id, consumed, errors);
            }
        }
        ExprKind::TemplateLiteral { parts, .. } => {
            thread_template_parts(ctx, parts, consumed, errors);
        }
        ExprKind::Block { stmts, result } => {
            thread_block(ctx, stmts, result, consumed, errors);
        }
        ExprKind::Let { pattern, init, .. } => {
            thread_expr(ctx, init, consumed, errors);
            unbind_binding_pattern(ctx, pattern, consumed);
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            thread_expr(ctx, cond, consumed, errors);
            let mut c_then = consumed.clone();
            let mut c_else = consumed.clone();
            thread_expr(ctx, then_branch, &mut c_then, errors);
            thread_expr(ctx, else_branch, &mut c_else, errors);
            *consumed = c_then;
            union_into(consumed, c_else);
        }
        ExprKind::Match { scrutinee, arms } => {
            thread_expr(ctx, scrutinee, consumed, errors);
            let arm_steps: Vec<(Vec<Name>, ExprId, ExprId)> = ctx
                .arena
                .get_arms(arms)
                .iter()
                .map(|a| {
                    (
                        match_pattern_bound_names(ctx, &a.pattern),
                        a.guard.unwrap_or(ExprId::INVALID),
                        a.body,
                    )
                })
                .collect();
            thread_arms(ctx, consumed, errors, &arm_steps);
        }
        ExprKind::For {
            pattern,
            iter,
            guard,
            body,
            ..
        } => {
            thread_expr(ctx, iter, consumed, errors);
            let unbind = binding_pattern_bound_names(ctx, pattern);
            thread_loop(ctx, consumed, errors, &unbind, &[guard, body]);
        }
        ExprKind::Loop { body, .. } => {
            thread_loop(ctx, consumed, errors, &[], &[body]);
        }
        ExprKind::While { cond, body, .. } => {
            // `cond` re-evaluates each iteration, so it is part of the loop body.
            thread_loop(ctx, consumed, errors, &[], &[cond, body]);
        }
        ExprKind::FunctionSeq(seq_id) => {
            thread_function_seq(ctx, seq_id, consumed, errors);
        }
        ExprKind::FunctionExp(exp_id) => {
            thread_function_exp_props(ctx, exp_id, consumed, errors);
        }
        ExprKind::Lambda { body, .. } => {
            // A closure captures by value at creation; a use of an
            // already-consumed binding inside it is still a use-after-free.
            // Internal consumes do NOT escape to the enclosing scope.
            let mut inner = consumed.clone();
            thread_expr(ctx, body, &mut inner, errors);
        }
        _ => thread_children(ctx, expr_id, consumed, errors),
    }
}

/// Thread each interpolated part of a template literal. An interpolation
/// `` `{x}` `` is a USE of `x`; the shared `child_ids` walker treats a template
/// literal as a leaf, so the parts are threaded here. `TemplateFull` has none.
fn thread_template_parts(
    ctx: &WalkCtx<'_>,
    parts: ori_ir::TemplatePartRange,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    let exprs: Vec<ExprId> = ctx
        .arena
        .get_template_parts(parts)
        .iter()
        .map(|p| p.expr)
        .collect();
    for e in exprs {
        thread_expr(ctx, e, consumed, errors);
    }
}

/// Thread the named-prop values of a function-expression form (`print`,
/// `recurse`, `parallel`, `cache`, `with`, `catch`, ...). The shared `child_ids`
/// walker treats `FunctionExp` as a leaf, so the prop values are threaded here
/// to reach a use of a consumed binding inside one (e.g. `print(msg: s)`).
fn thread_function_exp_props(
    ctx: &WalkCtx<'_>,
    exp_id: ori_ir::FunctionExpId,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    let props = ctx.arena.get_function_exp(exp_id).props;
    let values: Vec<ExprId> = ctx
        .arena
        .get_named_exprs(props)
        .iter()
        .map(|p| p.value)
        .collect();
    for v in values {
        thread_expr(ctx, v, consumed, errors);
    }
}

/// Thread every structural child of `expr_id` in evaluation order. Used for
/// non-control-flow nodes (binary/call/field/index/cast/...) where children
/// run left-to-right with no branching, so sequential threading is exact.
fn thread_children(
    ctx: &WalkCtx<'_>,
    expr_id: ExprId,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    for child in child_ids(ctx.arena, expr_id) {
        thread_expr(ctx, child, consumed, errors);
    }
}

/// Copy snapshot of a block statement (decouples the `arena` borrow from the
/// `thread_expr` recursion).
enum BlockStep {
    Expr(ExprId),
    Let(ori_ir::BindingPatternId, ExprId),
}

/// Thread a block / try-block: process statements in order (a `let` un-consumes
/// the names it binds, which shadow for the rest of the block), thread the
/// result, then RESTORE every block-local `let`-bound name to its pre-block
/// consumed state — those bindings go out of scope at block exit, so the outer
/// binding (if any) resumes. A consume of an OUTER binding inside the block is
/// NOT block-local and persists past the block.
fn thread_block(
    ctx: &WalkCtx<'_>,
    stmts: ori_ir::StmtRange,
    result: ExprId,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    let entry = consumed.clone();
    let mut block_bound: Vec<Name> = Vec::new();
    // Snapshot each statement as Copy data (the borrow of `arena` must end
    // before the `thread_expr` recursion below re-borrows `ctx`).
    let steps: Vec<BlockStep> = ctx
        .arena
        .get_stmt_range(stmts)
        .iter()
        .map(|s| match &s.kind {
            ori_ir::StmtKind::Expr(id) => BlockStep::Expr(*id),
            ori_ir::StmtKind::Let { pattern, init, .. } => BlockStep::Let(*pattern, *init),
        })
        .collect();
    for step in steps {
        match step {
            BlockStep::Expr(id) => thread_expr(ctx, id, consumed, errors),
            BlockStep::Let(pattern, init) => {
                thread_expr(ctx, init, consumed, errors);
                let names = binding_pattern_bound_names(ctx, pattern);
                for n in &names {
                    consumed.remove(n);
                }
                block_bound.extend(names);
            }
        }
    }
    thread_expr(ctx, result, consumed, errors);
    restore_scope(consumed, &entry, &block_bound);
}

/// Restore each name in `names` to its `entry` consumed state — used at block
/// exit so a block-local `let` binding does not leak its shadow (un-consume) or
/// its block-local consume past the block boundary.
fn restore_scope(consumed: &mut Consumed, entry: &Consumed, names: &[Name]) {
    for n in names {
        match entry.get(n) {
            Some(&span) => {
                consumed.insert(*n, span);
            }
            None => {
                consumed.remove(n);
            }
        }
    }
}

/// Thread a `FunctionSeq` surface construct (`match` / `try` / `for`-pattern).
fn thread_function_seq(
    ctx: &WalkCtx<'_>,
    seq_id: ori_ir::FunctionSeqId,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
) {
    match *ctx.arena.get_function_seq(seq_id) {
        ori_ir::FunctionSeq::Match {
            scrutinee, arms, ..
        } => {
            thread_expr(ctx, scrutinee, consumed, errors);
            let arm_steps: Vec<(Vec<Name>, ExprId, ExprId)> = ctx
                .arena
                .get_arms(arms)
                .iter()
                .map(|a| {
                    (
                        match_pattern_bound_names(ctx, &a.pattern),
                        a.guard.unwrap_or(ExprId::INVALID),
                        a.body,
                    )
                })
                .collect();
            thread_arms(ctx, consumed, errors, &arm_steps);
        }
        ori_ir::FunctionSeq::Try { stmts, result, .. } => {
            thread_block(ctx, stmts, result, consumed, errors);
        }
        ori_ir::FunctionSeq::ForPattern {
            over,
            map,
            ref arm,
            default,
            ..
        } => {
            thread_expr(ctx, over, consumed, errors);
            let unbind = match_pattern_bound_names(ctx, &arm.pattern);
            let guard = arm.guard.unwrap_or(ExprId::INVALID);
            let map_id = map.unwrap_or(ExprId::INVALID);
            let arm_body = arm.body;
            // Per element: evaluate map + guard (sequential), then the element
            // either MATCHES the arm OR falls to `default` — mutually-exclusive
            // branches, joined by union (not sequenced, which would falsely
            // treat `default` as running after `arm.body` every iteration).
            thread_loop_with(ctx, consumed, errors, &unbind, &|ctx, c, e| {
                thread_expr(ctx, map_id, c, e);
                thread_expr(ctx, guard, c, e);
                let mut c_arm = c.clone();
                let mut c_default = c.clone();
                thread_expr(ctx, arm_body, &mut c_arm, e);
                thread_expr(ctx, default, &mut c_default, e);
                *c = c_arm;
                union_into(c, c_default);
            });
        }
    }
}

/// Join a set of match arms by UNION: each arm starts from the post-scrutinee
/// consumed set (with the arm's pattern-bound names un-consumed), and the
/// resulting `consumed` is consumed-on-any-arm-path.
fn thread_arms(
    ctx: &WalkCtx<'_>,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
    arms: &[(Vec<Name>, ExprId, ExprId)],
) {
    if arms.is_empty() {
        return;
    }
    let base = consumed.clone();
    let mut joined: Consumed = FxHashMap::default();
    for (bound, guard, body) in arms {
        let mut c = base.clone();
        for n in bound {
            c.remove(n);
        }
        thread_expr(ctx, *guard, &mut c, errors);
        thread_expr(ctx, *body, &mut c, errors);
        union_into(&mut joined, c);
    }
    *consumed = joined;
}

/// Two-pass loop handling so a use in iteration N+1 of a binding consumed in
/// iteration N is caught, threading the per-iteration body via a sequential
/// `steps` list (`for` / `loop` / `while`).
fn thread_loop(
    ctx: &WalkCtx<'_>,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
    pre_unbind: &[Name],
    steps: &[ExprId],
) {
    thread_loop_with(ctx, consumed, errors, pre_unbind, &|ctx, c, e| {
        for &s in steps {
            thread_expr(ctx, s, c, e);
        }
    });
}

/// Two-pass loop core. `body_fn` threads ONE iteration's body (it may itself
/// branch, e.g. the `for`-pattern arm-vs-default branch). Pass A computes the
/// loop-carried consumed set (errors discarded); pass B emits seeded with
/// `entry ∪ carried` so a top-of-body use sees a bottom-of-body consume from
/// the prior iteration. `pre_unbind` names are bound fresh each iteration and
/// removed at the top of every pass.
fn thread_loop_with(
    ctx: &WalkCtx<'_>,
    consumed: &mut Consumed,
    errors: &mut Vec<TypeCheckError>,
    pre_unbind: &[Name],
    body_fn: &dyn Fn(&WalkCtx<'_>, &mut Consumed, &mut Vec<TypeCheckError>),
) {
    let entry = consumed.clone();

    // Pass A (dry, errors discarded): compute the loop-carried consumed set.
    let mut carried = entry.clone();
    for n in pre_unbind {
        carried.remove(n);
    }
    let mut scratch: Vec<TypeCheckError> = Vec::new();
    body_fn(ctx, &mut carried, &mut scratch);

    // Pass B (emitting): seed entry ∪ carried, loop-bound names fresh again.
    let mut seed = entry.clone();
    union_into(&mut seed, carried.clone());
    for n in pre_unbind {
        seed.remove(n);
    }
    body_fn(ctx, &mut seed, errors);

    // Post-loop (the loop ran 0+ times): consumed-on-some-path = entry ∪
    // carried. A binding consumed only inside a possibly-skipped loop is
    // conservatively treated as consumed afterward.
    let mut after = entry;
    union_into(&mut after, carried);
    *consumed = after;
}

/// Merge `src` into `dst` keeping `dst`'s span on key collision (the first
/// recorded consume site).
fn union_into(dst: &mut Consumed, src: Consumed) {
    for (k, v) in src {
        dst.entry(k).or_insert(v);
    }
}

#[cfg(test)]
mod tests;
