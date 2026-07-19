//! Generic one-structural-step child enumeration over `ExprKind`.
//!
//! Shared by the move-discipline validators (`partial_move`,
//! `consumed_binding`) that walk a body's AST. Each validator overlays its
//! own per-construct semantics; this module owns the single canonical answer
//! to "what `ExprId`s are reachable in one structural step from this node?".
//!
//! Mirrors the relevant arms of `ori_ir::visitor::walk_expr` but returns a
//! flat child list rather than driving a visitor, so an order-sensitive
//! dataflow walk can thread its own state through the children.

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Return every [`ExprId`] reachable in one structural step from `expr_id`.
///
/// `FunctionSeq` is treated as a leaf — its `match` / `try` / `for`-pattern
/// children are post-canonicalization surfaces each validator descends into
/// itself when relevant.
pub(crate) fn child_ids(arena: &ExprArena, expr_id: ExprId) -> Vec<ExprId> {
    // ExprKind is Copy.
    let kind = *arena.expr_kind(expr_id);
    let mut out: Vec<ExprId> = Vec::new();
    push_children_for_kind(arena, kind, &mut out);
    out
}

/// Push every relevant child expression for `kind` into `out`.
///
/// Per-variant arms below dispatch into small per-shape helpers
/// ([`push_call_children`], [`push_call_named_children`],
/// [`push_expr_list_children`], [`push_list_element_children`],
/// [`push_map_entry_children`], [`push_map_element_children`],
/// [`push_struct_children`], [`push_struct_lit_field_children`]) that own each
/// variant family.
fn push_children_for_kind(arena: &ExprArena, kind: ExprKind, out: &mut Vec<ExprId>) {
    if push_control_flow_children(arena, kind, out) {
        return;
    }
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
        ExprKind::AssignTarget { root, steps } => {
            // The assign target's root (`list` in `list[i] = x`) is read, and
            // each `[index]` step expression is a use; `.field` steps carry a
            // Name, not an expression.
            out.push(root);
            for step in arena.get_access_steps(steps) {
                if let ori_ir::AccessStep::Index(e) = step {
                    out.push(*e);
                }
            }
        }
        ExprKind::WithCapability { provider, body, .. } => {
            out.push(provider);
            out.push(body);
        }
        // Leaf variants in this generic one-step walker. FunctionSeq,
        // FunctionExp, and TemplateLiteral carry inner expressions but are
        // treated as leaves HERE: a consumer that must reach them descends in
        // its own dispatch (consumed_binding does, for E2054); partial_move
        // (E2043) instead treats their projection surfaces as
        // post-canonicalization checks and does not descend.
        _ => {}
    }
}

fn push_control_flow_children(arena: &ExprArena, kind: ExprKind, out: &mut Vec<ExprId>) -> bool {
    match kind {
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => out.extend([cond, then_branch, else_branch]),
        ExprKind::Match { scrutinee, arms } => {
            out.push(scrutinee);
            for arm in arena.get_arms(arms) {
                out.extend(arm.guard);
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
        ExprKind::While { cond, body, .. } => out.extend([cond, body]),
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
        _ => return false,
    }
    true
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
